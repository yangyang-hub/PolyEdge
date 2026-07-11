async fn apply_cached_reward_ai_advisories_to_cycle(
    state: &AppState,
    cycle: &mut RewardLiveCycle,
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
        plans = cycle.plans.len(),
        pre_ai_eligible_plans = cycle.pre_ai_eligible_condition_ids.len(),
        open_orders = cycle.open_orders.len(),
        positions = cycle.positions.len(),
        "applying cached reward AI advisories",
    );
    let min_confidence = cycle.config.ai_action_min_confidence;
    let model = reward_ai_model_for_provider(&state.settings.rewards, cycle.config.ai_provider);
    let request_format = reward_ai_effective_request_format_for_model(&cycle.config, model);
    info!(
        trace_id = %trace_id,
        request_format = request_format.as_str(),
        model,
        "resolved reward AI advisory endpoint format",
    );
    let fallback = resolve_reward_ai_fallback(&state.settings.rewards);
    let now = OffsetDateTime::now_utc();
    let mut advisories = current_reward_ai_advisories(
        &cycle.plans,
        &cycle.pre_ai_eligible_condition_ids,
        &cycle.config,
        model,
        fallback.as_ref(),
        now,
    );
    if model.is_empty() {
        warn!(
            trace_id = %trace_id,
            "reward AI advisory model is empty; blocking new eligible plans until provider filter passes"
        );
        apply_reward_ai_advisories(&mut cycle.plans, &advisories, &cycle.config, min_confidence);
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
        fallback.as_ref(),
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
        let candles = state
            .reward_bot_service
            .list_recent_market_candles(
                &condition_id,
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
            &candles,
            &cycle.config,
            cycle.config.ai_advisory_ttl_sec,
            cycle.config.ai_provider,
            request_format,
            model,
        )?;
        if let Some(cached) =
            latest_market_advisory_for_endpoints(state, &request, fallback.as_ref()).await?
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

    apply_reward_ai_advisories(&mut cycle.plans, &advisories, &cycle.config, min_confidence);
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

#[allow(clippy::too_many_arguments)]
fn reward_ai_advisory_candidate_condition_ids(
    plans: &[RewardQuotePlan],
    open_orders: &[ManagedRewardOrder],
    positions: &[RewardPosition],
    pre_ai_eligible_condition_ids: &[String],
    config: &RewardBotConfig,
    model: &str,
    fallback: Option<&RewardProviderFallback>,
    now: OffsetDateTime,
) -> Vec<String> {
    let plans_by_condition = reward_provider_plans_by_condition(plans);
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
            open_orders,
            positions,
            &order.condition_id,
            config,
            model,
            fallback,
            now,
        );
    }
    for position in positions {
        push_reward_ai_advisory_plan(
            &mut ordered,
            &mut seen,
            &plans_by_condition,
            &ai_required_condition_ids,
            open_orders,
            positions,
            &position.condition_id,
            config,
            model,
            fallback,
            now,
        );
    }
    for condition_id in pre_ai_eligible_condition_ids {
        push_reward_ai_advisory_plan(
            &mut ordered,
            &mut seen,
            &plans_by_condition,
            &ai_required_condition_ids,
            open_orders,
            positions,
            condition_id,
            config,
            model,
            fallback,
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
    fallback: Option<&RewardProviderFallback>,
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
        .filter(|advisory| {
            reward_ai_advisory_matches_config(advisory, config, model, fallback, now)
        })
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

#[allow(clippy::too_many_arguments)]
fn push_reward_ai_advisory_plan(
    ordered: &mut Vec<String>,
    seen: &mut HashSet<String>,
    plans_by_condition: &HashMap<&str, Vec<&RewardQuotePlan>>,
    ai_required_condition_ids: &HashSet<String>,
    open_orders: &[ManagedRewardOrder],
    positions: &[RewardPosition],
    condition_id: &str,
    config: &RewardBotConfig,
    model: &str,
    fallback: Option<&RewardProviderFallback>,
    now: OffsetDateTime,
) {
    let condition_id = condition_id.trim();
    if condition_id.is_empty() {
        return;
    }
    let has_active_exposure =
        reward_condition_has_active_exposure(condition_id, open_orders, positions);
    if !has_active_exposure && !ai_required_condition_ids.contains(condition_id) {
        return;
    }
    let Some(condition_plans) = plans_by_condition.get(condition_id) else {
        return;
    };
    if !condition_plans
        .iter()
        .any(|plan| reward_provider_plan_passes_pre_llm_gate(plan, config, has_active_exposure))
    {
        return;
    }
    if !condition_plans
        .iter()
        .any(|plan| reward_ai_plan_needs_advisory(plan, config, model, fallback, now))
    {
        return;
    }
    if seen.insert(condition_id.to_string()) {
        ordered.push(condition_id.to_string());
    }
}

fn reward_provider_plans_by_condition(
    plans: &[RewardQuotePlan],
) -> HashMap<&str, Vec<&RewardQuotePlan>> {
    let mut by_condition = HashMap::<&str, Vec<&RewardQuotePlan>>::new();
    for plan in plans {
        let condition_id = plan.condition_id.trim();
        if !condition_id.is_empty() {
            by_condition.entry(condition_id).or_default().push(plan);
        }
    }
    by_condition
}

fn reward_ai_plan_needs_advisory(
    plan: &RewardQuotePlan,
    config: &RewardBotConfig,
    model: &str,
    fallback: Option<&RewardProviderFallback>,
    now: OffsetDateTime,
) -> bool {
    let Some(advisory) = plan.ai_advisory.as_ref() else {
        return true;
    };
    if !reward_ai_advisory_matches_config(advisory, config, model, fallback, now) {
        return true;
    }
    reward_provider_cache_refresh_due(advisory.expires_at, config.ai_advisory_ttl_sec, now)
}

fn reward_ai_advisory_matches_config(
    advisory: &RewardMarketAdvisory,
    config: &RewardBotConfig,
    model: &str,
    fallback: Option<&RewardProviderFallback>,
    now: OffsetDateTime,
) -> bool {
    if advisory.expires_at <= now {
        return false;
    }
    let model = model.trim();
    let request_format = reward_ai_effective_request_format_for_model(config, model);
    if advisory.provider == config.ai_provider
        && advisory.request_format == request_format
        && advisory.model == model
    {
        return true;
    }
    // An advisory previously produced by the fallback endpoint (different
    // provider/model/format) must still satisfy the live-tick gate, otherwise a
    // fallback-saved advisory would be invisible and the condition would stay
    // "provider pending".
    fallback.is_some_and(|fb| {
        advisory.provider == fb.provider
            && advisory.request_format == fb.request_format
            && advisory.model == fb.model
    })
}

fn reward_ai_provider_endpoint_settings<'a>(
    rewards: &'a polyedge_infrastructure::settings::RewardsSettings,
    provider: polyedge_application::RewardAiProvider,
) -> (Option<&'a str>, &'a str) {
    match provider {
        polyedge_application::RewardAiProvider::OpenAi => (
            rewards.ai_openai_api_key.as_deref(),
            rewards.ai_openai_base_url.as_str(),
        ),
        polyedge_application::RewardAiProvider::Anthropic => (
            rewards.ai_anthropic_api_key.as_deref(),
            rewards.ai_anthropic_base_url.as_str(),
        ),
    }
}

fn reward_ai_model_for_provider<'a>(
    rewards: &'a polyedge_infrastructure::settings::RewardsSettings,
    _provider: polyedge_application::RewardAiProvider,
) -> &'a str {
    rewards.ai_model.trim()
}

