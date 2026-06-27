// Rewards LLM provider fallback. The rewards worker normally calls a single
// configured LLM endpoint (AI advisory + info-risk). This module adds an
// optional, fully-independent second endpoint: when the primary provider call
// fails for ANY reason (transport error, non-2xx status, or a 200 response that
// fails to decode/parse), the same logical request is retried against the
// fallback endpoint. The fallback may use a different provider, base_url,
// api_key, model and request format. Both attempts are recorded in `llm_calls`
// (`fallback_used` distinguishes them).
//
// Cache correctness: advisory/info-risk cache rows are keyed on
// (provider, request_format, model, input_hash). Because `input_hash` is
// derived from provider-independent market/plan/config/candle payload, a
// fallback request is produced by cloning the primary request and overriding
// only provider/request_format/model — yielding the correct distinct cache row
// without rebuilding the request from market inputs.

use polyedge_application::{RewardAiAdvisoryDecision, RewardInfoRiskAssessmentDecision};

/// Resolved fallback endpoint descriptor. Built once from `RewardsSettings` and
/// reused for both AI advisory and info-risk. A single shared endpoint matches
/// "a second LLM interface" semantics.
#[derive(Debug, Clone)]
struct RewardProviderFallback {
    provider: polyedge_application::RewardAiProvider,
    request_format: polyedge_application::RewardAiRequestFormat,
    model: String,
    api_key: String,
    base_url: String,
    /// Info-risk only. Forced false unless the fallback is OpenAI + Responses.
    web_search_enabled: bool,
}

/// A bundled view of one configured endpoint: the connector plus the
/// provider/model/format carried per-request. Lets call sites pass one
/// reference per endpoint to the retry wrapper.
struct RewardAiAdvisoryChannel<'a> {
    connector: &'a RewardAiAdvisoryConnector,
    provider: polyedge_application::RewardAiProvider,
    request_format: polyedge_application::RewardAiRequestFormat,
    model: String,
}

struct RewardInfoRiskChannel<'a> {
    connector: &'a RewardInfoRiskConnector,
    provider: polyedge_application::RewardAiProvider,
    request_format: polyedge_application::RewardAiRequestFormat,
    model: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RewardProviderEndpoint {
    Primary,
    Fallback,
}

/// Outcome of a primary-then-fallback attempt. On `Success` the winning request
/// is returned so the caller can build/save the advisory or info-risk from the
/// request that actually served the call (provider/model/format differ between
/// endpoints, so the cache key must match the winning request).
enum RewardProviderAttempt<Decision, Request> {
    Success {
        decision: Decision,
        endpoint: RewardProviderEndpoint,
        request: Request,
    },
    Failed {
        primary_error: AppError,
        fallback_error: Option<AppError>,
    },
}

