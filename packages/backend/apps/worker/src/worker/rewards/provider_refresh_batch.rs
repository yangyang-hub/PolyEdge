#[allow(clippy::too_many_arguments)]
async fn refresh_reward_ai_advisory_provider_batch(
    state: &AppState,
    connector: &RewardAiAdvisoryConnector,
    fallback_channel: Option<&RewardAiAdvisoryChannel<'_>>,
    cycle: &mut RewardLiveCycle,
    books: &HashMap<String, RewardOrderBook>,
    markets_by_condition: &HashMap<String, RewardMarket>,
    condition_ids: &[String],
    model: &str,
    trace_id: &str,
    report: &mut RewardAiAdvisoryRefreshReport,
    promoted_tokens: &mut Vec<String>,
) -> Result<bool> {
    let batch_size = reward_provider_configured_batch_size(cycle.config.ai_advisory_batch_size);
    if batch_size <= 1 || condition_ids.len() <= 1 {
        return refresh_reward_ai_advisory_provider_singles(
            state,
            connector,
            fallback_channel,
            cycle,
            books,
            markets_by_condition,
            condition_ids,
            model,
            trace_id,
            report,
            promoted_tokens,
        )
        .await;
    }

    for condition_batch in condition_ids.chunks(batch_size) {
        let requests = build_reward_ai_advisory_batch_requests(
            state,
            cycle,
            books,
            markets_by_condition,
            condition_batch,
            model,
            trace_id,
            report,
            promoted_tokens,
        )
        .await?;
        if requests.is_empty() {
            continue;
        }
        report.requested += requests.len();
        info!(
            trace_id = %trace_id,
            markets = requests.len(),
            batch_size,
            "requesting reward AI advisory provider batch",
        );
        let condition_ids_for_record = reward_ai_llm_condition_ids(&requests);
        let input_hash_for_record = reward_ai_llm_batch_input_hash(&requests);
        let started = Instant::now();
        let result = {
            let _provider_permit = acquire_reward_ai_advisory_provider_request_permit().await?;
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
            result.is_ok(),
            result.as_ref().ok().map(|items| json!(items)),
            result.as_ref().err().map(ToString::to_string),
            false,
            trace_id,
        )
        .await;
        let items = match result {
            Ok(items) => items,
            Err(error) => {
                report.failures += 1;
                warn!(
                    trace_id = %trace_id,
                    markets = requests.len(),
                    error = %error,
                    "reward AI advisory batch request failed; falling back to single requests",
                );
                if reward_ai_provider_is_overloaded(&error) {
                    return Ok(true);
                }
                let fallback_conditions = requests
                    .iter()
                    .map(|request| request.condition_id.clone())
                    .collect::<Vec<_>>();
                if refresh_reward_ai_advisory_provider_singles(
                    state,
                    connector,
                    fallback_channel,
                    cycle,
                    books,
                    markets_by_condition,
                    &fallback_conditions,
                    model,
                    trace_id,
                    report,
                    promoted_tokens,
                )
                .await?
                {
                    return Ok(true);
                }
                continue;
            }
        };

        let request_by_condition = requests
            .iter()
            .map(|request| (request.condition_id.clone(), request))
            .collect::<HashMap<_, _>>();
        let mut saved = HashSet::new();
        for item in items {
            let Some(request) = request_by_condition.get(&item.condition_id) else {
                continue;
            };
            let advisory = item.decision.into_advisory(
                request,
                cycle.config.ai_advisory_ttl_sec,
                OffsetDateTime::now_utc(),
            );
            state.reward_bot_service.save_market_advisory(&advisory).await?;
            report.saved += 1;
            saved.insert(item.condition_id.clone());
            apply_reward_ai_advisory_to_refresh_cycle(
                state,
                cycle,
                markets_by_condition,
                &item.condition_id,
                advisory,
                trace_id,
                promoted_tokens,
            )
            .await;
        }

        let missing = requests
            .iter()
            .filter(|request| !saved.contains(&request.condition_id))
            .map(|request| request.condition_id.clone())
            .collect::<Vec<_>>();
        if !missing.is_empty()
            && refresh_reward_ai_advisory_provider_singles(
                state,
                connector,
                fallback_channel,
                cycle,
                books,
                markets_by_condition,
                &missing,
                model,
                trace_id,
                report,
                promoted_tokens,
            )
            .await?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

#[allow(clippy::too_many_arguments)]
async fn build_reward_ai_advisory_batch_requests(
    state: &AppState,
    cycle: &mut RewardLiveCycle,
    books: &HashMap<String, RewardOrderBook>,
    markets_by_condition: &HashMap<String, RewardMarket>,
    condition_ids: &[String],
    model: &str,
    trace_id: &str,
    report: &mut RewardAiAdvisoryRefreshReport,
    promoted_tokens: &mut Vec<String>,
) -> Result<Vec<RewardAiAdvisoryRequest>> {
    let mut requests = Vec::new();
    let fallback = resolve_reward_ai_fallback(&state.settings.rewards);
    for condition_id in condition_ids {
        let Some(plan_for_request) = cycle
            .plans
            .iter()
            .find(|plan| plan.condition_id == condition_id.as_str())
            .cloned()
        else {
            report.skipped_missing_plan += 1;
            continue;
        };
        let Some(market) = markets_by_condition.get(condition_id) else {
            report.skipped_missing_market += 1;
            continue;
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
            &plan_for_request,
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
        if let Some(cached) =
            latest_market_advisory_for_endpoints(state, &request, fallback.as_ref()).await?
        {
            report.cache_hits += 1;
            let refresh_due = reward_provider_cache_refresh_due(
                cached.expires_at,
                cycle.config.ai_advisory_ttl_sec,
                OffsetDateTime::now_utc(),
            );
            apply_reward_ai_advisory_to_refresh_cycle(
                state,
                cycle,
                markets_by_condition,
                condition_id,
                cached,
                trace_id,
                promoted_tokens,
            )
            .await;
            if !refresh_due {
                continue;
            }
        }
        if !reward_market_books_available(market, books) {
            report.skipped_missing_book += 1;
            continue;
        }
        requests.push(request);
    }
    Ok(requests)
}

#[allow(clippy::too_many_arguments)]
async fn refresh_reward_ai_advisory_provider_singles(
    state: &AppState,
    connector: &RewardAiAdvisoryConnector,
    fallback_channel: Option<&RewardAiAdvisoryChannel<'_>>,
    cycle: &mut RewardLiveCycle,
    books: &HashMap<String, RewardOrderBook>,
    markets_by_condition: &HashMap<String, RewardMarket>,
    condition_ids: &[String],
    model: &str,
    trace_id: &str,
    report: &mut RewardAiAdvisoryRefreshReport,
    promoted_tokens: &mut Vec<String>,
) -> Result<bool> {
    for condition_id in condition_ids {
        let ai_step = refresh_reward_ai_advisory_for_condition(
            state,
            connector,
            fallback_channel,
            cycle,
            books,
            markets_by_condition,
            condition_id,
            model,
            trace_id,
            report,
        )
        .await?;
        if let Some(advisory) = ai_step.advisory {
            apply_reward_ai_advisory_to_refresh_cycle(
                state,
                cycle,
                markets_by_condition,
                condition_id,
                advisory,
                trace_id,
                promoted_tokens,
            )
            .await;
        }
        if ai_step.stop_cycle {
            return Ok(true);
        }
    }
    Ok(false)
}

async fn apply_reward_ai_advisory_to_refresh_cycle(
    state: &AppState,
    cycle: &mut RewardLiveCycle,
    markets_by_condition: &HashMap<String, RewardMarket>,
    condition_id: &str,
    advisory: RewardMarketAdvisory,
    trace_id: &str,
    promoted_tokens: &mut Vec<String>,
) {
    if let Some(market) = markets_by_condition.get(condition_id) {
        promote_reward_ai_provider_passed_market_to_eligible_source(
            state,
            market,
            &advisory,
            trace_id,
            promoted_tokens,
        )
        .await;
    }
    if let Some(plan) = cycle
        .plans
        .iter_mut()
        .find(|plan| plan.condition_id == condition_id)
    {
        plan.ai_advisory = Some(advisory);
    }
}

#[allow(clippy::too_many_arguments)]
async fn refresh_reward_info_risk_provider_batch(
    state: &AppState,
    connector: &RewardInfoRiskConnector,
    fallback_channel: Option<&RewardInfoRiskChannel<'_>>,
    cycle: &RewardLiveCycle,
    markets_by_condition: &HashMap<String, RewardMarket>,
    condition_ids: &[String],
    model: &str,
    trace_id: &str,
    report: &mut RewardInfoRiskScanReport,
) -> Result<bool> {
    let batch_size = reward_provider_configured_batch_size(cycle.config.info_risk_batch_size);
    if batch_size <= 1 || condition_ids.len() <= 1 {
        return refresh_reward_info_risk_provider_singles(
            state,
            connector,
            fallback_channel,
            cycle,
            markets_by_condition,
            condition_ids,
            model,
            trace_id,
            report,
        )
        .await;
    }

    for condition_batch in condition_ids.chunks(batch_size) {
        let requests = build_reward_info_risk_batch_requests(
            state,
            cycle,
            markets_by_condition,
            condition_batch,
            model,
            report,
        )
        .await?;
        if requests.is_empty() {
            continue;
        }
        report.requested += requests.len();
        info!(
            trace_id = %trace_id,
            markets = requests.len(),
            batch_size,
            "requesting reward info risk provider batch",
        );
        let condition_ids_for_record = reward_info_risk_llm_condition_ids(&requests);
        let input_hash_for_record = reward_info_risk_llm_batch_input_hash(&requests);
        let started = Instant::now();
        let result = {
            let _provider_permit = acquire_reward_info_risk_provider_request_permit().await?;
            connector.assess_batch(&requests).await
        };
        record_reward_provider_llm_call(
            state,
            REWARD_INFO_RISK_LLM_TASK_TYPE,
            REWARD_INFO_RISK_PROMPT_VERSION,
            model,
            &input_hash_for_record,
            &condition_ids_for_record,
            started.elapsed(),
            result.is_ok(),
            result.as_ref().ok().map(|items| json!(items)),
            result.as_ref().err().map(ToString::to_string),
            false,
            trace_id,
        )
        .await;
        let items = match result {
            Ok(items) => items,
            Err(error) => {
                report.failures += 1;
                warn!(
                    trace_id = %trace_id,
                    markets = requests.len(),
                    error = %error,
                    "reward info risk batch request failed; falling back to single requests",
                );
                if reward_ai_provider_is_overloaded(&error) {
                    return Ok(true);
                }
                let fallback_conditions = requests
                    .iter()
                    .map(|request| request.condition_id.clone())
                    .collect::<Vec<_>>();
                if refresh_reward_info_risk_provider_singles(
                    state,
                    connector,
                    fallback_channel,
                    cycle,
                    markets_by_condition,
                    &fallback_conditions,
                    model,
                    trace_id,
                    report,
                )
                .await?
                {
                    return Ok(true);
                }
                continue;
            }
        };

        let request_by_condition = requests
            .iter()
            .map(|request| (request.condition_id.clone(), request))
            .collect::<HashMap<_, _>>();
        let mut saved = HashSet::new();
        for item in items {
            let Some(request) = request_by_condition.get(&item.condition_id) else {
                continue;
            };
            let risk = item.decision.into_info_risk(
                request,
                cycle.config.info_risk_ttl_sec,
                OffsetDateTime::now_utc(),
            );
            state.reward_bot_service.save_market_info_risk(&risk).await?;
            report.saved += 1;
            saved.insert(item.condition_id.clone());
        }

        let missing = requests
            .iter()
            .filter(|request| !saved.contains(&request.condition_id))
            .map(|request| request.condition_id.clone())
            .collect::<Vec<_>>();
        if !missing.is_empty()
            && refresh_reward_info_risk_provider_singles(
                state,
                connector,
                fallback_channel,
                cycle,
                markets_by_condition,
                &missing,
                model,
                trace_id,
                report,
            )
            .await?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

async fn build_reward_info_risk_batch_requests(
    state: &AppState,
    cycle: &RewardLiveCycle,
    markets_by_condition: &HashMap<String, RewardMarket>,
    condition_ids: &[String],
    model: &str,
    report: &mut RewardInfoRiskScanReport,
) -> Result<Vec<RewardInfoRiskAssessmentRequest>> {
    let mut requests = Vec::new();
    let fallback = resolve_reward_ai_fallback(&state.settings.rewards);
    for condition_id in condition_ids {
        let Some(market) = markets_by_condition.get(condition_id) else {
            report.skipped_missing_market += 1;
            continue;
        };
        let plan_for_request = cycle
            .plans
            .iter()
            .find(|plan| plan.condition_id == condition_id.as_str());
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
        if let Some(cached) =
            latest_market_info_risk_for_endpoints(state, &request, fallback.as_ref()).await?
        {
            report.cache_hits += 1;
            if !reward_provider_cache_refresh_due(
                cached.expires_at,
                cycle.config.info_risk_ttl_sec,
                OffsetDateTime::now_utc(),
            ) {
                continue;
            }
        }
        requests.push(request);
    }
    Ok(requests)
}

#[allow(clippy::too_many_arguments)]
async fn refresh_reward_info_risk_provider_singles(
    state: &AppState,
    connector: &RewardInfoRiskConnector,
    fallback_channel: Option<&RewardInfoRiskChannel<'_>>,
    cycle: &RewardLiveCycle,
    markets_by_condition: &HashMap<String, RewardMarket>,
    condition_ids: &[String],
    model: &str,
    trace_id: &str,
    report: &mut RewardInfoRiskScanReport,
) -> Result<bool> {
    for condition_id in condition_ids {
        if refresh_reward_info_risk_for_condition(
            state,
            connector,
            fallback_channel,
            cycle,
            markets_by_condition,
            condition_id,
            model,
            trace_id,
            report,
        )
        .await?
        {
            return Ok(true);
        }
    }
    Ok(false)
}
