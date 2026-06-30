use polyedge_application::{
    RewardAiProvider, reward_ai_model_is_glm_reasoning,
    reward_ai_model_requires_openai_chat_completions,
};
use reqwest::RequestBuilder;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;

const PROVIDER_RESPONSE_PREVIEW_CHARS: usize = 240;

pub(crate) fn normalize_openai_base_url(base_url: impl Into<String>) -> String {
    let trimmed = base_url.into().trim_end_matches('/').to_string();
    if has_version_suffix(&trimmed) {
        trimmed
    } else {
        format!("{trimmed}/v1")
    }
}

pub(crate) fn openai_compatible_endpoint(base_url: &str, path: &str) -> String {
    format!(
        "{}/{}",
        normalize_openai_base_url(base_url),
        path.trim_start_matches('/')
    )
}

pub(crate) fn with_openai_compatible_auth(
    builder: RequestBuilder,
    api_key: &str,
) -> RequestBuilder {
    builder.bearer_auth(api_key).header("api-key", api_key)
}

pub(crate) fn is_openai_compatible_chat_provider(provider: RewardAiProvider) -> bool {
    matches!(provider, RewardAiProvider::OpenAi)
}

pub(crate) fn openai_compatible_chat_response_format(
    model: &str,
    name: &str,
    schema: Value,
) -> Value {
    if reward_ai_model_requires_openai_chat_completions(model) {
        json!({
            "type": "json_object"
        })
    } else {
        json!({
            "type": "json_schema",
            "json_schema": {
                "name": name,
                "schema": schema,
                "strict": true
            }
        })
    }
}

pub(crate) fn openai_compatible_chat_token_limit_field(model: &str) -> &'static str {
    if reward_ai_model_requires_openai_chat_completions(model) {
        "max_tokens"
    } else {
        "max_completion_tokens"
    }
}

/// For GLM reasoning models that emit chain-of-thought by default (the glm-4.7
/// family), returns a `{"type": "disabled"}` value to assign to the
/// `thinking` field of a chat-completions body. Disabling thinking keeps the
/// output budget in `content` instead of burning it on `reasoning_content`
/// (which truncates `content` to empty under a tight `max_tokens`). Returns
/// `None` for non-reasoning models so the field is never sent to providers that
/// would reject it.
pub(crate) fn openai_compatible_chat_thinking_disabled(model: &str) -> Option<Value> {
    reward_ai_model_is_glm_reasoning(model).then(|| json!({"type": "disabled"}))
}

pub(crate) fn provider_json_candidates(text: &str) -> Vec<Value> {
    let mut candidates = Vec::new();
    push_json_candidate(text.trim(), &mut candidates);
    if let Some(fenced) = markdown_fence_body(text.trim()) {
        push_json_candidate(fenced.trim(), &mut candidates);
    }
    for (index, ch) in text.char_indices() {
        if ch == '{' || ch == '[' {
            push_json_candidate(&text[index..], &mut candidates);
        }
    }
    candidates
}

pub(crate) fn provider_response_preview(text: &str) -> String {
    let mut preview = text
        .chars()
        .take(PROVIDER_RESPONSE_PREVIEW_CHARS)
        .collect::<String>()
        .replace(['\r', '\n', '\t'], " ");
    if text.chars().count() > PROVIDER_RESPONSE_PREVIEW_CHARS {
        preview.push_str("...");
    }
    preview
}

fn has_version_suffix(value: &str) -> bool {
    value
        .rsplit('/')
        .find(|segment| !segment.is_empty())
        .is_some_and(|segment| {
            let Some(rest) = segment.strip_prefix('v') else {
                return false;
            };
            !rest.is_empty() && rest.chars().all(|ch| ch.is_ascii_digit())
        })
}

fn push_json_candidate(text: &str, candidates: &mut Vec<Value>) {
    if text.is_empty() {
        return;
    }
    if let Ok(value) = serde_json::from_str::<Value>(text) {
        push_unwrapped_json_candidate(value, candidates);
        return;
    }
    let mut deserializer = serde_json::Deserializer::from_str(text);
    if let Ok(value) = Value::deserialize(&mut deserializer) {
        push_unwrapped_json_candidate(value, candidates);
    }
}

fn push_unwrapped_json_candidate(value: Value, candidates: &mut Vec<Value>) {
    match value {
        Value::String(text) => {
            if let Ok(inner) = serde_json::from_str::<Value>(&text) {
                push_unwrapped_json_candidate(inner, candidates);
            }
        }
        Value::Array(items) => {
            if let Some(item) = items.iter().find(|item| item.is_object()).cloned() {
                candidates.push(item);
            }
            candidates.push(Value::Array(items));
        }
        other => candidates.push(other),
    }
}