fn reward_ai_effective_request_format_for_model(
    config: &RewardBotConfig,
    model: &str,
) -> polyedge_application::RewardAiRequestFormat {
    polyedge_application::reward_ai_effective_request_format(
        config.ai_provider,
        config.ai_request_format,
        model,
    )
}

/// Build the combined reward provider connector from the primary AI advisory
/// endpoint settings. Returns `Ok(None)` when the primary api_key is empty (the
/// refresh task then logs and skips, exactly like the legacy per-section
/// builders). web_search is attached only when info-risk is requested, the
/// provider is OpenAI and the effective request format is OpenAI Responses.
fn build_reward_provider_connector(
    state: &AppState,
    config: &RewardBotConfig,
) -> Result<Option<RewardProviderConnector>> {
    let rewards = &state.settings.rewards;
    let (api_key, base_url) = reward_ai_provider_endpoint_settings(rewards, config.ai_provider);
    let Some(api_key) = api_key.filter(|value| !value.trim().is_empty()) else {
        return Ok(None);
    };
    let model = reward_ai_model_for_provider(rewards, config.ai_provider);
    let request_format = reward_ai_effective_request_format_for_model(config, model);
    let web_search_enabled = rewards.info_risk_web_search_enabled
        && config.ai_provider == polyedge_application::RewardAiProvider::OpenAi
        && request_format == polyedge_application::RewardAiRequestFormat::OpenAiResponses;
    RewardProviderConnector::new(base_url, api_key, rewards.ai_request_timeout_secs.max(1), web_search_enabled)
        .map(Some)
}

