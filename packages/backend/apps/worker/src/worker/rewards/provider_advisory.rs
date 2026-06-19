async fn apply_cached_reward_ai_advisories_to_cycle(
    state: &AppState,
    cycle: &mut RewardLiveCycle,
    books: &HashMap<String, RewardOrderBook>,
    trace_id: &str,
) -> Result<()> {
    if !cycle.config.ai_advisory_enabled {
        info!(
            trace_id = %trace_id,
            plans = cycle.plans.len(),
            "skipping reward AI advisory refresh because it is disabled in rewards config",
        );
        return Ok(());
    }
    if cycle.plans.is_empty() {
        info!(
            trace_id = %trace_id,
            "skipping reward AI advisory refresh because no quote plans were built",
        );
        return Ok(());
    }
    info!(
        trace_id = %trace_id,
        provider = cycle.config.ai_provider.as_str(),
        request_format = cycle.config.ai_request_format.as_str(),
        plans = cycle.plans.len(),
        pre_ai_eligible_plans = cycle.pre_ai_eligible_condition_ids.len(),
        open_orders = cycle.open_orders.len(),
        positions = cycle.positions.len(),
        "applying cached reward AI advisories",
    );
    let min_confidence = reward_ai_min_confidence(state.settings.rewards.ai_min_confidence_bps);
    let model = state.settings.rewards.ai_model.trim();
    let now = OffsetDateTime::now_utc();
    let mut advisories = current_reward_ai_advisories(
        &cycle.plans,
        &cycle.pre_ai_eligible_condition_ids,
        &cycle.config,
        model,
        now,
    );
    if model.is_empty() {
        warn!(
            trace_id = %trace_id,
            "reward AI advisory model is empty; blocking new eligible plans until provider filter passes"
        );
        apply_reward_ai_advisories(
            &mut cycle.plans,
            &advisories,
            &cycle.config,
            min_confidence,
        );
        return Ok(());
    }

    let markets_by_condition = cycle
        .markets
        .iter()
        .map(|market| (market.condition_id.clone(), market.clone()))
        .collect::<HashMap<_, _>>();
    let existing_advisories = advisories.len();
    let candidate_condition_ids = reward_ai_advisory_candidate_condition_ids(
        &cycle.plans,
        &cycle.open_orders,
        &cycle.positions,
        &cycle.pre_ai_eligible_condition_ids,
        &cycle.config,
        model,
        now,
    );
    let candidates = candidate_condition_ids.len();
    let mut cache_hits = 0usize;
    let mut skipped_missing_market = 0usize;

    for condition_id in candidate_condition_ids {
        let Some(plan_index) = cycle
            .plans
            .iter()
            .position(|plan| plan.condition_id == condition_id)
        else {
            continue;
        };
        let plan_for_request = cycle.plans[plan_index].clone();
        let Some(market) = markets_by_condition.get(condition_id.as_str()) else {
            skipped_missing_market += 1;
            continue;
        };
        let request = build_reward_ai_advisory_request(
            market,
            &plan_for_request,
            &cycle.account,
            &cycle.positions,
            &cycle.open_orders,
            books,
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
            cache_hits += 1;
            advisories.insert(condition_id.clone(), cached.clone());
        }
    }

    let ai_pending_plans =
        count_missing_reward_ai_advisories(&cycle.pre_ai_eligible_condition_ids, &advisories);
    info!(
        trace_id = %trace_id,
        pre_ai_eligible_plans = cycle.pre_ai_eligible_condition_ids.len(),
        ai_existing_advisories = existing_advisories,
        ai_request_candidates = candidates,
        ai_pending_plans,
        candidates,
        cache_hits,
        skipped_missing_market,
        applied = advisories.len(),
        "completed cached reward AI advisory application",
    );

    apply_reward_ai_advisories(
        &mut cycle.plans,
        &advisories,
        &cycle.config,
        min_confidence,
    );
    Ok(())
}

