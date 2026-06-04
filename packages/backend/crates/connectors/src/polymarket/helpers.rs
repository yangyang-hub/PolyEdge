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

fn candidate_order_ids_from_trade_message(
    taker_order_id: Option<&str>,
    maker_orders: &[MakerOrder],
) -> Vec<String> {
    let mut order_ids = Vec::new();

    if let Some(order_id) = normalize_optional(taker_order_id) {
        order_ids.push(order_id);
    }

    for maker_order in maker_orders {
        let Some(order_id) = normalize_optional(Some(maker_order.order_id.as_str())) else {
            continue;
        };
        if !order_ids.iter().any(|candidate| candidate == &order_id) {
            order_ids.push(order_id);
        }
    }

    order_ids
}

fn normalize_optional(value: Option<&str>) -> Option<String> {
    value.and_then(|value| {
        let normalized = value.trim();
        if normalized.is_empty() {
            None
        } else {
            Some(normalized.to_string())
        }
    })
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

fn parse_address(field_name: &str, value: &str, error_code: &'static str) -> Result<Address> {
    Address::from_str(value.trim()).map_err(|error| {
        AppError::invalid_input(error_code, format!("invalid {field_name}: {error}"))
    })
}

fn parse_b256(field_name: &str, value: &str, error_code: &'static str) -> Result<B256> {
    B256::from_str(value.trim()).map_err(|error| {
        AppError::invalid_input(error_code, format!("invalid {field_name}: {error}"))
    })
}

fn parse_u256(field_name: &str, value: &str, error_code: &'static str) -> Result<U256> {
    U256::from_str(value.trim()).map_err(|error| {
        AppError::invalid_input(error_code, format!("invalid {field_name}: {error}"))
    })
}

fn best_bid_level(orders: Vec<OrderSummary>) -> Result<Option<PolymarketBookLevel>> {
    orders
        .into_iter()
        .max_by(|left, right| left.price.cmp(&right.price))
        .map(book_level_from_order)
        .transpose()
}

fn best_ask_level(orders: Vec<OrderSummary>) -> Result<Option<PolymarketBookLevel>> {
    orders
        .into_iter()
        .min_by(|left, right| left.price.cmp(&right.price))
        .map(book_level_from_order)
        .transpose()
}

fn book_level_from_order(order: OrderSummary) -> Result<PolymarketBookLevel> {
    Ok(PolymarketBookLevel {
        price: Probability::new(order.price).map_err(|error| {
            AppError::internal(
                "POLYMARKET_ORDER_BOOK_PRICE_INVALID",
                format!("failed to decode Polymarket order book price: {error}"),
            )
        })?,
        size: Quantity::new(order.size).map_err(|error| {
            AppError::internal(
                "POLYMARKET_ORDER_BOOK_SIZE_INVALID",
                format!("failed to decode Polymarket order book size: {error}"),
            )
        })?,
    })
}

fn max_time(left: OffsetDateTime, right: OffsetDateTime) -> OffsetDateTime {
    if left >= right { left } else { right }
}

fn validate_live_order_request(request: &LivePolymarketOrderRequest) -> Result<()> {
    if request.execution_request_id.trim().is_empty() {
        return Err(AppError::invalid_input(
            "POLYMARKET_EXECUTION_REQUEST_ID_REQUIRED",
            "execution_request_id must not be empty",
        ));
    }

    if request.connector_name != POLYMARKET_CONNECTOR_NAME {
        return Err(AppError::invalid_input(
            "POLYMARKET_CONNECTOR_UNSUPPORTED",
            format!(
                "polymarket live connector only handles connector_name={POLYMARKET_CONNECTOR_NAME}, got {}",
                request.connector_name
            ),
        ));
    }

    if request.market_id.trim().is_empty() {
        return Err(AppError::invalid_input(
            "POLYMARKET_MARKET_ID_REQUIRED",
            "market_id must not be empty",
        ));
    }

    if request.limit_price.value() <= Decimal::ZERO {
        return Err(AppError::invalid_input(
            "POLYMARKET_LIMIT_PRICE_INVALID",
            "polymarket live connector requires a positive limit price",
        ));
    }

    if request.quantity.value() <= Decimal::ZERO {
        return Err(AppError::invalid_input(
            "POLYMARKET_QUANTITY_INVALID",
            "polymarket live connector requires a positive quantity",
        ));
    }

    if request.notional.value() < POLYMARKET_MIN_NOTIONAL_USD {
        return Err(AppError::invalid_input(
            "POLYMARKET_NOTIONAL_INVALID",
            "polymarket live connector requires notional >= 1.00 USD",
        ));
    }

    Ok(())
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
                "polymarket live connector only handles connector_name={POLYMARKET_CONNECTOR_NAME}, got {}",
                request.connector_name
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
            "polymarket live connector requires a positive limit price",
        ));
    }

    if request.quantity.value() <= Decimal::ZERO {
        return Err(AppError::invalid_input(
            "POLYMARKET_QUANTITY_INVALID",
            "polymarket live connector requires a positive quantity",
        ));
    }

    if request.limit_price.value() * request.quantity.value() < POLYMARKET_MIN_NOTIONAL_USD {
        return Err(AppError::invalid_input(
            "POLYMARKET_NOTIONAL_INVALID",
            "polymarket live connector requires notional >= 1.00 USD",
        ));
    }

    Ok(())
}