/// Build the combined reward provider connector for a resolved fallback
/// endpoint. Only called when a fallback is configured, so the descriptor's
/// base_url/api_key are guaranteed non-empty.
fn build_reward_provider_fallback_connector(
    state: &AppState,
    fallback: &RewardProviderFallback,
) -> Result<RewardProviderConnector> {
    RewardProviderConnector::new(
        fallback.base_url.as_str(),
        fallback.api_key.as_str(),
        state.settings.rewards.ai_request_timeout_secs.max(1),
        fallback.web_search_enabled,
    )
}

const REWARD_PROVIDER_LLM_TASK_TYPE: &str = "reward_provider";
const REWARD_PROVIDER_PROMPT_VERSION: &str = "reward_provider_combined_v1";

#[allow(clippy::too_many_arguments)]
async fn record_reward_provider_llm_call(
    state: &AppState,
    task_type: &'static str,
    prompt_version: &'static str,
    model: &str,
    input_hash: &str,
    condition_ids: &[String],
    latency: Duration,
    success: bool,
    parsed_output: Option<Value>,
    error: Option<String>,
    fallback_used: bool,
    trace_id: &str,
) {
    let latency_ms = i64::try_from(latency.as_millis()).unwrap_or(i64::MAX);
    let call = RewardLlmCallRecord {
        id: format!("llm_{}", Uuid::now_v7()),
        task_type: task_type.to_string(),
        model_version: model.to_string(),
        prompt_version: prompt_version.to_string(),
        input_hash: input_hash.to_string(),
        raw_output: None,
        parsed_output,
        validation_result: json!({
            "success": success,
            "condition_ids": condition_ids,
            "condition_count": condition_ids.len(),
            "error": error,
        }),
        fallback_used,
        latency_ms,
        cost_estimate: None,
        trace_id: trace_id.to_string(),
        created_at: OffsetDateTime::now_utc(),
    };
    if let Err(error) = state.reward_bot_service.record_llm_call(&call).await {
        warn!(
            trace_id = %trace_id,
            task_type,
            error = %error,
            "failed to record reward provider LLM call",
        );
    }
}

const REWARD_PROVIDER_MAX_ENDPOINT_CONCURRENCY: u16 = 10;

#[derive(Clone)]
struct RewardProviderConcurrency {
    primary: Arc<tokio::sync::Semaphore>,
    fallback: Arc<tokio::sync::Semaphore>,
    primary_limit: usize,
    fallback_limit: usize,
}

fn reward_provider_concurrency(config: &RewardBotConfig) -> RewardProviderConcurrency {
    let primary_limit = reward_provider_endpoint_concurrency(
        config.ai_provider_concurrency_enabled,
        config.ai_provider_primary_max_concurrency,
    );
    let fallback_limit = reward_provider_endpoint_concurrency(
        config.ai_provider_concurrency_enabled,
        config.ai_provider_fallback_max_concurrency,
    );
    RewardProviderConcurrency {
        primary: Arc::new(tokio::sync::Semaphore::new(primary_limit)),
        fallback: Arc::new(tokio::sync::Semaphore::new(fallback_limit)),
        primary_limit,
        fallback_limit,
    }
}

fn reward_provider_endpoint_concurrency(enabled: bool, configured: u16) -> usize {
    if !enabled {
        return 1;
    }
    usize::from(configured.clamp(1, REWARD_PROVIDER_MAX_ENDPOINT_CONCURRENCY))
}

fn reward_provider_condition_parallelism(
    concurrency: &RewardProviderConcurrency,
    fallback_configured: bool,
) -> usize {
    if fallback_configured {
        concurrency.primary_limit + concurrency.fallback_limit
    } else {
        concurrency.primary_limit
    }
    .max(1)
}

