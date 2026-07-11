fn spawn_reward_market_provider_refresh(
    state: &AppState,
    cycle: &RewardLiveCycle,
    trace_id: &str,
) {
    if !cycle.config.ai_advisory_enabled && !cycle.config.info_risk_enabled {
        return;
    }
    if cycle.plans.is_empty() && cycle.markets.is_empty() {
        return;
    }
    if REWARD_PROVIDER_REFRESH_RUNNING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        debug!(
            trace_id = %trace_id,
            "skipping reward provider refresh because another refresh is running",
        );
        return;
    }

    let state = state.clone();
    let cycle = cycle.clone();
    let trace_id = trace_id.to_string();
    tokio::spawn(async move {
        let refresh_timeout = reward_provider_refresh_timeout(&state);
        let result = tokio::time::timeout(
            refresh_timeout,
            refresh_reward_provider_cache(&state, cycle, &trace_id),
        )
        .await;
        REWARD_PROVIDER_REFRESH_RUNNING.store(false, Ordering::Release);
        match result {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => {
                warn!(
                    trace_id = %trace_id,
                    error = %error,
                    "reward provider refresh failed",
                );
            }
            Err(_) => {
                warn!(
                    trace_id = %trace_id,
                    timeout_ms = refresh_timeout.as_millis(),
                    "reward provider refresh timed out; stopping this refresh cycle",
                );
            }
        }
    });
}

const REWARD_PROVIDER_BATCH_CONDITIONS: usize = 10;
const REWARD_PROVIDER_REFRESH_MIN_TIMEOUT_SECS: u64 = 30;
const REWARD_PROVIDER_REFRESH_MAX_TIMEOUT_SECS: u64 = 120;

