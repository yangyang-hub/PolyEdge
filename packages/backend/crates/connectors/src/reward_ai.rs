use crate::openai_compat::{
    openai_compatible_endpoint, provider_json_candidates, provider_response_preview,
    with_openai_compatible_auth,
};
use polyedge_application::{
    RewardAiAdvisoryBatchItem, RewardAiAdvisoryDecision, RewardAiAdvisoryRequest, RewardAiProvider,
    RewardAiRequestFormat, RewardAiSuitability, RewardPlanQuoteMode,
};
use polyedge_domain::{AppError, Result};
use reqwest::Client;
use rust_decimal::Decimal;
use serde_json::{Value, json};
use std::{collections::HashSet, str::FromStr, time::Duration};

const REWARD_AI_CHAT_COMPLETION_MAX_TOKENS: u32 = 4096;
const REWARD_AI_BATCH_MAX_TOKENS_CAP: u32 = 16_384;

#[derive(Debug, Clone)]
pub struct RewardAiAdvisoryConnector {
    client: Client,
    base_url: String,
    api_key: String,
}

impl RewardAiAdvisoryConnector {
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
                    "REWARD_AI_CLIENT_BUILD_FAILED",
                    format!("failed to build reward AI HTTP client: {error}"),
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
        request: &RewardAiAdvisoryRequest,
    ) -> Result<RewardAiAdvisoryDecision> {
        let text = match request.request_format {
            RewardAiRequestFormat::OpenAiResponses => self.call_openai_responses(request).await?,
            RewardAiRequestFormat::OpenAiChatCompletions => {
                self.call_openai_chat_completions(request).await?
            }
            RewardAiRequestFormat::AnthropicMessages => {
                self.call_anthropic_messages(request).await?
            }
        };
        parse_reward_ai_decision(&text)
    }

    /// Assess multiple markets in a single provider call. All requests must share
    /// the same provider / request_format / model (the caller assembles the
    /// batch). Returned items are matched by `condition_id`; any market the model
    /// omitted or mislabeled is absent from the result, and the caller retries it
    /// via the single-market [`Self::advise`] path.
    pub async fn advise_batch(
        &self,
        requests: &[RewardAiAdvisoryRequest],
    ) -> Result<Vec<RewardAiAdvisoryBatchItem>> {
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
        parse_reward_ai_batch_decision(&text, &condition_ids)
    }

    async fn call_openai_responses(&self, request: &RewardAiAdvisoryRequest) -> Result<String> {
        ensure_provider(request, RewardAiProvider::OpenAi)?;
        let response = with_openai_compatible_auth(
            self.client
                .post(openai_compatible_endpoint(&self.base_url, "responses")),
            &self.api_key,
        )
            .json(&json!({
                "model": request.model,
                "input": [
                    {"role": "system", "content": [{"type": "input_text", "text": reward_ai_system_prompt()}]},
                    {"role": "user", "content": [{"type": "input_text", "text": reward_ai_user_prompt(request)}]}
                ],
                "text": {
                    "format": {
                        "type": "json_schema",
                        "name": "reward_market_advisory",
                        "schema": reward_ai_json_schema(),
                        "strict": true
                    }
                },
                "temperature": 0
            }))
            .send()
            .await
            .map_err(reward_ai_http_error)?;
        let status = response.status();
        let body: Value = response.json().await.map_err(reward_ai_decode_error)?;
        if !status.is_success() {
            return Err(reward_ai_status_error(status.as_u16(), body));
        }
        extract_openai_responses_text(&body)
    }

    async fn call_openai_chat_completions(
        &self,
        request: &RewardAiAdvisoryRequest,
    ) -> Result<String> {
        ensure_provider(request, RewardAiProvider::OpenAi)?;
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
                {"role": "system", "content": reward_ai_system_prompt()},
                {"role": "user", "content": reward_ai_user_prompt(request)}
            ],
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "reward_market_advisory",
                    "schema": reward_ai_json_schema(),
                    "strict": true
                }
            },
            "temperature": 0,
            "max_completion_tokens": REWARD_AI_CHAT_COMPLETION_MAX_TOKENS
        }))
        .send()
        .await
        .map_err(reward_ai_http_error)?;
        let status = response.status();
        let body: Value = response.json().await.map_err(reward_ai_decode_error)?;
        if !status.is_success() {
            return Err(reward_ai_status_error(status.as_u16(), body));
        }
        body.pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .ok_or_else(|| {
                AppError::dependency_unavailable(
                    "REWARD_AI_RESPONSE_INVALID",
                    "OpenAI chat completion response did not include message content",
                )
            })
    }

    async fn call_anthropic_messages(&self, request: &RewardAiAdvisoryRequest) -> Result<String> {
        ensure_provider(request, RewardAiProvider::Anthropic)?;
        let response = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&json!({
                "model": request.model,
                "max_tokens": 1200,
                "temperature": 0,
                "system": reward_ai_system_prompt(),
                "messages": [
                    {"role": "user", "content": reward_ai_user_prompt(request)}
                ]
            }))
            .send()
            .await
            .map_err(reward_ai_http_error)?;
        let status = response.status();
        let body: Value = response.json().await.map_err(reward_ai_decode_error)?;
        if !status.is_success() {
            return Err(reward_ai_status_error(status.as_u16(), body));
        }
        body.pointer("/content/0/text")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .ok_or_else(|| {
                AppError::dependency_unavailable(
                    "REWARD_AI_RESPONSE_INVALID",
                    "Anthropic messages response did not include text content",
                )
            })
    }

    async fn call_openai_responses_batch(
        &self,
        requests: &[RewardAiAdvisoryRequest],
    ) -> Result<String> {
        ensure_batch_provider(requests, RewardAiProvider::OpenAi)?;
        let response = with_openai_compatible_auth(
            self.client
                .post(openai_compatible_endpoint(&self.base_url, "responses")),
            &self.api_key,
        )
            .json(&json!({
                "model": requests[0].model,
                "input": [
                    {"role": "system", "content": [{"type": "input_text", "text": reward_ai_batch_system_prompt()}]},
                    {"role": "user", "content": [{"type": "input_text", "text": reward_ai_batch_user_prompt(requests)}]}
                ],
                "text": {
                    "format": {
                        "type": "json_schema",
                        "name": "reward_market_advisories",
                        "schema": reward_ai_batch_json_schema(),
                        "strict": true
                    }
                },
                "temperature": 0
            }))
            .send()
            .await
            .map_err(reward_ai_http_error)?;
        let status = response.status();
        let body: Value = response.json().await.map_err(reward_ai_decode_error)?;
        if !status.is_success() {
            return Err(reward_ai_status_error(status.as_u16(), body));
        }
        extract_openai_responses_text(&body)
    }

    async fn call_openai_chat_completions_batch(
        &self,
        requests: &[RewardAiAdvisoryRequest],
    ) -> Result<String> {
        ensure_batch_provider(requests, RewardAiProvider::OpenAi)?;
        let max_tokens = reward_ai_batch_max_tokens(requests.len());
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
                {"role": "system", "content": reward_ai_batch_system_prompt()},
                {"role": "user", "content": reward_ai_batch_user_prompt(requests)}
            ],
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "reward_market_advisories",
                    "schema": reward_ai_batch_json_schema(),
                    "strict": true
                }
            },
            "temperature": 0,
            "max_completion_tokens": max_tokens
        }))
        .send()
        .await
        .map_err(reward_ai_http_error)?;
        let status = response.status();
        let body: Value = response.json().await.map_err(reward_ai_decode_error)?;
        if !status.is_success() {
            return Err(reward_ai_status_error(status.as_u16(), body));
        }
        body.pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .ok_or_else(|| {
                AppError::dependency_unavailable(
                    "REWARD_AI_RESPONSE_INVALID",
                    "OpenAI chat completion response did not include message content",
                )
            })
    }

    async fn call_anthropic_messages_batch(
        &self,
        requests: &[RewardAiAdvisoryRequest],
    ) -> Result<String> {
        ensure_batch_provider(requests, RewardAiProvider::Anthropic)?;
        let max_tokens = reward_ai_batch_max_tokens(requests.len());
        let response = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&json!({
                "model": requests[0].model,
                "max_tokens": max_tokens,
                "temperature": 0,
                "system": reward_ai_batch_system_prompt(),
                "messages": [
                    {"role": "user", "content": reward_ai_batch_user_prompt(requests)}
                ]
            }))
            .send()
            .await
            .map_err(reward_ai_http_error)?;
        let status = response.status();
        let body: Value = response.json().await.map_err(reward_ai_decode_error)?;
        if !status.is_success() {
            return Err(reward_ai_status_error(status.as_u16(), body));
        }
        body.pointer("/content/0/text")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .ok_or_else(|| {
                AppError::dependency_unavailable(
                    "REWARD_AI_RESPONSE_INVALID",
                    "Anthropic messages response did not include text content",
                )
            })
    }
}

