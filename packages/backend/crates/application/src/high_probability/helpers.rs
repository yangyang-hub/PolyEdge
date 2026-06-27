pub fn validate_high_probability_list_limit(limit: Option<u16>) -> u16 {
    limit
        .unwrap_or(DEFAULT_HIGH_PROBABILITY_LIST_LIMIT)
        .clamp(1, MAX_HIGH_PROBABILITY_LIST_LIMIT)
}

pub fn validate_high_probability_sample_input_limit(limit: Option<u32>) -> u32 {
    limit
        .unwrap_or(DEFAULT_HIGH_PROBABILITY_SAMPLE_INPUT_LIMIT)
        .clamp(1, MAX_HIGH_PROBABILITY_SAMPLE_INPUT_LIMIT)
}

fn clamp_decimal(value: Decimal, min: Decimal, max: Decimal) -> Decimal {
    value.max(min).min(max)
}

fn non_empty_or(value: String, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        fallback.to_string()
    } else {
        value.to_ascii_lowercase()
    }
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_ascii_lowercase())
    })
}

fn normalize_string_list(values: Vec<String>) -> Vec<String> {
    let mut output = values
        .into_iter()
        .filter_map(|value| {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_ascii_lowercase())
        })
        .collect::<Vec<_>>();
    output.sort();
    output.dedup();
    output
}