fn reward_provider_refresh_timeout(state: &AppState) -> Duration {
    let timeout_secs = state
        .settings
        .rewards
        .poll_interval_secs
        .saturating_mul(2)
        .clamp(
            REWARD_PROVIDER_REFRESH_MIN_TIMEOUT_SECS,
            REWARD_PROVIDER_REFRESH_MAX_TIMEOUT_SECS,
        );
    Duration::from_secs(timeout_secs)
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct RewardProviderRefreshReport {
    candidates: usize,
    checked: usize,
    cache_hits: usize,
    requested: usize,
    saved: usize,
    failures: usize,
    skipped_missing_market: usize,
    advisory_saved: usize,
    info_risk_saved: usize,
    applied_plans: usize,
    request_cap_exhausted: bool,
}

#[derive(Debug, Default)]
struct RewardProviderConditionOutcome {
    report: RewardProviderRefreshReport,
    stop_refresh: bool,
    saved_advisory: Option<(String, RewardMarketAdvisory)>,
}

impl RewardProviderRefreshReport {
    fn merge_condition(&mut self, outcome: &RewardProviderConditionOutcome) {
        self.checked += outcome.report.checked;
        self.cache_hits += outcome.report.cache_hits;
        self.requested += outcome.report.requested;
        self.saved += outcome.report.saved;
        self.failures += outcome.report.failures;
        self.skipped_missing_market += outcome.report.skipped_missing_market;
        self.advisory_saved += outcome.report.advisory_saved;
        self.info_risk_saved += outcome.report.info_risk_saved;
        self.request_cap_exhausted |= outcome.report.request_cap_exhausted;
    }
}

fn try_consume_reward_provider_request_budget(remaining: &AtomicUsize) -> bool {
    let mut current = remaining.load(Ordering::Acquire);
    loop {
        if current == 0 {
            return false;
        }
        match remaining.compare_exchange_weak(
            current,
            current - 1,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => return true,
            Err(next) => current = next,
        }
    }
}

async fn reward_provider_markets_by_condition(
    state: &AppState,
    cycle: &RewardLiveCycle,
) -> Result<HashMap<String, RewardMarket>> {
    let mut markets_by_condition = state
        .reward_bot_service
        .list_active_reward_markets()
        .await?
        .into_iter()
        .map(|market| (market.condition_id.clone(), market))
        .collect::<HashMap<_, _>>();
    for market in &cycle.markets {
        markets_by_condition.insert(market.condition_id.clone(), market.clone());
    }
    Ok(markets_by_condition)
}

/// Combined rewards provider refresh: for each selected market a SINGLE provider
/// call carries the AI advisory context, the info-risk context, or both. Each
/// section is saved into its own cache table (keyed/TTL'd independently) so the
/// live-tick gate behavior is unchanged. Multi-market batching is gone — one
/// call per market, primary-then-fallback.
async fn refresh_reward_provider_cache(
    state: &AppState,
    mut cycle: RewardLiveCycle,
    trace_id: &str,
) -> Result<RewardProviderRefreshReport> {
    let mut report = RewardProviderRefreshReport::default();
    if !cycle.config.ai_advisory_enabled && !cycle.config.info_risk_enabled {
        return Ok(report);
    }

    let model = reward_ai_model_for_provider(&state.settings.rewards, cycle.config.ai_provider);
    let request_format = reward_ai_effective_request_format_for_model(&cycle.config, model);
    if model.is_empty() {
        warn!(
            trace_id = %trace_id,
            "reward provider model is empty; skipping provider refresh"
        );
        return Ok(report);
    }

    let Some(connector) = build_reward_provider_connector(state, &cycle.config)? else {
        warn!(
            trace_id = %trace_id,
            provider = cycle.config.ai_provider.as_str(),
            "reward provider is enabled but configuration is incomplete; skipping refresh",
        );
        return Ok(report);
    };
    let fallback_descriptor = resolve_reward_ai_fallback(&state.settings.rewards);
    let fallback_connector = match &fallback_descriptor {
        Some(descriptor) => Some(build_reward_provider_fallback_connector(state, descriptor)?),
        None => None,
    };
    let fallback_channel = match (&fallback_descriptor, fallback_connector.as_ref()) {
        (Some(descriptor), Some(connector)) => Some(RewardProviderChannel {
            connector,
            provider: descriptor.provider,
            request_format: descriptor.request_format,
            model: descriptor.model.clone(),
        }),
        _ => None,
    };
    if fallback_channel.is_some() {
        info!(
            trace_id = %trace_id,
            "reward provider fallback endpoint is configured",
        );
    }
    let provider_concurrency = reward_provider_concurrency(&cycle.config);
    let condition_parallelism =
        reward_provider_condition_parallelism(&provider_concurrency, fallback_channel.is_some());
    let markets_by_condition = reward_provider_markets_by_condition(state, &cycle).await?;

    let now = OffsetDateTime::now_utc();
    let mut union: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    if cycle.config.ai_advisory_enabled {
        for condition_id in reward_ai_advisory_candidate_condition_ids(
            &cycle.plans,
            &cycle.open_orders,
            &cycle.positions,
            &cycle.pre_ai_eligible_condition_ids,
            &cycle.config,
            model,
            fallback_descriptor.as_ref(),
            now,
        ) {
            if seen.insert(condition_id.clone()) {
                union.push(condition_id);
            }
        }
    }
    if cycle.config.info_risk_enabled {
        for condition_id in reward_info_risk_candidate_conditions(
            &cycle.markets,
            &cycle.plans,
            &cycle.open_orders,
            &cycle.positions,
            &cycle.config,
            model,
            fallback_descriptor.as_ref(),
            now,
        ) {
            if seen.insert(condition_id.clone()) {
                union.push(condition_id);
            }
        }
    }
    report.candidates = union.len();

    let ordered = reward_provider_refresh_candidate_condition_ids(
        &union,
        &cycle.plans,
        &cycle.open_orders,
        &cycle.positions,
        &cycle.config,
    );
    let original_ordered_conditions = ordered.len();
    let max_conditions = reward_provider_max_conditions_per_cycle(state);
    let request_budget = Arc::new(AtomicUsize::new(max_conditions));

    info!(
        trace_id = %trace_id,
        provider = cycle.config.ai_provider.as_str(),
        request_format = request_format.as_str(),
        ordered_conditions = original_ordered_conditions,
        max_provider_requests = max_conditions,
        primary_concurrency = provider_concurrency.primary_limit,
        fallback_concurrency = provider_concurrency.fallback_limit,
        condition_parallelism,
        provider_candidates = report.candidates,
        "starting reward provider refresh",
    );

    let refresh_result: Result<()> = async {
        if max_conditions == 0 {
            debug!(
                trace_id = %trace_id,
                "reward provider refresh request cap is zero; skipping provider requests",
            );
        } else {
            let mut stop_refresh = false;
            for condition_batch in ordered.chunks(REWARD_PROVIDER_BATCH_CONDITIONS) {
                let cycle_snapshot = Arc::new(cycle.clone());
                let markets_snapshot = Arc::new(markets_by_condition.clone());
                let outcomes = futures::stream::iter(condition_batch.iter().cloned())
                    .map(|condition_id| {
                        let state = state.clone();
                        let connector = connector.clone();
                        let fallback_descriptor = fallback_descriptor.clone();
                        let fallback_connector = fallback_connector.clone();
                        let provider_concurrency = provider_concurrency.clone();
                        let cycle = cycle_snapshot.clone();
                        let markets_by_condition = markets_snapshot.clone();
                        let model = model.to_string();
                        let trace_id = trace_id.to_string();
                        let request_budget = request_budget.clone();
                        async move {
                            let fallback_channel =
                                match (&fallback_descriptor, fallback_connector.as_ref()) {
                                    (Some(descriptor), Some(connector)) => {
                                        Some(RewardProviderChannel {
                                            connector,
                                            provider: descriptor.provider,
                                            request_format: descriptor.request_format,
                                            model: descriptor.model.clone(),
                                        })
                                    }
                                    _ => None,
                                };
                            refresh_reward_provider_for_condition(
                                &state,
                                &connector,
                                fallback_channel.as_ref(),
                                &provider_concurrency,
                                &cycle,
                                &markets_by_condition,
                                &condition_id,
                                &model,
                                &request_budget,
                                &trace_id,
                            )
                            .await
                        }
                    })
                    .buffer_unordered(condition_parallelism)
                    .collect::<Vec<_>>()
                    .await;

                for outcome in outcomes {
                    let outcome = outcome?;
                    report.merge_condition(&outcome);
                    if let Some((condition_id, advisory)) = outcome.saved_advisory {
                        apply_reward_ai_advisory_to_refresh_cycle(
                            &mut cycle,
                            &condition_id,
                            advisory,
                            trace_id,
                        );
                    }
                    if outcome.stop_refresh {
                        stop_refresh = true;
                        break;
                    }
                }
                if stop_refresh {
                    break;
                }
            }
        }
        Ok(())
    }
    .await;

    refresh_result?;

    report.applied_plans = apply_cached_reward_info_risks(state, trace_id).await?;
    info!(
        trace_id = %trace_id,
        provider_candidates = report.candidates,
        provider_checked = report.checked,
        provider_cache_hits = report.cache_hits,
        provider_requested = report.requested,
        provider_request_cap_exhausted = report.request_cap_exhausted,
        provider_saved = report.saved,
        provider_failures = report.failures,
        provider_skipped_missing_market = report.skipped_missing_market,
        advisory_saved = report.advisory_saved,
        info_risk_saved = report.info_risk_saved,
        applied_plans = report.applied_plans,
        "completed reward provider refresh",
    );
    Ok(report)
}

/// Evaluate a single market through the combined provider. Returns `true` to
/// stop the cycle early (provider overloaded). The combined request only
/// carries the sections that are both enabled and due for refresh, so a market
/// with a fresh advisory but a stale info-risk (or vice versa) still gets a
/// single call for just the stale section.
#[allow(clippy::too_many_arguments)]
async fn refresh_reward_provider_for_condition(
    state: &AppState,
    connector: &RewardProviderConnector,
    fallback_channel: Option<&RewardProviderChannel<'_>>,
    provider_concurrency: &RewardProviderConcurrency,
    cycle: &RewardLiveCycle,
    markets_by_condition: &HashMap<String, RewardMarket>,
    condition_id: &str,
    model: &str,
    request_budget: &AtomicUsize,
    trace_id: &str,
) -> Result<RewardProviderConditionOutcome> {
    let mut outcome = RewardProviderConditionOutcome::default();
    outcome.report.checked += 1;
    let Some(market) = markets_by_condition.get(condition_id) else {
        outcome.report.skipped_missing_market += 1;
        return Ok(outcome);
    };
    let plan = cycle.plans.iter().find(|p| p.condition_id == condition_id);
    let request_format = reward_ai_effective_request_format_for_model(&cycle.config, model);
    let now = OffsetDateTime::now_utc();
    let fallback = resolve_reward_ai_fallback(&state.settings.rewards);

    // Build the advisory sub-request only when AI advisory is enabled AND a plan
    // exists for this condition.
    let advisory_request = if cycle.config.ai_advisory_enabled {
        if let Some(plan_for_request) = plan {
            let candles = state
                .reward_bot_service
                .list_recent_market_candles(
                    condition_id,
                    REWARD_AI_CANDLE_SOURCE_INTERVAL_SEC,
                    REWARD_AI_CANDLE_SOURCE_LIMIT_PER_TOKEN,
                )
                .await?;
            Some(build_reward_ai_advisory_request(
                market,
                plan_for_request,
                &cycle.account,
                &cycle.positions,
                &cycle.open_orders,
                &candles,
                &cycle.config,
                cycle.config.ai_advisory_ttl_sec,
                cycle.config.ai_provider,
                request_format,
                model,
            )?)
        } else {
            None
        }
    } else {
        None
    };

    // Build the info-risk sub-request only when info-risk is enabled.
    let info_risk_request = if cycle.config.info_risk_enabled {
        Some(build_reward_info_risk_assessment_request(
            market,
            plan,
            &cycle.account,
            &cycle.positions,
            &cycle.open_orders,
            &cycle.config,
            cycle.config.ai_provider,
            request_format,
            model,
        )?)
    } else {
        None
    };

    if advisory_request.is_none() && info_risk_request.is_none() {
        return Ok(outcome);
    }

    // Cache checks: a section is "due" only when it is enabled, has a request,
    // and either has no cached row or the cached row is inside the early-refresh
    // window. A cache hit still counts toward the report.
    let mut advisory_due = advisory_request.is_some();
    if let Some(request) = &advisory_request {
        if let Some(cached) =
            latest_market_advisory_for_endpoints(state, request, fallback.as_ref()).await?
        {
            outcome.report.cache_hits += 1;
            if !reward_provider_cache_refresh_due(
                cached.expires_at,
                cycle.config.ai_advisory_ttl_sec,
                now,
            ) {
                advisory_due = false;
            }
        }
    }
    let mut info_risk_due = info_risk_request.is_some();
    if let Some(request) = &info_risk_request {
        if let Some(cached) =
            latest_market_info_risk_for_endpoints(state, request, fallback.as_ref()).await?
        {
            outcome.report.cache_hits += 1;
            if !reward_provider_cache_refresh_due(
                cached.expires_at,
                cycle.config.info_risk_ttl_sec,
                now,
            ) {
                info_risk_due = false;
            }
        }
    }

    if !advisory_due && !info_risk_due {
        return Ok(outcome);
    }

    // Build the combined request with only the DUE sections. At least one is
    // Some because we returned early when both were not due.
    let advisory = advisory_request.clone().filter(|_| advisory_due);
    let info_risk = info_risk_request.clone().filter(|_| info_risk_due);
    let combined = polyedge_application::RewardProviderRequest {
        condition_id: condition_id.to_string(),
        provider: cycle.config.ai_provider,
        request_format,
        model: model.to_string(),
        advisory,
        info_risk,
    };

    if !try_consume_reward_provider_request_budget(request_budget) {
        outcome.report.request_cap_exhausted = true;
        outcome.stop_refresh = true;
        debug!(
            trace_id = %trace_id,
            condition_id = %condition_id,
            "reward provider request cap exhausted; stopping refresh before provider call",
        );
        return Ok(outcome);
    }
    outcome.report.requested += 1;
    info!(
        trace_id = %trace_id,
        condition_id = %condition_id,
        wants_advisory = combined.wants_advisory(),
        wants_info_risk = combined.wants_info_risk(),
        requested = outcome.report.requested,
        "requesting reward provider",
    );
    let primary_channel = RewardProviderChannel {
        connector,
        provider: cycle.config.ai_provider,
        request_format,
        model: model.to_string(),
    };
    let attempt = evaluate_with_fallback(
        state,
        &primary_channel,
        fallback_channel,
        provider_concurrency,
        &combined,
        trace_id,
    )
    .await?;
    match attempt {
        RewardProviderAttempt::Success {
            decision,
            endpoint,
            request: winning,
        } => {
            if let Some(advisory_decision) = decision.advisory {
                if let Some(request) = &winning.advisory {
                    let advisory = advisory_decision.into_advisory(
                        request,
                        cycle.config.ai_advisory_ttl_sec,
                        OffsetDateTime::now_utc(),
                    );
                    state
                        .reward_bot_service
                        .save_market_advisory(&advisory)
                        .await?;
                    outcome.report.saved += 1;
                    outcome.report.advisory_saved += 1;
                    outcome.saved_advisory =
                        Some((condition_id.to_string(), advisory.clone()));
                    info!(
                        trace_id = %trace_id,
                        condition_id = %condition_id,
                        endpoint = ?endpoint,
                        saved = outcome.report.saved,
                        "saved reward provider advisory",
                    );
                }
            }
            if let Some(info_risk_decision) = decision.info_risk {
                if let Some(request) = &winning.info_risk {
                    let risk = info_risk_decision.into_info_risk(
                        request,
                        cycle.config.info_risk_ttl_sec,
                        OffsetDateTime::now_utc(),
                    );
                    state
                        .reward_bot_service
                        .save_market_info_risk(&risk)
                        .await?;
                    outcome.report.saved += 1;
                    outcome.report.info_risk_saved += 1;
                    info!(
                        trace_id = %trace_id,
                        condition_id = %condition_id,
                        endpoint = ?endpoint,
                        saved = outcome.report.saved,
                        "saved reward provider info risk",
                    );
                }
            }
            Ok(outcome)
        }
        RewardProviderAttempt::Failed {
            primary_error,
            fallback_error,
        } => {
            outcome.report.failures += 1;
            warn!(
                trace_id = %trace_id,
                condition_id = %condition_id,
                error = %primary_error,
                "reward provider request failed",
            );
            if let Some(fb_error) = &fallback_error {
                warn!(
                    trace_id = %trace_id,
                    condition_id = %condition_id,
                    error = %fb_error,
                    "reward provider fallback request also failed",
                );
            }

            // Content-filter rejections are non-retryable input rejections. Write
            // a fail-closed cache row for each due section and treat it as
            // handled (not overload).
            let mut handled = false;
            if advisory_due {
                if let Some(request) = &advisory_request {
                    if let Some(advisory) = cache_reward_ai_content_filter_if_rejected(
                        state,
                        request,
                        cycle.config.ai_advisory_ttl_sec,
                        &primary_error,
                        fallback_error.as_ref(),
                        trace_id,
                    )
                    .await?
                    {
                        outcome.report.saved += 1;
                        outcome.report.advisory_saved += 1;
                        outcome.saved_advisory =
                            Some((condition_id.to_string(), advisory));
                        handled = true;
                    }
                }
            }
            if info_risk_due {
                if let Some(request) = &info_risk_request {
                    if cache_reward_info_risk_content_filter_if_rejected(
                        state,
                        request,
                        cycle.config.info_risk_ttl_sec,
                        &primary_error,
                        fallback_error.as_ref(),
                        trace_id,
                    )
                    .await?
                    .is_some()
                    {
                        outcome.report.saved += 1;
                        outcome.report.info_risk_saved += 1;
                        handled = true;
                    }
                }
            }
            if handled {
                return Ok(outcome);
            }

            if reward_combined_provider_overloaded(&primary_error, fallback_error.as_ref()) {
                warn!(
                    trace_id = %trace_id,
                    requested = outcome.report.requested,
                    failures = outcome.report.failures,
                    "reward provider is overloaded; stopping market provider requests for this cycle",
                );
                outcome.stop_refresh = true;
                return Ok(outcome);
            }
            Ok(outcome)
        }
    }
}

/// Apply a freshly saved advisory to the refresh task's in-memory cycle. The
/// live tick owns `rewards_eligible` registration after saving final quote
/// plans, so provider refresh does not mutate that orderbook source directly.
fn apply_reward_ai_advisory_to_refresh_cycle(
    cycle: &mut RewardLiveCycle,
    condition_id: &str,
    advisory: RewardMarketAdvisory,
    trace_id: &str,
) {
    let mut applied = 0usize;
    for plan in cycle
        .plans
        .iter_mut()
        .filter(|plan| plan.condition_id == condition_id)
    {
        plan.ai_advisory = Some(advisory.clone());
        applied += 1;
    }
    if applied > 0 {
        debug!(
            trace_id = %trace_id,
            condition_id = %condition_id,
            profiles = applied,
            "applied saved reward provider advisory to refresh cycle",
        );
    }
}