fn validate_live_cancel_order_request(request: &LivePolymarketCancelOrderRequest) -> Result<()> {
    if request.connector_name != POLYMARKET_CONNECTOR_NAME {
        return Err(AppError::invalid_input(
            "POLYMARKET_CONNECTOR_UNSUPPORTED",
            format!(
                "polymarket live connector only handles connector_name={POLYMARKET_CONNECTOR_NAME}, got {}",
                request.connector_name
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

fn validate_live_order_status_request(request: &LivePolymarketOrderStatusRequest) -> Result<()> {
    if request.connector_name != POLYMARKET_CONNECTOR_NAME {
        return Err(AppError::invalid_input(
            "POLYMARKET_CONNECTOR_UNSUPPORTED",
            format!(
                "polymarket live connector only handles connector_name={POLYMARKET_CONNECTOR_NAME}, got {}",
                request.connector_name
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

fn validate_live_trade_sync_request(request: &LivePolymarketTradeSyncRequest) -> Result<()> {
    validate_live_order_status_request(&LivePolymarketOrderStatusRequest {
        connector_name: request.connector_name.clone(),
        external_order_id: request.external_order_id.clone(),
    })?;

    if request.account_id.trim().is_empty() {
        return Err(AppError::invalid_input(
            "POLYMARKET_ACCOUNT_ID_REQUIRED",
            "account_id must not be empty",
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

fn trade_matches_order(
    trade: &TradeResponse,
    external_order_id: &str,
) -> bool {
    trade_order_fill(trade, external_order_id).is_some()
}

#[derive(Debug, Clone, Copy)]
struct OrderSpecificTradeFill {
    price: Decimal,
    size: Decimal,
    fee_rate_bps: Decimal,
}

fn trade_order_fill(
    trade: &TradeResponse,
    external_order_id: &str,
) -> Option<OrderSpecificTradeFill> {
    if trade.taker_order_id == external_order_id {
        return Some(OrderSpecificTradeFill {
            price: trade.price,
            size: trade.size,
            fee_rate_bps: trade.fee_rate_bps,
        });
    }

    // A single trade can list the same maker order more than once (multiple
    // maker fills crossed in one match event). Aggregate every matching entry
    // so the full matched size is credited — not just the first — otherwise the
    // order's filled size and inventory are understated.
    let mut total_size = Decimal::ZERO;
    let mut total_notional = Decimal::ZERO;
    let mut fee_weighted = Decimal::ZERO;
    for maker_order in trade
        .maker_orders
        .iter()
        .filter(|maker_order| maker_order.order_id == external_order_id)
    {
        total_size += maker_order.matched_amount;
        total_notional += maker_order.price * maker_order.matched_amount;
        fee_weighted += maker_order.fee_rate_bps * maker_order.matched_amount;
    }

    if total_size <= Decimal::ZERO {
        return None;
    }

    Some(OrderSpecificTradeFill {
        // Size-weighted average price/fee across the aggregated maker fills.
        price: total_notional / total_size,
        size: total_size,
        fee_rate_bps: fee_weighted / total_size,
    })
}
