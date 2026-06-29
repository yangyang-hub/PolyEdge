fn reward_provider_content_filter_rejected(error: &AppError) -> bool {
    if !matches!(
        error.code(),
        "REWARD_AI_STATUS_FAILED" | "REWARD_INFO_RISK_STATUS_FAILED"
            | "REWARD_PROVIDER_STATUS_FAILED"
    ) {
        return false;
    }

    let message = error.message();
    let lower = message.to_ascii_lowercase();
    lower.contains("contentfilter")
        || lower.contains("content_filter")
        || lower.contains("content filter")
        || lower.contains("\"code\":\"1301\"")
        || lower.contains("\"code\": \"1301\"")
        || lower.contains("sensitive content")
        || lower.contains("unsafe content")
}

fn reward_combined_provider_content_filtered(
    primary_error: &AppError,
    fallback_error: Option<&AppError>,
) -> bool {
    reward_provider_content_filter_rejected(primary_error)
        && fallback_error.map_or(true, reward_provider_content_filter_rejected)
}

fn reward_provider_error_summary(error: &AppError) -> String {
    const MAX_ERROR_SUMMARY_BYTES: usize = 512;

    let message = error.to_string();
    if message.len() <= MAX_ERROR_SUMMARY_BYTES {
        return message;
    }

    let mut end = MAX_ERROR_SUMMARY_BYTES;
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &message[..end])
}

fn reward_provider_content_filter_metrics(
    primary_error: &AppError,
    fallback_error: Option<&AppError>,
) -> Value {
    json!({
        "provider_failure": "content_filter",
        "primary_error": reward_provider_error_summary(primary_error),
        "fallback_error": fallback_error.map(reward_provider_error_summary),
    })
}

fn reward_content_filter_ai_decision(
    exit_policy: PostFillStrategy,
    primary_error: &AppError,
    fallback_error: Option<&AppError>,
) -> polyedge_application::RewardAiAdvisoryDecision {
    polyedge_application::RewardAiAdvisoryDecision {
        suitability: polyedge_application::RewardAiSuitability::Avoid,
        quote_mode: RewardPlanQuoteMode::None,
        exit_policy,
        confidence: Decimal::ONE,
        reasons: vec![
            "provider content filter rejected the advisory payload; blocking quote until cache expires"
                .to_string(),
        ],
        metrics: reward_provider_content_filter_metrics(primary_error, fallback_error),
    }
}

fn reward_content_filter_info_risk_decision(
    primary_error: &AppError,
    fallback_error: Option<&AppError>,
) -> polyedge_application::RewardInfoRiskAssessmentDecision {
    polyedge_application::RewardInfoRiskAssessmentDecision {
        risk_level: polyedge_application::RewardInfoRiskLevel::Critical,
        risk_type: polyedge_application::RewardInfoRiskType::Unknown,
        directional_risk: polyedge_application::RewardInfoDirectionalRisk::Unclear,
        resolution_imminent: false,
        expected_event_at: None,
        confidence: Decimal::ONE,
        summary: "provider content filter rejected the info-risk payload; blocking quote until cache expires"
            .to_string(),
        sources: Vec::new(),
        metrics: reward_provider_content_filter_metrics(primary_error, fallback_error),
    }
}

async fn save_reward_ai_content_filter_advisory(
    state: &AppState,
    request: &RewardAiAdvisoryRequest,
    ttl_sec: u64,
    exit_policy: PostFillStrategy,
    primary_error: &AppError,
    fallback_error: Option<&AppError>,
) -> Result<RewardMarketAdvisory> {
    let advisory = reward_content_filter_ai_decision(exit_policy, primary_error, fallback_error)
        .into_advisory(request, ttl_sec, OffsetDateTime::now_utc());
    state
        .reward_bot_service
        .save_market_advisory(&advisory)
        .await?;
    Ok(advisory)
}

async fn save_reward_info_risk_content_filter(
    state: &AppState,
    request: &RewardInfoRiskAssessmentRequest,
    ttl_sec: u64,
    primary_error: &AppError,
    fallback_error: Option<&AppError>,
) -> Result<RewardMarketInfoRisk> {
    let risk = reward_content_filter_info_risk_decision(primary_error, fallback_error)
        .into_info_risk(request, ttl_sec, OffsetDateTime::now_utc());
    state
        .reward_bot_service
        .save_market_info_risk(&risk)
        .await?;
    Ok(risk)
}

