// Event-driven AI advisory batch worker. Consumes condition_ids enqueued when
// their orderbook first becomes ready (see `orderbook_events.rs`), batches them
// into a single provider call, and falls back to per-condition single requests
// for any market the model omitted or mislabeled. Coexists with the full-tick
// provider refresh (`provider_refresh.rs`); both rely on advisory cache-miss
// dedup + the AI advisory provider request semaphore, so overlap only ever wastes at
// most one duplicate call. The watch/avoid markets are unsubscribed
// automatically through plan `eligible=false` persistence + the periodic
// orderbook token registration task, so this path never issues unsubscribes.

#[derive(Default)]
struct RewardAiAdvisoryBatchReport {
    batched: usize,
    cache_hits: usize,
    batch_calls: usize,
    fallback_singles: usize,
    saved: usize,
    failures: usize,
    skipped_not_eligible: usize,
    skipped_missing_plan: usize,
    skipped_missing_market: usize,
    skipped_missing_book: usize,
    info_risk: RewardInfoRiskScanReport,
}

async fn run_reward_ai_advisory_batch_worker(
    state: AppState,
    cache: Arc<RewardOrderbookLocalCache>,
    mut ready_rx: tokio::sync::mpsc::Receiver<String>,
) {
    if !state.settings.rewards.ai_advisory_event_driven_enabled {
        debug!("reward AI advisory event-driven batch worker is disabled");
        return;
    }
    let batch_size = reward_ai_advisory_batch_size(state.settings.rewards.ai_advisory_batch_size);
    let batch_timeout = Duration::from_secs(state.settings.rewards.ai_advisory_batch_timeout_secs.max(1));
    let mut buffer: Vec<String> = Vec::with_capacity(batch_size);
    info!(
        batch_size,
        batch_timeout_secs = batch_timeout.as_secs(),
        "starting reward AI advisory event-driven batch worker",
    );

    loop {
        if buffer.len() >= batch_size {
            flush_reward_ai_advisory_batch(&state, cache.as_ref(), &buffer).await;
            buffer.clear();
            continue;
        }
        tokio::select! {
            recv = ready_rx.recv() => match recv {
                Some(condition_id) => {
                    buffer.push(condition_id);
                    if buffer.len() >= batch_size {
                        flush_reward_ai_advisory_batch(&state, cache.as_ref(), &buffer).await;
                        buffer.clear();
                    }
                }
                None => {
                    if !buffer.is_empty() {
                        flush_reward_ai_advisory_batch(&state, cache.as_ref(), &buffer).await;
                    }
                    break;
                }
            },
            _ = tokio::time::sleep(batch_timeout) => {
                if !buffer.is_empty() {
                    flush_reward_ai_advisory_batch(&state, cache.as_ref(), &buffer).await;
                    buffer.clear();
                }
            },
        }
    }
    info!("reward AI advisory event-driven batch worker stopped");
}

fn reward_ai_advisory_batch_size(configured: usize) -> usize {
    configured.clamp(1, 12)
}

/// Evaluate a batch of ready conditions, then unconditionally clear their
/// `notified_ready` markers so the next orderbook change can re-trigger them
/// (e.g. after an advisory TTL expiry).
async fn flush_reward_ai_advisory_batch(
    state: &AppState,
    cache: &RewardOrderbookLocalCache,
    condition_ids: &[String],
) {
    let trace_id = new_trace_id();
    let mut report = RewardAiAdvisoryBatchReport::default();
    let result = run_reward_ai_advisory_batch_flush(state, cache, condition_ids, &trace_id, &mut report)
        .await;
    info!(
        trace_id = %trace_id,
        flushed = condition_ids.len(),
        batched = report.batched,
        cache_hits = report.cache_hits,
        batch_calls = report.batch_calls,
        fallback_singles = report.fallback_singles,
        saved = report.saved,
        failures = report.failures,
        skipped_not_eligible = report.skipped_not_eligible,
        skipped_missing_plan = report.skipped_missing_plan,
        skipped_missing_market = report.skipped_missing_market,
        skipped_missing_book = report.skipped_missing_book,
        info_risk_requested = report.info_risk.requested,
        info_risk_saved = report.info_risk.saved,
        "flushed reward AI advisory batch",
    );
    if let Err(error) = result {
        warn!(
            trace_id = %trace_id,
            error = %error,
            "reward AI advisory batch flush failed; keeping existing cached state",
        );
    }
    cache.clear_notified_ready(condition_ids).await;
}

