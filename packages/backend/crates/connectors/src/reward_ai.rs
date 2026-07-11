use polyedge_application::{
    RewardAiAdvisoryDecision, RewardAiAdvisoryRequest, RewardAiProvider, RewardProviderAction,
};
use polyedge_domain::{AppError, Result};
use rust_decimal::Decimal;
use serde_json::{Value, json};
use std::str::FromStr;

use crate::openai_compat::{
    is_openai_compatible_chat_provider, provider_json_candidates, provider_response_preview,
};

// Single-market advisory output is a bounded slow-risk action. Kept `pub(crate)` because the
// reward_ai GLM/DeepSeek chat-completion tests assert against this token budget.
#[allow(dead_code)]
pub(crate) const REWARD_AI_CHAT_COMPLETION_MAX_TOKENS: u32 = 1024;

#[allow(dead_code)]
pub(crate) fn ensure_provider(
    request: &RewardAiAdvisoryRequest,
    expected: RewardAiProvider,
) -> Result<()> {
    if request.provider == expected {
        return Ok(());
    }
    Err(AppError::invalid_input(
        "REWARD_AI_PROVIDER_FORMAT_MISMATCH",
        "reward AI request format does not match provider",
    ))
}

#[allow(dead_code)]
pub(crate) fn ensure_openai_compatible_chat_provider(
    request: &RewardAiAdvisoryRequest,
) -> Result<()> {
    if is_openai_compatible_chat_provider(request.provider) {
        return Ok(());
    }
    Err(AppError::invalid_input(
        "REWARD_AI_PROVIDER_FORMAT_MISMATCH",
        "reward AI chat completion request format requires an OpenAI-compatible provider",
    ))
}

pub(crate) fn reward_ai_json_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["action", "size_multiplier", "edge_buffer_cents", "confidence", "reasons", "metrics"],
        "properties": {
            "action": {"type": "string", "enum": ["allow", "reduce", "stop_new"]},
            "size_multiplier": {"type": "number", "minimum": 0, "maximum": 1},
            "edge_buffer_cents": {"type": "number", "minimum": 0, "maximum": 10},
            "confidence": {"type": "number", "minimum": 0, "maximum": 1},
            "reasons": {"type": "array", "items": {"type": "string"}, "maxItems": 6},
            "metrics": {"type": "object"}
        }
    })
}

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
                "REWARD_AI_RESPONSE_INVALID",
                "OpenAI responses output did not include text",
            )
        })
}

#[allow(dead_code)]
pub(crate) fn parse_reward_ai_decision(text: &str) -> Result<RewardAiAdvisoryDecision> {
    let value = parse_reward_ai_value(text)?;
    parse_reward_ai_decision_value(&value)
}

#[allow(dead_code)]
pub(crate) fn parse_reward_ai_value(text: &str) -> Result<Value> {
    let mut last_error = None;
    for value in provider_json_candidates(text) {
        if !reward_ai_candidate_has_known_field(&value) {
            continue;
        }
        match parse_reward_ai_decision_value(&value) {
            Ok(_) => return Ok(value),
            Err(error) => last_error = Some(error),
        }
    }
    if let Some(error) = last_error {
        return Err(error);
    }
    Err(AppError::dependency_unavailable(
        "REWARD_AI_RESPONSE_INVALID_JSON",
        format!(
            "reward AI response was not valid JSON; preview={}",
            provider_response_preview(text)
        ),
    ))
}

pub(crate) fn parse_reward_ai_decision_value(value: &Value) -> Result<RewardAiAdvisoryDecision> {
    if value.get("action").is_some() {
        return parse_reward_ai_v2_decision_value(value);
    }
    if value.get("allow_quote").is_some() {
        return parse_reward_ai_binary_decision_value(value);
    }

    // Ingress-only compatibility: older compatible models may return a
    // `suitability` object. Convert it to the bounded V2 action immediately;
    // legacy direction/price/exit fields are ignored and never persisted.
    let suitability = value
        .get("suitability")
        .and_then(Value::as_str)
        .ok_or_else(|| reward_ai_missing_field("suitability"))?;
    let confidence = parse_confidence(value.get("confidence"))
        .ok_or_else(|| reward_ai_missing_field("confidence"))?;
    let reasons = value
        .get("reasons")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let metrics = value
        .get("metrics")
        .cloned()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    let action = match suitability {
        "allow" => RewardProviderAction::Allow,
        "watch" | "avoid" => RewardProviderAction::StopNew,
        other => {
            return Err(AppError::dependency_unavailable(
                "REWARD_AI_SUITABILITY_INVALID",
                format!("unknown legacy reward AI suitability: {other}"),
            ));
        }
    };
    Ok(RewardAiAdvisoryDecision {
        action,
        size_multiplier: if action == RewardProviderAction::Allow {
            Decimal::ONE
        } else {
            Decimal::ZERO
        },
        edge_buffer_cents: Decimal::ZERO,
        confidence,
        reasons,
        metrics,
    })
}

