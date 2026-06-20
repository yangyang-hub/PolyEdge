fn spawn_reward_market_provider_refresh(
    state: &AppState,
    cycle: &RewardLiveCycle,
    books: &HashMap<String, RewardOrderBook>,
    trace_id: &str,
) {
    if !cycle.config.ai_advisory_enabled && !cycle.config.info_risk_enabled {
        return;
    }
    if cycle.plans.is_empty() && cycle.markets.is_empty() {
        return;
    }
    if REWARD_MARKET_PROVIDER_REFRESH_RUNNING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        debug!(
            trace_id = %trace_id,
            "skipping reward market provider refresh because another refresh is running",
        );
        return;
    }

    let state = state.clone();
    let cycle = cycle.clone();
    let books = books.clone();
    let trace_id = trace_id.to_string();
    tokio::spawn(async move {
        let result = refresh_reward_market_provider_cache(&state, cycle, books, &trace_id).await;
        REWARD_MARKET_PROVIDER_REFRESH_RUNNING.store(false, Ordering::Release);
        if let Err(error) = result {
            warn!(
                trace_id = %trace_id,
                error = %error,
                "reward market provider refresh failed",
            );
        }
    });
}

async fn refresh_reward_market_provider_cache(
    state: &AppState,
    mut cycle: RewardLiveCycle,
    books: HashMap<String, RewardOrderBook>,
    trace_id: &str,
) -> Result<RewardProviderRefreshReport> {
    let ai_refresh_enabled = cycle.config.ai_advisory_enabled && !cycle.plans.is_empty();
    let info_risk_refresh_enabled = cycle.config.info_risk_enabled;
    if !ai_refresh_enabled && !info_risk_refresh_enabled {
        return Ok(RewardProviderRefreshReport::default());
    }

    let model = state.settings.rewards.ai_model.trim();
    if model.is_empty() {
        warn!(
            trace_id = %trace_id,
            "reward provider model is empty; skipping market provider refresh"
        );
        return Ok(RewardProviderRefreshReport::default());
    }

    let ai_connector = if ai_refresh_enabled {
        match build_reward_ai_advisory_connector(state, &cycle.config)? {
            Some(connector) => Some(connector),
            None => {
                warn!(
                    trace_id = %trace_id,
                    provider = cycle.config.ai_provider.as_str(),
                    "reward AI advisory is enabled but provider configuration is incomplete; skipping AI provider refresh",
                );
                None
            }
        }
    } else {
        None
    };
    let info_risk_connector = if info_risk_refresh_enabled {
        match build_reward_info_risk_connector(state, &cycle.config)? {
            Some(connector) => Some(connector),
            None => {
                warn!(
                    trace_id = %trace_id,
                    provider = cycle.config.ai_provider.as_str(),
                    "reward info risk is enabled but provider configuration is incomplete; skipping info-risk provider refresh",
                );
                None
            }
        }
    } else {
        None
    };

    if ai_connector.is_none() && info_risk_connector.is_none() {
        return Ok(RewardProviderRefreshReport::default());
    }

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
    for market in &cycle.markets {
        markets_by_condition.insert(market.condition_id.clone(), market.clone());
    }

    let now = OffsetDateTime::now_utc();
    let ai_candidate_condition_ids = if ai_connector.is_some() {
        reward_ai_advisory_candidate_condition_ids(
            &cycle.plans,
            &cycle.open_orders,
            &cycle.positions,
            &cycle.pre_ai_eligible_condition_ids,
            &cycle.config,
            model,
            now,
        )
    } else {
        Vec::new()
    };
    let info_risk_candidate_condition_ids = if info_risk_connector.is_some() {
        reward_info_risk_candidate_conditions(
            &cycle.markets,
            &cycle.plans,
            &cycle.open_orders,
            &cycle.positions,
        )
    } else {
        Vec::new()
    };
    let ai_candidate_conditions = ai_candidate_condition_ids
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    let info_risk_candidate_conditions = info_risk_candidate_condition_ids
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    let mut ordered_conditions = reward_provider_refresh_candidate_condition_ids(
        &info_risk_candidate_condition_ids,
        &ai_candidate_condition_ids,
    );
    let original_ordered_conditions = ordered_conditions.len();
    let max_conditions = reward_provider_max_conditions_per_cycle(state);
    if ordered_conditions.len() > max_conditions {
        ordered_conditions.truncate(max_conditions);
    }

    let mut report = RewardProviderRefreshReport::default();
    report.ai.candidates = ai_candidate_condition_ids.len();
    report.info_risk.candidates = info_risk_candidate_condition_ids.len();
    info!(
        trace_id = %trace_id,
        provider = cycle.config.ai_provider.as_str(),
        request_format = cycle.config.ai_request_format.as_str(),
        conditions = original_ordered_conditions,
        selected_conditions = ordered_conditions.len(),
        max_conditions,
        ai_candidates = report.ai.candidates,
        info_risk_candidates = report.info_risk.candidates,
        "starting reward market provider refresh",
    );

    for condition_id in ordered_conditions {
        if let Some(connector) = ai_connector.as_ref()
            && ai_candidate_conditions.contains(&condition_id)
        {
            let ai_step = refresh_reward_ai_advisory_for_condition(
                state,
                connector,
                &cycle,
                &books,
                &markets_by_condition,
                &condition_id,
                model,
                trace_id,
                &mut report.ai,
            )
            .await?;
            if let Some(advisory) = ai_step.advisory
                && let Some(plan) = cycle
                    .plans
                    .iter_mut()
                    .find(|plan| plan.condition_id == condition_id)
            {
                plan.ai_advisory = Some(advisory);
            }
            if ai_step.stop_cycle {
                break;
            }
        }

        if let Some(connector) = info_risk_connector.as_ref()
            && info_risk_candidate_conditions.contains(&condition_id)
            && refresh_reward_info_risk_for_condition(
                state,
                connector,
                &cycle,
                &markets_by_condition,
                &condition_id,
                model,
                trace_id,
                &mut report.info_risk,
            )
            .await?
        {
            break;
        }
    }

    if cycle.config.info_risk_enabled {
        report.info_risk.applied_plans = apply_cached_reward_info_risks(state, trace_id).await?;
    }

    info!(
        trace_id = %trace_id,
        ai_candidates = report.ai.candidates,
        ai_cache_hits = report.ai.cache_hits,
        ai_requested = report.ai.requested,
        ai_saved = report.ai.saved,
        ai_failures = report.ai.failures,
        ai_skipped_missing_market = report.ai.skipped_missing_market,
        ai_skipped_missing_plan = report.ai.skipped_missing_plan,
        ai_skipped_missing_book = report.ai.skipped_missing_book,
        info_risk_candidates = report.info_risk.candidates,
        info_risk_cache_hits = report.info_risk.cache_hits,
        info_risk_requested = report.info_risk.requested,
        info_risk_saved = report.info_risk.saved,
        info_risk_failures = report.info_risk.failures,
        info_risk_skipped_missing_market = report.info_risk.skipped_missing_market,
        info_risk_applied_plans = report.info_risk.applied_plans,
        "completed reward market provider refresh",
    );
    Ok(report)
}

