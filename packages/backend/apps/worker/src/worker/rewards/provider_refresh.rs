fn spawn_reward_market_provider_refresh(
    state: &AppState,
    cycle: &RewardLiveCycle,
    books: &HashMap<String, RewardOrderBook>,
    trace_id: &str,
) {
    if cycle.config.ai_advisory_enabled {
        spawn_reward_ai_advisory_provider_refresh(state, cycle, books, trace_id);
    }
    if cycle.config.info_risk_enabled {
        spawn_reward_info_risk_provider_refresh(state, cycle, trace_id);
    }
}

fn spawn_reward_ai_advisory_provider_refresh(
    state: &AppState,
    cycle: &RewardLiveCycle,
    books: &HashMap<String, RewardOrderBook>,
    trace_id: &str,
) {
    if cycle.plans.is_empty() {
        return;
    }
    if REWARD_AI_PROVIDER_REFRESH_RUNNING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        debug!(
            trace_id = %trace_id,
            "skipping reward AI advisory provider refresh because another AI refresh is running",
        );
        return;
    }

    let state = state.clone();
    let cycle = cycle.clone();
    let books = books.clone();
    let trace_id = trace_id.to_string();
    tokio::spawn(async move {
        let result =
            refresh_reward_ai_advisory_provider_cache(&state, cycle, books, &trace_id).await;
        REWARD_AI_PROVIDER_REFRESH_RUNNING.store(false, Ordering::Release);
        if let Err(error) = result {
            warn!(
                trace_id = %trace_id,
                error = %error,
                "reward AI advisory provider refresh failed",
            );
        }
    });
}

fn spawn_reward_info_risk_provider_refresh(
    state: &AppState,
    cycle: &RewardLiveCycle,
    trace_id: &str,
) {
    if cycle.plans.is_empty() && cycle.markets.is_empty() {
        return;
    }
    if REWARD_INFO_RISK_PROVIDER_REFRESH_RUNNING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        debug!(
            trace_id = %trace_id,
            "skipping reward info risk provider refresh because another info-risk refresh is running",
        );
        return;
    }

    let state = state.clone();
    let cycle = cycle.clone();
    let trace_id = trace_id.to_string();
    tokio::spawn(async move {
        let result = refresh_reward_info_risk_provider_cache(&state, cycle, &trace_id).await;
        REWARD_INFO_RISK_PROVIDER_REFRESH_RUNNING.store(false, Ordering::Release);
        if let Err(error) = result {
            warn!(
                trace_id = %trace_id,
                error = %error,
                "reward info risk provider refresh failed",
            );
        }
    });
}

const REWARD_AI_PROVIDER_ORDERBOOK_SOURCE: &str = "rewards_ai_provider";
const REWARD_AI_PROVIDER_ORDERBOOK_MARKETS_PER_BATCH: usize = 10;
const REWARD_AI_PROVIDER_ORDERBOOK_WAIT_ATTEMPTS: usize = 8;
const REWARD_AI_PROVIDER_ORDERBOOK_WAIT_DELAY: Duration = Duration::from_secs(2);

