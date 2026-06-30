//! Combined rewards provider connector: a single LLM HTTP call that can carry
//! both the AI advisory context and the info-risk context for one market. The
//! response is one JSON object with optional `advisory` and `info_risk` sections,
//! each parsed back into the existing decision types so the two cache tables stay
//! independently keyed/TTL'd. This replaces the previous two separate per-market
//! calls (`RewardAiAdvisoryConnector::advise` + `RewardInfoRiskConnector::assess`)
//! and removes multi-market batching.

use crate::openai_compat::{
    is_openai_compatible_chat_provider, openai_compatible_chat_response_format,
    openai_compatible_chat_thinking_disabled, openai_compatible_chat_token_limit_field,
    openai_compatible_endpoint, provider_json_candidates, provider_response_preview,
    with_openai_compatible_auth,
};
use crate::reward_ai::{
    extract_openai_responses_text, parse_reward_ai_decision_value,
    reward_ai_candidate_has_known_field, reward_ai_json_schema,
};
use crate::reward_info_risk::{
    parse_reward_info_risk_decision_value, reward_info_risk_candidate_has_known_field,
    reward_info_risk_json_schema,
};
use polyedge_application::{
    RewardAiProvider, RewardAiRequestFormat, RewardProviderDecision, RewardProviderRequest,
    reward_ai_effective_request_format,
};
use polyedge_domain::{AppError, Result};
use reqwest::Client;
use serde_json::{Map, Value, json};
use std::time::Duration;

// Combined output is the union of the advisory object (~allow_quote, confidence,
// conservative strategy_hint, reasons, metrics) and the info-risk object
// (~allow_quote, confidence, summary, sources, metrics). GLM reasoning models
// (glm-4.7 family) have thinking disabled in `call_openai_chat_completions`, so
// this budget lands in `content` instead of being consumed by
// `reasoning_content`; keep headroom for the rare case thinking cannot be turned
// off so the message is not truncated to empty (`finish_reason: length`).
const REWARD_PROVIDER_CHAT_COMPLETION_MAX_TOKENS: u32 = 8192;
const REWARD_PROVIDER_ANTHROPIC_MAX_TOKENS: u32 = 2048;

#[derive(Debug, Clone)]
pub struct RewardProviderConnector {
    client: Client,
    base_url: String,
    api_key: String,
    web_search_enabled: bool,
}

