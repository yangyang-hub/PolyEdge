#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct SmartMoneySignalAdvisoryPrepReport {
    candidates: usize,
    cache_hits: usize,
    requests_built: usize,
    provider_requests: usize,
    provider_saved: usize,
    provider_failures: usize,
}

struct SmartMoneySignalAdvisoryProvider {
    provider: polyedge_application::RewardAiProvider,
    request_format: polyedge_application::RewardAiRequestFormat,
    model: String,
    connector: Option<SmartSignalAdvisoryConnector>,
}

async fn prepare_smart_money_signal_advisory_requests(
    state: &AppState,
    config: &SmartMoneyConfig,
    trace_id: &str,
) -> Result<SmartMoneySignalAdvisoryPrepReport> {
    if !config.signal_advisory_enabled {
        return Ok(SmartMoneySignalAdvisoryPrepReport::default());
    }

    let signals = state
        .smart_money_service
        .list_signals(Some(SMART_MONEY_SIGNAL_ADVISORY_SIGNAL_LIMIT))
        .await?
        .into_iter()
        .filter(|signal| signal.status == SmartSignalStatus::Observe)
        .collect::<Vec<_>>();
    if signals.is_empty() {
        return Ok(SmartMoneySignalAdvisoryPrepReport::default());
    }

    let trades_by_id = state
        .smart_money_service
        .list_trades(Some(SMART_MONEY_SIGNAL_ADVISORY_CONTEXT_LIMIT))
        .await?
        .into_iter()
        .map(|trade| (trade.id.clone(), trade))
        .collect::<HashMap<_, _>>();
    let profiles_by_wallet = state
        .smart_money_service
        .list_profiles(Some(SMART_MONEY_SIGNAL_ADVISORY_CONTEXT_LIMIT))
        .await?
        .into_iter()
        .map(|profile| (profile.wallet_address.to_lowercase(), profile))
        .collect::<HashMap<_, _>>();
    let scores_by_wallet = state
        .smart_money_service
        .list_scores(None, Some(SMART_MONEY_SIGNAL_ADVISORY_CONTEXT_LIMIT))
        .await?
        .into_iter()
        .map(|score| (score.wallet_address.to_lowercase(), score))
        .collect::<HashMap<_, _>>();

    let provider = smart_money_signal_advisory_provider(state).await?;
    let now = OffsetDateTime::now_utc();
    let mut report = SmartMoneySignalAdvisoryPrepReport {
        candidates: signals.len(),
        ..SmartMoneySignalAdvisoryPrepReport::default()
    };

    for signal in &signals {
        let wallet_key = signal.wallet_address.to_lowercase();
        let context = SmartSignalAdvisoryContext {
            source_trade: trades_by_id.get(&signal.source_trade_id),
            profile: profiles_by_wallet.get(&wallet_key),
            score: scores_by_wallet.get(&wallet_key),
            now,
            ttl_sec: SMART_MONEY_SIGNAL_ADVISORY_TTL_SEC,
        };
        let request = state.smart_money_service.build_signal_advisory_request(
            provider.provider.as_str(),
            provider.request_format.as_str(),
            &provider.model,
            config,
            signal,
            context,
        )?;
        let lookup = SmartSignalAdvisoryLookup {
            signal_id: request.signal_id,
            provider: request.provider.clone(),
            request_format: request.request_format.clone(),
            model: request.model.clone(),
            input_hash: request.input_hash.clone(),
        };
        if state
            .smart_money_service
            .latest_signal_advisory(&lookup, now)
            .await?
            .is_some()
        {
            report.cache_hits += 1;
            continue;
        }

        report.requests_built += 1;
        if let Some(connector) = provider.connector.as_ref() {
            report.provider_requests += 1;
            let started = Instant::now();
            match connector.advise(&request).await {
                Ok(decision) => {
                    let latency = started.elapsed();
                    let saved_at = OffsetDateTime::now_utc();
                    let advisory = SmartSignalAdvisory {
                        id: 0,
                        signal_id: request.signal_id,
                        provider: request.provider.clone(),
                        request_format: request.request_format.clone(),
                        model: request.model.clone(),
                        input_hash: request.input_hash.clone(),
                        recommendation: decision.recommendation,
                        confidence: decision.confidence,
                        risk_tags: decision.risk_tags.clone(),
                        summary: decision.summary.clone(),
                        reasons: decision.reasons.clone(),
                        raw_output: decision.raw_output.clone(),
                        expires_at: saved_at
                            + TimeDuration::seconds(SMART_MONEY_SIGNAL_ADVISORY_TTL_SEC as i64),
                        created_at: saved_at,
                    };
                    state
                        .smart_money_service
                        .save_signal_advisory(&advisory)
                        .await?;
                    report.provider_saved += 1;
                    record_smart_signal_advisory_llm_call(
                        state,
                        &request,
                        latency,
                        true,
                        Some(decision.raw_output),
                        None,
                        trace_id,
                    )
                    .await;
                }
                Err(error) => {
                    let latency = started.elapsed();
                    report.provider_failures += 1;
                    record_smart_signal_advisory_llm_call(
                        state,
                        &request,
                        latency,
                        false,
                        None,
                        Some(error.to_string()),
                        trace_id,
                    )
                    .await;
                    warn!(
                        trace_id = %trace_id,
                        signal_id = request.signal_id,
                        error = %error,
                        "failed to refresh smart money signal advisory from provider"
                    );
                }
            }
        }
        debug!(
            trace_id = %trace_id,
            signal_id = request.signal_id,
            input_hash = %request.input_hash,
            "prepared smart money signal advisory request payload"
        );
    }

    info!(
        trace_id = %trace_id,
        candidates = report.candidates,
        cache_hits = report.cache_hits,
        requests_built = report.requests_built,
        provider_requests = report.provider_requests,
        provider_saved = report.provider_saved,
        provider_failures = report.provider_failures,
        "completed smart money signal advisory request preparation"
    );
    Ok(report)
}