fn ensure_provider(request: &RewardAiAdvisoryRequest, expected: RewardAiProvider) -> Result<()> {
    if request.provider == expected {
        return Ok(());
    }
    Err(AppError::invalid_input(
        "REWARD_AI_PROVIDER_FORMAT_MISMATCH",
        "reward AI request format does not match provider",
    ))
}

fn reward_ai_system_prompt() -> &'static str {
    "You are a risk reviewer for Polymarket rewards maker orders. Return exactly one JSON object and nothing else. Do not use markdown, comments, prose, or unquoted keys. Do not suggest bypassing deterministic risk checks. Favor watch/avoid when data is thin, concentrated, stale, or reversal risk is unclear."
}

fn reward_ai_user_prompt(request: &RewardAiAdvisoryRequest) -> String {
    format!(
        "Assess whether this rewards market is suitable for maker quoting. Return one valid JSON object with double-quoted keys and these fields: suitability allow|watch|avoid, quote_mode double|single_yes|single_no|none, exit_policy exit_at_markup|hold_and_requote|flatten_immediately, confidence 0..1, reasons string array, metrics object. Use {{}} for metrics when unsure.\nInput:\n{}",
        request.payload
    )
}

fn reward_ai_json_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["suitability", "quote_mode", "exit_policy", "confidence", "reasons", "metrics"],
        "properties": {
            "suitability": {"type": "string", "enum": ["allow", "watch", "avoid"]},
            "quote_mode": {"type": "string", "enum": ["double", "single_yes", "single_no", "none"]},
            "exit_policy": {"type": "string", "enum": ["exit_at_markup", "hold_and_requote", "flatten_immediately"]},
            "confidence": {"type": "number", "minimum": 0, "maximum": 1},
            "reasons": {"type": "array", "items": {"type": "string"}, "maxItems": 6},
            "metrics": {"type": "object"}
        }
    })
}

