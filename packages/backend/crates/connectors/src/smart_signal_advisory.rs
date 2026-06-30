use crate::openai_compat::{
    openai_compatible_chat_response_format, openai_compatible_chat_token_limit_field,
    openai_compatible_endpoint, provider_json_candidates, provider_response_preview,
    with_openai_compatible_auth,
};
use polyedge_application::{
    RewardAiProvider, RewardAiRequestFormat, SmartSignalAdvisoryDecision,
    SmartSignalAdvisoryRequest, SmartSignalDecisionValue, reward_ai_effective_request_format,
};
use polyedge_domain::{AppError, Result};
use reqwest::Client;
use rust_decimal::Decimal;
use serde_json::{Value, json};
use std::{str::FromStr, time::Duration};

const SMART_SIGNAL_ADVISORY_CHAT_COMPLETION_MAX_TOKENS: u32 = 2048;

#[derive(Debug, Clone)]
pub struct SmartSignalAdvisoryConnector {
    client: Client,
    base_url: String,
    api_key: String,
}

impl SmartSignalAdvisoryConnector {
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        timeout_secs: u64,
    ) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs.max(1)))
            .build()
            .map_err(|error| {
                AppError::dependency_unavailable(
                    "SMART_SIGNAL_ADVISORY_CLIENT_BUILD_FAILED",
                    format!("failed to build smart signal advisory HTTP client: {error}"),
                )
            })?;
        Ok(Self {
            client,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
        })
    }

    pub async fn advise(
        &self,
        request: &SmartSignalAdvisoryRequest,
    ) -> Result<SmartSignalAdvisoryDecision> {
        let provider = RewardAiProvider::from_str(&request.provider)?;
        let configured_format = RewardAiRequestFormat::from_str(&request.request_format)?;
        let request_format =
            reward_ai_effective_request_format(provider, configured_format, &request.model);
        let text = match request_format {
            RewardAiRequestFormat::OpenAiResponses => {
                self.call_openai_responses(request, provider).await?
            }
            RewardAiRequestFormat::OpenAiChatCompletions => {
                self.call_openai_chat_completions(request, provider).await?
            }
            RewardAiRequestFormat::AnthropicMessages => {
                self.call_anthropic_messages(request, provider).await?
            }
        };
        parse_smart_signal_advisory_decision(&text)
    }

    async fn call_openai_responses(
        &self,
        request: &SmartSignalAdvisoryRequest,
        provider: RewardAiProvider,
    ) -> Result<String> {
        ensure_smart_signal_provider(provider, RewardAiProvider::OpenAi)?;
        let response = with_openai_compatible_auth(
            self.client
                .post(openai_compatible_endpoint(&self.base_url, "responses")),
            &self.api_key,
        )
        .json(&json!({
            "model": request.model,
            "input": [
                {"role": "system", "content": [{"type": "input_text", "text": smart_signal_advisory_system_prompt()}]},
                {"role": "user", "content": [{"type": "input_text", "text": smart_signal_advisory_user_prompt(request)}]}
            ],
            "text": {
                "format": {
                    "type": "json_schema",
                    "name": "smart_signal_advisory",
                    "schema": smart_signal_advisory_json_schema(),
                    "strict": true
                }
            },
            "temperature": 0
        }))
        .send()
        .await
        .map_err(smart_signal_advisory_http_error)?;
        let status = response.status();
        let body: Value = response
            .json()
            .await
            .map_err(smart_signal_advisory_decode_error)?;
        if !status.is_success() {
            return Err(smart_signal_advisory_status_error(status.as_u16(), body));
        }
        extract_openai_responses_text(&body)
    }

    async fn call_openai_chat_completions(
        &self,
        request: &SmartSignalAdvisoryRequest,
        provider: RewardAiProvider,
    ) -> Result<String> {
        ensure_smart_signal_provider(provider, RewardAiProvider::OpenAi)?;
        let mut body = json!({
            "model": request.model,
            "messages": [
                {"role": "system", "content": smart_signal_advisory_system_prompt()},
                {"role": "user", "content": smart_signal_advisory_user_prompt(request)}
            ],
            "response_format": openai_compatible_chat_response_format(
                &request.model,
                "smart_signal_advisory",
                smart_signal_advisory_json_schema(),
            ),
            "temperature": 0
        });
        body[openai_compatible_chat_token_limit_field(&request.model)] =
            json!(SMART_SIGNAL_ADVISORY_CHAT_COMPLETION_MAX_TOKENS);
        let response = with_openai_compatible_auth(
            self.client.post(openai_compatible_endpoint(
                &self.base_url,
                "chat/completions",
            )),
            &self.api_key,
        )
        .json(&body)
        .send()
        .await
        .map_err(smart_signal_advisory_http_error)?;
        let status = response.status();
        let body: Value = response
            .json()
            .await
            .map_err(smart_signal_advisory_decode_error)?;
        if !status.is_success() {
            return Err(smart_signal_advisory_status_error(status.as_u16(), body));
        }
        body.pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .ok_or_else(|| {
                AppError::dependency_unavailable(
                    "SMART_SIGNAL_ADVISORY_RESPONSE_INVALID",
                    "OpenAI chat completion response did not include message content",
                )
            })
    }

    async fn call_anthropic_messages(
        &self,
        request: &SmartSignalAdvisoryRequest,
        provider: RewardAiProvider,
    ) -> Result<String> {
        ensure_smart_signal_provider(provider, RewardAiProvider::Anthropic)?;
        let response = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&json!({
                "model": request.model,
                "max_tokens": 1200,
                "temperature": 0,
                "system": smart_signal_advisory_system_prompt(),
                "messages": [
                    {"role": "user", "content": smart_signal_advisory_user_prompt(request)}
                ]
            }))
            .send()
            .await
            .map_err(smart_signal_advisory_http_error)?;
        let status = response.status();
        let body: Value = response
            .json()
            .await
            .map_err(smart_signal_advisory_decode_error)?;
        if !status.is_success() {
            return Err(smart_signal_advisory_status_error(status.as_u16(), body));
        }
        body.pointer("/content/0/text")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .ok_or_else(|| {
                AppError::dependency_unavailable(
                    "SMART_SIGNAL_ADVISORY_RESPONSE_INVALID",
                    "Anthropic messages response did not include text content",
                )
            })
    }
}

