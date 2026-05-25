use polyedge_domain::{AppError, Result};

pub(crate) fn validate_list_limit(
    limit: Option<u16>,
    default: u16,
    max: u16,
    zero_error_code: &'static str,
    zero_error_message: impl Into<String>,
    too_large_error_code: &'static str,
    too_large_error_message: impl Into<String>,
) -> Result<u16> {
    let limit = limit.unwrap_or(default);
    if limit == 0 {
        return Err(AppError::invalid_input(
            zero_error_code,
            zero_error_message.into(),
        ));
    }

    if limit > max {
        return Err(AppError::invalid_input(
            too_large_error_code,
            too_large_error_message.into(),
        ));
    }

    Ok(limit)
}

pub(crate) fn normalize_optional_filter_id(
    field_name: &str,
    value: Option<String>,
    error_code: &'static str,
    empty_message: impl FnOnce(&str) -> String,
) -> Result<Option<String>> {
    value
        .map(|raw| {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                Err(AppError::invalid_input(
                    error_code,
                    empty_message(field_name),
                ))
            } else {
                Ok(trimmed.to_string())
            }
        })
        .transpose()
}
