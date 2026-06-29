use polyedge_application::{
    RewardAiProvider, RewardInfoDirectionalRisk, RewardInfoRiskAssessmentDecision,
    RewardInfoRiskAssessmentRequest, RewardInfoRiskLevel, RewardInfoRiskSource, RewardInfoRiskType,
};
use polyedge_domain::{AppError, Result};
use rust_decimal::Decimal;
use serde_json::{Value, json};
use std::str::FromStr;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::openai_compat::{
    is_openai_compatible_chat_provider, provider_json_candidates, provider_response_preview,
};

// Single-market info-risk output is a small JSON object (allow_quote, confidence,
// summary, sources, metrics). Kept `pub(crate)` because the reward_info_risk
// DeepSeek chat-completion test asserts against this token budget.
#[allow(dead_code)]
pub(crate) const REWARD_INFO_RISK_CHAT_COMPLETION_MAX_TOKENS: u32 = 1536;

#[allow(dead_code)]
pub(crate) fn ensure_info_risk_provider(
    request: &RewardInfoRiskAssessmentRequest,
    expected: RewardAiProvider,
) -> Result<()> {
    if request.provider == expected {
        return Ok(());
    }
    Err(AppError::invalid_input(
        "REWARD_INFO_RISK_PROVIDER_FORMAT_MISMATCH",
        "reward info risk request format does not match provider",
    ))
}

#[allow(dead_code)]
pub(crate) fn ensure_info_risk_openai_compatible_chat_provider(
    request: &RewardInfoRiskAssessmentRequest,
) -> Result<()> {
    if is_openai_compatible_chat_provider(request.provider) {
        return Ok(());
    }
    Err(AppError::invalid_input(
        "REWARD_INFO_RISK_PROVIDER_FORMAT_MISMATCH",
        "reward info risk chat completion request format requires an OpenAI-compatible provider",
    ))
}

pub(crate) fn reward_info_risk_json_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "allow_quote",
            "confidence",
            "summary",
            "sources",
            "metrics"
        ],
        "properties": {
            "allow_quote": {"type": "boolean"},
            "confidence": {"type": "number", "minimum": 0, "maximum": 1},
            "summary": {"type": "string"},
            "sources": {
                "type": "array",
                "maxItems": 8,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["url", "title", "published_at", "snippet"],
                    "properties": {
                        "url": {"type": "string"},
                        "title": {"type": "string"},
                        "published_at": {"type": ["string", "null"]},
                        "snippet": {"type": ["string", "null"]}
                    }
                }
            },
            "metrics": {"type": "object"}
        }
    })
}

#[allow(dead_code)]
pub(crate) fn parse_reward_info_risk_decision(
    text: &str,
) -> Result<RewardInfoRiskAssessmentDecision> {
    let value = parse_reward_info_risk_value(text)?;
    parse_reward_info_risk_decision_value(&value)
}

#[allow(dead_code)]
pub(crate) fn parse_reward_info_risk_value(text: &str) -> Result<Value> {
    let mut last_error = None;
    for value in provider_json_candidates(text) {
        if !reward_info_risk_candidate_has_known_field(&value) {
            continue;
        }
        match parse_reward_info_risk_decision_value(&value) {
            Ok(_) => return Ok(value),
            Err(error) => last_error = Some(error),
        }
    }
    if let Some(error) = last_error {
        return Err(error);
    }
    Err(AppError::dependency_unavailable(
        "REWARD_INFO_RISK_RESPONSE_INVALID_JSON",
        format!(
            "reward info risk response was not valid JSON; preview={}",
            provider_response_preview(text)
        ),
    ))
}

pub(crate) fn parse_reward_info_risk_decision_value(
    value: &Value,
) -> Result<RewardInfoRiskAssessmentDecision> {
    if value.get("allow_quote").is_some() {
        return parse_reward_info_risk_binary_decision_value(value);
    }

    let risk_level = value
        .get("risk_level")
        .and_then(Value::as_str)
        .ok_or_else(|| reward_info_risk_missing_field("risk_level"))
        .and_then(RewardInfoRiskLevel::from_str)?;
    let risk_type = value
        .get("risk_type")
        .and_then(Value::as_str)
        .ok_or_else(|| reward_info_risk_missing_field("risk_type"))
        .and_then(RewardInfoRiskType::from_str)?;
    let directional_risk = value
        .get("directional_risk")
        .and_then(Value::as_str)
        .ok_or_else(|| reward_info_risk_missing_field("directional_risk"))
        .and_then(RewardInfoDirectionalRisk::from_str)?;
    let resolution_imminent = value
        .get("resolution_imminent")
        .and_then(Value::as_bool)
        .ok_or_else(|| reward_info_risk_missing_field("resolution_imminent"))?;
    let confidence = parse_confidence(value.get("confidence"))
        .ok_or_else(|| reward_info_risk_missing_field("confidence"))?;
    let summary = value
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("no summary returned")
        .to_string();
    let sources = parse_info_risk_sources(value.get("sources"));
    let metrics = value
        .get("metrics")
        .cloned()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));

    Ok(RewardInfoRiskAssessmentDecision {
        risk_level,
        risk_type,
        directional_risk,
        resolution_imminent,
        expected_event_at: parse_optional_rfc3339(value.get("expected_event_at")),
        confidence,
        summary,
        sources,
        metrics,
    })
}