fn smart_signal_advisory_system_prompt() -> &'static str {
    "You are a risk reviewer for Polymarket smart-money copy signals. Return exactly one JSON object and nothing else. Do not use markdown, comments, prose, or unquoted keys. The recommendation field must be only allow, observe, or reject. Do not propose order placement; deterministic trading and risk checks are enforced outside the model."
}

fn smart_signal_advisory_user_prompt(request: &SmartSignalAdvisoryRequest) -> String {
    format!(
        "Assess whether this smart-money source trade remains copyable over the provider_cache_policy TTL horizon. Return one valid JSON object with double-quoted keys and these fields: recommendation string allow|observe|reject, confidence 0..1, risk_tags string array, summary string, reasons string array. Use observe for uncertainty, missing context, or stale evidence; use reject for clear non-copyable risk. Input:\n{}",
        request.payload
    )
}

fn smart_signal_advisory_json_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["recommendation", "confidence", "risk_tags", "summary", "reasons"],
        "properties": {
            "recommendation": {
                "type": "string",
                "enum": ["allow", "observe", "reject"]
            },
            "confidence": {"type": "number", "minimum": 0, "maximum": 1},
            "risk_tags": {"type": "array", "items": {"type": "string"}, "maxItems": 8},
            "summary": {"type": "string"},
            "reasons": {"type": "array", "items": {"type": "string"}, "maxItems": 8}
        }
    })
}

fn parse_smart_signal_advisory_decision(text: &str) -> Result<SmartSignalAdvisoryDecision> {
    let mut last_error = None;
    for value in provider_json_candidates(text) {
        if value.get("recommendation").is_none() {
            continue;
        }
        match parse_smart_signal_advisory_decision_value(value) {
            Ok(decision) => return Ok(decision),
            Err(error) => last_error = Some(error),
        }
    }
    if let Some(error) = last_error {
        return Err(error);
    }
    Err(AppError::dependency_unavailable(
        "SMART_SIGNAL_ADVISORY_RESPONSE_INVALID_JSON",
        format!(
            "smart signal advisory response was not valid JSON; preview={}",
            provider_response_preview(text)
        ),
    ))
}