/// Resolve the optional fallback endpoint from settings. Returns `None` (no
/// fallback) unless provider + request_format + api_key + model + base_url are
/// all set. Invalid provider/format values disable the fallback with a warning
/// rather than failing settings load.
fn resolve_reward_ai_fallback(
    rewards: &polyedge_infrastructure::settings::RewardsSettings,
) -> Option<RewardProviderFallback> {
    let provider_raw = rewards.ai_fallback_provider.as_deref()?.trim();
    let format_raw = rewards.ai_fallback_request_format.as_deref()?.trim();
    let model = rewards.ai_fallback_model.as_deref()?.trim();
    let api_key = rewards.ai_fallback_api_key.as_deref()?.trim();
    if provider_raw.is_empty() || format_raw.is_empty() || model.is_empty() || api_key.is_empty() {
        return None;
    }
    let provider = match provider_raw {
        "openai" | "open_ai" | "glm" | "bigmodel" | "zhipu" | "deepseek" | "deep_seek" => {
            polyedge_application::RewardAiProvider::OpenAi
        }
        "anthropic" => polyedge_application::RewardAiProvider::Anthropic,
        other => {
            warn!(
                provider = other,
                "reward AI fallback provider is invalid; fallback disabled",
            );
            return None;
        }
    };
    let request_format = match format_raw {
        "openai_responses" | "open_ai_responses" => {
            polyedge_application::RewardAiRequestFormat::OpenAiResponses
        }
        "openai_chat_completions" | "open_ai_chat_completions" => {
            polyedge_application::RewardAiRequestFormat::OpenAiChatCompletions
        }
        "anthropic_messages" => polyedge_application::RewardAiRequestFormat::AnthropicMessages,
        other => {
            warn!(
                request_format = other,
                "reward AI fallback request format is invalid; fallback disabled",
            );
            return None;
        }
    };
    let request_format = normalize_reward_provider_format(provider, request_format, model);
    let web_search_enabled = rewards.info_risk_web_search_enabled
        && provider == polyedge_application::RewardAiProvider::OpenAi
        && request_format == polyedge_application::RewardAiRequestFormat::OpenAiResponses;
    let base_url = rewards.ai_fallback_base_url.trim();
    if base_url.is_empty() {
        warn!("reward AI fallback base url is empty; fallback disabled");
        return None;
    }
    Some(RewardProviderFallback {
        provider,
        request_format,
        model: model.to_string(),
        api_key: api_key.to_string(),
        base_url: base_url.to_string(),
        web_search_enabled,
    })
}

/// Apply the same provider+format coupling normalization as the primary config:
/// Anthropic forces `AnthropicMessages`; OpenAI-compatible GLM/DeepSeek models
/// force Chat Completions based on their model names; an OpenAI provider cannot
/// use `AnthropicMessages` and falls back to `OpenAiResponses`. Without this the
/// connector's `ensure_provider` guard rejects every fallback call.
fn normalize_reward_provider_format(
    provider: polyedge_application::RewardAiProvider,
    request_format: polyedge_application::RewardAiRequestFormat,
    model: &str,
) -> polyedge_application::RewardAiRequestFormat {
    polyedge_application::reward_ai_effective_request_format(provider, request_format, model)
}

/// Build the fallback request for an endpoint by cloning the primary request
/// and overriding only the provider-specific fields. `input_hash` and `payload`
/// are provider-independent, so this produces a correctly-keyed cache row.
fn reward_ai_advisory_request_for_endpoint(
    source: &RewardAiAdvisoryRequest,
    provider: polyedge_application::RewardAiProvider,
    request_format: polyedge_application::RewardAiRequestFormat,
    model: &str,
) -> RewardAiAdvisoryRequest {
    RewardAiAdvisoryRequest {
        condition_id: source.condition_id.clone(),
        provider,
        request_format,
        model: model.trim().to_string(),
        input_hash: source.input_hash.clone(),
        payload: source.payload.clone(),
    }
}

fn reward_info_risk_request_for_endpoint(
    source: &RewardInfoRiskAssessmentRequest,
    provider: polyedge_application::RewardAiProvider,
    request_format: polyedge_application::RewardAiRequestFormat,
    model: &str,
) -> RewardInfoRiskAssessmentRequest {
    RewardInfoRiskAssessmentRequest {
        condition_id: source.condition_id.clone(),
        provider,
        request_format,
        model: model.trim().to_string(),
        query: source.query.clone(),
        query_hash: source.query_hash.clone(),
        input_hash: source.input_hash.clone(),
        payload: source.payload.clone(),
    }
}

/// Read the freshest non-expired cached advisory for the primary endpoint OR
/// the fallback endpoint. Without this, an advisory previously saved by the
/// fallback (different provider/model/format) would be invisible to the live
/// tick and the condition would stay "provider pending".
async fn latest_market_advisory_for_endpoints(
    state: &AppState,
    primary_request: &RewardAiAdvisoryRequest,
    fallback: Option<&RewardProviderFallback>,
) -> Result<Option<RewardMarketAdvisory>> {
    let primary = state
        .reward_bot_service
        .latest_market_advisory(primary_request)
        .await?;
    let Some(fb) = fallback else {
        return Ok(primary);
    };
    let fallback_request = reward_ai_advisory_request_for_endpoint(
        primary_request,
        fb.provider,
        fb.request_format,
        &fb.model,
    );
    let fallback_cached = state
        .reward_bot_service
        .latest_market_advisory(&fallback_request)
        .await?;
    Ok(freshest_reward_cache_row(primary, fallback_cached))
}

