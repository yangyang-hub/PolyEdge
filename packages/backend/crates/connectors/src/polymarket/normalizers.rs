pub fn normalize_polymarket_order_status_update(
    event_id: &str,
    external_order_id: &str,
    status: &str,
) -> Result<ConnectorOrderStatusUpdate> {
    let event_id = normalize_required("event_id", event_id, "POLYMARKET_EVENT_ID_REQUIRED")?;
    let external_order_id = normalize_required(
        "order_id",
        external_order_id,
        "POLYMARKET_ORDER_ID_REQUIRED",
    )?;
    let status = match status.trim().to_ascii_lowercase().as_str() {
        "live" => OrderStatus::Open,
        "canceled" | "cancelled" => OrderStatus::Canceled,
        "matched" | "delayed" => {
            return Err(AppError::invalid_input(
                "POLYMARKET_ORDER_STATUS_REQUIRES_TRADE_CALLBACK",
                "matched or delayed Polymarket order updates must be handled via the trade fill callback",
            ));
        }
        other => {
            return Err(AppError::invalid_input(
                "POLYMARKET_ORDER_STATUS_INVALID",
                format!("unsupported Polymarket order status: {other}"),
            ));
        }
    };

    Ok(ConnectorOrderStatusUpdate {
        event_id,
        connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
        external_order_id,
        status,
    })
}

pub fn normalize_polymarket_trade_fill_update(
    event_id: &str,
    external_order_id: &str,
    account_id: &str,
    external_trade_id: &str,
    fill_price: Probability,
    filled_quantity: Quantity,
    fee: UsdAmount,
) -> Result<ConnectorTradeFillUpdate> {
    let event_id = normalize_required("event_id", event_id, "POLYMARKET_EVENT_ID_REQUIRED")?;
    let external_order_id = normalize_required(
        "order_id",
        external_order_id,
        "POLYMARKET_ORDER_ID_REQUIRED",
    )?;
    let account_id =
        normalize_required("account_id", account_id, "POLYMARKET_ACCOUNT_ID_REQUIRED")?;
    let external_trade_id = normalize_required(
        "trade_id",
        external_trade_id,
        "POLYMARKET_TRADE_ID_REQUIRED",
    )?;

    if filled_quantity.value().is_zero() {
        return Err(AppError::invalid_input(
            "POLYMARKET_FILLED_QUANTITY_REQUIRED",
            "size must be greater than zero",
        ));
    }

    Ok(ConnectorTradeFillUpdate {
        event_id,
        connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
        external_order_id,
        account_id,
        external_trade_id,
        fill_price,
        filled_quantity,
        fee,
    })
}

pub fn normalize_polymarket_ws_order_message(
    message: &PolymarketWsOrderMessage,
) -> Result<Option<ConnectorOrderStatusUpdate>> {
    let external_order_id =
        normalize_required("order_id", &message.id, "POLYMARKET_ORDER_ID_REQUIRED")?;
    let mapped_status = match message.status.as_ref() {
        Some(
            SdkOrderStatusType::Live
            | SdkOrderStatusType::Unmatched
            | SdkOrderStatusType::Matched
            | SdkOrderStatusType::Delayed,
        ) => Some(OrderStatus::Open),
        Some(SdkOrderStatusType::Canceled) => Some(OrderStatus::Canceled),
        Some(SdkOrderStatusType::Unknown(_)) | Some(_) => None,
        None => match message.msg_type.as_ref() {
            Some(
                PolymarketWsOrderMessageType::Placement | PolymarketWsOrderMessageType::Update,
            ) => Some(OrderStatus::Open),
            Some(PolymarketWsOrderMessageType::Cancellation) => Some(OrderStatus::Canceled),
            Some(PolymarketWsOrderMessageType::Unknown(_)) | Some(_) | None => None,
        },
    };

    let Some(status) = mapped_status else {
        return Ok(None);
    };

    let status_marker = match status {
        OrderStatus::Open => "open",
        OrderStatus::Canceled => "canceled",
        _ => unreachable!("websocket order updates only map to open/canceled"),
    };
    let event_time = message
        .timestamp
        .map_or_else(|| "na".to_string(), |timestamp| timestamp.to_string());

    Ok(Some(ConnectorOrderStatusUpdate {
        event_id: format!("evt_pm_ws_order:{external_order_id}:{status_marker}:{event_time}"),
        connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
        external_order_id,
        status,
    }))
}

pub fn normalize_polymarket_ws_trade_message(
    message: &PolymarketWsTradeMessage,
    account_id: &str,
) -> Result<Vec<ConnectorTradeFillUpdate>> {
    if matches!(message.status, PolymarketWsTradeMessageStatus::Unknown(_)) {
        return Ok(Vec::new());
    }

    let trade_id = normalize_required("trade_id", &message.id, "POLYMARKET_TRADE_ID_REQUIRED")?;
    let account_id =
        normalize_required("account_id", account_id, "POLYMARKET_ACCOUNT_ID_REQUIRED")?;
    let order_ids = candidate_order_ids_from_trade_message(
        message.taker_order_id.as_deref(),
        &message.maker_orders,
    );
    if order_ids.is_empty() {
        return Ok(Vec::new());
    }

    let multiple_orders = order_ids.len() > 1;
    let mut updates = Vec::with_capacity(order_ids.len());
    for order_id in order_ids {
        let external_trade_id = if multiple_orders {
            format!("{trade_id}:{order_id}")
        } else {
            trade_id.clone()
        };
        let Some(fill) = websocket_trade_order_fill(message, &order_id) else {
            continue;
        };
        let fill_price = Probability::new(fill.price).map_err(|error| {
            AppError::internal(
                "POLYMARKET_TRADE_PRICE_INVALID",
                format!("failed to decode websocket trade price for {trade_id}: {error}"),
            )
        })?;
        let filled_quantity = Quantity::new(fill.size).map_err(|error| {
            AppError::internal(
                "POLYMARKET_TRADE_SIZE_INVALID",
                format!("failed to decode websocket trade size for {trade_id}: {error}"),
            )
        })?;
        let fee = UsdAmount::new(
            fill.price * fill.size * message.fee_rate_bps.unwrap_or(Decimal::ZERO)
                / Decimal::from(10_000_u64),
        )
        .map_err(|error| {
            AppError::internal(
                "POLYMARKET_TRADE_FEE_INVALID",
                format!("failed to decode websocket trade fee for {trade_id}: {error}"),
            )
        })?;
        updates.push(normalize_polymarket_trade_fill_update(
            &format!("evt_pm_ws_trade:{trade_id}:{order_id}"),
            &order_id,
            &account_id,
            &external_trade_id,
            fill_price,
            filled_quantity,
            fee,
        )?);
    }

    Ok(updates)
}

#[derive(Debug, Clone, Copy)]
struct WebsocketOrderSpecificTradeFill {
    price: Decimal,
    size: Decimal,
}

fn websocket_trade_order_fill(
    message: &PolymarketWsTradeMessage,
    external_order_id: &str,
) -> Option<WebsocketOrderSpecificTradeFill> {
    if message.taker_order_id.as_deref() == Some(external_order_id) {
        return Some(WebsocketOrderSpecificTradeFill {
            price: message.price,
            size: message.size,
        });
    }

    message
        .maker_orders
        .iter()
        .find(|maker_order| maker_order.order_id == external_order_id)
        .map(|maker_order| WebsocketOrderSpecificTradeFill {
            price: maker_order.price,
            size: maker_order.matched_amount,
        })
}