impl RewardProviderConnector {
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
                    "REWARD_PROVIDER_CLIENT_BUILD_FAILED",
                    format!("failed to build reward provider HTTP client: {error}"),
                )
            })?;
        Ok(Self {
            client,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            web_search_enabled,
        })
    }

    /// Evaluate one market in a single provider call. The request may carry the
    /// advisory context, the info-risk context, or both; the returned decision has
    /// `Some` only for the sections that were requested (and parseable).
    pub async fn evaluate(
        &self,
        request: &RewardProviderRequest,
    ) -> Result<RewardProviderDecision> {
        if !request.wants_advisory() && !request.wants_info_risk() {
            return Err(AppError::invalid_input(
                "REWARD_PROVIDER_EMPTY_REQUEST",
                "reward provider request must include advisory or info_risk",
            ));
        }
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
        parse_reward_provider_decision(&text, request)
    }

    async fn call_openai_responses(&self, request: &RewardProviderRequest) -> Result<String> {
        ensure_openai_provider(request)?;
        let wants_advisory = request.wants_advisory();
        let wants_info_risk = request.wants_info_risk();
        let mut body = json!({
            "model": request.model,
            "input": [
                {"role": "system", "content": [{"type": "input_text", "text": reward_provider_system_prompt(wants_advisory, wants_info_risk)}]},
                {"role": "user", "content": [{"type": "input_text", "text": reward_provider_user_prompt(request)}]}
            ],
            "text": {
                "format": {
                    "type": "json_schema",
                    "name": "reward_provider_decision",
                    "schema": reward_provider_combined_json_schema(wants_advisory, wants_info_risk),
                    "strict": true
                }
            },
            "temperature": 0
        });
        // Web search only aids the info-risk section and only OpenAI Responses
        // exposes the tool; advisory-only or chat/anthropic paths never send it.
        if wants_info_risk && self.web_search_enabled {
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
        .map_err(reward_provider_http_error)?;
        let status = response.status();
        let body: Value = response
            .json()
            .await
            .map_err(reward_provider_decode_error)?;
        if !status.is_success() {
            return Err(reward_provider_status_error(status.as_u16(), body));
        }
        extract_openai_responses_text(&body)
    }

    async fn call_openai_chat_completions(
        &self,
        request: &RewardProviderRequest,
    ) -> Result<String> {
        ensure_openai_compatible_chat_provider(request)?;
        let wants_advisory = request.wants_advisory();
        let wants_info_risk = request.wants_info_risk();
        let mut body = json!({
            "model": request.model,
            "messages": [
                {"role": "system", "content": reward_provider_system_prompt(wants_advisory, wants_info_risk)},
                {"role": "user", "content": reward_provider_user_prompt(request)}
            ],
            "response_format": openai_compatible_chat_response_format(
                &request.model,
                "reward_provider_decision",
                reward_provider_combined_json_schema(wants_advisory, wants_info_risk),
            ),
            "temperature": 0
        });
        body[openai_compatible_chat_token_limit_field(&request.model)] =
            json!(REWARD_PROVIDER_CHAT_COMPLETION_MAX_TOKENS);
        if let Some(thinking) = openai_compatible_chat_thinking_disabled(&request.model) {
            body["thinking"] = thinking;
        }
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
        .map_err(reward_provider_http_error)?;
        let status = response.status();
        let body: Value = response
            .json()
            .await
            .map_err(reward_provider_decode_error)?;
        if !status.is_success() {
            return Err(reward_provider_status_error(status.as_u16(), body));
        }
        body.pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .ok_or_else(|| {
                AppError::dependency_unavailable(
                    "REWARD_PROVIDER_RESPONSE_INVALID",
                    "OpenAI chat completion response did not include message content",
                )
            })
    }

    async fn call_anthropic_messages(&self, request: &RewardProviderRequest) -> Result<String> {
        ensure_anthropic_provider(request)?;
        let wants_advisory = request.wants_advisory();
        let wants_info_risk = request.wants_info_risk();
        let response = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&json!({
                "model": request.model,
                "max_tokens": REWARD_PROVIDER_ANTHROPIC_MAX_TOKENS,
                "temperature": 0,
                "system": reward_provider_system_prompt(wants_advisory, wants_info_risk),
                "messages": [
                    {"role": "user", "content": reward_provider_user_prompt(request)}
                ]
            }))
            .send()
            .await
            .map_err(reward_provider_http_error)?;
        let status = response.status();
        let body: Value = response
            .json()
            .await
            .map_err(reward_provider_decode_error)?;
        if !status.is_success() {
            return Err(reward_provider_status_error(status.as_u16(), body));
        }
        body.pointer("/content/0/text")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .ok_or_else(|| {
                AppError::dependency_unavailable(
                    "REWARD_PROVIDER_RESPONSE_INVALID",
                    "Anthropic messages response did not include text content",
                )
            })
    }
}

fn ensure_openai_provider(request: &RewardProviderRequest) -> Result<()> {
    if request.provider == RewardAiProvider::OpenAi {
        return Ok(());
    }
    Err(AppError::invalid_input(
        "REWARD_PROVIDER_FORMAT_MISMATCH",
        "reward provider OpenAI Responses request requires the openai provider",
    ))
}

fn ensure_openai_compatible_chat_provider(request: &RewardProviderRequest) -> Result<()> {
    if is_openai_compatible_chat_provider(request.provider) {
        return Ok(());
    }
    Err(AppError::invalid_input(
        "REWARD_PROVIDER_FORMAT_MISMATCH",
        "reward provider chat completion request requires an OpenAI-compatible provider",
    ))
}

fn ensure_anthropic_provider(request: &RewardProviderRequest) -> Result<()> {
    if request.provider == RewardAiProvider::Anthropic {
        return Ok(());
    }
    Err(AppError::invalid_input(
        "REWARD_PROVIDER_FORMAT_MISMATCH",
        "reward provider Anthropic messages request requires the anthropic provider",
    ))
}