fn markdown_fence_body(text: &str) -> Option<&str> {
    let rest = text.strip_prefix("```")?;
    let start = rest.find('\n').map_or(0, |index| index + 1);
    let body = &rest[start..];
    let end = body.rfind("```").unwrap_or(body.len());
    Some(&body[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_openai_base_url_preserves_existing_v1() {
        assert_eq!(
            normalize_openai_base_url("https://api.openai.com/v1"),
            "https://api.openai.com/v1"
        );
        assert_eq!(
            normalize_openai_base_url("https://proxy.example.com/custom/v1/"),
            "https://proxy.example.com/custom/v1"
        );
    }

    #[test]
    fn normalize_openai_base_url_preserves_versioned_provider_base() {
        assert_eq!(
            normalize_openai_base_url("https://open.bigmodel.cn/api/coding/paas/v4"),
            "https://open.bigmodel.cn/api/coding/paas/v4"
        );
    }

    #[test]
    fn normalize_openai_base_url_adds_v1_for_root_gateway() {
        assert_eq!(
            normalize_openai_base_url("http://100.87.45.72:33001"),
            "http://100.87.45.72:33001/v1"
        );
    }

    #[test]
    fn openai_compatible_endpoint_joins_path() {
        assert_eq!(
            openai_compatible_endpoint("http://100.87.45.72:33001", "chat/completions"),
            "http://100.87.45.72:33001/v1/chat/completions"
        );
    }

    #[test]
    fn chat_response_format_uses_model_specific_json_mode() {
        assert_eq!(
            openai_compatible_chat_response_format(
                "gpt-4.1-mini",
                "shape",
                json!({"type": "object"})
            )
            .pointer("/type"),
            Some(&json!("json_schema"))
        );
        assert_eq!(
            openai_compatible_chat_response_format(
                "deepseek-v4-flash",
                "shape",
                json!({"type": "object"})
            ),
            json!({"type": "json_object"})
        );
        assert_eq!(
            openai_compatible_chat_response_format("glm-4.7", "shape", json!({"type": "object"})),
            json!({"type": "json_object"})
        );
        assert_eq!(
            openai_compatible_chat_response_format(
                "agnes-2.0-flash",
                "shape",
                json!({"type": "object"})
            )
            .pointer("/type"),
            Some(&json!("json_schema"))
        );
    }

    #[test]
    fn chat_token_limit_field_is_model_specific() {
        assert_eq!(
            openai_compatible_chat_token_limit_field("gpt-4.1-mini"),
            "max_completion_tokens"
        );
        assert_eq!(
            openai_compatible_chat_token_limit_field("glm-4.7"),
            "max_tokens"
        );
        assert_eq!(
            openai_compatible_chat_token_limit_field("deepseek-v4-flash"),
            "max_tokens"
        );
        assert_eq!(
            openai_compatible_chat_token_limit_field("agnes-2.0-flash"),
            "max_completion_tokens"
        );
    }

    #[test]
    fn chat_thinking_disabled_only_for_glm_reasoning_models() {
        assert_eq!(
            openai_compatible_chat_thinking_disabled("glm-4.7-flashx"),
            Some(json!({"type": "disabled"}))
        );
        assert_eq!(
            openai_compatible_chat_thinking_disabled("GLM-4.7"),
            Some(json!({"type": "disabled"}))
        );
        assert_eq!(
            openai_compatible_chat_thinking_disabled("glm-4.5-flash"),
            None
        );
        assert_eq!(
            openai_compatible_chat_thinking_disabled("deepseek-v4-flash"),
            None
        );
        assert_eq!(
            openai_compatible_chat_thinking_disabled("gpt-4.1-mini"),
            None
        );
    }

    #[test]
    fn provider_json_candidates_scan_embedded_objects() {
        let values = provider_json_candidates(
            r#"shape: {"example": true}
final: {"suitability":"allow","quote_mode":"double"} trailing"#,
        );

        assert!(
            values
                .iter()
                .any(|value| value.get("suitability").is_some())
        );
    }

    #[test]
    fn provider_json_candidates_unwrap_markdown_fence() {
        let values = provider_json_candidates(
            r#"```json
{"risk_level":"low","sources":[]}
```"#,
        );

        assert!(values.iter().any(|value| value.get("risk_level").is_some()));
    }
}