#[cfg(test)]
fn apply_reward_ai_advisory_to_quote_plan(
    plans: &mut [RewardQuotePlan],
    config: &RewardBotConfig,
    condition_id: &str,
    advisory: RewardMarketAdvisory,
    min_confidence: Decimal,
) -> bool {
    let Some(plan) = plans
        .iter_mut()
        .find(|plan| plan.condition_id == condition_id)
    else {
        return false;
    };
    let advisories = HashMap::from([(condition_id.to_string(), advisory)]);
    apply_reward_ai_advisories(
        std::slice::from_mut(plan),
        &advisories,
        config,
        min_confidence,
    );
    true
}

fn reward_ai_advisory_candidate_condition_ids(
    plans: &[RewardQuotePlan],
    open_orders: &[ManagedRewardOrder],
    positions: &[RewardPosition],
    pre_ai_eligible_condition_ids: &[String],
    config: &RewardBotConfig,
    model: &str,
    now: OffsetDateTime,
) -> Vec<String> {
    let plans_by_condition = plans
        .iter()
        .map(|plan| (plan.condition_id.as_str(), plan))
        .collect::<HashMap<_, _>>();
    let ai_required_condition_ids = pre_ai_eligible_condition_ids
        .iter()
        .map(|condition_id| condition_id.trim().to_string())
        .filter(|condition_id| !condition_id.is_empty())
        .collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    let mut ordered = Vec::with_capacity(ai_required_condition_ids.len());

    for order in open_orders {
        push_reward_ai_advisory_plan(
            &mut ordered,
            &mut seen,
            &plans_by_condition,
            &ai_required_condition_ids,
            &order.condition_id,
            config,
            model,
            now,
        );
    }
    for position in positions {
        push_reward_ai_advisory_plan(
            &mut ordered,
            &mut seen,
            &plans_by_condition,
            &ai_required_condition_ids,
            &position.condition_id,
            config,
            model,
            now,
        );
    }
    for condition_id in pre_ai_eligible_condition_ids {
        push_reward_ai_advisory_plan(
            &mut ordered,
            &mut seen,
            &plans_by_condition,
            &ai_required_condition_ids,
            condition_id,
            config,
            model,
            now,
        );
    }

    ordered
}

fn current_reward_ai_advisories(
    plans: &[RewardQuotePlan],
    pre_ai_eligible_condition_ids: &[String],
    config: &RewardBotConfig,
    model: &str,
    now: OffsetDateTime,
) -> HashMap<String, RewardMarketAdvisory> {
    let ai_required_condition_ids = pre_ai_eligible_condition_ids
        .iter()
        .map(|condition_id| condition_id.trim().to_string())
        .filter(|condition_id| !condition_id.is_empty())
        .collect::<HashSet<_>>();

    plans
        .iter()
        .filter(|plan| ai_required_condition_ids.contains(plan.condition_id.trim()))
        .filter_map(|plan| plan.ai_advisory.as_ref())
        .filter(|advisory| reward_ai_advisory_matches_config(advisory, config, model, now))
        .map(|advisory| (advisory.condition_id.clone(), advisory.clone()))
        .collect()
}

fn count_missing_reward_ai_advisories(
    pre_ai_eligible_condition_ids: &[String],
    advisories: &HashMap<String, RewardMarketAdvisory>,
) -> usize {
    pre_ai_eligible_condition_ids
        .iter()
        .filter(|condition_id| !advisories.contains_key(condition_id.trim()))
        .count()
}