#[allow(clippy::too_many_arguments)]
async fn cache_reward_ai_content_filter_if_rejected(
    state: &AppState,
    request: &RewardAiAdvisoryRequest,
    ttl_sec: u64,
    exit_policy: PostFillStrategy,
    primary_error: &AppError,
    fallback_error: Option<&AppError>,
    trace_id: &str,
) -> Result<Option<RewardMarketAdvisory>> {
    if !reward_combined_provider_content_filtered(primary_error, fallback_error) {
        return Ok(None);
    }

    let advisory = save_reward_ai_content_filter_advisory(
        state,
        request,
        ttl_sec,
        exit_policy,
        primary_error,
        fallback_error,
    )
    .await?;
    info!(
        trace_id = %trace_id,
        condition_id = %request.condition_id,
        "cached reward AI advisory content-filter rejection",
    );
    Ok(Some(advisory))
}

async fn cache_reward_info_risk_content_filter_if_rejected(
    state: &AppState,
    request: &RewardInfoRiskAssessmentRequest,
    ttl_sec: u64,
    primary_error: &AppError,
    fallback_error: Option<&AppError>,
    trace_id: &str,
) -> Result<Option<RewardMarketInfoRisk>> {
    if !reward_combined_provider_content_filtered(primary_error, fallback_error) {
        return Ok(None);
    }

    let risk =
        save_reward_info_risk_content_filter(state, request, ttl_sec, primary_error, fallback_error)
            .await?;
    info!(
        trace_id = %trace_id,
        condition_id = %request.condition_id,
        "cached reward info risk content-filter rejection",
    );
    Ok(Some(risk))
}

#[cfg(test)]
mod reward_provider_content_filter_tests {
    use super::*;

    #[test]
    fn content_filter_detects_glm_1301_status_error() {
        let error = AppError::dependency_unavailable(
            "REWARD_AI_STATUS_FAILED",
            r#"reward AI provider returned HTTP 400: {"contentFilter":[{"level":1,"role":"user"}],"error":{"code":"1301"}}"#,
        );
        assert!(reward_provider_content_filter_rejected(&error));
    }

    #[test]
    fn content_filter_detects_combined_provider_status_error() {
        // The combined provider emits `REWARD_PROVIDER_STATUS_FAILED`; this code
        // must stay in the allowlist or content-filter rejections are not cached
        // and the same condition is re-requested every refresh cycle.
        let error = AppError::dependency_unavailable(
            "REWARD_PROVIDER_STATUS_FAILED",
            r#"reward provider returned HTTP 400: {"contentFilter":[{"level":1,"role":"user"}],"error":{"code":"1301","message":"系统检测到输入或生成内容可能包含不安全或敏感内容"}}"#,
        );
        assert!(reward_provider_content_filter_rejected(&error));
    }

    #[test]
    fn content_filter_ignores_plain_bad_request() {
        let error = AppError::dependency_unavailable(
            "REWARD_AI_STATUS_FAILED",
            r#"reward AI provider returned HTTP 400: {"error":{"code":"invalid_request"}}"#,
        );
        assert!(!reward_provider_content_filter_rejected(&error));
    }

    #[test]
    fn combined_content_filter_requires_fallback_filter_when_present() {
        let filtered = AppError::dependency_unavailable(
            "REWARD_INFO_RISK_STATUS_FAILED",
            r#"reward info risk provider returned HTTP 400: {"error":{"code":"1301"}}"#,
        );
        let transient = AppError::dependency_unavailable(
            "REWARD_INFO_RISK_HTTP_FAILED",
            "reward info risk HTTP request failed: operation timed out",
        );

        assert!(reward_combined_provider_content_filtered(&filtered, None));
        assert!(reward_combined_provider_content_filtered(
            &filtered,
            Some(&filtered)
        ));
        assert!(!reward_combined_provider_content_filtered(
            &filtered,
            Some(&transient)
        ));
    }
}