fn reward_provider_system_prompt(wants_advisory: bool, wants_info_risk: bool) -> String {
    let mut sections = Vec::new();
    if wants_advisory {
        sections.push(
            "\"advisory\": maker-quoting suitability plus a conservative strategy_hint (quote_mode in double|single_yes|single_no|none, bid_rank 1..3 where larger is more conservative, max_condition_notional_usd non-negative number)",
        );
    }
    if wants_info_risk {
        sections.push(
            "\"info_risk\": event/news risk over the cache TTL (summary, sources[] of {url,title,published_at,snippet})",
        );
    }
    format!(
        "You are a risk reviewer for Polymarket rewards maker orders. Return exactly one JSON object and nothing else. Do not use markdown, comments, prose, or unquoted keys. Assess the market and return each requested section: {}. Each section's only decision field is allow_quote boolean (true means maker quoting is allowed, false means not allowed). Do not return watch, avoid, risk levels, or other status categories. Use each section's evaluation_time_utc as the current UTC time; do not infer today's date from model training or stale context. Use web search when a search tool is available; otherwise use the supplied context and mark uncertainty in summary/metrics.",
        sections.join("; ")
    )
}

fn reward_provider_user_prompt(request: &RewardProviderRequest) -> String {
    let wants_advisory = request.wants_advisory();
    let wants_info_risk = request.wants_info_risk();
    let mut instruction = String::from(
        "Assess this rewards market and return one valid JSON object with double-quoted keys. ",
    );
    if wants_advisory {
        instruction.push_str("Include an \"advisory\" object: allow_quote boolean, confidence 0..1, strategy_hint {quote_mode, bid_rank, max_condition_notional_usd}, reasons string array, metrics object. Set allow_quote=false when live orderbook pricing/spread/midpoint/quote edge/stale-book age make deterministic quotes unreasonable; choose strategy_hint.quote_mode=none when not allowed. ");
    }
    if wants_info_risk {
        instruction.push_str("Include an \"info_risk\" object: allow_quote boolean, confidence 0..1, summary string, sources array of {url,title,published_at,snippet}, metrics object. Set allow_quote=false if recent/imminent information (official result/resolution, a confirmed near-term resolution-driving event, breaking news, stale facts, unresolved uncertainty) could make passive maker quoting unsafe before cache expiry. ");
    }
    instruction.push_str("Use [] for sources and {} for metrics when unsure.");
    let mut sections = Vec::new();
    if let Some(advisory) = &request.advisory {
        sections.push(format!("advisory_context:\n{}", advisory.payload));
    }
    if let Some(info_risk) = &request.info_risk {
        sections.push(format!("info_risk_context:\n{}", info_risk.payload));
    }
    format!("{}\n{}", instruction, sections.join("\n"))
}

fn reward_provider_combined_json_schema(wants_advisory: bool, wants_info_risk: bool) -> Value {
    let mut properties = Map::new();
    let mut required: Vec<Value> = Vec::new();
    if wants_advisory {
        properties.insert("advisory".to_string(), reward_ai_json_schema());
        required.push(json!("advisory"));
    }
    if wants_info_risk {
        properties.insert("info_risk".to_string(), reward_info_risk_json_schema());
        required.push(json!("info_risk"));
    }
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": required,
        "properties": properties
    })
}

fn parse_reward_provider_decision(
    text: &str,
    request: &RewardProviderRequest,
) -> Result<RewardProviderDecision> {
    let wants_advisory = request.wants_advisory();
    let wants_info_risk = request.wants_info_risk();
    let mut last_error: Option<AppError> = None;
    for value in provider_json_candidates(text) {
        match parse_reward_provider_value(&value, wants_advisory, wants_info_risk) {
            Ok(decision) => return Ok(decision),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| {
        AppError::dependency_unavailable(
            "REWARD_PROVIDER_RESPONSE_INVALID_JSON",
            format!(
                "reward provider response was not usable; preview={}",
                provider_response_preview(text)
            ),
        )
    }))
}