async fn refresh_reward_ai_advisory_provider_cache(
    state: &AppState,
    mut cycle: RewardLiveCycle,
    books: HashMap<String, RewardOrderBook>,
    trace_id: &str,
) -> Result<RewardAiAdvisoryRefreshReport> {
    let mut report = RewardAiAdvisoryRefreshReport::default();
    if !cycle.config.ai_advisory_enabled || cycle.plans.is_empty() {
        return Ok(report);
    }

    let model = state.settings.rewards.ai_model.trim();
    if model.is_empty() {
        warn!(
            trace_id = %trace_id,
            "reward AI advisory model is empty; skipping AI provider refresh"
        );
        return Ok(report);
    }

    let Some(connector) = build_reward_ai_advisory_connector(state, &cycle.config)? else {
        warn!(
            trace_id = %trace_id,
            provider = cycle.config.ai_provider.as_str(),
            "reward AI advisory is enabled but provider configuration is incomplete; skipping AI provider refresh",
        );
        return Ok(report);
    };
    let fallback_descriptor = resolve_reward_ai_fallback(&state.settings.rewards);
    let fallback_connector = match &fallback_descriptor {
        Some(descriptor) => Some(build_reward_ai_advisory_fallback_connector(state, descriptor)?),
        None => None,
    };
    let fallback_channel = match (&fallback_descriptor, fallback_connector.as_ref()) {
        (Some(descriptor), Some(connector)) => Some(RewardAiAdvisoryChannel {
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
            "reward AI advisory fallback endpoint is configured",
        );
    }
    let markets_by_condition = reward_provider_markets_by_condition(state, &cycle).await?;

    let now = OffsetDateTime::now_utc();
    let ai_candidate_condition_ids = reward_ai_advisory_candidate_condition_ids(
        &cycle.plans,
        &cycle.open_orders,
        &cycle.positions,
        &cycle.pre_ai_eligible_condition_ids,
        &cycle.config,
        model,
        fallback_descriptor.as_ref(),
        now,
    );
    let mut ordered_conditions = reward_provider_refresh_candidate_condition_ids(
        &ai_candidate_condition_ids,
        &cycle.plans,
        &cycle.open_orders,
        &cycle.positions,
        &cycle.config,
    );
    let original_ordered_conditions = ordered_conditions.len();
    let max_conditions = reward_provider_max_conditions_per_cycle(state);
    if ordered_conditions.len() > max_conditions {
        ordered_conditions.truncate(max_conditions);
    }

    report.candidates = ai_candidate_condition_ids.len();
    info!(
        trace_id = %trace_id,
        provider = cycle.config.ai_provider.as_str(),
        request_format = cycle.config.ai_request_format.as_str(),
        conditions = original_ordered_conditions,
        selected_conditions = ordered_conditions.len(),
        max_conditions,
        ai_candidates = report.candidates,
        "starting reward AI advisory provider refresh",
    );

    let refresh_result: Result<()> = async {
        let mut ai_promoted_tokens = Vec::new();
        for condition_batch in ordered_conditions.chunks(REWARD_AI_PROVIDER_ORDERBOOK_MARKETS_PER_BATCH) {
            let batch_books = prepare_reward_ai_provider_orderbook_batch(
                state,
                &books,
                &markets_by_condition,
                condition_batch,
                trace_id,
            )
            .await?;

            if refresh_reward_ai_advisory_provider_batch(
                state,
                &connector,
                fallback_channel.as_ref(),
                &mut cycle,
                &batch_books,
                &markets_by_condition,
                condition_batch,
                model,
                trace_id,
                &mut report,
                &mut ai_promoted_tokens,
            )
            .await?
            {
                break;
            }
        }
        Ok(())
    }
    .await;

    if let Err(error) = state
        .orderbook_registry
        .register_tokens(REWARD_AI_PROVIDER_ORDERBOOK_SOURCE, &[])
        .await
    {
        warn!(
            trace_id = %trace_id,
            source = REWARD_AI_PROVIDER_ORDERBOOK_SOURCE,
            error = %error,
            "failed to clear temporary reward AI provider orderbook source",
        );
    }
    refresh_result?;

    info!(
        trace_id = %trace_id,
        ai_candidates = report.candidates,
        ai_cache_hits = report.cache_hits,
        ai_requested = report.requested,
        ai_saved = report.saved,
        ai_failures = report.failures,
        ai_skipped_missing_market = report.skipped_missing_market,
        ai_skipped_missing_plan = report.skipped_missing_plan,
        ai_skipped_missing_book = report.skipped_missing_book,
        "completed reward AI advisory provider refresh",
    );
    Ok(report)
}

async fn refresh_reward_info_risk_provider_cache(
    state: &AppState,
    cycle: RewardLiveCycle,
    trace_id: &str,
) -> Result<RewardInfoRiskScanReport> {
    let mut report = RewardInfoRiskScanReport::default();
    if !cycle.config.info_risk_enabled {
        return Ok(report);
    }

    let model = state.settings.rewards.ai_model.trim();
    if model.is_empty() {
        warn!(trace_id = %trace_id, "reward info risk model is empty");
        return Ok(report);
    }

    let Some(connector) = build_reward_info_risk_connector(state, &cycle.config)? else {
        warn!(
            trace_id = %trace_id,
            provider = cycle.config.ai_provider.as_str(),
            "reward info risk is enabled but provider configuration is incomplete; skipping info-risk provider refresh",
        );
        return Ok(report);
    };
    let info_risk_fallback_descriptor = resolve_reward_ai_fallback(&state.settings.rewards);
    let info_risk_fallback_connector = match &info_risk_fallback_descriptor {
        Some(descriptor) => Some(build_reward_info_risk_fallback_connector(state, descriptor)?),
        None => None,
    };
    let info_risk_fallback_channel = match (
        &info_risk_fallback_descriptor,
        info_risk_fallback_connector.as_ref(),
    ) {
        (Some(descriptor), Some(connector)) => Some(RewardInfoRiskChannel {
            connector,
            provider: descriptor.provider,
            request_format: descriptor.request_format,
            model: descriptor.model.clone(),
        }),
        _ => None,
    };

    let markets_by_condition = reward_provider_markets_by_condition(state, &cycle).await?;
    let info_risk_candidate_condition_ids = reward_info_risk_candidate_conditions(
        &cycle.markets,
        &cycle.plans,
        &cycle.open_orders,
        &cycle.positions,
        &cycle.config,
    );
    let mut ordered_conditions = reward_provider_refresh_candidate_condition_ids(
        &info_risk_candidate_condition_ids,
        &cycle.plans,
        &cycle.open_orders,
        &cycle.positions,
        &cycle.config,
    );
    let original_ordered_conditions = ordered_conditions.len();
    let max_conditions = reward_provider_max_conditions_per_cycle(state);
    if ordered_conditions.len() > max_conditions {
        ordered_conditions.truncate(max_conditions);
    }

    report.candidates = info_risk_candidate_condition_ids.len();
    info!(
        trace_id = %trace_id,
        provider = cycle.config.ai_provider.as_str(),
        request_format = cycle.config.ai_request_format.as_str(),
        conditions = original_ordered_conditions,
        selected_conditions = ordered_conditions.len(),
        max_conditions,
        info_risk_candidates = report.candidates,
        "starting reward info risk provider refresh",
    );

    if refresh_reward_info_risk_provider_batch(
        state,
        &connector,
        info_risk_fallback_channel.as_ref(),
        &cycle,
        &markets_by_condition,
        &ordered_conditions,
        model,
        trace_id,
        &mut report,
    )
    .await?
    {
        debug!(
            trace_id = %trace_id,
            requested = report.requested,
            failures = report.failures,
            "stopped reward info risk provider refresh early",
        );
    }

    report.applied_plans = apply_cached_reward_info_risks(state, trace_id).await?;
    info!(
        trace_id = %trace_id,
        info_risk_candidates = report.candidates,
        info_risk_cache_hits = report.cache_hits,
        info_risk_requested = report.requested,
        info_risk_saved = report.saved,
        info_risk_failures = report.failures,
        info_risk_skipped_missing_market = report.skipped_missing_market,
        info_risk_applied_plans = report.applied_plans,
        "completed reward info risk provider refresh",
    );
    Ok(report)
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

struct RewardAiAdvisoryRefreshStep {
    stop_cycle: bool,
    advisory: Option<RewardMarketAdvisory>,
}

#[allow(clippy::too_many_arguments)]
async fn refresh_reward_ai_advisory_for_condition(
    state: &AppState,
    connector: &RewardAiAdvisoryConnector,
    fallback_channel: Option<&RewardAiAdvisoryChannel<'_>>,
    cycle: &RewardLiveCycle,
    books: &HashMap<String, RewardOrderBook>,
    markets_by_condition: &HashMap<String, RewardMarket>,
    condition_id: &str,
    model: &str,
    trace_id: &str,
    report: &mut RewardAiAdvisoryRefreshReport,
) -> Result<RewardAiAdvisoryRefreshStep> {
    let Some(plan_for_request) = cycle
        .plans
        .iter()
        .find(|plan| plan.condition_id == condition_id)
    else {
        report.skipped_missing_plan += 1;
        return Ok(RewardAiAdvisoryRefreshStep {
            stop_cycle: false,
            advisory: None,
        });
    };
    let Some(market) = markets_by_condition.get(condition_id) else {
        report.skipped_missing_market += 1;
        return Ok(RewardAiAdvisoryRefreshStep {
            stop_cycle: false,
            advisory: None,
        });
    };
    let candles = state
        .reward_bot_service
        .list_recent_market_candles(
            condition_id,
            REWARD_AI_CANDLE_SOURCE_INTERVAL_SEC,
            REWARD_AI_CANDLE_SOURCE_LIMIT_PER_TOKEN,
        )
        .await?;
    let request = build_reward_ai_advisory_request(
        market,
        plan_for_request,
        &cycle.account,
        &cycle.positions,
        &cycle.open_orders,
        books,
        &candles,
        &cycle.config,
        cycle.config.ai_advisory_ttl_sec,
        cycle.config.ai_provider,
        cycle.config.ai_request_format,
        model,
    )?;
    let mut cached_advisory = None;
    if let Some(cached) = state
        .reward_bot_service
        .latest_market_advisory(&request)
        .await?
    {
        report.cache_hits += 1;
        if !reward_provider_cache_refresh_due(
            cached.expires_at,
            cycle.config.ai_advisory_ttl_sec,
            OffsetDateTime::now_utc(),
        ) {
            return Ok(RewardAiAdvisoryRefreshStep {
                stop_cycle: false,
                advisory: Some(cached),
            });
        }
        cached_advisory = Some(cached);
    }

    // Defer the advisory until the orderbook service has published real books
    // for this market. The request payload otherwise carries null bids/asks,
    // and the system prompt tells the model to favor watch/avoid when data is
    // thin — so a no-book market would get cached as a watch/avoid "no
    // orderbook" advisory for the full TTL, blocking the market even after the
    // book arrives in a later tick. The advisory cache key excludes book data
    // by design, so the only safe fix is to not request until books exist.
    if !reward_market_books_available(market, books) {
        report.skipped_missing_book += 1;
        return Ok(RewardAiAdvisoryRefreshStep {
            stop_cycle: false,
            advisory: cached_advisory,
        });
    }

    report.requested += 1;
    info!(
        trace_id = %trace_id,
        condition_id = %condition_id,
        requested = report.requested,
        "requesting reward AI advisory provider",
    );
    let primary_channel = RewardAiAdvisoryChannel {
        connector,
        provider: cycle.config.ai_provider,
        request_format: cycle.config.ai_request_format,
        model: model.to_string(),
    };
    let attempt = advise_with_fallback(
        state,
        &primary_channel,
        fallback_channel,
        &request,
        trace_id,
    )
    .await?;
    match attempt {
        RewardProviderAttempt::Success {
            decision,
            endpoint,
            request: winning_request,
        } => {
            let advisory = decision.into_advisory(
                &winning_request,
                cycle.config.ai_advisory_ttl_sec,
                OffsetDateTime::now_utc(),
            );
            state
                .reward_bot_service
                .save_market_advisory(&advisory)
                .await?;
            report.saved += 1;
            info!(
                trace_id = %trace_id,
                condition_id = %condition_id,
                endpoint = ?endpoint,
                saved = report.saved,
                "saved reward AI advisory",
            );
            Ok(RewardAiAdvisoryRefreshStep {
                stop_cycle: false,
                advisory: Some(advisory),
            })
        }
        RewardProviderAttempt::Failed {
            primary_error,
            fallback_error,
        } => {
            report.failures += 1;
            warn!(
                trace_id = %trace_id,
                condition_id = %condition_id,
                error = %primary_error,
                "reward AI advisory request failed; keeping existing cached state",
            );
            if let Some(fb_error) = &fallback_error {
                warn!(
                    trace_id = %trace_id,
                    condition_id = %condition_id,
                    error = %fb_error,
                    "reward AI advisory fallback request also failed",
                );
            }
            if reward_combined_provider_overloaded(&primary_error, fallback_error.as_ref()) {
                warn!(
                    trace_id = %trace_id,
                    requested = report.requested,
                    failures = report.failures,
                    "reward AI advisory provider is overloaded; stopping market provider requests for this cycle",
                );
                return Ok(RewardAiAdvisoryRefreshStep {
                    stop_cycle: true,
                    advisory: cached_advisory,
                });
            }
            Ok(RewardAiAdvisoryRefreshStep {
                stop_cycle: false,
                advisory: cached_advisory,
            })
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn refresh_reward_info_risk_for_condition(
    state: &AppState,
    connector: &RewardInfoRiskConnector,
    fallback_channel: Option<&RewardInfoRiskChannel<'_>>,
    cycle: &RewardLiveCycle,
    markets_by_condition: &HashMap<String, RewardMarket>,
    condition_id: &str,
    model: &str,
    trace_id: &str,
    report: &mut RewardInfoRiskScanReport,
) -> Result<bool> {
    let Some(market) = markets_by_condition.get(condition_id) else {
        report.skipped_missing_market += 1;
        return Ok(false);
    };
    let plan_for_request = cycle
        .plans
        .iter()
        .find(|plan| plan.condition_id == condition_id);
    let request = build_reward_info_risk_assessment_request(
        market,
        plan_for_request,
        &cycle.account,
        &cycle.positions,
        &cycle.open_orders,
        &cycle.config,
        cycle.config.ai_provider,
        cycle.config.ai_request_format,
        model,
    )?;
    let primary_cached = state
        .reward_bot_service
        .latest_market_info_risk(&request)
        .await?;
    let cached = match fallback_channel {
        Some(fb) => {
            let fb_request = reward_info_risk_request_for_endpoint(
                &request,
                fb.provider,
                fb.request_format,
                &fb.model,
            );
            let fb_cached = state
                .reward_bot_service
                .latest_market_info_risk(&fb_request)
                .await?;
            freshest_reward_cache_row(primary_cached, fb_cached)
        }
        None => primary_cached,
    };
    if let Some(risk) = cached {
        report.cache_hits += 1;
        if !reward_provider_cache_refresh_due(
            risk.expires_at,
            cycle.config.info_risk_ttl_sec,
            OffsetDateTime::now_utc(),
        ) {
            return Ok(false);
        }
    }

    report.requested += 1;
    info!(
        trace_id = %trace_id,
        condition_id = %condition_id,
        requested = report.requested,
        "requesting reward info risk provider",
    );
    let primary_channel = RewardInfoRiskChannel {
        connector,
        provider: cycle.config.ai_provider,
        request_format: cycle.config.ai_request_format,
        model: model.to_string(),
    };
    let attempt = assess_with_fallback(
        state,
        &primary_channel,
        fallback_channel,
        &request,
        trace_id,
    )
    .await?;
    match attempt {
        RewardProviderAttempt::Success {
            decision,
            endpoint,
            request: winning_request,
        } => {
            let risk = decision.into_info_risk(
                &winning_request,
                cycle.config.info_risk_ttl_sec,
                OffsetDateTime::now_utc(),
            );
            state.reward_bot_service.save_market_info_risk(&risk).await?;
            report.saved += 1;
            info!(
                trace_id = %trace_id,
                condition_id = %condition_id,
                endpoint = ?endpoint,
                saved = report.saved,
                "saved reward info risk",
            );
            Ok(false)
        }
        RewardProviderAttempt::Failed {
            primary_error,
            fallback_error,
        } => {
            report.failures += 1;
            warn!(
                trace_id = %trace_id,
                condition_id = %condition_id,
                error = %primary_error,
                "reward info risk request failed; keeping existing cached state",
            );
            if let Some(fb_error) = &fallback_error {
                warn!(
                    trace_id = %trace_id,
                    condition_id = %condition_id,
                    error = %fb_error,
                    "reward info risk fallback request also failed",
                );
            }
            if reward_combined_provider_overloaded(&primary_error, fallback_error.as_ref()) {
                warn!(
                    trace_id = %trace_id,
                    requested = report.requested,
                    failures = report.failures,
                    "reward info risk provider is overloaded; stopping market provider requests for this cycle",
                );
                return Ok(true);
            }
            Ok(false)
        }
    }
}
