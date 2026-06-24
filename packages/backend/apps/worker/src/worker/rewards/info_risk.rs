const REWARD_INFO_RISK_ADVISORY_LOCK_KEY: i64 = 0x504f_4c59_494e_464f;

async fn scan_reward_info_risks_once(
    state: &AppState,
    trace_id: &str,
) -> Result<RewardInfoRiskScanReport> {
    let Some(lease) = state
        .try_acquire_postgres_advisory_lease(REWARD_INFO_RISK_ADVISORY_LOCK_KEY)
        .await?
    else {
        info!("skipping reward info risk scan because another worker holds the lease");
        return Ok(RewardInfoRiskScanReport::default());
    };
    let result = scan_reward_info_risks_unlocked(state, trace_id).await;
    finish_reward_worker_lease(lease, result).await
}

async fn poll_reward_info_risks(
    state: &AppState,
    max_cycles: Option<usize>,
) -> Result<RewardInfoRiskScanReport> {
    let mut total = RewardInfoRiskScanReport::default();
    let mut cycles = 0usize;
    let interval = Duration::from_secs(state.settings.rewards.info_risk_interval_secs.max(30));

    loop {
        let trace_id = new_trace_id();
        let report = scan_reward_info_risks_once(state, &trace_id).await?;
        accumulate_info_risk_report(&mut total, &report);
        cycles += 1;
        info!(
            trace_id = %trace_id,
            candidates = report.candidates,
            cache_hits = report.cache_hits,
            requested = report.requested,
            saved = report.saved,
            applied_plans = report.applied_plans,
            "completed reward info risk scan",
        );
        if max_cycles.is_some_and(|limit| cycles >= limit) {
            break;
        }
        tokio::time::sleep(interval).await;
    }

    Ok(total)
}