/// Read the freshest non-expired cached info-risk for the primary OR fallback
/// endpoint. The info-risk apply path is condition_id-keyed, but this
/// pre-request check still needs to honor a fallback cache row so we do not
/// re-call the provider when a valid fallback result already exists.
async fn latest_market_info_risk_for_endpoints(
    state: &AppState,
    primary_request: &RewardInfoRiskAssessmentRequest,
    fallback: Option<&RewardProviderFallback>,
) -> Result<Option<RewardMarketInfoRisk>> {
    let primary = state
        .reward_bot_service
        .latest_market_info_risk(primary_request)
        .await?;
    let Some(fb) = fallback else {
        return Ok(primary);
    };
    let fallback_request = reward_info_risk_request_for_endpoint(
        primary_request,
        fb.provider,
        fb.request_format,
        &fb.model,
    );
    let fallback_cached = state
        .reward_bot_service
        .latest_market_info_risk(&fallback_request)
        .await?;
    Ok(freshest_reward_cache_row(primary, fallback_cached))
}

/// Pick the cache row with the greater `expires_at` (None < Some). Both
/// advisory and info-risk rows expose `expires_at: OffsetDateTime`.
fn freshest_reward_cache_row<T>(primary: Option<T>, fallback: Option<T>) -> Option<T>
where
    T: CacheExpiresAt,
{
    match (primary, fallback) {
        (Some(a), Some(b)) => {
            if a.expires_at() >= b.expires_at() {
                Some(a)
            } else {
                Some(b)
            }
        }
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

trait CacheExpiresAt {
    fn expires_at(&self) -> OffsetDateTime;
}

impl CacheExpiresAt for RewardMarketAdvisory {
    fn expires_at(&self) -> OffsetDateTime {
        self.expires_at
    }
}

impl CacheExpiresAt for RewardMarketInfoRisk {
    fn expires_at(&self) -> OffsetDateTime {
        self.expires_at
    }
}

/// After both endpoints have been tried and failed, decide whether to stop the
/// round. Conservative OR: if either endpoint shows capacity/auth/rate-limit
/// strain, treat the round as overloaded. Parse-only failures on both sides do
/// not stop the round (matches today's behavior for parse errors).
fn reward_combined_provider_overloaded(
    primary_error: &AppError,
    fallback_error: Option<&AppError>,
) -> bool {
    reward_ai_provider_is_overloaded(primary_error)
        || fallback_error.is_some_and(reward_ai_provider_is_overloaded)
}

/// Try the primary AI advisory endpoint; on any failure retry against the
/// fallback (if configured). Records an `llm_calls` row for each attempt.
/// Permits are acquired per attempt (single-flight is still preserved — the
/// semaphore has concurrency 1) so recording stays outside the critical section
/// exactly like the existing single-call sites.
#[allow(clippy::too_many_arguments)]
async fn advise_with_fallback(
    state: &AppState,
    primary: &RewardAiAdvisoryChannel<'_>,
    fallback: Option<&RewardAiAdvisoryChannel<'_>>,
    primary_request: &RewardAiAdvisoryRequest,
    trace_id: &str,
) -> Result<RewardProviderAttempt<RewardAiAdvisoryDecision, RewardAiAdvisoryRequest>> {
    let condition_ids = vec![primary_request.condition_id.clone()];
    let input_hash = primary_request.input_hash.as_str();

    let started = Instant::now();
    let primary_result = {
        let _permit = acquire_reward_ai_advisory_provider_request_permit().await?;
        primary.connector.advise(primary_request).await
    };
    record_reward_provider_llm_call(
        state,
        REWARD_AI_ADVISORY_LLM_TASK_TYPE,
        REWARD_AI_ADVISORY_PROMPT_VERSION,
        primary.model.as_str(),
        input_hash,
        &condition_ids,
        started.elapsed(),
        primary_result.is_ok(),
        primary_result.as_ref().ok().map(|decision| json!(decision)),
        primary_result.as_ref().err().map(ToString::to_string),
        false,
        trace_id,
    )
    .await;
    let primary_error = match primary_result {
        Ok(decision) => {
            return Ok(RewardProviderAttempt::Success {
                decision,
                endpoint: RewardProviderEndpoint::Primary,
                request: primary_request.clone(),
            });
        }
        Err(error) => error,
    };

    let Some(fb) = fallback else {
        return Ok(RewardProviderAttempt::Failed {
            primary_error,
            fallback_error: None,
        });
    };

    let fallback_request = reward_ai_advisory_request_for_endpoint(
        primary_request,
        fb.provider,
        fb.request_format,
        &fb.model,
    );
    let started = Instant::now();
    let fallback_result = match acquire_reward_ai_advisory_provider_request_permit().await {
        Ok(_permit) => fb.connector.advise(&fallback_request).await,
        Err(error) => Err(error),
    };
    record_reward_provider_llm_call(
        state,
        REWARD_AI_ADVISORY_LLM_TASK_TYPE,
        REWARD_AI_ADVISORY_PROMPT_VERSION,
        fb.model.as_str(),
        input_hash,
        &condition_ids,
        started.elapsed(),
        fallback_result.is_ok(),
        fallback_result
            .as_ref()
            .ok()
            .map(|decision| json!(decision)),
        fallback_result.as_ref().err().map(ToString::to_string),
        true,
        trace_id,
    )
    .await;
    match fallback_result {
        Ok(decision) => Ok(RewardProviderAttempt::Success {
            decision,
            endpoint: RewardProviderEndpoint::Fallback,
            request: fallback_request,
        }),
        Err(fallback_error) => Ok(RewardProviderAttempt::Failed {
            primary_error,
            fallback_error: Some(fallback_error),
        }),
    }
}

/// Try the primary info-risk endpoint; on any failure retry against the
/// fallback (if configured). Mirrors `advise_with_fallback`.
#[allow(clippy::too_many_arguments)]
async fn assess_with_fallback(
    state: &AppState,
    primary: &RewardInfoRiskChannel<'_>,
    fallback: Option<&RewardInfoRiskChannel<'_>>,
    primary_request: &RewardInfoRiskAssessmentRequest,
    trace_id: &str,
) -> Result<RewardProviderAttempt<RewardInfoRiskAssessmentDecision, RewardInfoRiskAssessmentRequest>>
{
    let condition_ids = vec![primary_request.condition_id.clone()];
    let input_hash = primary_request.input_hash.as_str();

    let started = Instant::now();
    let primary_result = {
        let _permit = acquire_reward_info_risk_provider_request_permit().await?;
        primary.connector.assess(primary_request).await
    };
    record_reward_provider_llm_call(
        state,
        REWARD_INFO_RISK_LLM_TASK_TYPE,
        REWARD_INFO_RISK_PROMPT_VERSION,
        primary.model.as_str(),
        input_hash,
        &condition_ids,
        started.elapsed(),
        primary_result.is_ok(),
        primary_result.as_ref().ok().map(|decision| json!(decision)),
        primary_result.as_ref().err().map(ToString::to_string),
        false,
        trace_id,
    )
    .await;
    let primary_error = match primary_result {
        Ok(decision) => {
            return Ok(RewardProviderAttempt::Success {
                decision,
                endpoint: RewardProviderEndpoint::Primary,
                request: primary_request.clone(),
            });
        }
        Err(error) => error,
    };

    let Some(fb) = fallback else {
        return Ok(RewardProviderAttempt::Failed {
            primary_error,
            fallback_error: None,
        });
    };

    let fallback_request = reward_info_risk_request_for_endpoint(
        primary_request,
        fb.provider,
        fb.request_format,
        &fb.model,
    );
    let started = Instant::now();
    let fallback_result = match acquire_reward_info_risk_provider_request_permit().await {
        Ok(_permit) => fb.connector.assess(&fallback_request).await,
        Err(error) => Err(error),
    };
    record_reward_provider_llm_call(
        state,
        REWARD_INFO_RISK_LLM_TASK_TYPE,
        REWARD_INFO_RISK_PROMPT_VERSION,
        fb.model.as_str(),
        input_hash,
        &condition_ids,
        started.elapsed(),
        fallback_result.is_ok(),
        fallback_result
            .as_ref()
            .ok()
            .map(|decision| json!(decision)),
        fallback_result.as_ref().err().map(ToString::to_string),
        true,
        trace_id,
    )
    .await;
    match fallback_result {
        Ok(decision) => Ok(RewardProviderAttempt::Success {
            decision,
            endpoint: RewardProviderEndpoint::Fallback,
            request: fallback_request,
        }),
        Err(fallback_error) => Ok(RewardProviderAttempt::Failed {
            primary_error,
            fallback_error: Some(fallback_error),
        }),
    }
}

#[cfg(test)]
mod reward_provider_fallback_tests {
    use super::*;
    use polyedge_infrastructure::settings::RewardsSettings;

    fn settings_with_fallback() -> RewardsSettings {
        let mut settings = RewardsSettings::default();
        settings.ai_fallback_provider = Some("openai".to_string());
        settings.ai_fallback_request_format = Some("openai_chat_completions".to_string());
        settings.ai_fallback_api_key = Some("key".to_string());
        settings.ai_fallback_model = Some("gpt-4o".to_string());
        settings.ai_fallback_base_url = "https://api.openai.com/v1".to_string();
        settings
    }

    #[test]
    fn fallback_disabled_when_any_required_field_missing() {
        let mut settings = settings_with_fallback();
        settings.ai_fallback_api_key = None;
        assert!(resolve_reward_ai_fallback(&settings).is_none());

        let mut settings = settings_with_fallback();
        settings.ai_fallback_model = Some("   ".to_string());
        assert!(resolve_reward_ai_fallback(&settings).is_none());

        let mut settings = settings_with_fallback();
        settings.ai_fallback_provider = None;
        assert!(resolve_reward_ai_fallback(&settings).is_none());

        assert!(resolve_reward_ai_fallback(&RewardsSettings::default()).is_none());
    }

    #[test]
    fn fallback_disabled_when_provider_or_format_invalid() {
        let mut settings = settings_with_fallback();
        settings.ai_fallback_provider = Some("gemini".to_string());
        assert!(resolve_reward_ai_fallback(&settings).is_none());

        let mut settings = settings_with_fallback();
        settings.ai_fallback_request_format = Some("weird".to_string());
        assert!(resolve_reward_ai_fallback(&settings).is_none());
    }

    #[test]
    fn fallback_normalizes_anthropic_provider_to_messages_format() {
        let mut settings = settings_with_fallback();
        settings.ai_fallback_provider = Some("anthropic".to_string());
        settings.ai_fallback_request_format = Some("openai_chat_completions".to_string());
        let fallback = resolve_reward_ai_fallback(&settings).expect("fallback configured");
        assert_eq!(
            fallback.request_format,
            polyedge_application::RewardAiRequestFormat::AnthropicMessages
        );
        // Anthropic must disable web search regardless of the global flag.
        assert!(!fallback.web_search_enabled);
    }

    #[test]
    fn fallback_coerces_anthropic_messages_format_for_openai_provider() {
        let mut settings = settings_with_fallback();
        settings.ai_fallback_provider = Some("openai".to_string());
        settings.ai_fallback_request_format = Some("anthropic_messages".to_string());
        let fallback = resolve_reward_ai_fallback(&settings).expect("fallback configured");
        assert_eq!(
            fallback.request_format,
            polyedge_application::RewardAiRequestFormat::OpenAiResponses
        );
    }

    #[test]
    fn fallback_web_search_only_for_openai_responses() {
        let mut settings = settings_with_fallback();
        settings.info_risk_web_search_enabled = true;
        settings.ai_fallback_request_format = Some("openai_responses".to_string());
        let fallback = resolve_reward_ai_fallback(&settings).expect("fallback configured");
        assert!(fallback.web_search_enabled);

        settings.ai_fallback_request_format = Some("openai_chat_completions".to_string());
        let fallback = resolve_reward_ai_fallback(&settings).expect("fallback configured");
        assert!(!fallback.web_search_enabled);
    }

    #[test]
    fn fallback_uses_model_name_to_normalize_glm_and_deepseek_to_chat_completions() {
        let mut settings = settings_with_fallback();
        settings.ai_fallback_provider = Some("openai".to_string());
        settings.ai_fallback_request_format = Some("openai_responses".to_string());
        settings.ai_fallback_model = Some("glm-4.7".to_string());
        let fallback = resolve_reward_ai_fallback(&settings).expect("fallback configured");
        assert_eq!(
            fallback.provider,
            polyedge_application::RewardAiProvider::OpenAi
        );
        assert_eq!(
            fallback.request_format,
            polyedge_application::RewardAiRequestFormat::OpenAiChatCompletions
        );

        settings.ai_fallback_model = Some("deepseek-v4-flash".to_string());
        let fallback = resolve_reward_ai_fallback(&settings).expect("fallback configured");
        assert_eq!(
            fallback.provider,
            polyedge_application::RewardAiProvider::OpenAi
        );
        assert_eq!(
            fallback.request_format,
            polyedge_application::RewardAiRequestFormat::OpenAiChatCompletions
        );
    }

    #[test]
    fn combined_overload_is_conservative_or() {
        let transport = AppError::dependency_unavailable(
            "REWARD_AI_HTTP_FAILED",
            "reward AI HTTP request failed: operation timed out",
        );
        let parse_err = AppError::dependency_unavailable(
            "REWARD_AI_RESPONSE_INVALID",
            "provider response missing field",
        );
        // primary overloaded, no fallback -> stop.
        assert!(reward_combined_provider_overloaded(&transport, None));
        // both parse-only -> do not stop.
        assert!(!reward_combined_provider_overloaded(
            &parse_err,
            Some(&parse_err)
        ));
        // primary parse, fallback overloaded -> stop (either side strained).
        assert!(reward_combined_provider_overloaded(
            &parse_err,
            Some(&transport)
        ));
    }

    #[test]
    fn advisory_request_for_endpoint_preserves_input_hash_and_payload() {
        let primary = RewardAiAdvisoryRequest {
            condition_id: "0xcond".to_string(),
            provider: polyedge_application::RewardAiProvider::OpenAi,
            request_format: polyedge_application::RewardAiRequestFormat::OpenAiResponses,
            model: "gpt-4.1-mini".to_string(),
            input_hash: "hash123".to_string(),
            payload: json!({"keep": true}),
        };
        let fallback = reward_ai_advisory_request_for_endpoint(
            &primary,
            polyedge_application::RewardAiProvider::Anthropic,
            polyedge_application::RewardAiRequestFormat::AnthropicMessages,
            "claude-3-5",
        );
        assert_eq!(fallback.condition_id, primary.condition_id);
        assert_eq!(fallback.input_hash, primary.input_hash);
        assert_eq!(fallback.payload, primary.payload);
        assert_eq!(
            fallback.provider,
            polyedge_application::RewardAiProvider::Anthropic
        );
        assert_eq!(
            fallback.request_format,
            polyedge_application::RewardAiRequestFormat::AnthropicMessages
        );
        assert_eq!(fallback.model, "claude-3-5");
    }
}