fn push_reward_ai_advisory_plan<'a>(
    ordered: &mut Vec<String>,
    seen: &mut HashSet<String>,
    plans_by_condition: &HashMap<&str, &'a RewardQuotePlan>,
    ai_required_condition_ids: &HashSet<String>,
    condition_id: &str,
    config: &RewardBotConfig,
    model: &str,
    now: OffsetDateTime,
) {
    let condition_id = condition_id.trim();
    if condition_id.is_empty() {
        return;
    }
    if !ai_required_condition_ids.contains(condition_id) {
        return;
    }
    let Some(plan) = plans_by_condition.get(condition_id) else {
        return;
    };
    if !reward_ai_plan_needs_advisory(plan, config, model, now) {
        return;
    }
    if seen.insert(condition_id.to_string()) {
        ordered.push(condition_id.to_string());
    }
}

fn reward_ai_plan_needs_advisory(
    plan: &RewardQuotePlan,
    config: &RewardBotConfig,
    model: &str,
    now: OffsetDateTime,
) -> bool {
    !plan
        .ai_advisory
        .as_ref()
        .is_some_and(|advisory| reward_ai_advisory_matches_config(advisory, config, model, now))
}

fn reward_ai_advisory_matches_config(
    advisory: &RewardMarketAdvisory,
    config: &RewardBotConfig,
    model: &str,
    now: OffsetDateTime,
) -> bool {
    advisory.expires_at > now
        && advisory.provider == config.ai_provider
        && advisory.request_format == config.ai_request_format
        && advisory.model == model.trim()
}

fn build_reward_ai_advisory_connector(
    state: &AppState,
    config: &RewardBotConfig,
) -> Result<Option<RewardAiAdvisoryConnector>> {
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
    RewardAiAdvisoryConnector::new(
        base_url,
        api_key,
        rewards.ai_request_timeout_secs.max(1),
    )
    .map(Some)
}

fn reward_ai_min_confidence(bps: u16) -> Decimal {
    Decimal::from(bps.min(10_000)) / Decimal::from(10_000_u64)
}

async fn acquire_reward_ai_provider_request_permit()
-> Result<tokio::sync::SemaphorePermit<'static>> {
    REWARD_AI_PROVIDER_REQUEST_SEMAPHORE
        .acquire()
        .await
        .map_err(|error| {
            AppError::internal(
                "REWARD_AI_PROVIDER_SEMAPHORE_CLOSED",
                format!("reward AI provider request semaphore closed: {error}"),
            )
        })
}

fn reward_ai_provider_is_overloaded(error: &AppError) -> bool {
    if !matches!(
        error.code(),
        "REWARD_AI_STATUS_FAILED" | "REWARD_INFO_RISK_STATUS_FAILED"
    ) {
        return false;
    }
    let message = error.message().to_ascii_lowercase();
    message.contains("http 503")
        || message.contains("system_cpu_overloaded")
        || message.contains("overloaded")
}

#[cfg(test)]
mod reward_ai_provider_error_tests {
    use super::*;

    #[test]
    fn reward_ai_provider_overload_detects_status_errors() {
        let error = AppError::dependency_unavailable(
            "REWARD_INFO_RISK_STATUS_FAILED",
            r#"reward info risk provider returned HTTP 503: {"error":{"code":"system_cpu_overloaded"}}"#,
        );
        assert!(reward_ai_provider_is_overloaded(&error));
    }

    #[test]
    fn reward_ai_provider_overload_ignores_non_status_errors() {
        let error = AppError::dependency_unavailable(
            "REWARD_INFO_RISK_RESPONSE_INVALID",
            "provider response missing risk_level",
        );
        assert!(!reward_ai_provider_is_overloaded(&error));
    }

    #[tokio::test]
    async fn reward_ai_provider_request_permit_is_single_flight() {
        let first = acquire_reward_ai_provider_request_permit()
            .await
            .expect("acquire first permit");
        let second = tokio::time::timeout(
            Duration::from_millis(10),
            acquire_reward_ai_provider_request_permit(),
        )
        .await;
        assert!(second.is_err());

        drop(first);
        let _second = tokio::time::timeout(
            Duration::from_millis(100),
            acquire_reward_ai_provider_request_permit(),
        )
        .await
        .expect("second permit should acquire after release")
        .expect("acquire second permit");
    }
}