async fn scan_reward_info_risks_unlocked(
    state: &AppState,
    trace_id: &str,
) -> Result<RewardInfoRiskScanReport> {
    let config = state.reward_bot_service.read_config().await?;
    if !config.info_risk_enabled {
        info!(
            trace_id = %trace_id,
            "skipping reward info risk scan because it is disabled in rewards config",
        );
        return Ok(RewardInfoRiskScanReport::default());
    }
    if config.ai_advisory_enabled {
        info!(
            trace_id = %trace_id,
            "skipping standalone reward info risk provider scan because the rewards full tick starts a dedicated info-risk provider refresh task",
        );
        return Ok(RewardInfoRiskScanReport::default());
    }
    info!(
        trace_id = %trace_id,
        provider = config.ai_provider.as_str(),
        request_format = config.ai_request_format.as_str(),
        mode = config.info_risk_mode.as_str(),
        web_search_enabled = state.settings.rewards.info_risk_web_search_enabled,
        "starting reward info risk scan",
    );
    let Some(connector) = build_reward_info_risk_connector(state, &config)? else {
        warn!(
            trace_id = %trace_id,
            provider = config.ai_provider.as_str(),
            "reward info risk is enabled but provider configuration is incomplete",
        );
        return Ok(RewardInfoRiskScanReport::default());
    };
    let model = state.settings.rewards.ai_model.trim();
    if model.is_empty() {
        warn!(trace_id = %trace_id, "reward info risk model is empty");
        return Ok(RewardInfoRiskScanReport::default());
    }

    let mut report = RewardInfoRiskScanReport::default();
    let cycle = state.reward_bot_service.current_live_cycle_state().await?;
    let candidate_markets = state
        .reward_bot_service
        .list_reward_run_candidate_markets()
        .await?;
    let active_markets = state.reward_bot_service.list_active_reward_markets().await?;
    let mut markets_by_condition = active_markets
        .iter()
        .map(|market| (market.condition_id.clone(), market))
        .collect::<HashMap<_, _>>();
    for market in &candidate_markets {
        markets_by_condition.insert(market.condition_id.clone(), market);
    }
    let plans_by_condition = cycle
        .plans
        .iter()
        .map(|plan| (plan.condition_id.as_str(), plan))
        .collect::<HashMap<_, _>>();
    let mut ordered_conditions = reward_info_risk_candidate_conditions(
        &candidate_markets,
        &cycle.plans,
        &cycle.open_orders,
        &cycle.positions,
        &config,
    );
    report.candidates = ordered_conditions.len();
    let max_conditions = reward_provider_max_conditions_per_cycle(state);
    if ordered_conditions.len() > max_conditions {
        ordered_conditions.truncate(max_conditions);
    }
    info!(
        trace_id = %trace_id,
        candidates = report.candidates,
        selected_conditions = ordered_conditions.len(),
        max_conditions,
        "prepared reward info risk provider candidates",
    );

    for condition_id in ordered_conditions {
        let Some(market) = markets_by_condition.get(&condition_id) else {
            report.skipped_missing_market += 1;
            continue;
        };
        let request = build_reward_info_risk_assessment_request(
            market,
            plans_by_condition.get(condition_id.as_str()).copied(),
            &cycle.account,
            &cycle.positions,
            &cycle.open_orders,
            &config,
            config.ai_provider,
            config.ai_request_format,
            model,
        )?;
        if state
            .reward_bot_service
            .latest_market_info_risk(&request)
            .await?
            .is_some_and(|risk| {
                report.cache_hits += 1;
                !reward_provider_cache_refresh_due(
                    risk.expires_at,
                    config.info_risk_ttl_sec,
                    OffsetDateTime::now_utc(),
                )
            })
        {
            continue;
        }

        report.requested += 1;
        info!(
            trace_id = %trace_id,
            condition_id = %condition_id,
            requested = report.requested,
            "requesting reward info risk provider",
        );
        let _provider_permit = acquire_reward_info_risk_provider_request_permit().await?;
        match connector.assess(&request).await {
            Ok(decision) => {
                let risk = decision.into_info_risk(
                    &request,
                    config.info_risk_ttl_sec,
                    OffsetDateTime::now_utc(),
                );
                state.reward_bot_service.save_market_info_risk(&risk).await?;
                report.saved += 1;
                info!(
                    trace_id = %trace_id,
                    condition_id = %condition_id,
                    saved = report.saved,
                    "saved reward info risk",
                );
            }
            Err(error) => {
                report.failures += 1;
                warn!(
                    trace_id = %trace_id,
                    condition_id = %condition_id,
                    error = %error,
                    "reward info risk request failed; keeping existing cached state",
                );
                if reward_ai_provider_is_overloaded(&error) {
                    warn!(
                        trace_id = %trace_id,
                        requested = report.requested,
                        failures = report.failures,
                        "reward info risk provider is overloaded; stopping provider requests for this cycle",
                    );
                    break;
                }
            }
        }
    }

    report.applied_plans = apply_cached_reward_info_risks(state, trace_id).await?;
    info!(
        trace_id = %trace_id,
        candidates = report.candidates,
        cache_hits = report.cache_hits,
        requested = report.requested,
        saved = report.saved,
        failures = report.failures,
        skipped_missing_market = report.skipped_missing_market,
        applied_plans = report.applied_plans,
        "completed reward info risk scan",
    );
    Ok(report)
}

async fn apply_cached_reward_info_risks(state: &AppState, trace_id: &str) -> Result<usize> {
    let config = state.reward_bot_service.read_config().await?;
    if !config.info_risk_enabled {
        return Ok(0);
    }
    let mut plans = state
        .reward_bot_service
        .current_live_cycle_state()
        .await?
        .plans;
    let applied = apply_cached_reward_info_risks_to_plans(state, &config, &mut plans, trace_id).await?;
    state.reward_bot_service.save_quote_plans(&plans).await?;
    Ok(applied)
}

async fn apply_cached_reward_info_risks_to_cycle(
    state: &AppState,
    cycle: &mut RewardLiveCycle,
    trace_id: &str,
) -> Result<usize> {
    if !cycle.config.info_risk_enabled {
        return Ok(0);
    }
    apply_cached_reward_info_risks_to_plans(state, &cycle.config, &mut cycle.plans, trace_id).await
}

