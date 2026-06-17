use crate::openai_compat::{openai_compatible_endpoint, with_openai_compatible_auth};
use polyedge_application::{
    RewardAiProvider, RewardAiRequestFormat, RewardInfoDirectionalRisk,
    RewardInfoRiskAssessmentDecision, RewardInfoRiskAssessmentRequest, RewardInfoRiskLevel,
    RewardInfoRiskSource, RewardInfoRiskType,
};
use polyedge_domain::{AppError, Result};
use reqwest::Client;
use rust_decimal::Decimal;
use serde_json::{Value, json};
use std::{str::FromStr, time::Duration};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

#[derive(Debug, Clone)]
pub struct RewardInfoRiskConnector {
    client: Client,
    base_url: String,
    api_key: String,
    web_search_enabled: bool,
}

impl RewardInfoRiskConnector {
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        timeout_secs: u64,
        web_search_enabled: bool,
    ) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs.max(1)))
            .build()
            .map_err(|error| {
                AppError::dependency_unavailable(
                    "REWARD_INFO_RISK_CLIENT_BUILD_FAILED",
                    format!("failed to build reward info risk HTTP client: {error}"),
                )
            })?;
        Ok(Self {
            client,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            web_search_enabled,
        })
    }

    pub async fn assess(
        &self,
        request: &RewardInfoRiskAssessmentRequest,
    ) -> Result<RewardInfoRiskAssessmentDecision> {
        let text = match request.request_format {
            RewardAiRequestFormat::OpenAiResponses => self.call_openai_responses(request).await?,
            RewardAiRequestFormat::OpenAiChatCompletions => {
                self.call_openai_chat_completions(request).await?
            }
            RewardAiRequestFormat::AnthropicMessages => {
                self.call_anthropic_messages(request).await?
            }
        };
        parse_reward_info_risk_decision(&text)
    }

    async fn call_openai_responses(
        &self,
        request: &RewardInfoRiskAssessmentRequest,
    ) -> Result<String> {
        ensure_info_risk_provider(request, RewardAiProvider::OpenAi)?;
        let mut body = json!({
            "model": request.model,
            "input": [
                {"role": "system", "content": [{"type": "input_text", "text": reward_info_risk_system_prompt()}]},
                {"role": "user", "content": [{"type": "input_text", "text": reward_info_risk_user_prompt(request)}]}
            ],
            "text": {
                "format": {
                    "type": "json_schema",
                    "name": "reward_market_info_risk",
                    "schema": reward_info_risk_json_schema(),
                    "strict": true
                }
            }
        });
        if self.web_search_enabled {
            body["tools"] = json!([{ "type": "web_search_preview" }]);
        }
        let response = with_openai_compatible_auth(
            self.client
                .post(openai_compatible_endpoint(&self.base_url, "responses")),
            &self.api_key,
        )
        .json(&body)
        .send()
        .await
        .map_err(reward_info_risk_http_error)?;
        let status = response.status();
        let body: Value = response
            .json()
            .await
            .map_err(reward_info_risk_decode_error)?;
        if !status.is_success() {
            return Err(reward_info_risk_status_error(status.as_u16(), body));
        }
        extract_openai_responses_text(&body)
    }

    async fn call_openai_chat_completions(
        &self,
        request: &RewardInfoRiskAssessmentRequest,
    ) -> Result<String> {
        ensure_info_risk_provider(request, RewardAiProvider::OpenAi)?;
        let response = with_openai_compatible_auth(
            self.client.post(openai_compatible_endpoint(
                &self.base_url,
                "chat/completions",
            )),
            &self.api_key,
        )
        .json(&json!({
            "model": request.model,
            "messages": [
                {"role": "system", "content": reward_info_risk_system_prompt()},
                {"role": "user", "content": reward_info_risk_user_prompt(request)}
            ],
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "reward_market_info_risk",
                    "schema": reward_info_risk_json_schema(),
                    "strict": true
                }
            }
        }))
        .send()
        .await
        .map_err(reward_info_risk_http_error)?;
        let status = response.status();
        let body: Value = response
            .json()
            .await
            .map_err(reward_info_risk_decode_error)?;
        if !status.is_success() {
            return Err(reward_info_risk_status_error(status.as_u16(), body));
        }
        body.pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .ok_or_else(|| {
                AppError::dependency_unavailable(
                    "REWARD_INFO_RISK_RESPONSE_INVALID",
                    "OpenAI chat completion response did not include message content",
                )
            })
    }

    async fn call_anthropic_messages(
        &self,
        request: &RewardInfoRiskAssessmentRequest,
    ) -> Result<String> {
        ensure_info_risk_provider(request, RewardAiProvider::Anthropic)?;
        let response = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&json!({
                "model": request.model,
                "max_tokens": 1400,
                "system": reward_info_risk_system_prompt(),
                "messages": [
                    {"role": "user", "content": reward_info_risk_user_prompt(request)}
                ]
            }))
            .send()
            .await
            .map_err(reward_info_risk_http_error)?;
        let status = response.status();
        let body: Value = response
            .json()
            .await
            .map_err(reward_info_risk_decode_error)?;
        if !status.is_success() {
            return Err(reward_info_risk_status_error(status.as_u16(), body));
        }
        body.pointer("/content/0/text")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .ok_or_else(|| {
                AppError::dependency_unavailable(
                    "REWARD_INFO_RISK_RESPONSE_INVALID",
                    "Anthropic messages response did not include text content",
                )
            })
    }
}