pub(crate) fn parse_reward_info_risk_binary_decision_value(
    value: &Value,
) -> Result<RewardInfoRiskAssessmentDecision> {
    let allow_quote = value
        .get("allow_quote")
        .and_then(Value::as_bool)
        .ok_or_else(|| reward_info_risk_missing_field("allow_quote"))?;
    let confidence = parse_confidence(value.get("confidence"))
        .ok_or_else(|| reward_info_risk_missing_field("confidence"))?;
    let summary = value
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("no summary returned")
        .to_string();
    let sources = parse_info_risk_sources(value.get("sources"));
    let metrics = value
        .get("metrics")
        .cloned()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));

    Ok(RewardInfoRiskAssessmentDecision {
        risk_level: if allow_quote {
            RewardInfoRiskLevel::Low
        } else {
            RewardInfoRiskLevel::Critical
        },
        risk_type: if allow_quote {
            RewardInfoRiskType::None
        } else {
            RewardInfoRiskType::Unknown
        },
        directional_risk: RewardInfoDirectionalRisk::Unclear,
        resolution_imminent: false,
        expected_event_at: None,
        confidence,
        summary,
        sources,
        metrics,
    })
}

pub(crate) fn reward_info_risk_candidate_has_known_field(value: &Value) -> bool {
    value.get("allow_quote").is_some()
        || value.get("risk_level").is_some()
        || value.get("risk_type").is_some()
        || value.get("directional_risk").is_some()
        || value.get("resolution_imminent").is_some()
        || value.get("confidence").is_some()
}

pub(crate) fn parse_info_risk_sources(value: Option<&Value>) -> Vec<RewardInfoRiskSource> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some(RewardInfoRiskSource {
                        url: item.get("url")?.as_str()?.to_string(),
                        title: item
                            .get("title")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        published_at: parse_optional_rfc3339(item.get("published_at")),
                        snippet: item
                            .get("snippet")
                            .and_then(Value::as_str)
                            .filter(|value| !value.trim().is_empty())
                            .map(ToString::to_string),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn parse_optional_rfc3339(value: Option<&Value>) -> Option<OffsetDateTime> {
    value
        .and_then(Value::as_str)
        .and_then(|value| OffsetDateTime::parse(value, &Rfc3339).ok())
}

#[allow(dead_code)]
pub(crate) fn extract_openai_responses_text(body: &Value) -> Result<String> {
    if let Some(text) = body.get("output_text").and_then(Value::as_str) {
        return Ok(text.to_string());
    }
    body.get("output")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find_map(|item| {
                item.get("content")
                    .and_then(Value::as_array)
                    .and_then(|content| {
                        content.iter().find_map(|part| {
                            part.get("text")
                                .or_else(|| part.get("content"))
                                .and_then(Value::as_str)
                        })
                    })
            })
        })
        .map(ToString::to_string)
        .ok_or_else(|| {
            AppError::dependency_unavailable(
                "REWARD_INFO_RISK_RESPONSE_INVALID",
                "OpenAI responses output did not include text",
            )
        })
}

pub(crate) fn parse_confidence(value: Option<&Value>) -> Option<Decimal> {
    let raw = value?;
    let parsed = if let Some(number) = raw.as_f64() {
        Decimal::from_str(&number.to_string()).ok()
    } else {
        raw.as_str().and_then(|value| Decimal::from_str(value).ok())
    }?;
    Some(parsed.max(Decimal::ZERO).min(Decimal::ONE))
}

pub(crate) fn reward_info_risk_missing_field(field: &'static str) -> AppError {
    AppError::dependency_unavailable(
        "REWARD_INFO_RISK_RESPONSE_MISSING_FIELD",
        format!("reward info risk response missing {field}"),
    )
}

#[allow(dead_code)]
pub(crate) fn reward_info_risk_http_error(error: reqwest::Error) -> AppError {
    AppError::dependency_unavailable(
        "REWARD_INFO_RISK_HTTP_FAILED",
        format!("reward info risk HTTP request failed: {error}"),
    )
}

#[allow(dead_code)]
pub(crate) fn reward_info_risk_decode_error(error: reqwest::Error) -> AppError {
    AppError::dependency_unavailable(
        "REWARD_INFO_RISK_RESPONSE_DECODE_FAILED",
        format!("failed to decode reward info risk response: {error}"),
    )
}

#[allow(dead_code)]
pub(crate) fn reward_info_risk_status_error(status: u16, body: Value) -> AppError {
    AppError::dependency_unavailable(
        "REWARD_INFO_RISK_STATUS_FAILED",
        format!("reward info risk provider returned HTTP {status}: {body}"),
    )
}

#[cfg(test)]
#[path = "reward_info_risk_tests.rs"]
mod tests;