fn ensure_batch_provider(
    requests: &[RewardAiAdvisoryRequest],
    expected: RewardAiProvider,
) -> Result<()> {
    if requests.is_empty() {
        return Err(AppError::invalid_input(
            "REWARD_AI_BATCH_EMPTY",
            "reward AI advisory batch must not be empty",
        ));
    }
    ensure_provider(&requests[0], expected)
}

fn reward_ai_batch_max_tokens(batch_size: usize) -> u32 {
    REWARD_AI_CHAT_COMPLETION_MAX_TOKENS
        .saturating_mul(batch_size.max(1) as u32)
        .min(REWARD_AI_BATCH_MAX_TOKENS_CAP)
}

fn reward_ai_batch_system_prompt() -> &'static str {
    "You are a risk reviewer for Polymarket rewards maker orders. You will receive a JSON object containing a \"markets\" array; assess EACH market independently of the others. Return exactly one JSON object of shape {\"advisories\":[...]} and nothing else. Do not use markdown, comments, prose, or unquoted keys. Do not suggest bypassing deterministic risk checks. Favor watch/avoid when data is thin, concentrated, stale, or reversal risk is unclear. Each advisory object must include the market's condition_id copied verbatim from the input."
}

fn reward_ai_batch_user_prompt(requests: &[RewardAiAdvisoryRequest]) -> String {
    let markets: Vec<Value> = requests
        .iter()
        .map(|request| json!({"condition_id": request.condition_id, "market": request.payload}))
        .collect();
    format!(
        "Assess whether each of these rewards markets is suitable for maker quoting. Return one valid JSON object with double-quoted keys and a field \"advisories\": an array with exactly one object per input market, each containing condition_id (must match one of the input markets verbatim) plus suitability allow|watch|avoid, quote_mode double|single_yes|single_no|none, exit_policy exit_at_markup|hold_and_requote|flatten_immediately, confidence 0..1, reasons string array, metrics object. Use {{}} for metrics when unsure.\nInput:\n{{\"markets\":{}}}",
        serde_json::to_string(&markets).unwrap_or_else(|_| "[]".to_string())
    )
}

