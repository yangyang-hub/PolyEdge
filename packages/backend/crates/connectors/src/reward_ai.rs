use polyedge_application::{
    RewardAiAdvisoryDecision, RewardAiAdvisoryRequest, RewardAiProvider, RewardAiSuitability,
    RewardPlanQuoteMode,
};
use polyedge_domain::{AppError, Result};
use rust_decimal::Decimal;
use serde_json::{Value, json};
use std::str::FromStr;

use crate::openai_compat::{
    is_openai_compatible_chat_provider, provider_json_candidates, provider_response_preview,
};

// Single-market advisory output is a small JSON object (allow_quote, confidence,
// conservative strategy_hint, reasons, metrics). Kept `pub(crate)` because the
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
        "required": ["allow_quote", "confidence", "strategy_hint", "reasons", "metrics"],
        "properties": {
            "allow_quote": {"type": "boolean"},
            "confidence": {"type": "number", "minimum": 0, "maximum": 1},
            "strategy_hint": reward_ai_strategy_hint_json_schema(),
            "reasons": {"type": "array", "items": {"type": "string"}, "maxItems": 6},
            "metrics": {"type": "object"}
        }
    })
}

pub(crate) fn reward_ai_strategy_hint_json_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["quote_mode", "bid_rank", "max_condition_notional_usd"],
        "properties": {
            "quote_mode": {
                "type": "string",
                "enum": ["double", "single_yes", "single_no", "none"]
            },
            "bid_rank": {"type": "integer", "minimum": 1, "maximum": 3},
            "max_condition_notional_usd": {"type": "number", "minimum": 0}
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
    if value.get("allow_quote").is_some() {
        return parse_reward_ai_binary_decision_value(value);
    }

    // Legacy 3-way fallback: some compatible models (e.g. MiMo over chat
    // completions) ignore the binary `allow_quote` contract and still return a
    // `suitability` object. Apply fail-closed binary semantics here — only an
    // explicit `allow` is honoured; `watch` and any other non-allow verdict
    // collapse to `avoid` so the advisory gate blocks the market instead of
    // silently letting an unendorsed market through. `quote_mode`/`exit_policy`
    // are only required for the explicit-allow shape; a blocked verdict uses
    // the canonical avoid defaults. Mirrored by advisory `schema_version` 8.
    let suitability = value
        .get("suitability")
        .and_then(Value::as_str)
        .ok_or_else(|| reward_ai_missing_field("suitability"))
        .and_then(RewardAiSuitability::from_str)?;
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
    if suitability != RewardAiSuitability::Allow {
        return Ok(RewardAiAdvisoryDecision {
            suitability: RewardAiSuitability::Avoid,
            quote_mode: RewardPlanQuoteMode::None,
            exit_policy: polyedge_application::PostFillStrategy::FlattenImmediately,
            confidence,
            reasons,
            metrics,
        });
    }
    let quote_mode = value
        .get("quote_mode")
        .and_then(Value::as_str)
        .ok_or_else(|| reward_ai_missing_field("quote_mode"))
        .and_then(RewardPlanQuoteMode::from_str)?;
    let exit_policy = value
        .get("exit_policy")
        .and_then(Value::as_str)
        .ok_or_else(|| reward_ai_missing_field("exit_policy"))
        .and_then(polyedge_application::PostFillStrategy::from_str)?;
    Ok(RewardAiAdvisoryDecision {
        suitability: RewardAiSuitability::Allow,
        quote_mode,
        exit_policy,
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
    let strategy_hint = parse_reward_ai_strategy_hint_value(value.get("strategy_hint"))?;
    Ok(RewardAiAdvisoryDecision {
        suitability: if allow_quote {
            RewardAiSuitability::Allow
        } else {
            RewardAiSuitability::Avoid
        },
        quote_mode: if allow_quote {
            RewardPlanQuoteMode::Double
        } else {
            RewardPlanQuoteMode::None
        },
        exit_policy: if allow_quote {
            polyedge_application::PostFillStrategy::ExitAtMarkup
        } else {
            polyedge_application::PostFillStrategy::FlattenImmediately
        },
        confidence,
        reasons,
        metrics: reward_ai_metrics_with_strategy_hint(metrics, strategy_hint),
    })
}

pub(crate) fn parse_reward_ai_strategy_hint_value(value: Option<&Value>) -> Result<Option<Value>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let quote_mode = value
        .get("quote_mode")
        .and_then(Value::as_str)
        .ok_or_else(|| reward_ai_missing_field("strategy_hint.quote_mode"))
        .and_then(RewardPlanQuoteMode::from_str)?;
    let bid_rank = value
        .get("bid_rank")
        .and_then(Value::as_u64)
        .ok_or_else(|| reward_ai_missing_field("strategy_hint.bid_rank"))?;
    if !(1..=3).contains(&bid_rank) {
        return Err(AppError::dependency_unavailable(
            "REWARD_AI_RESPONSE_INVALID_STRATEGY_HINT",
            "reward AI strategy_hint.bid_rank must be between 1 and 3",
        ));
    }
    let max_condition_notional_usd = value
        .get("max_condition_notional_usd")
        .and_then(parse_decimal_value)
        .ok_or_else(|| reward_ai_missing_field("strategy_hint.max_condition_notional_usd"))?;
    if max_condition_notional_usd < Decimal::ZERO {
        return Err(AppError::dependency_unavailable(
            "REWARD_AI_RESPONSE_INVALID_STRATEGY_HINT",
            "reward AI strategy_hint.max_condition_notional_usd must be non-negative",
        ));
    }

    Ok(Some(json!({
        "quote_mode": quote_mode.as_str(),
        "bid_rank": bid_rank,
        "max_condition_notional_usd": max_condition_notional_usd,
    })))
}

pub(crate) fn reward_ai_metrics_with_strategy_hint(
    metrics: Value,
    strategy_hint: Option<Value>,
) -> Value {
    let Some(strategy_hint) = strategy_hint else {
        return metrics;
    };
    let mut object = metrics.as_object().cloned().unwrap_or_default();
    object.insert("strategy_hint".to_string(), strategy_hint);
    Value::Object(object)
}

pub(crate) fn reward_ai_candidate_has_known_field(value: &Value) -> bool {
    value.get("allow_quote").is_some()
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