fn parse_smart_signal_advisory_decision_value(value: Value) -> Result<SmartSignalAdvisoryDecision> {
    let recommendation = value
        .get("recommendation")
        .and_then(Value::as_str)
        .ok_or_else(|| smart_signal_advisory_missing_field("recommendation"))
        .and_then(SmartSignalDecisionValue::from_str)?;
    let confidence = parse_confidence(value.get("confidence"))
        .ok_or_else(|| smart_signal_advisory_missing_field("confidence"))?;
    let risk_tags = string_array(value.get("risk_tags"));
    let summary = value
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let reasons = string_array(value.get("reasons"));
    Ok(SmartSignalAdvisoryDecision {
        recommendation,
        confidence,
        risk_tags,
        summary,
        reasons,
        raw_output: value,
    })
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::trim))
                .filter(|item| !item.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn parse_confidence(value: Option<&Value>) -> Option<Decimal> {
    value
        .and_then(parse_decimal_value)
        .map(|confidence| confidence.clamp(Decimal::ZERO, Decimal::ONE))
}

fn parse_decimal_value(value: &Value) -> Option<Decimal> {
    if let Some(number) = value.as_f64() {
        return Decimal::from_f64_retain(number);
    }
    value
        .as_str()
        .and_then(|text| Decimal::from_str(text.trim()).ok())
}

fn ensure_smart_signal_provider(
    provider: RewardAiProvider,
    expected: RewardAiProvider,
) -> Result<()> {
    if provider == expected {
        return Ok(());
    }
    Err(AppError::invalid_input(
        "SMART_SIGNAL_ADVISORY_PROVIDER_FORMAT_MISMATCH",
        "smart signal advisory request format does not match provider",
    ))
}

fn extract_openai_responses_text(body: &Value) -> Result<String> {
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
                "SMART_SIGNAL_ADVISORY_RESPONSE_INVALID",
                "OpenAI responses output did not include text",
            )
        })
}

fn smart_signal_advisory_missing_field(field: &'static str) -> AppError {
    AppError::dependency_unavailable(
        "SMART_SIGNAL_ADVISORY_RESPONSE_MISSING_FIELD",
        format!("smart signal advisory response missing field {field}"),
    )
}

fn smart_signal_advisory_http_error(error: reqwest::Error) -> AppError {
    AppError::dependency_unavailable(
        "SMART_SIGNAL_ADVISORY_HTTP_FAILED",
        format!("smart signal advisory provider request failed: {error}"),
    )
}

fn smart_signal_advisory_decode_error(error: reqwest::Error) -> AppError {
    AppError::dependency_unavailable(
        "SMART_SIGNAL_ADVISORY_RESPONSE_DECODE_FAILED",
        format!("failed to decode smart signal advisory provider response: {error}"),
    )
}

fn smart_signal_advisory_status_error(status: u16, body: Value) -> AppError {
    AppError::dependency_unavailable(
        "SMART_SIGNAL_ADVISORY_PROVIDER_ERROR",
        format!("smart signal advisory provider returned HTTP {status}: {body}"),
    )
}

#[cfg(test)]
mod smart_signal_advisory_tests {
    use super::*;