struct RewardAiAdvisoryRefreshStep {
    stop_cycle: bool,
    advisory: Option<RewardMarketAdvisory>,
}

#[allow(clippy::too_many_arguments)]
async fn refresh_reward_ai_advisory_for_condition(
    state: &AppState,
    connector: &RewardAiAdvisoryConnector,
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
            REWARD_AI_CANDLE_INTERVAL_SEC,
            REWARD_AI_CANDLE_LIMIT_PER_TOKEN,
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
        cycle.config.ai_provider,
        cycle.config.ai_request_format,
        model,
    )?;
    if let Some(cached) = state
        .reward_bot_service
        .latest_market_advisory(&request)
        .await?
    {
        report.cache_hits += 1;
        return Ok(RewardAiAdvisoryRefreshStep {
            stop_cycle: false,
            advisory: Some(cached),
        });
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
            advisory: None,
        });
    }

    report.requested += 1;
    info!(
        trace_id = %trace_id,
        condition_id = %condition_id,
        requested = report.requested,
        "requesting reward AI advisory provider",
    );
    let result = {
        let _provider_permit = acquire_reward_ai_provider_request_permit().await?;
        connector.advise(&request).await
    };
    match result {
        Ok(decision) => {
            let advisory = decision.into_advisory(
                &request,
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
                saved = report.saved,
                "saved reward AI advisory",
            );
            Ok(RewardAiAdvisoryRefreshStep {
                stop_cycle: false,
                advisory: Some(advisory),
            })
        }
        Err(error) => {
            report.failures += 1;
            warn!(
                trace_id = %trace_id,
                condition_id = %condition_id,
                error = %error,
                "reward AI advisory request failed; keeping existing cached state",
            );
            if reward_ai_provider_is_overloaded(&error) {
                warn!(
                    trace_id = %trace_id,
                    requested = report.requested,
                    failures = report.failures,
                    "reward AI advisory provider is overloaded; stopping market provider requests for this cycle",
                );
                return Ok(RewardAiAdvisoryRefreshStep {
                    stop_cycle: true,
                    advisory: None,
                });
            }
            Ok(RewardAiAdvisoryRefreshStep {
                stop_cycle: false,
                advisory: None,
            })
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn refresh_reward_info_risk_for_condition(
    state: &AppState,
    connector: &RewardInfoRiskConnector,
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
    if state
        .reward_bot_service
        .latest_market_info_risk(&request)
        .await?
        .is_some()
    {
        report.cache_hits += 1;
        return Ok(false);
    }

    report.requested += 1;
    info!(
        trace_id = %trace_id,
        condition_id = %condition_id,
        requested = report.requested,
        "requesting reward info risk provider",
    );
    let result = {
        let _provider_permit = acquire_reward_ai_provider_request_permit().await?;
        connector.assess(&request).await
    };
    match result {
        Ok(decision) => {
            let risk = decision.into_info_risk(
                &request,
                cycle.config.info_risk_ttl_sec,
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
            Ok(false)
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
                    "reward info risk provider is overloaded; stopping market provider requests for this cycle",
                );
                return Ok(true);
            }
            Ok(false)
        }
    }
}

fn reward_provider_refresh_candidate_condition_ids(
    info_risk_condition_ids: &[String],
    ai_condition_ids: &[String],
) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut ordered = Vec::with_capacity(info_risk_condition_ids.len() + ai_condition_ids.len());
    for condition_id in info_risk_condition_ids.iter().chain(ai_condition_ids) {
        let condition_id = condition_id.trim();
        if condition_id.is_empty() || !seen.insert(condition_id.to_string()) {
            continue;
        }
        ordered.push(condition_id.to_string());
    }
    ordered
}

fn reward_provider_max_conditions_per_cycle(state: &AppState) -> usize {
    usize::from(state.settings.rewards.info_risk_max_markets_per_cycle)
}