async fn apply_cached_reward_info_risks_to_plans(
    state: &AppState,
    config: &RewardBotConfig,
    plans: &mut [RewardQuotePlan],
    trace_id: &str,
) -> Result<usize> {
    if plans.is_empty() {
        return Ok(0);
    }
    let condition_ids = plans
        .iter()
        .map(|plan| plan.condition_id.clone())
        .collect::<Vec<_>>();
    let risks = state
        .reward_bot_service
        .latest_market_info_risks(&condition_ids)
        .await?
        .into_iter()
        .map(|risk| (risk.condition_id.clone(), risk))
        .collect::<HashMap<String, RewardMarketInfoRisk>>();
    let before = plans
        .iter()
        .filter(|plan| plan.info_risk.is_some())
        .count();
    let risk_count = risks.len();
    let min_confidence =
        reward_ai_min_confidence(state.settings.rewards.info_risk_min_confidence_bps);
    apply_reward_info_risks(plans, &risks, config, min_confidence);
    let after = plans
        .iter()
        .filter(|plan| plan.info_risk.is_some())
        .count();
    debug!(
        trace_id = %trace_id,
        risks = risk_count,
        applied = after.saturating_sub(before),
        "applied cached reward info risks to quote plans",
    );
    Ok(risk_count)
}

fn reward_info_risk_candidate_conditions(
    markets: &[RewardMarket],
    plans: &[RewardQuotePlan],
    open_orders: &[ManagedRewardOrder],
    positions: &[RewardPosition],
    config: &RewardBotConfig,
) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut condition_ids = Vec::new();
    let plans_by_condition = plans
        .iter()
        .map(|plan| (plan.condition_id.as_str(), plan))
        .collect::<HashMap<_, _>>();

    for order in open_orders {
        push_info_risk_condition(
            &mut condition_ids,
            &mut seen,
            &order.condition_id,
        );
    }
    for position in positions {
        push_info_risk_condition(
            &mut condition_ids,
            &mut seen,
            &position.condition_id,
        );
    }
    for plan in plans.iter().filter(|plan| {
        let has_active_exposure =
            reward_condition_has_active_exposure(&plan.condition_id, open_orders, positions);
        reward_provider_plan_passes_pre_llm_gate(plan, config, has_active_exposure)
    }) {
        push_info_risk_condition(
            &mut condition_ids,
            &mut seen,
            &plan.condition_id,
        );
    }
    for market in markets {
        let condition_id = market.condition_id.trim();
        if condition_id.is_empty() || seen.contains(condition_id) {
            continue;
        }
        let has_active_exposure =
            reward_condition_has_active_exposure(condition_id, open_orders, positions);
        let passes_pre_llm_gate = has_active_exposure
            || plans_by_condition.get(condition_id).is_some_and(|plan| {
                reward_provider_plan_passes_pre_llm_gate(plan, config, false)
            });
        if !passes_pre_llm_gate {
            continue;
        }
        push_info_risk_condition(
            &mut condition_ids,
            &mut seen,
            condition_id,
        );
    }
    condition_ids
}

fn push_info_risk_condition(
    condition_ids: &mut Vec<String>,
    seen: &mut HashSet<String>,
    condition_id: &str,
) {
    let condition_id = condition_id.trim();
    if condition_id.is_empty() || !seen.insert(condition_id.to_string()) {
        return;
    }
    condition_ids.push(condition_id.to_string());
}

fn build_reward_info_risk_connector(
    state: &AppState,
    config: &RewardBotConfig,
) -> Result<Option<RewardInfoRiskConnector>> {
    let rewards = &state.settings.rewards;
    let (api_key, base_url) = match config.ai_provider {
        polyedge_application::RewardAiProvider::OpenAi => (
            rewards.ai_openai_api_key.as_deref(),
            rewards.ai_openai_base_url.as_str(),
        ),
        polyedge_application::RewardAiProvider::Anthropic => (
            rewards.ai_anthropic_api_key.as_deref(),
            rewards.ai_anthropic_base_url.as_str(),
        ),
    };
    let Some(api_key) = api_key.filter(|value| !value.trim().is_empty()) else {
        return Ok(None);
    };
    RewardInfoRiskConnector::new(
        base_url,
        api_key,
        rewards.ai_request_timeout_secs.max(1),
        rewards.info_risk_web_search_enabled
            && config.ai_provider == polyedge_application::RewardAiProvider::OpenAi
            && config.ai_request_format == polyedge_application::RewardAiRequestFormat::OpenAiResponses,
    )
    .map(Some)
}

fn accumulate_info_risk_report(
    total: &mut RewardInfoRiskScanReport,
    report: &RewardInfoRiskScanReport,
) {
    total.candidates += report.candidates;
    total.cache_hits += report.cache_hits;
    total.requested += report.requested;
    total.saved += report.saved;
    total.failures += report.failures;
    total.skipped_missing_market += report.skipped_missing_market;
    total.applied_plans += report.applied_plans;
}