    #[tokio::test]
    async fn smart_signal_advisory_chat_completion_request_uses_json_object_for_glm() {
        let (base_url, captured) = crate::test_http::spawn_json_response_server(
            r#"{"choices":[{"message":{"content":"{\"recommendation\":\"allow\",\"confidence\":0.82,\"risk_tags\":[\"copyable\"],\"summary\":\"still copyable\",\"reasons\":[\"fresh signal\"]}"}}]}"#,
        )
        .await;
        let connector = SmartSignalAdvisoryConnector::new(
            format!("{base_url}/api/coding/paas/v4"),
            "test-key",
            5,
        )
        .expect("build connector");
        let request = smart_signal_advisory_test_request("glm-4.7");

        let decision = connector
            .advise(&request)
            .await
            .expect("mock smart signal advisory");
        let captured = captured.await.expect("captured request");
        let headers = captured.headers.to_ascii_lowercase();

        assert_eq!(
            captured.request_line,
            "POST /api/coding/paas/v4/chat/completions HTTP/1.1"
        );
        assert!(headers.contains("authorization: bearer test-key"));
        assert!(headers.contains("api-key: test-key"));
        assert_eq!(captured.body["model"], json!("glm-4.7"));
        assert_eq!(
            captured.body.pointer("/response_format/type"),
            Some(&json!("json_object"))
        );
        assert_eq!(
            captured.body["max_tokens"],
            json!(SMART_SIGNAL_ADVISORY_CHAT_COMPLETION_MAX_TOKENS)
        );
        assert_eq!(decision.recommendation, SmartSignalDecisionValue::Allow);
        assert_eq!(decision.risk_tags, vec!["copyable"]);
    }

    #[tokio::test]
    async fn smart_signal_advisory_uses_chat_completions_with_strict_schema_for_agnes() {
        let (base_url, captured) = crate::test_http::spawn_json_response_server(
            r#"{"choices":[{"message":{"content":"{\"recommendation\":\"observe\",\"confidence\":0.66,\"risk_tags\":[\"uncertain\"],\"summary\":\"wait for confirmation\",\"reasons\":[\"thin context\"]}"}}]}"#,
        )
        .await;
        let connector =
            SmartSignalAdvisoryConnector::new(base_url, "test-key", 5).expect("build connector");
        let mut request = smart_signal_advisory_test_request("agnes-2.0-flash");
        request.request_format = RewardAiRequestFormat::OpenAiResponses.as_str().to_string();

        let decision = connector
            .advise(&request)
            .await
            .expect("mock agnes smart signal advisory");
        let captured = captured.await.expect("captured request");

        assert_eq!(captured.request_line, "POST /v1/chat/completions HTTP/1.1");
        assert_eq!(captured.body["model"], json!("agnes-2.0-flash"));
        assert_eq!(
            captured.body.pointer("/response_format/type"),
            Some(&json!("json_schema"))
        );
        assert_eq!(
            captured.body["max_completion_tokens"],
            json!(SMART_SIGNAL_ADVISORY_CHAT_COMPLETION_MAX_TOKENS)
        );
        assert!(captured.body.get("max_tokens").is_none());
        assert_eq!(decision.recommendation, SmartSignalDecisionValue::Observe);
    }

    #[test]
    fn smart_signal_advisory_parse_clamps_confidence_and_extracts_arrays() {
        let decision = parse_smart_signal_advisory_decision(
            r#"{
                "recommendation":"observe",
                "confidence":1.3,
                "risk_tags":["stale", "", "thin_book"],
                "summary":"wait for better context",
                "reasons":["book is thin"]
            }"#,
        )
        .expect("parse smart signal advisory");

        assert_eq!(decision.recommendation, SmartSignalDecisionValue::Observe);
        assert_eq!(decision.confidence, Decimal::ONE);
        assert_eq!(decision.risk_tags, vec!["stale", "thin_book"]);
        assert_eq!(decision.reasons, vec!["book is thin"]);
    }

    #[test]
    fn smart_signal_advisory_parse_embedded_json_object() {
        let decision = parse_smart_signal_advisory_decision(
            r#"Example: {"example":true}
Final: {"recommendation":"reject","confidence":"0.44","risk_tags":["late"],"summary":"too late","reasons":[]}"#,
        )
        .expect("parse embedded advisory");

        assert_eq!(decision.recommendation, SmartSignalDecisionValue::Reject);
        assert_eq!(decision.confidence, Decimal::from_str("0.44").unwrap());
    }

    fn smart_signal_advisory_test_request(model: &str) -> SmartSignalAdvisoryRequest {
        SmartSignalAdvisoryRequest {
            signal_id: 42,
            provider: RewardAiProvider::OpenAi.as_str().to_string(),
            request_format: RewardAiRequestFormat::OpenAiChatCompletions
                .as_str()
                .to_string(),
            model: model.to_string(),
            input_hash: "hash".to_string(),
            payload: json!({"signal": {"id": 42}}),
        }
    }
}
