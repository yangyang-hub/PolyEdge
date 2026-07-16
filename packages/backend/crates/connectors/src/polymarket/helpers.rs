fn normalize_required(field_name: &str, value: &str, error_code: &'static str) -> Result<String> {
    let normalized = value.trim().to_string();
    if normalized.is_empty() {
        return Err(AppError::invalid_input(
            error_code,
            format!("{field_name} must not be empty"),
        ));
    }
    Ok(normalized)
}

fn normalize_optional(value: Option<&str>) -> Option<String> {
    value.and_then(|value| {
        let normalized = value.trim();
        (!normalized.is_empty()).then(|| normalized.to_string())
    })
}

fn clob_page_is_terminal(next_cursor: &str, count: u64, requested_cursor: Option<&str>) -> bool {
    next_cursor.is_empty()
        || next_cursor == CLOB_TERMINAL_CURSOR
        || count == 0
        || requested_cursor == Some(next_cursor)
}

fn maybe_credentials(config: &LivePolymarketConfig) -> Result<Option<Credentials>> {
    let api_key = normalize_optional(config.api_key.as_deref());
    let api_secret = normalize_optional(config.api_secret.as_deref());
    let api_passphrase = normalize_optional(config.api_passphrase.as_deref());
    match (api_key, api_secret, api_passphrase) {
        (None, None, None) => Ok(None),
        (Some(key), Some(secret), Some(passphrase)) => {
            let key = Uuid::parse_str(&key).map_err(|error| {
                AppError::invalid_input(
                    "POLYMARKET_API_KEY_INVALID",
                    format!("invalid polymarket api_key: {error}"),
                )
            })?;
            Ok(Some(Credentials::new(key, secret, passphrase)))
        }
        _ => Err(AppError::invalid_input(
            "POLYMARKET_CREDENTIALS_INCOMPLETE",
            "api_key, api_secret, and api_passphrase must all be set together for live mode",
        )),
    }
}

fn explicit_order_post_rejection(
    error: &PolymarketSdkError,
) -> Option<PolymarketOrderRejection> {
    let status = error.downcast_ref::<PolymarketSdkStatus>()?;
    status
        .status_code
        .is_client_error()
        .then(|| PolymarketOrderRejection {
            code: "POLYMARKET_ORDER_REJECTED".to_string(),
            message: format!(
                "CLOB rejected order with HTTP {}: {}",
                status.status_code, status.message
            ),
        })
}

fn parse_address(field_name: &str, value: &str, error_code: &'static str) -> Result<Address> {
    Address::from_str(value.trim()).map_err(|error| {
        AppError::invalid_input(error_code, format!("invalid {field_name}: {error}"))
    })
}

fn parse_u256(field_name: &str, value: &str, error_code: &'static str) -> Result<U256> {
    U256::from_str(value.trim()).map_err(|error| {
        AppError::invalid_input(error_code, format!("invalid {field_name}: {error}"))
    })
}

fn validate_live_token_order_request(request: &LivePolymarketTokenOrderRequest) -> Result<()> {
    if request.client_order_id.trim().is_empty() {
        return Err(AppError::invalid_input(
            "POLYMARKET_CLIENT_ORDER_ID_REQUIRED",
            "client_order_id must not be empty",
        ));
    }
    if request.connector_name != POLYMARKET_CONNECTOR_NAME {
        return Err(AppError::invalid_input(
            "POLYMARKET_CONNECTOR_UNSUPPORTED",
            format!(
                "polymarket connector requires connector_name={POLYMARKET_CONNECTOR_NAME}"
            ),
        ));
    }
    if request.token_id.trim().is_empty() {
        return Err(AppError::invalid_input(
            "POLYMARKET_TOKEN_ID_REQUIRED",
            "token_id must not be empty",
        ));
    }
    if request.limit_price.value() <= Decimal::ZERO {
        return Err(AppError::invalid_input(
            "POLYMARKET_LIMIT_PRICE_INVALID",
            "limit price must be positive",
        ));
    }
    if request.quantity.value() <= Decimal::ZERO {
        return Err(AppError::invalid_input(
            "POLYMARKET_QUANTITY_INVALID",
            "quantity must be positive",
        ));
    }
    if request.limit_price.value() * request.quantity.value() < POLYMARKET_MIN_NOTIONAL_USD {
        return Err(AppError::invalid_input(
            "POLYMARKET_NOTIONAL_INVALID",
            "order notional must be at least 1.00 USD",
        ));
    }
    Ok(())
}

fn validate_live_cancel_order_request(request: &LivePolymarketCancelOrderRequest) -> Result<()> {
    if request.connector_name != POLYMARKET_CONNECTOR_NAME {
        return Err(AppError::invalid_input(
            "POLYMARKET_CONNECTOR_UNSUPPORTED",
            format!(
                "polymarket connector requires connector_name={POLYMARKET_CONNECTOR_NAME}"
            ),
        ));
    }
    if request.external_order_id.trim().is_empty() {
        return Err(AppError::invalid_input(
            "POLYMARKET_ORDER_ID_REQUIRED",
            "external_order_id must not be empty",
        ));
    }
    Ok(())
}

fn adjusted_order_quantity(limit_price: Probability, quantity: Quantity) -> Result<Quantity> {
    let rounded = quantity.value().round_dp(2);
    let adjusted = adjust_size_for_cost_precision(limit_price.value(), rounded);
    Quantity::new(adjusted).map_err(|error| {
        AppError::invalid_input(
            "POLYMARKET_QUANTITY_INVALID",
            format!("adjusted polymarket quantity is invalid: {error}"),
        )
    })
}

fn cost_precision_step(price: Decimal) -> (u64, u64, u64) {
    let scale = price.scale();
    let denom = 10_u64.pow(scale);
    let numer = (price * Decimal::from(denom)).round().to_u64().unwrap_or(1);
    if numer == 0 {
        return (1, 0, denom);
    }
    let gcd = greatest_common_divisor(numer, denom);
    (denom / gcd, numer, denom)
}

fn adjust_size_for_cost_precision(price: Decimal, size: Decimal) -> Decimal {
    let cost = price * size;
    if cost == cost.round_dp(2) {
        return size;
    }
    let (step, numer, _) = cost_precision_step(price);
    if numer == 0 {
        return size;
    }
    let size_as_hundredths = (size * Decimal::from(100_u64))
        .round()
        .to_u64()
        .unwrap_or(0);
    if step == 0 || size_as_hundredths < step {
        return Decimal::ZERO;
    }
    let rounded = (size_as_hundredths / step) * step;
    Decimal::new(rounded as i64, 2)
}

fn greatest_common_divisor(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}