fn parse_reward_ai_v2_decision_value(value: &Value) -> Result<RewardAiAdvisoryDecision> {
    let action = value
        .get("action")
        .and_then(Value::as_str)
        .ok_or_else(|| reward_ai_missing_field("action"))
        .and_then(RewardProviderAction::from_str)?;
    if !matches!(
        action,
        RewardProviderAction::Allow | RewardProviderAction::Reduce | RewardProviderAction::StopNew
    ) {
        return Err(AppError::dependency_unavailable(
            "REWARD_AI_RESPONSE_INVALID_ACTION",
            "AI advisory may only return allow, reduce, or stop_new",
        ));
    }
    let parsed_size_multiplier = parse_decimal_value(
        value
            .get("size_multiplier")
            .ok_or_else(|| reward_ai_missing_field("size_multiplier"))?,
    )
    .ok_or_else(|| reward_ai_missing_field("size_multiplier"))?
    .max(Decimal::ZERO)
    .min(Decimal::ONE);
    let parsed_edge_buffer_cents = parse_decimal_value(
        value
            .get("edge_buffer_cents")
            .ok_or_else(|| reward_ai_missing_field("edge_buffer_cents"))?,
    )
    .ok_or_else(|| reward_ai_missing_field("edge_buffer_cents"))?
    .max(Decimal::ZERO)
    .min(Decimal::from(10_u64));
    let confidence = parse_confidence(value.get("confidence"))
        .ok_or_else(|| reward_ai_missing_field("confidence"))?;
    let reasons = value
        .get("reasons")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let metrics = value
        .get("metrics")
        .cloned()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    let (size_multiplier, edge_buffer_cents) = match action {
        RewardProviderAction::Allow => (Decimal::ONE, Decimal::ZERO),
        RewardProviderAction::Reduce => (
            parsed_size_multiplier.max(Decimal::new(1, 1)),
            parsed_edge_buffer_cents,
        ),
        RewardProviderAction::StopNew => (Decimal::ZERO, Decimal::ZERO),
        _ => unreachable!("advisory action validated above"),
    };
    Ok(RewardAiAdvisoryDecision {
        action,
        size_multiplier,
        edge_buffer_cents,
        confidence,
        reasons,
        metrics,
    })
}

pub(crate) fn parse_reward_ai_binary_decision_value(
    value: &Value,
) -> Result<RewardAiAdvisoryDecision> {
    let allow_quote = value
        .get("allow_quote")
        .and_then(Value::as_bool)
        .ok_or_else(|| reward_ai_missing_field("allow_quote"))?;
    let confidence = parse_confidence(value.get("confidence"))
        .ok_or_else(|| reward_ai_missing_field("confidence"))?;
    let reasons = value
        .get("reasons")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let metrics = value
        .get("metrics")
        .cloned()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    Ok(RewardAiAdvisoryDecision {
        action: if allow_quote {
            RewardProviderAction::Allow
        } else {
            RewardProviderAction::StopNew
        },
        size_multiplier: if allow_quote {
            Decimal::ONE
        } else {
            Decimal::ZERO
        },
        edge_buffer_cents: Decimal::ZERO,
        confidence,
        reasons,
        metrics,
    })
}

pub(crate) fn reward_ai_candidate_has_known_field(value: &Value) -> bool {
    value.get("action").is_some()
        || value.get("allow_quote").is_some()
        || value.get("suitability").is_some()
        || value.get("quote_mode").is_some()
        || value.get("exit_policy").is_some()
        || value.get("strategy_hint").is_some()
        || value.get("confidence").is_some()
}

pub(crate) fn parse_confidence(value: Option<&Value>) -> Option<Decimal> {
    parse_decimal_value(value?).map(|parsed| parsed.max(Decimal::ZERO).min(Decimal::ONE))
}

pub(crate) fn parse_decimal_value(raw: &Value) -> Option<Decimal> {
    if let Some(number) = raw.as_f64() {
        Decimal::from_str(&number.to_string()).ok()
    } else {
        raw.as_str().and_then(|value| Decimal::from_str(value).ok())
    }
}

pub(crate) fn reward_ai_missing_field(field: &'static str) -> AppError {
    AppError::dependency_unavailable(
        "REWARD_AI_RESPONSE_MISSING_FIELD",
        format!("reward AI response missing {field}"),
    )
}

#[allow(dead_code)]
pub(crate) fn reward_ai_http_error(error: reqwest::Error) -> AppError {
    AppError::dependency_unavailable(
        "REWARD_AI_HTTP_FAILED",
        format!("reward AI HTTP request failed: {error}"),
    )
}

#[allow(dead_code)]
pub(crate) fn reward_ai_decode_error(error: reqwest::Error) -> AppError {
    AppError::dependency_unavailable(
        "REWARD_AI_RESPONSE_DECODE_FAILED",
        format!("failed to decode reward AI response: {error}"),
    )
}

#[allow(dead_code)]
pub(crate) fn reward_ai_status_error(status: u16, body: Value) -> AppError {
    AppError::dependency_unavailable(
        "REWARD_AI_STATUS_FAILED",
        format!("reward AI provider returned HTTP {status}: {body}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    include!("reward_ai_tests.rs");
}
