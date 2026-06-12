fn reconcile_live_trade(
    trade: &TradeResponse,
    external_order_id: &str,
    account_id: &str,
) -> Result<LivePolymarketTradeReconciliation> {
    match live_trade_settlement(&trade.status) {
        LivePolymarketTradeSettlement::Confirmed => {}
        LivePolymarketTradeSettlement::SettledWithoutFill => {
            return Ok(LivePolymarketTradeReconciliation::SettledWithoutFill);
        }
        LivePolymarketTradeSettlement::Pending => {
            return Ok(LivePolymarketTradeReconciliation::Pending);
        }
    }

    let Some(fill) = trade_order_fill(trade, external_order_id) else {
        warn!(
            external_trade_id = %trade.id,
            external_order_id,
            "polymarket trade response did not include order-specific fill details"
        );
        return Ok(LivePolymarketTradeReconciliation::Pending);
    };

    let fill_price = Probability::new(fill.price).map_err(|error| {
        AppError::internal(
            "POLYMARKET_TRADE_PRICE_INVALID",
            format!("failed to decode trade price for {}: {error}", trade.id),
        )
    })?;
    let filled_quantity = Quantity::new(fill.size).map_err(|error| {
        AppError::internal(
            "POLYMARKET_TRADE_SIZE_INVALID",
            format!("failed to decode trade size for {}: {error}", trade.id),
        )
    })?;
    let fee =
        UsdAmount::new(fill.price * fill.size * fill.fee_rate_bps / Decimal::from(10_000_u64))
            .map_err(|error| {
                AppError::internal(
                    "POLYMARKET_TRADE_FEE_INVALID",
                    format!("failed to decode trade fee for {}: {error}", trade.id),
                )
            })?;

    Ok(LivePolymarketTradeReconciliation::Confirmed(
        ConnectorTradeFillUpdate {
            event_id: format!("evt_pm_trade_poll:{}:{}", external_order_id, trade.id),
            connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
            external_order_id: external_order_id.to_string(),
            account_id: account_id.to_string(),
            external_trade_id: trade.id.clone(),
            fill_price,
            filled_quantity,
            fee,
        },
    ))
}

enum LivePolymarketTradeReconciliation {
    Confirmed(ConnectorTradeFillUpdate),
    SettledWithoutFill,
    Pending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LivePolymarketTradeSettlement {
    Confirmed,
    SettledWithoutFill,
    Pending,
}

fn live_trade_settlement(status: &SdkTradeStatusType) -> LivePolymarketTradeSettlement {
    match status {
        SdkTradeStatusType::Confirmed => LivePolymarketTradeSettlement::Confirmed,
        SdkTradeStatusType::Failed => LivePolymarketTradeSettlement::SettledWithoutFill,
        _ => LivePolymarketTradeSettlement::Pending,
    }
}

fn reconciled_order_status_update(
    order: &polymarket_client_sdk::clob::types::response::OpenOrderResponse,
    associated_trades_terminal: bool,
) -> Option<ConnectorOrderStatusUpdate> {
    let status = match order.status {
        SdkOrderStatusType::Live => OrderStatus::Open,
        SdkOrderStatusType::Canceled if associated_trades_terminal => OrderStatus::Canceled,
        SdkOrderStatusType::Matched if associated_trades_terminal => OrderStatus::Filled,
        SdkOrderStatusType::Unmatched
            if matches!(order.order_type, OrderType::FAK | OrderType::FOK) =>
        {
            if !associated_trades_terminal {
                return None;
            }
            OrderStatus::Canceled
        }
        SdkOrderStatusType::Unmatched => OrderStatus::Open,
        _ => return None,
    };
    Some(ConnectorOrderStatusUpdate {
        event_id: format!("evt_pm_order_reconcile:{}:{}", order.id, status.as_str()),
        connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
        external_order_id: order.id.clone(),
        status,
    })
}
