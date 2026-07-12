fn validate_source(source: String) -> Result<String, ApiError> {
    let source = source.trim();
    if source.is_empty() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "orderbook source must not be empty",
        ));
    }
    if source.len() > MAX_SOURCE_LEN {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            format!("orderbook source must be at most {MAX_SOURCE_LEN} bytes"),
        ));
    }
    if !source
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b':'))
    {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "orderbook source contains unsupported characters",
        ));
    }
    Ok(source.to_string())
}

fn authorize_write(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let Some(expected) = state
        .settings
        .orderbook
        .write_token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
    else {
        return Err(error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "orderbook write endpoints are disabled until POLYEDGE_ORDERBOOK__WRITE_TOKEN is configured",
        ));
    };
    let actual = headers
        .get("x-polyedge-orderbook-token")
        .and_then(|value| value.to_str().ok());
    if actual != Some(expected) {
        return Err(error_response(
            StatusCode::UNAUTHORIZED,
            "invalid orderbook write token",
        ));
    }
    Ok(())
}

fn validate_token_ids(token_ids: Vec<String>, max_tokens: usize) -> Result<Vec<String>, ApiError> {
    if token_ids.len() > max_tokens {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            format!("orderbook request supports at most {max_tokens} token ids"),
        ));
    }

    let mut seen = HashSet::new();
    let mut normalized = Vec::with_capacity(token_ids.len());
    for token_id in token_ids {
        let token_id = validate_token_id(token_id)?;
        if seen.insert(token_id.clone()) {
            normalized.push(token_id);
        }
    }
    Ok(normalized)
}

fn validate_token_id(token_id: String) -> Result<String, ApiError> {
    let token_id = token_id.trim();
    if token_id.is_empty() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "orderbook token id must not be empty",
        ));
    }
    if U256::from_str(token_id).is_err() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            format!("invalid orderbook token id '{token_id}'"),
        ));
    }
    Ok(token_id.to_string())
}

fn parse_levels(
    levels: Vec<LevelResponse>,
    max_levels: usize,
    descending: bool,
) -> Result<Vec<CachedBookLevel>, ApiError> {
    let mut seen_prices = HashSet::new();
    let mut parsed = Vec::with_capacity(levels.len());
    for level in levels {
        let price = Decimal::from_str(&level.price).map_err(|error| {
            error_response(
                StatusCode::BAD_REQUEST,
                format!("invalid orderbook level price '{}': {error}", level.price),
            )
        })?;
        let size = Decimal::from_str(&level.size).map_err(|error| {
            error_response(
                StatusCode::BAD_REQUEST,
                format!("invalid orderbook level size '{}': {error}", level.size),
            )
        })?;
        if price <= Decimal::ZERO || price >= Decimal::ONE {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                format!("orderbook price must be strictly between 0 and 1, got {price}"),
            ));
        }
        if size <= Decimal::ZERO {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                format!("orderbook size must be positive, got {size}"),
            ));
        }
        if !seen_prices.insert(price) {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                format!("duplicate orderbook price level {price}"),
            ));
        }
        parsed.push(CachedBookLevel { price, size });
    }
    // Keep the BEST levels (bids descending, asks ascending) before trimming, so
    // an unsorted ingest payload never drops top-of-book.
    if descending {
        parsed.sort_by(|a, b| b.price.cmp(&a.price));
    } else {
        parsed.sort_by(|a, b| a.price.cmp(&b.price));
    }
    parsed.truncate(max_levels.max(1));
    Ok(parsed)
}

fn validate_ingest_observed_at(observed_at: i64, now: i64) -> Result<(), ApiError> {
    if observed_at <= 0
        || observed_at > now.saturating_add(MAX_INGEST_CLOCK_SKEW_MS)
        || observed_at < now.saturating_sub(MAX_INGEST_OBSERVED_AGE_MS)
    {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "orderbook observed_at is outside the accepted service time window",
        ));
    }
    Ok(())
}

fn message_response(message: impl Into<String>) -> Json<MessageResponse> {
    Json(MessageResponse {
        message: message.into(),
    })
}

fn error_response(status: StatusCode, message: impl Into<String>) -> ApiError {
    (status, message_response(message))
}

fn registry_error_response(error: polyedge_domain::AppError) -> ApiError {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("orderbook registry update failed: {error}"),
    )
}

fn cache_error_response(error: polyedge_domain::AppError) -> ApiError {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("orderbook cache update failed: {error}"),
    )
}

fn to_response(book: polyedge_application::CachedOrderBook) -> OrderbookResponse {
    let confirmed_at = book.confirmation_time_ms();
    OrderbookResponse {
        token_id: book.token_id,
        bids: book
            .bids
            .into_iter()
            .map(|l| LevelResponse {
                price: l.price.to_string(),
                size: l.size.to_string(),
            })
            .collect(),
        asks: book
            .asks
            .into_iter()
            .map(|l| LevelResponse {
                price: l.price.to_string(),
                size: l.size.to_string(),
            })
            .collect(),
        observed_at: book.observed_at,
        confirmed_at,
        source: book.source.to_string(),
    }
}