fn parse_reward_provider_value(
    value: &Value,
    wants_advisory: bool,
    wants_info_risk: bool,
) -> Result<RewardProviderDecision> {
    let advisory = if wants_advisory {
        if let Some(advisory) = value.get("advisory") {
            Some(parse_reward_ai_decision_value(advisory)?)
        } else if !wants_info_risk && reward_ai_candidate_has_known_field(value) {
            // Single-section fallback: some models omit the wrapper and return the
            // advisory fields at the top level when only advisory was requested.
            Some(parse_reward_ai_decision_value(value)?)
        } else {
            return Err(reward_provider_missing_section("advisory"));
        }
    } else {
        None
    };
    let info_risk = if wants_info_risk {
        if let Some(info_risk) = value.get("info_risk") {
            Some(parse_reward_info_risk_decision_value(info_risk)?)
        } else if !wants_advisory && reward_info_risk_candidate_has_known_field(value) {
            Some(parse_reward_info_risk_decision_value(value)?)
        } else {
            return Err(reward_provider_missing_section("info_risk"));
        }
    } else {
        None
    };
    Ok(RewardProviderDecision {
        advisory,
        info_risk,
    })
}

fn reward_provider_missing_section(section: &'static str) -> AppError {
    AppError::dependency_unavailable(
        "REWARD_PROVIDER_RESPONSE_MISSING_SECTION",
        format!("reward provider response missing {section} section"),
    )
}

fn reward_provider_http_error(error: reqwest::Error) -> AppError {
    AppError::dependency_unavailable(
        "REWARD_PROVIDER_HTTP_FAILED",
        format!("reward provider HTTP request failed: {error}"),
    )
}

fn reward_provider_decode_error(error: reqwest::Error) -> AppError {
    AppError::dependency_unavailable(
        "REWARD_PROVIDER_RESPONSE_DECODE_FAILED",
        format!("failed to decode reward provider response: {error}"),
    )
}

fn reward_provider_status_error(status: u16, body: Value) -> AppError {
    AppError::dependency_unavailable(
        "REWARD_PROVIDER_STATUS_FAILED",
        format!("reward provider returned HTTP {status}: {body}"),
    )
}

#[cfg(test)]
mod reward_provider_tests {
    use super::*;
    use polyedge_application::{
        RewardAiAdvisoryRequest, RewardInfoRiskAssessmentRequest, RewardInfoRiskLevel,
        RewardPlanQuoteMode,
    };
    use rust_decimal::Decimal;
    use std::str::FromStr;

    fn provider_request(advisory: bool, info_risk: bool) -> RewardProviderRequest {
        RewardProviderRequest {
            condition_id: "0xcond".to_string(),
            provider: RewardAiProvider::OpenAi,
            request_format: RewardAiRequestFormat::OpenAiChatCompletions,
            model: "gpt-4.1-mini".to_string(),
            advisory: advisory.then(|| RewardAiAdvisoryRequest {
                condition_id: "0xcond".to_string(),
                provider: RewardAiProvider::OpenAi,
                request_format: RewardAiRequestFormat::OpenAiChatCompletions,
                model: "gpt-4.1-mini".to_string(),
                input_hash: "ai_hash".to_string(),
                payload: json!({"market": {"question": "Will X happen?"}}),
            }),
            info_risk: info_risk.then(|| RewardInfoRiskAssessmentRequest {
                condition_id: "0xcond".to_string(),
                provider: RewardAiProvider::OpenAi,
                request_format: RewardAiRequestFormat::OpenAiChatCompletions,
                model: "gpt-4.1-mini".to_string(),
                query: "Will X happen? latest official result".to_string(),
                query_hash: "query_hash".to_string(),
                input_hash: "risk_hash".to_string(),
                payload: json!({"search_query": "Will X happen? latest official result"}),
            }),
        }
    }

    #[tokio::test]
    async fn reward_provider_uses_chat_completions_with_strict_schema_for_agnes() {
        let (base_url, captured) = crate::test_http::spawn_json_response_server(
            r#"{"choices":[{"message":{"content":"{\"advisory\":{\"allow_quote\":true,\"confidence\":0.82,\"strategy_hint\":{\"quote_mode\":\"double\",\"bid_rank\":2,\"max_condition_notional_usd\":15},\"reasons\":[\"pricing ok\"],\"metrics\":{}},\"info_risk\":{\"allow_quote\":true,\"confidence\":0.76,\"summary\":\"no imminent result found\",\"sources\":[],\"metrics\":{}}}"}}]}"#,
        )
        .await;
        let connector =
            RewardProviderConnector::new(base_url, "test-key", 5, false).expect("build connector");
        let mut request = provider_request(true, true);
        request.request_format = RewardAiRequestFormat::OpenAiResponses;
        request.model = "agnes-2.0-flash".to_string();

        let decision = connector
            .evaluate(&request)
            .await
            .expect("mock agnes reward provider");
        let captured = captured.await.expect("captured request");

        assert_eq!(captured.request_line, "POST /v1/chat/completions HTTP/1.1");
        assert_eq!(captured.body["model"], json!("agnes-2.0-flash"));
        assert_eq!(
            captured.body.pointer("/response_format/type"),
            Some(&json!("json_schema"))
        );
        assert_eq!(
            captured.body["max_completion_tokens"],
            json!(REWARD_PROVIDER_CHAT_COMPLETION_MAX_TOKENS)
        );
        assert!(captured.body.get("max_tokens").is_none());
        assert!(decision.advisory.is_some());
        assert!(decision.info_risk.is_some());
    }