async fn smart_money_signal_advisory_provider(
    state: &AppState,
) -> Result<SmartMoneySignalAdvisoryProvider> {
    let reward_config = state.reward_bot_service.read_config().await?;
    let model = reward_ai_model_for_provider(&state.settings.rewards, reward_config.ai_provider)
        .trim()
        .to_string();
    let request_format = polyedge_application::reward_ai_effective_request_format(
        reward_config.ai_provider,
        reward_config.ai_request_format,
        &model,
    );
    let (api_key, base_url) =
        reward_ai_provider_endpoint_settings(&state.settings.rewards, reward_config.ai_provider);
    let connector = api_key
        .filter(|value| !value.trim().is_empty())
        .map(|api_key| {
            SmartSignalAdvisoryConnector::new(
                base_url,
                api_key,
                state.settings.rewards.ai_request_timeout_secs.max(1),
            )
        })
        .transpose()?;
    Ok(SmartMoneySignalAdvisoryProvider {
        provider: reward_config.ai_provider,
        request_format,
        model,
        connector,
    })
}

async fn record_smart_signal_advisory_llm_call(
    state: &AppState,
    request: &polyedge_application::SmartSignalAdvisoryRequest,
    latency: Duration,
    success: bool,
    parsed_output: Option<Value>,
    error: Option<String>,
    trace_id: &str,
) {
    let latency_ms = i64::try_from(latency.as_millis()).unwrap_or(i64::MAX);
    let call = RewardLlmCallRecord {
        id: format!("llm_{}", Uuid::now_v7()),
        task_type: SMART_MONEY_SIGNAL_ADVISORY_LLM_TASK_TYPE.to_string(),
        model_version: request.model.clone(),
        prompt_version: SMART_MONEY_SIGNAL_ADVISORY_PROMPT_VERSION.to_string(),
        input_hash: request.input_hash.clone(),
        raw_output: None,
        parsed_output,
        validation_result: json!({
            "success": success,
            "signal_id": request.signal_id,
            "error": error,
        }),
        fallback_used: false,
        latency_ms,
        cost_estimate: None,
        trace_id: trace_id.to_string(),
        created_at: OffsetDateTime::now_utc(),
    };
    if let Err(error) = state.reward_bot_service.record_llm_call(&call).await {
        warn!(
            trace_id = %trace_id,
            signal_id = request.signal_id,
            error = %error,
            "failed to record smart money signal advisory LLM call",
        );
    }
}