fn reward_ai_batch_json_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["advisories"],
        "properties": {
            "advisories": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["condition_id", "suitability", "quote_mode", "exit_policy", "confidence", "reasons", "metrics"],
                    "properties": {
                        "condition_id": {"type": "string"},
                        "suitability": {"type": "string", "enum": ["allow", "watch", "avoid"]},
                        "quote_mode": {"type": "string", "enum": ["double", "single_yes", "single_no", "none"]},
                        "exit_policy": {"type": "string", "enum": ["exit_at_markup", "hold_and_requote", "flatten_immediately"]},
                        "confidence": {"type": "number", "minimum": 0, "maximum": 1},
                        "reasons": {"type": "array", "items": {"type": "string"}, "maxItems": 6},
                        "metrics": {"type": "object"}
                    }
                }
            }
        }
    })
}

/// Parse a batch advisory response into items keyed by `condition_id`. Entries
/// whose `condition_id` is not in `expected_condition_ids` (typos/extras) and
/// duplicates are dropped. Markets the model omitted are simply absent — the
/// caller retries them via the single-market path. When exactly one market was
/// requested, a bare advisory object (no `{"advisories":[...]}` wrapper) is
/// accepted as a compatibility fallback.
fn parse_reward_ai_batch_decision(
    text: &str,
    expected_condition_ids: &[String],
) -> Result<Vec<RewardAiAdvisoryBatchItem>> {
    let expected: HashSet<&str> = expected_condition_ids.iter().map(String::as_str).collect();
    let single_market_batch = expected_condition_ids.len() == 1;
    let single_condition = expected_condition_ids.first().map(String::as_str);
    let mut last_error: Option<AppError> = None;
    for value in provider_json_candidates(text) {
        if let Some(advisories) = value.get("advisories").and_then(Value::as_array) {
            let mut items: Vec<RewardAiAdvisoryBatchItem> = Vec::new();
            let mut seen: HashSet<String> = HashSet::new();
            for entry in advisories {
                let Some(condition_id) = entry.get("condition_id").and_then(Value::as_str) else {
                    continue;
                };
                if !expected.contains(condition_id) || !seen.insert(condition_id.to_string()) {
                    continue;
                }
                match parse_reward_ai_decision_value(entry) {
                    Ok(decision) => items.push(RewardAiAdvisoryBatchItem {
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
            && reward_ai_candidate_has_known_field(&value)
            && let Some(condition_id) = single_condition
            && let Ok(decision) = parse_reward_ai_decision_value(&value)
        {
            return Ok(vec![RewardAiAdvisoryBatchItem {
                condition_id: condition_id.to_string(),
                decision,
            }]);
        }
    }
    Err(last_error.unwrap_or_else(|| {
        AppError::dependency_unavailable(
            "REWARD_AI_BATCH_RESPONSE_INVALID_JSON",
            format!(
                "reward AI batch response had no usable advisories; preview={}",
                provider_response_preview(text)
            ),
        )
    }))
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
                "REWARD_AI_RESPONSE_INVALID",
                "OpenAI responses output did not include text",
            )
        })
}

fn parse_reward_ai_decision(text: &str) -> Result<RewardAiAdvisoryDecision> {
    let value = parse_reward_ai_value(text)?;
    parse_reward_ai_decision_value(&value)
}

fn parse_reward_ai_value(text: &str) -> Result<Value> {
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

fn parse_reward_ai_decision_value(value: &Value) -> Result<RewardAiAdvisoryDecision> {
    let suitability = value
        .get("suitability")
        .and_then(Value::as_str)
        .ok_or_else(|| reward_ai_missing_field("suitability"))
        .and_then(RewardAiSuitability::from_str)?;
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
        suitability,
        quote_mode,
        exit_policy,
        confidence,
        reasons,
        metrics,
    })
}

fn reward_ai_candidate_has_known_field(value: &Value) -> bool {
    value.get("suitability").is_some()
        || value.get("quote_mode").is_some()
        || value.get("exit_policy").is_some()
        || value.get("confidence").is_some()
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

fn reward_ai_missing_field(field: &'static str) -> AppError {
    AppError::dependency_unavailable(
        "REWARD_AI_RESPONSE_MISSING_FIELD",
        format!("reward AI response missing {field}"),
    )
}

fn reward_ai_http_error(error: reqwest::Error) -> AppError {
    AppError::dependency_unavailable(
        "REWARD_AI_HTTP_FAILED",
        format!("reward AI HTTP request failed: {error}"),
    )
}

fn reward_ai_decode_error(error: reqwest::Error) -> AppError {
    AppError::dependency_unavailable(
        "REWARD_AI_RESPONSE_DECODE_FAILED",
        format!("failed to decode reward AI response: {error}"),
    )
}

fn reward_ai_status_error(status: u16, body: Value) -> AppError {
    AppError::dependency_unavailable(
        "REWARD_AI_STATUS_FAILED",
        format!("reward AI provider returned HTTP {status}: {body}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reward_ai_confidence_is_clamped_to_unit_interval() {
        let high = parse_reward_ai_decision(
            r#"{
                "suitability": "allow",
                "quote_mode": "double",
                "exit_policy": "exit_at_markup",
                "confidence": 1.5,
                "reasons": [],
                "metrics": {}
            }"#,
        )
        .expect("parse high confidence");
        assert_eq!(high.confidence, Decimal::ONE);

        let low = parse_reward_ai_decision(
            r#"{
                "suitability": "watch",
                "quote_mode": "none",
                "exit_policy": "flatten_immediately",
                "confidence": "-0.2",
                "reasons": [],
                "metrics": {}
            }"#,
        )
        .expect("parse low confidence");
        assert_eq!(low.confidence, Decimal::ZERO);
    }

    #[test]
    fn reward_ai_parse_skips_embedded_example_object() {
        let parsed = parse_reward_ai_decision(
            r#"Example shape: {"example": true}
Final:
{"suitability":"allow","quote_mode":"double","exit_policy":"exit_at_markup","confidence":0.8,"reasons":[],"metrics":{}}
"#,
        )
        .expect("parse embedded response");

        assert_eq!(parsed.suitability, RewardAiSuitability::Allow);
        assert_eq!(parsed.quote_mode, RewardPlanQuoteMode::Double);
    }

    #[test]
    fn reward_ai_parse_accepts_json_string_payload() {
        let parsed = parse_reward_ai_decision(
            r#""{\"suitability\":\"watch\",\"quote_mode\":\"none\",\"exit_policy\":\"flatten_immediately\",\"confidence\":0.4,\"reasons\":[],\"metrics\":{}}""#,
        )
        .expect("parse json string payload");

        assert_eq!(parsed.suitability, RewardAiSuitability::Watch);
        assert_eq!(parsed.confidence, Decimal::from_str("0.4").unwrap());
    }

    #[test]
    fn reward_ai_batch_parse_full_array() {
        let items = parse_reward_ai_batch_decision(
            r#"{"advisories":[
                {"condition_id":"c1","suitability":"allow","quote_mode":"double","exit_policy":"exit_at_markup","confidence":0.8,"reasons":[],"metrics":{}},
                {"condition_id":"c2","suitability":"watch","quote_mode":"none","exit_policy":"flatten_immediately","confidence":0.4,"reasons":[],"metrics":{}}
            ]}"#,
            &["c1".to_string(), "c2".to_string()],
        )
        .expect("parse full batch array");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].condition_id, "c1");
        assert_eq!(items[0].decision.suitability, RewardAiSuitability::Allow);
        assert_eq!(items[1].condition_id, "c2");
        assert_eq!(items[1].decision.suitability, RewardAiSuitability::Watch);
    }

    #[test]
    fn reward_ai_batch_parse_matches_by_condition_id_regardless_of_order() {
        let items = parse_reward_ai_batch_decision(
            r#"{"advisories":[
                {"condition_id":"c2","suitability":"avoid","quote_mode":"none","exit_policy":"flatten_immediately","confidence":0.2,"reasons":[],"metrics":{}},
                {"condition_id":"c1","suitability":"allow","quote_mode":"double","exit_policy":"exit_at_markup","confidence":0.9,"reasons":[],"metrics":{}}
            ]}"#,
            &["c1".to_string(), "c2".to_string()],
        )
        .expect("parse reordered batch");
        assert_eq!(items.len(), 2);
        let by_id: std::collections::HashMap<&str, &RewardAiAdvisoryDecision> = items
            .iter()
            .map(|item| (item.condition_id.as_str(), &item.decision))
            .collect();
        assert_eq!(by_id["c1"].suitability, RewardAiSuitability::Allow);
        assert_eq!(by_id["c2"].suitability, RewardAiSuitability::Avoid);
    }

    #[test]
    fn reward_ai_batch_parse_drops_unknown_and_duplicate_condition_ids() {
        let items = parse_reward_ai_batch_decision(
            r#"{"advisories":[
                {"condition_id":"c1","suitability":"allow","quote_mode":"double","exit_policy":"exit_at_markup","confidence":0.7,"reasons":[],"metrics":{}},
                {"condition_id":"typo","suitability":"allow","quote_mode":"double","exit_policy":"exit_at_markup","confidence":0.7,"reasons":[],"metrics":{}},
                {"condition_id":"c1","suitability":"watch","quote_mode":"none","exit_policy":"flatten_immediately","confidence":0.3,"reasons":[],"metrics":{}}
            ]}"#,
            &["c1".to_string()],
        )
        .expect("parse batch with unknown and duplicate condition ids");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].condition_id, "c1");
        assert_eq!(items[0].decision.suitability, RewardAiSuitability::Allow);
    }

    #[test]
    fn reward_ai_batch_parse_returns_partial_when_one_market_omitted() {
        // The omitted market is simply absent; the caller retries it via the single-request fallback.
        let result = parse_reward_ai_batch_decision(
            r#"{"advisories":[
                {"condition_id":"c1","suitability":"allow","quote_mode":"double","exit_policy":"exit_at_markup","confidence":0.7,"reasons":[],"metrics":{}}
            ]}"#,
            &["c1".to_string(), "c2".to_string()],
        )
        .expect("parse partial batch");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].condition_id, "c1");
    }

    #[test]
    fn reward_ai_batch_parse_single_object_fallback_for_one_market() {
        let items = parse_reward_ai_batch_decision(
            r#"{"suitability":"allow","quote_mode":"double","exit_policy":"exit_at_markup","confidence":0.8,"reasons":[],"metrics":{}}"#,
            &["only".to_string()],
        )
        .expect("parse single object fallback");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].condition_id, "only");
        assert_eq!(
            items[0].decision.confidence,
            Decimal::from_str("0.8").unwrap()
        );
    }

    #[test]
    fn reward_ai_batch_parse_rejects_single_object_when_multiple_markets_expected() {
        let result = parse_reward_ai_batch_decision(
            r#"{"suitability":"allow","quote_mode":"double","exit_policy":"exit_at_markup","confidence":0.8,"reasons":[],"metrics":{}}"#,
            &["c1".to_string(), "c2".to_string()],
        );
        assert!(result.is_err());
    }
}