async fn run_reward_ai_advisory_batch_flush(
    state: &AppState,
    cache: &RewardOrderbookLocalCache,
    condition_ids: &[String],
    trace_id: &str,
    report: &mut RewardAiAdvisoryBatchReport,
) -> Result<()> {
    let cycle = state
        .reward_bot_service
        .current_live_cycle_state()
        .await?;
    if !cycle.config.ai_advisory_enabled {
        return Ok(());
    }
    let model = state.settings.rewards.ai_model.trim();
    if model.is_empty() {
        return Ok(());
    }
    let connector = match build_reward_ai_advisory_connector(state, &cycle.config)? {
        Some(connector) => connector,
        None => return Ok(()),
    };
    let info_risk_connector = build_reward_info_risk_connector(state, &cycle.config)?;
    let fallback_descriptor = resolve_reward_ai_fallback(&state.settings.rewards);
    let advisory_fallback_connector = match &fallback_descriptor {
        Some(descriptor) => Some(build_reward_ai_advisory_fallback_connector(state, descriptor)?),
        None => None,
    };
    let advisory_fallback_channel =
        match (&fallback_descriptor, advisory_fallback_connector.as_ref()) {
            (Some(descriptor), Some(connector)) => Some(RewardAiAdvisoryChannel {
                connector,
                provider: descriptor.provider,
                request_format: descriptor.request_format,
                model: descriptor.model.clone(),
            }),
            _ => None,
        };
    let info_risk_fallback_connector = match &fallback_descriptor {
        Some(descriptor) => Some(build_reward_info_risk_fallback_connector(state, descriptor)?),
        None => None,
    };
    let info_risk_fallback_channel =
        match (&fallback_descriptor, info_risk_fallback_connector.as_ref()) {
            (Some(descriptor), Some(connector)) => Some(RewardInfoRiskChannel {
                connector,
                provider: descriptor.provider,
                request_format: descriptor.request_format,
                model: descriptor.model.clone(),
            }),
            _ => None,
        };

    // markets_by_condition: active reward markets + candidate markets. The
    // lightweight `current_live_cycle_state` cycle has an empty `markets`, so we
    // fetch the candidate set to cover pre_ai_eligible conditions too.
    let mut markets_by_condition = if info_risk_connector.is_some() {
        state
            .reward_bot_service
            .list_active_reward_markets()
            .await?
            .into_iter()
            .map(|market| (market.condition_id.clone(), market))
            .collect::<HashMap<_, _>>()
    } else {
        HashMap::new()
    };
    let candidate_markets = state
        .reward_bot_service
        .list_reward_run_candidate_markets()
        .await?;
    for market in candidate_markets {
        markets_by_condition
            .entry(market.condition_id.clone())
            .or_insert(market);
    }

    let pre_ai_eligible: HashSet<&str> = cycle
        .pre_ai_eligible_condition_ids
        .iter()
        .map(String::as_str)
        .collect();

    let mut requests: Vec<RewardAiAdvisoryRequest> = Vec::new();
    let mut request_by_condition: HashMap<String, RewardAiAdvisoryRequest> = HashMap::new();
    // Conditions that passed the eligibility filter — swept for info-risk below
    // (matches the per-condition AI->info-risk ordering of the tick refresh),
    // whether AI was freshly saved or a cache hit.
    let mut info_risk_condition_ids: Vec<String> = Vec::new();

    for raw in condition_ids {
        let condition_id = raw.trim();
        if condition_id.is_empty() {
            continue;
        }
        if !pre_ai_eligible.contains(condition_id) {
            report.skipped_not_eligible += 1;
            continue;
        }
        let Some(plan) = cycle
            .plans
            .iter()
            .find(|plan| plan.condition_id == condition_id)
        else {
            report.skipped_missing_plan += 1;
            continue;
        };
        let Some(market) = markets_by_condition.get(condition_id) else {
            report.skipped_missing_market += 1;
            continue;
        };
        info_risk_condition_ids.push(condition_id.to_string());
        let books = build_reward_batch_books_from_cache(cache, market).await;
        let candles = state
            .reward_bot_service
            .list_recent_market_candles(
                condition_id,
                REWARD_AI_CANDLE_SOURCE_INTERVAL_SEC,
                REWARD_AI_CANDLE_SOURCE_LIMIT_PER_TOKEN,
            )
            .await?;
        let request = match build_reward_ai_advisory_request(
            market,
            plan,
            &cycle.account,
            &cycle.positions,
            &cycle.open_orders,
            &books,
            &candles,
            &cycle.config,
            cycle.config.ai_advisory_ttl_sec,
            cycle.config.ai_provider,
            cycle.config.ai_request_format,
            model,
        ) {
            Ok(request) => request,
            Err(error) => {
                warn!(
                    trace_id = %trace_id,
                    condition_id = %condition_id,
                    error = %error,
                    "failed to build reward AI advisory request in batch",
                );
                report.skipped_missing_plan += 1;
                continue;
            }
        };
        if let Some(cached) = latest_market_advisory_for_endpoints(
            state,
            &request,
            fallback_descriptor.as_ref(),
        )
        .await?
        {
            report.cache_hits += 1;
            if !reward_provider_cache_refresh_due(
                cached.expires_at,
                cycle.config.ai_advisory_ttl_sec,
                OffsetDateTime::now_utc(),
            ) {
                continue;
            }
        }
        // Re-check book readiness at flush time: the local cache entry may have
        // expired since the orderbook event enqueued this condition.
        if !reward_market_books_available(market, &books) {
            report.skipped_missing_book += 1;
            continue;
        }
        report.batched += 1;
        request_by_condition.insert(condition_id.to_string(), request.clone());
        requests.push(request);
    }

    if !requests.is_empty() {
        report.batch_calls += 1;
        let condition_ids_for_record = reward_ai_llm_condition_ids(&requests);
        let input_hash_for_record = reward_ai_llm_batch_input_hash(&requests);
        let started = Instant::now();
        let batch_outcome = {
            let _permit = acquire_reward_ai_advisory_provider_request_permit().await?;
            connector.advise_batch(&requests).await
        };
        record_reward_provider_llm_call(
            state,
            REWARD_AI_ADVISORY_LLM_TASK_TYPE,
            REWARD_AI_ADVISORY_PROMPT_VERSION,
            model,
            &input_hash_for_record,
            &condition_ids_for_record,
            started.elapsed(),
            batch_outcome.is_ok(),
            batch_outcome.as_ref().ok().map(|items| json!(items)),
            batch_outcome.as_ref().err().map(ToString::to_string),
            false,
            trace_id,
        )
        .await;
        match batch_outcome {
            Ok(items) => {
                let mut saved_set: HashSet<String> = HashSet::new();
                for item in items {
                    let Some(request) = request_by_condition.get(&item.condition_id) else {
                        continue;
                    };
                    let advisory = item.decision.into_advisory(
                        request,
                        cycle.config.ai_advisory_ttl_sec,
                        OffsetDateTime::now_utc(),
                    );
                    if let Err(error) = state
                        .reward_bot_service
                        .save_market_advisory(&advisory)
                        .await
                    {
                        report.failures += 1;
                        warn!(
                            trace_id = %trace_id,
                            condition_id = %item.condition_id,
                            error = %error,
                            "failed to save reward AI advisory from batch",
                        );
                        continue;
                    }
                    report.saved += 1;
                    saved_set.insert(item.condition_id.clone());
                }
                // Per-condition fallback for markets the batch omitted/mislabeled.
                let mut overloaded = false;
                for request in &requests {
                    if saved_set.contains(&request.condition_id) {
                        continue;
                    }
                    report.fallback_singles += 1;
                    match single_reward_ai_advise_and_save(
                        state,
                        &connector,
                        advisory_fallback_channel.as_ref(),
                        request,
                        &cycle.config,
                        trace_id,
                    )
                    .await
                    {
                        SingleAdviseOutcome::Saved => {
                            report.saved += 1;
                        }
                        SingleAdviseOutcome::Failed => report.failures += 1,
                        SingleAdviseOutcome::Overloaded => {
                            report.failures += 1;
                            overloaded = true;
                            break;
                        }
                    }
                }
                if overloaded {
                    warn!(
                        trace_id = %trace_id,
                        "reward AI advisory provider overloaded during batch fallback; stopping remaining singles",
                    );
                }
            }
            Err(error) => {
                report.failures += 1;
                warn!(
                    trace_id = %trace_id,
                    error = %error,
                    "reward AI advisory batch request failed; falling back to per-condition requests",
                );
                if !reward_ai_provider_is_overloaded(&error) {
                    for request in &requests {
                        report.fallback_singles += 1;
                        match single_reward_ai_advise_and_save(
                            state,
                            &connector,
                            advisory_fallback_channel.as_ref(),
                            request,
                            &cycle.config,
                            trace_id,
                        )
                        .await
                        {
                            SingleAdviseOutcome::Saved => report.saved += 1,
                            SingleAdviseOutcome::Failed => report.failures += 1,
                            SingleAdviseOutcome::Overloaded => {
                                report.failures += 1;
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    // Info-risk sweep for every condition that passed the eligibility filter,
    // mirroring the tick refresh's per-condition AI->info-risk ordering.
    if let Some(info_connector) = info_risk_connector.as_ref() {
        for condition_id in &info_risk_condition_ids {
            if refresh_reward_info_risk_for_condition(
                state,
                info_connector,
                info_risk_fallback_channel.as_ref(),
                &cycle,
                &markets_by_condition,
                condition_id,
                model,
                trace_id,
                &mut report.info_risk,
            )
            .await?
            {
                break;
            }
        }
    }

    Ok(())
}

async fn build_reward_batch_books_from_cache(
    cache: &RewardOrderbookLocalCache,
    market: &RewardMarket,
) -> HashMap<String, RewardOrderBook> {
    let token_ids: Vec<String> = market
        .tokens
        .iter()
        .map(|token| token.token_id.clone())
        .collect();
    cache
        .get_books(&token_ids)
        .await
        .into_iter()
        .map(|book| {
            let token_id = book.token_id.clone();
            (token_id, cached_order_book_to_reward(&book))
        })
        .collect()
}

enum SingleAdviseOutcome {
    Saved,
    Failed,
    Overloaded,
}

async fn single_reward_ai_advise_and_save(
    state: &AppState,
    connector: &RewardAiAdvisoryConnector,
    fallback_channel: Option<&RewardAiAdvisoryChannel<'_>>,
    request: &RewardAiAdvisoryRequest,
    config: &RewardBotConfig,
    trace_id: &str,
) -> SingleAdviseOutcome {
    let primary_channel = RewardAiAdvisoryChannel {
        connector,
        provider: request.provider,
        request_format: request.request_format,
        model: request.model.clone(),
    };
    let attempt = match advise_with_fallback(
        state,
        &primary_channel,
        fallback_channel,
        request,
        trace_id,
    )
    .await
    {
        Ok(attempt) => attempt,
        Err(error) => {
            warn!(
                trace_id = %trace_id,
                condition_id = %request.condition_id,
                error = %error,
                "reward AI advisory single fallback provider request permit failed",
            );
            return SingleAdviseOutcome::Failed;
        }
    };
    match attempt {
        RewardProviderAttempt::Success {
            decision,
            request: winning_request,
            ..
        } => {
            let advisory = decision.into_advisory(
                &winning_request,
                config.ai_advisory_ttl_sec,
                OffsetDateTime::now_utc(),
            );
            match state
                .reward_bot_service
                .save_market_advisory(&advisory)
                .await
            {
                Ok(()) => SingleAdviseOutcome::Saved,
                Err(error) => {
                    warn!(
                        trace_id = %trace_id,
                        condition_id = %request.condition_id,
                        error = %error,
                        "failed to save reward AI advisory (single fallback)",
                    );
                    SingleAdviseOutcome::Failed
                }
            }
        }
        RewardProviderAttempt::Failed {
            primary_error,
            fallback_error,
        } => {
            warn!(
                trace_id = %trace_id,
                condition_id = %request.condition_id,
                error = %primary_error,
                "reward AI advisory single fallback request failed",
            );
            if let Some(fb_error) = &fallback_error {
                warn!(
                    trace_id = %trace_id,
                    condition_id = %request.condition_id,
                    error = %fb_error,
                    "reward AI advisory single fallback also failed on fallback endpoint",
                );
            }
            if reward_combined_provider_overloaded(&primary_error, fallback_error.as_ref()) {
                SingleAdviseOutcome::Overloaded
            } else {
                SingleAdviseOutcome::Failed
            }
        }
    }
}

#[cfg(test)]
mod reward_ai_advisory_batch_tests {
    use super::*;

    #[test]
    fn reward_ai_advisory_batch_size_is_bounded() {
        assert_eq!(reward_ai_advisory_batch_size(0), 1);
        assert_eq!(reward_ai_advisory_batch_size(8), 8);
        assert_eq!(reward_ai_advisory_batch_size(100), 12);
    }

    // flush_reward_ai_advisory_batch is exercised end-to-end via the worker test
    // harness in tests.rs; the connector parsing layer is covered in
    // polyedge-connectors. The orderbook readiness state machine is covered in
    // orderbook_events.rs tests.
}