    #[test]
    fn parses_combined_advisory_and_info_risk_response() {
        let decision = parse_reward_provider_decision(
            r#"{
                "advisory": {
                    "allow_quote": true,
                    "confidence": 0.82,
                    "strategy_hint": {
                        "quote_mode": "single_yes",
                        "bid_rank": 2,
                        "max_condition_notional_usd": 15
                    },
                    "reasons": ["pricing ok"],
                    "metrics": {"edge": "ok"}
                },
                "info_risk": {
                    "allow_quote": false,
                    "confidence": 0.91,
                    "summary": "official result may be imminent",
                    "sources": [],
                    "metrics": {"risk": "event"}
                }
            }"#,
            &provider_request(true, true),
        )
        .expect("parse combined response");

        let advisory = decision.advisory.expect("advisory section");
        assert_eq!(advisory.quote_mode, RewardPlanQuoteMode::Double);
        assert_eq!(
            advisory.metrics.pointer("/strategy_hint/quote_mode"),
            Some(&json!("single_yes"))
        );
        assert_eq!(advisory.confidence, Decimal::from_str("0.82").unwrap());

        let info_risk = decision.info_risk.expect("info-risk section");
        assert_eq!(info_risk.risk_level, RewardInfoRiskLevel::Critical);
        assert_eq!(info_risk.confidence, Decimal::from_str("0.91").unwrap());
    }

    #[test]
    fn single_section_advisory_accepts_top_level_object() {
        let decision = parse_reward_provider_decision(
            r#"{"allow_quote":true,"confidence":0.7,"strategy_hint":{"quote_mode":"double","bid_rank":1,"max_condition_notional_usd":0},"reasons":[],"metrics":{}}"#,
            &provider_request(true, false),
        )
        .expect("parse top-level advisory");

        assert!(decision.advisory.is_some());
        assert!(decision.info_risk.is_none());
    }

    #[test]
    fn combined_response_requires_requested_sections() {
        let error = parse_reward_provider_decision(
            r#"{"advisory":{"allow_quote":true,"confidence":0.7,"strategy_hint":{"quote_mode":"double","bid_rank":1,"max_condition_notional_usd":0},"reasons":[],"metrics":{}}}"#,
            &provider_request(true, true),
        )
        .expect_err("missing info-risk section should fail");

        assert_eq!(error.code(), "REWARD_PROVIDER_RESPONSE_MISSING_SECTION");
    }

    #[test]
    fn combined_prompt_includes_only_requested_contexts() {
        let combined = provider_request(true, true);
        let prompt = reward_provider_user_prompt(&combined);
        assert!(prompt.contains("advisory_context:"));
        assert!(prompt.contains("info_risk_context:"));

        let advisory_only = provider_request(true, false);
        let prompt = reward_provider_user_prompt(&advisory_only);
        assert!(prompt.contains("advisory_context:"));
        assert!(!prompt.contains("info_risk_context:"));
    }

    #[test]
    fn combined_schema_requires_requested_sections_only() {
        let schema = reward_provider_combined_json_schema(true, true);
        assert_eq!(
            schema.pointer("/required").and_then(Value::as_array),
            Some(&vec![json!("advisory"), json!("info_risk")])
        );

        let schema = reward_provider_combined_json_schema(false, true);
        assert_eq!(
            schema.pointer("/required").and_then(Value::as_array),
            Some(&vec![json!("info_risk")])
        );
        assert!(schema.pointer("/properties/advisory").is_none());
    }
}