fn ensure_info_risk_provider(
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

fn reward_info_risk_system_prompt() -> &'static str {
    "You are a cautious event-risk researcher for Polymarket rewards maker orders. Return only JSON. Identify whether recent or imminent information could make the market probability move one-sided, settle soon, or become unsafe for passive maker orders. Use web search when a search tool is available; otherwise use the supplied context and clearly mark uncertainty."
}

fn reward_info_risk_user_prompt(request: &RewardInfoRiskAssessmentRequest) -> String {
    format!(
        "Assess information risk for this market. Return JSON with fields: risk_level low|medium|high|critical|unknown, risk_type imminent_resolution|breaking_news|scheduled_event|official_result|rumor|stale|none|unknown, directional_risk yes|no|unclear, resolution_imminent boolean, expected_event_at RFC3339 string or null, confidence 0..1, summary string, sources array of {{url,title,published_at,snippet}}, metrics object.\nSearch query: {}\nInput:\n{}",
        request.query, request.payload
    )
}

fn reward_info_risk_json_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "risk_level",
            "risk_type",
            "directional_risk",
            "resolution_imminent",
            "expected_event_at",
            "confidence",
            "summary",
            "sources",
            "metrics"
        ],
        "properties": {
            "risk_level": {"type": "string", "enum": ["low", "medium", "high", "critical", "unknown"]},
            "risk_type": {"type": "string", "enum": ["imminent_resolution", "breaking_news", "scheduled_event", "official_result", "rumor", "stale", "none", "unknown"]},
            "directional_risk": {"type": "string", "enum": ["yes", "no", "unclear"]},
            "resolution_imminent": {"type": "boolean"},
            "expected_event_at": {"type": ["string", "null"]},
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

fn parse_reward_info_risk_decision(text: &str) -> Result<RewardInfoRiskAssessmentDecision> {
    let value: Value = serde_json::from_str(text)
        .or_else(|_| extract_json_object(text).and_then(|json| serde_json::from_str(json)))
        .map_err(|error| {
            AppError::dependency_unavailable(
                "REWARD_INFO_RISK_RESPONSE_INVALID_JSON",
                format!("reward info risk response was not valid JSON: {error}"),
            )
        })?;
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

fn parse_info_risk_sources(value: Option<&Value>) -> Vec<RewardInfoRiskSource> {
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

fn parse_optional_rfc3339(value: Option<&Value>) -> Option<OffsetDateTime> {
    value
        .and_then(Value::as_str)
        .and_then(|value| OffsetDateTime::parse(value, &Rfc3339).ok())
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
                "REWARD_INFO_RISK_RESPONSE_INVALID",
                "OpenAI responses output did not include text",
            )
        })
}

fn parse_confidence(value: Option<&Value>) -> Option<Decimal> {
    let raw = value?;
    let parsed = if let Some(number) = raw.as_f64() {
        Decimal::from_str(&number.to_string()).ok()
    } else {
        raw.as_str().and_then(|value| Decimal::from_str(value).ok())
    }?;
    Some(parsed.max(Decimal::ZERO).min(Decimal::ONE))
}

fn extract_json_object(text: &str) -> std::result::Result<&str, serde_json::Error> {
    let start = text.find('{').unwrap_or(0);
    let end = text.rfind('}').map_or(text.len(), |index| index + 1);
    Ok(&text[start..end])
}

fn reward_info_risk_missing_field(field: &'static str) -> AppError {
    AppError::dependency_unavailable(
        "REWARD_INFO_RISK_RESPONSE_MISSING_FIELD",
        format!("reward info risk response missing {field}"),
    )
}

fn reward_info_risk_http_error(error: reqwest::Error) -> AppError {
    AppError::dependency_unavailable(
        "REWARD_INFO_RISK_HTTP_FAILED",
        format!("reward info risk HTTP request failed: {error}"),
    )
}

fn reward_info_risk_decode_error(error: reqwest::Error) -> AppError {
    AppError::dependency_unavailable(
        "REWARD_INFO_RISK_RESPONSE_DECODE_FAILED",
        format!("failed to decode reward info risk response: {error}"),
    )
}

fn reward_info_risk_status_error(status: u16, body: Value) -> AppError {
    AppError::dependency_unavailable(
        "REWARD_INFO_RISK_STATUS_FAILED",
        format!("reward info risk provider returned HTTP {status}: {body}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reward_info_risk_confidence_is_clamped_to_unit_interval() {
        let high = parse_reward_info_risk_decision(
            r#"{
                "risk_level": "high",
                "risk_type": "breaking_news",
                "directional_risk": "unclear",
                "resolution_imminent": false,
                "confidence": 1.7,
                "summary": "test",
                "sources": [],
                "metrics": {}
            }"#,
        )
        .expect("parse high confidence");
        assert_eq!(high.confidence, Decimal::ONE);

        let low = parse_reward_info_risk_decision(
            r#"{
                "risk_level": "unknown",
                "risk_type": "unknown",
                "directional_risk": "unclear",
                "resolution_imminent": false,
                "confidence": "-0.1",
                "summary": "test",
                "sources": [],
                "metrics": {}
            }"#,
        )
        .expect("parse low confidence");
        assert_eq!(low.confidence, Decimal::ZERO);
    }
}
