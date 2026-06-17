use reqwest::RequestBuilder;

pub(crate) fn normalize_openai_base_url(base_url: impl Into<String>) -> String {
    let trimmed = base_url.into().trim_end_matches('/').to_string();
    if has_v1_suffix(&trimmed) {
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

fn has_v1_suffix(value: &str) -> bool {
    value
        .rsplit('/')
        .find(|segment| !segment.is_empty())
        .is_some_and(|segment| segment == "v1")
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
}