async fn acquire_reward_provider_request_permit(
    semaphore: Arc<tokio::sync::Semaphore>,
    endpoint: &'static str,
) -> Result<tokio::sync::OwnedSemaphorePermit> {
    semaphore.acquire_owned().await.map_err(|error| {
        AppError::internal(
            "REWARD_PROVIDER_SEMAPHORE_CLOSED",
            format!("reward provider {endpoint} request semaphore closed: {error}"),
        )
    })
}

fn reward_ai_provider_is_overloaded(error: &AppError) -> bool {
    // Status-class failures carry an HTTP status from the provider. Treat the
    // capacity / auth / rate-limit family as overload so a refresh stops instead
    // of hammering a strained endpoint (429 / 5xx / auth / explicit overload /
    // timeout markers).
    if matches!(
        error.code(),
        "REWARD_AI_STATUS_FAILED"
            | "REWARD_INFO_RISK_STATUS_FAILED"
            | "REWARD_PROVIDER_STATUS_FAILED"
    ) {
        let message = error.message().to_ascii_lowercase();
        return message.contains("http 401")
            || message.contains("http 403")
            || message.contains("http 408")
            || message.contains("http 409")
            || message.contains("http 429")
            || message.contains("http 500")
            || message.contains("http 502")
            || message.contains("http 503")
            || message.contains("http 504")
            || message.contains("rate limit")
            || message.contains("too many requests")
            || message.contains("unauthorized")
            || message.contains("forbidden")
            || message.contains("invalid api key")
            || message.contains("authentication")
            || message.contains("timeout")
            || message.contains("timed out")
            || message.contains("system_cpu_overloaded")
            || message.contains("overloaded");
    }
    // Transport failures (reqwest could not complete the request) are transient by
    // nature — connection reset, DNS, TLS, or a stream dropped mid-response — and a
    // SINGLE one must not abort the whole refresh cycle, because the next condition
    // may still succeed. reqwest surfaces these (including the client-side timeout)
    // as an opaque `error sending request for url ...` with no HTTP status, so we
    // only treat the request as overload when the message explicitly mentions a
    // timeout; otherwise the cycle continues and the per-task wall-clock refresh
    // timeout bounds sustained failures. Previously every transport failure
    // hard-stopped the cycle, so one dropped provider connection skipped the
    // remaining candidates for the entire cycle.
    if matches!(
        error.code(),
        "REWARD_AI_HTTP_FAILED"
            | "REWARD_INFO_RISK_HTTP_FAILED"
            | "REWARD_PROVIDER_HTTP_FAILED"
    ) {
        let message = error.message().to_ascii_lowercase();
        return message.contains("timeout") || message.contains("timed out");
    }
    false
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

    #[test]
    fn reward_ai_provider_overload_detects_transport_errors() {
        let error = AppError::dependency_unavailable(
            "REWARD_AI_HTTP_FAILED",
            "reward AI HTTP request failed: operation timed out",
        );
        assert!(reward_ai_provider_is_overloaded(&error));
    }

    #[test]
    fn reward_ai_provider_transient_transport_error_is_not_overloaded() {
        // reqwest surfaces a dropped/reset connection as an opaque
        // "error sending request for url ..." with no HTTP status and no timeout
        // marker. Such a transient transport error must NOT be treated as provider
        // overload, otherwise a single dropped connection aborts the whole refresh
        // cycle and skips the remaining candidates (the observed glm-5.2 failure at
        // ~84s produced exactly this message).
        let error = AppError::dependency_unavailable(
            "REWARD_INFO_RISK_HTTP_FAILED",
            "reward info risk HTTP request failed: error sending request for url (https://open.bigmodel.cn/api/coding/paas/v4/chat/completions)",
        );
        assert!(!reward_ai_provider_is_overloaded(&error));
    }

    #[test]
    fn reward_provider_concurrency_defaults_to_serial_until_enabled() {
        let mut config = RewardBotConfig {
            ai_provider_primary_max_concurrency: 4,
            ai_provider_fallback_max_concurrency: 3,
            ..RewardBotConfig::default()
        };
        let concurrency = reward_provider_concurrency(&config);
        assert_eq!(concurrency.primary_limit, 1);
        assert_eq!(concurrency.fallback_limit, 1);

        config.ai_provider_concurrency_enabled = true;
        let concurrency = reward_provider_concurrency(&config);
        assert_eq!(concurrency.primary_limit, 4);
        assert_eq!(concurrency.fallback_limit, 3);
    }
}
