use crate::openai_compat::{
    openai_compatible_endpoint, provider_json_candidates, provider_response_preview,
    with_openai_compatible_auth,
};
use polyedge_application::{
    RewardAiProvider, RewardAiRequestFormat, RewardInfoDirectionalRisk,
    RewardInfoRiskAssessmentBatchItem, RewardInfoRiskAssessmentDecision,
    RewardInfoRiskAssessmentRequest, RewardInfoRiskLevel, RewardInfoRiskSource, RewardInfoRiskType,
};
use polyedge_domain::{AppError, Result};
use reqwest::Client;
use rust_decimal::Decimal;
use serde_json::{Value, json};
use std::{collections::HashSet, str::FromStr, time::Duration};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

const REWARD_INFO_RISK_CHAT_COMPLETION_MAX_TOKENS: u32 = 6144;
const REWARD_INFO_RISK_BATCH_MAX_TOKENS_CAP: u32 = 16_384;

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

    pub async fn assess_batch(
        &self,
        requests: &[RewardInfoRiskAssessmentRequest],
    ) -> Result<Vec<RewardInfoRiskAssessmentBatchItem>> {
        if requests.is_empty() {
            return Ok(Vec::new());
        }
        let text = match requests[0].request_format {
            RewardAiRequestFormat::OpenAiResponses => {
                self.call_openai_responses_batch(requests).await?
            }
            RewardAiRequestFormat::OpenAiChatCompletions => {
                self.call_openai_chat_completions_batch(requests).await?
            }
            RewardAiRequestFormat::AnthropicMessages => {
                self.call_anthropic_messages_batch(requests).await?
            }
        };
        let condition_ids: Vec<String> = requests
            .iter()
            .map(|request| request.condition_id.clone())
            .collect();
        parse_reward_info_risk_batch_decision(&text, &condition_ids)
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
            },
            "temperature": 0
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
            },
            "temperature": 0,
            "max_completion_tokens": REWARD_INFO_RISK_CHAT_COMPLETION_MAX_TOKENS
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
                "temperature": 0,
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

    async fn call_openai_responses_batch(
        &self,
        requests: &[RewardInfoRiskAssessmentRequest],
    ) -> Result<String> {
        ensure_info_risk_batch_provider(requests, RewardAiProvider::OpenAi)?;
        let mut body = json!({
            "model": requests[0].model,
            "input": [
                {"role": "system", "content": [{"type": "input_text", "text": reward_info_risk_batch_system_prompt()}]},
                {"role": "user", "content": [{"type": "input_text", "text": reward_info_risk_batch_user_prompt(requests)}]}
            ],
            "text": {
                "format": {
                    "type": "json_schema",
                    "name": "reward_market_info_risks",
                    "schema": reward_info_risk_batch_json_schema(),
                    "strict": true
                }
            },
            "temperature": 0
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

    async fn call_openai_chat_completions_batch(
        &self,
        requests: &[RewardInfoRiskAssessmentRequest],
    ) -> Result<String> {
        ensure_info_risk_batch_provider(requests, RewardAiProvider::OpenAi)?;
        let max_tokens = reward_info_risk_batch_max_tokens(requests.len());
        let response = with_openai_compatible_auth(
            self.client.post(openai_compatible_endpoint(
                &self.base_url,
                "chat/completions",
            )),
            &self.api_key,
        )
        .json(&json!({
            "model": requests[0].model,
            "messages": [
                {"role": "system", "content": reward_info_risk_batch_system_prompt()},
                {"role": "user", "content": reward_info_risk_batch_user_prompt(requests)}
            ],
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "reward_market_info_risks",
                    "schema": reward_info_risk_batch_json_schema(),
                    "strict": true
                }
            },
            "temperature": 0,
            "max_completion_tokens": max_tokens
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

    async fn call_anthropic_messages_batch(
        &self,
        requests: &[RewardInfoRiskAssessmentRequest],
    ) -> Result<String> {
        ensure_info_risk_batch_provider(requests, RewardAiProvider::Anthropic)?;
        let max_tokens = reward_info_risk_batch_max_tokens(requests.len());
        let response = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&json!({
                "model": requests[0].model,
                "max_tokens": max_tokens,
                "temperature": 0,
                "system": reward_info_risk_batch_system_prompt(),
                "messages": [
                    {"role": "user", "content": reward_info_risk_batch_user_prompt(requests)}
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

fn ensure_info_risk_batch_provider(
    requests: &[RewardInfoRiskAssessmentRequest],
    expected: RewardAiProvider,
) -> Result<()> {
    if requests.is_empty() {
        return Err(AppError::invalid_input(
            "REWARD_INFO_RISK_BATCH_EMPTY",
            "reward info risk batch must not be empty",
        ));
    }
    ensure_info_risk_provider(&requests[0], expected)?;
    let first = &requests[0];
    for request in &requests[1..] {
        ensure_info_risk_provider(request, expected)?;
        if request.request_format != first.request_format || request.model != first.model {
            return Err(AppError::invalid_input(
                "REWARD_INFO_RISK_BATCH_MISMATCH",
                "reward info risk batch requests must share request format and model",
            ));
        }
    }
    Ok(())
}

fn reward_info_risk_batch_max_tokens(batch_size: usize) -> u32 {
    REWARD_INFO_RISK_CHAT_COMPLETION_MAX_TOKENS
        .saturating_mul(batch_size.max(1) as u32)
        .min(REWARD_INFO_RISK_BATCH_MAX_TOKENS_CAP)
}

fn reward_info_risk_system_prompt() -> &'static str {
    "You are a cautious event-risk researcher for Polymarket rewards maker orders. Return exactly one JSON object and nothing else. Do not use markdown, comments, prose, or unquoted keys. Identify whether recent or imminent information could make the market probability move one-sided, settle soon, or become unsafe for passive maker orders. Use web search when a search tool is available; otherwise use the supplied context and clearly mark uncertainty."
}

fn reward_info_risk_batch_system_prompt() -> &'static str {
    "You are a cautious event-risk researcher for Polymarket rewards maker orders. You will receive a JSON object containing a \"markets\" array; assess EACH market independently of the others. Return exactly one JSON object of shape {\"risks\":[...]} and nothing else. Do not use markdown, comments, prose, or unquoted keys. Each risk object must include the market's condition_id copied verbatim from the input."
}

fn reward_info_risk_user_prompt(request: &RewardInfoRiskAssessmentRequest) -> String {
    format!(
        "Assess information risk for this market. Return one valid JSON object with double-quoted keys and these fields: risk_level low|medium|high|critical|unknown, risk_type imminent_resolution|breaking_news|scheduled_event|official_result|rumor|stale|none|unknown, directional_risk yes|no|unclear, resolution_imminent boolean, expected_event_at RFC3339 string or null, confidence 0..1, summary string, sources array of objects with url,title,published_at,snippet, metrics object. Use [] for sources and {{}} for metrics when unsure.\nSearch query: {}\nInput:\n{}",
        request.query, request.payload
    )
}

fn reward_info_risk_batch_user_prompt(requests: &[RewardInfoRiskAssessmentRequest]) -> String {
    let markets: Vec<Value> = requests
        .iter()
        .map(|request| {
            json!({
                "condition_id": request.condition_id,
                "search_query": request.query,
                "market": request.payload
            })
        })
        .collect();
    format!(
        "Assess information risk for each market. Return one valid JSON object with double-quoted keys and a field \"risks\": an array with exactly one object per input market. Each object must contain condition_id (must match one input market verbatim), risk_level low|medium|high|critical|unknown, risk_type imminent_resolution|breaking_news|scheduled_event|official_result|rumor|stale|none|unknown, directional_risk yes|no|unclear, resolution_imminent boolean, expected_event_at RFC3339 string or null, confidence 0..1, summary string, sources array of objects with url,title,published_at,snippet, metrics object. Use [] for sources and {{}} for metrics when unsure.\nInput:\n{{\"markets\":{}}}",
        serde_json::to_string(&markets).unwrap_or_else(|_| "[]".to_string())
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

fn reward_info_risk_batch_json_schema() -> Value {
    let item_schema = reward_info_risk_json_schema();
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["risks"],
        "properties": {
            "risks": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": [
                        "condition_id",
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
                        "condition_id": {"type": "string"},
                        "risk_level": item_schema["properties"]["risk_level"].clone(),
                        "risk_type": item_schema["properties"]["risk_type"].clone(),
                        "directional_risk": item_schema["properties"]["directional_risk"].clone(),
                        "resolution_imminent": item_schema["properties"]["resolution_imminent"].clone(),
                        "expected_event_at": item_schema["properties"]["expected_event_at"].clone(),
                        "confidence": item_schema["properties"]["confidence"].clone(),
                        "summary": item_schema["properties"]["summary"].clone(),
                        "sources": item_schema["properties"]["sources"].clone(),
                        "metrics": item_schema["properties"]["metrics"].clone()
                    }
                }
            }
        }
    })
}

fn parse_reward_info_risk_batch_decision(
    text: &str,
    expected_condition_ids: &[String],
) -> Result<Vec<RewardInfoRiskAssessmentBatchItem>> {
    let expected: HashSet<&str> = expected_condition_ids.iter().map(String::as_str).collect();
    let single_market_batch = expected_condition_ids.len() == 1;
    let single_condition = expected_condition_ids.first().map(String::as_str);
    let mut last_error: Option<AppError> = None;
    for value in provider_json_candidates(text) {
        if let Some(risks) = value.get("risks").and_then(Value::as_array) {
            let mut items = Vec::new();
            let mut seen: HashSet<String> = HashSet::new();
            for entry in risks {
                let Some(condition_id) = entry.get("condition_id").and_then(Value::as_str) else {
                    continue;
                };
                if !expected.contains(condition_id) || !seen.insert(condition_id.to_string()) {
                    continue;
                }
                match parse_reward_info_risk_decision_value(entry) {
                    Ok(decision) => items.push(RewardInfoRiskAssessmentBatchItem {
                        condition_id: condition_id.to_string(),
                        decision,
                    }),
                    Err(error) => last_error = Some(error),
                }
            }
            if !items.is_empty() {
                return Ok(items);
            }
        }
        if single_market_batch
            && reward_info_risk_candidate_has_known_field(&value)
            && let Some(condition_id) = single_condition
            && let Ok(decision) = parse_reward_info_risk_decision_value(&value)
        {
            return Ok(vec![RewardInfoRiskAssessmentBatchItem {
                condition_id: condition_id.to_string(),
                decision,
            }]);
        }
    }
    Err(last_error.unwrap_or_else(|| {
        AppError::dependency_unavailable(
            "REWARD_INFO_RISK_BATCH_RESPONSE_INVALID_JSON",
            format!(
                "reward info risk batch response had no usable risks; preview={}",
                provider_response_preview(text)
            ),
        )
    }))
}

fn parse_reward_info_risk_decision(text: &str) -> Result<RewardInfoRiskAssessmentDecision> {
    let value = parse_reward_info_risk_value(text)?;
    parse_reward_info_risk_decision_value(&value)
}

fn parse_reward_info_risk_value(text: &str) -> Result<Value> {
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

fn parse_reward_info_risk_decision_value(
    value: &Value,
) -> Result<RewardInfoRiskAssessmentDecision> {
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

fn reward_info_risk_candidate_has_known_field(value: &Value) -> bool {
    value.get("risk_level").is_some()
        || value.get("risk_type").is_some()
        || value.get("directional_risk").is_some()
        || value.get("resolution_imminent").is_some()
        || value.get("confidence").is_some()
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
#[path = "reward_info_risk_tests.rs"]
mod tests;
