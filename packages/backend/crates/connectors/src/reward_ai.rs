use crate::openai_compat::{
    is_openai_compatible_chat_provider, openai_compatible_chat_response_format,
    openai_compatible_chat_token_limit_field, openai_compatible_endpoint, provider_json_candidates,
    provider_response_preview, with_openai_compatible_auth,
};
use polyedge_application::{
    RewardAiAdvisoryBatchItem, RewardAiAdvisoryDecision, RewardAiAdvisoryRequest, RewardAiProvider,
    RewardAiRequestFormat, RewardAiSuitability, RewardPlanQuoteMode,
    reward_ai_effective_request_format,
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
        let _provider_permit = crate::llm_provider::acquire_llm_provider_request_permit().await?;
        let request_format = reward_ai_effective_request_format(
            request.provider,
            request.request_format,
            &request.model,
        );
        let text = match request_format {
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
        let _provider_permit = crate::llm_provider::acquire_llm_provider_request_permit().await?;
        let request_format = reward_ai_effective_request_format(
            requests[0].provider,
            requests[0].request_format,
            &requests[0].model,
        );
        let text = match request_format {
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
        ensure_openai_compatible_chat_provider(request)?;
        let mut body = json!({
            "model": request.model,
            "messages": [
                {"role": "system", "content": reward_ai_system_prompt()},
                {"role": "user", "content": reward_ai_user_prompt(request)}
            ],
            "response_format": openai_compatible_chat_response_format(
                &request.model,
                "reward_market_advisory",
                reward_ai_json_schema(),
            ),
            "temperature": 0
        });
        body[openai_compatible_chat_token_limit_field(&request.model)] =
            json!(REWARD_AI_CHAT_COMPLETION_MAX_TOKENS);
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
        ensure_batch_openai_compatible_chat_provider(requests)?;
        let max_tokens = reward_ai_batch_max_tokens(requests.len());
        let mut body = json!({
            "model": requests[0].model,
            "messages": [
                {"role": "system", "content": reward_ai_batch_system_prompt()},
                {"role": "user", "content": reward_ai_batch_user_prompt(requests)}
            ],
            "response_format": openai_compatible_chat_response_format(
                &requests[0].model,
                "reward_market_advisories",
                reward_ai_batch_json_schema(),
            ),
            "temperature": 0
        });
        body[openai_compatible_chat_token_limit_field(&requests[0].model)] = json!(max_tokens);
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

fn ensure_openai_compatible_chat_provider(request: &RewardAiAdvisoryRequest) -> Result<()> {
    if is_openai_compatible_chat_provider(request.provider) {
        return Ok(());
    }
    Err(AppError::invalid_input(
        "REWARD_AI_PROVIDER_FORMAT_MISMATCH",
        "reward AI chat completion request format requires an OpenAI-compatible provider",
    ))
}

fn reward_ai_system_prompt() -> &'static str {
    "You are a risk reviewer for Polymarket rewards maker orders. Return exactly one JSON object and nothing else. Do not use markdown, comments, prose, or unquoted keys. Your decision field is only allow_quote: true means maker quoting is allowed, false means maker quoting is not allowed. Also return a conservative strategy_hint for live execution. Do not return watch, avoid, risk levels, or other status categories. Do not suggest bypassing deterministic risk checks."
}

fn reward_ai_user_prompt(request: &RewardAiAdvisoryRequest) -> String {
    format!(
        "Assess whether this rewards market is suitable for maker quoting over the full provider_cache_policy TTL horizon. Use current pricing_context to judge whether the live orderbook prices, spreads, binary midpoint sum, quote edge, and stale-book age make deterministic quote prices reasonable. Return one valid JSON object with double-quoted keys and these decision fields: allow_quote boolean, confidence 0..1, strategy_hint object, reasons string array, metrics object. strategy_hint.quote_mode must be one of double, single_yes, single_no, none; choose none when allow_quote=false or the market should be skipped. strategy_hint.bid_rank must be an integer 1..3 where larger means more conservative. strategy_hint.max_condition_notional_usd must be a non-negative number for the whole condition; use 0 when allow_quote=false. Prefer conservative smaller notional caps when uncertainty, exit depth, stale pricing, or reversal risk is material. Use {{}} for metrics when unsure.\nInput:\n{}",
        request.payload
    )
}

fn reward_ai_json_schema() -> Value {
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

fn reward_ai_strategy_hint_json_schema() -> Value {
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
    ensure_provider(&requests[0], expected)?;
    let first = &requests[0];
    for request in &requests[1..] {
        ensure_provider(request, expected)?;
        if request.request_format != first.request_format || request.model != first.model {
            return Err(AppError::invalid_input(
                "REWARD_AI_BATCH_MISMATCH",
                "reward AI advisory batch requests must share request format and model",
            ));
        }
    }
    Ok(())
}

fn ensure_batch_openai_compatible_chat_provider(
    requests: &[RewardAiAdvisoryRequest],
) -> Result<()> {
    if requests.is_empty() {
        return Err(AppError::invalid_input(
            "REWARD_AI_BATCH_EMPTY",
            "reward AI advisory batch must not be empty",
        ));
    }
    ensure_openai_compatible_chat_provider(&requests[0])?;
    let first = &requests[0];
    for request in &requests[1..] {
        ensure_openai_compatible_chat_provider(request)?;
        if request.provider != first.provider
            || request.request_format != first.request_format
            || request.model != first.model
        {
            return Err(AppError::invalid_input(
                "REWARD_AI_BATCH_MISMATCH",
                "reward AI advisory batch requests must share provider, request format and model",
            ));
        }
    }
    Ok(())
}

fn reward_ai_batch_max_tokens(batch_size: usize) -> u32 {
    REWARD_AI_CHAT_COMPLETION_MAX_TOKENS
        .saturating_mul(batch_size.max(1) as u32)
        .min(REWARD_AI_BATCH_MAX_TOKENS_CAP)
}

fn reward_ai_batch_system_prompt() -> &'static str {
    "You are a risk reviewer for Polymarket rewards maker orders. You will receive a JSON object containing a \"markets\" array; assess EACH market independently of the others. Return exactly one JSON object of shape {\"advisories\":[...]} and nothing else. Do not use markdown, comments, prose, or unquoted keys. Each advisory object must include the market's condition_id copied verbatim from the input, allow_quote, and a conservative strategy_hint. Do not return watch, avoid, risk levels, or other status categories."
}

fn reward_ai_batch_user_prompt(requests: &[RewardAiAdvisoryRequest]) -> String {
    let markets: Vec<Value> = requests
        .iter()
        .map(|request| json!({"condition_id": request.condition_id, "market": request.payload}))
        .collect();
    format!(
        "Assess whether each rewards market is suitable for maker quoting over its full provider_cache_policy TTL horizon. Use each market's current pricing_context to judge whether live orderbook prices, spreads, binary midpoint sum, quote edge, and stale-book age make deterministic quote prices reasonable. Return one valid JSON object with double-quoted keys and a field \"advisories\": an array with exactly one object per input market, each containing condition_id (must match one input market verbatim), allow_quote boolean, confidence 0..1, strategy_hint object, reasons string array, metrics object. strategy_hint.quote_mode must be one of double, single_yes, single_no, none; choose none when allow_quote=false. strategy_hint.bid_rank must be 1..3 where larger is more conservative. strategy_hint.max_condition_notional_usd must be a non-negative number for the whole condition; use 0 when allow_quote=false. Use {{}} for metrics when unsure.\nInput:\n{{\"markets\":{}}}",
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
                    "required": ["condition_id", "allow_quote", "confidence", "strategy_hint", "reasons", "metrics"],
                    "properties": {
                        "condition_id": {"type": "string"},
                        "allow_quote": {"type": "boolean"},
                        "confidence": {"type": "number", "minimum": 0, "maximum": 1},
                        "strategy_hint": reward_ai_strategy_hint_json_schema(),
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

fn parse_reward_ai_binary_decision_value(value: &Value) -> Result<RewardAiAdvisoryDecision> {
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

fn parse_reward_ai_strategy_hint_value(value: Option<&Value>) -> Result<Option<Value>> {
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

fn reward_ai_metrics_with_strategy_hint(metrics: Value, strategy_hint: Option<Value>) -> Value {
    let Some(strategy_hint) = strategy_hint else {
        return metrics;
    };
    let mut object = metrics.as_object().cloned().unwrap_or_default();
    object.insert("strategy_hint".to_string(), strategy_hint);
    Value::Object(object)
}

fn reward_ai_candidate_has_known_field(value: &Value) -> bool {
    value.get("allow_quote").is_some()
        || value.get("suitability").is_some()
        || value.get("quote_mode").is_some()
        || value.get("exit_policy").is_some()
        || value.get("strategy_hint").is_some()
        || value.get("confidence").is_some()
}

fn parse_confidence(value: Option<&Value>) -> Option<Decimal> {
    parse_decimal_value(value?).map(|parsed| parsed.max(Decimal::ZERO).min(Decimal::ONE))
}

fn parse_decimal_value(raw: &Value) -> Option<Decimal> {
    if let Some(number) = raw.as_f64() {
        Decimal::from_str(&number.to_string()).ok()
    } else {
        raw.as_str().and_then(|value| Decimal::from_str(value).ok())
    }
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

    include!("reward_ai_tests.rs");
}
