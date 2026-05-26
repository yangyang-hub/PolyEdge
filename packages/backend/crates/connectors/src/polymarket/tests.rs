#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn live_status_maps_to_open() {
        let update =
            normalize_polymarket_order_status_update("evt_1", "pm_ord_1", "live").expect("map");

        assert_eq!(update.connector_name, POLYMARKET_CONNECTOR_NAME);
        assert_eq!(update.status, OrderStatus::Open);
    }

    #[test]
    fn canceled_alias_maps_to_canceled() {
        let update = normalize_polymarket_order_status_update("evt_1", "pm_ord_1", "cancelled")
            .expect("map");

        assert_eq!(update.status, OrderStatus::Canceled);
    }

    #[test]
    fn matched_status_requires_trade_callback() {
        let error = normalize_polymarket_order_status_update("evt_1", "pm_ord_1", "matched")
            .expect_err("matched should be rejected");

        assert_eq!(
            error.code(),
            "POLYMARKET_ORDER_STATUS_REQUIRES_TRADE_CALLBACK"
        );
    }

    #[test]
    fn trade_fill_normalization_preserves_trade_fields() {
        let update = normalize_polymarket_trade_fill_update(
            "evt_trade_1",
            "pm_ord_1",
            "acct_1",
            "pm_trade_1",
            Probability::new(Decimal::new(48, 2)).expect("price"),
            Quantity::new(Decimal::ONE).expect("quantity"),
            UsdAmount::new(Decimal::ZERO).expect("fee"),
        )
        .expect("trade fill");

        assert_eq!(update.connector_name, POLYMARKET_CONNECTOR_NAME);
        assert_eq!(update.external_trade_id, "pm_trade_1");
        assert_eq!(update.filled_quantity.value(), Decimal::ONE);
    }

    #[test]
    fn websocket_cancellation_message_maps_to_canceled() {
        let message = PolymarketWsOrderMessage::builder()
            .id("pm_ord_1".to_string())
            .market(B256::ZERO)
            .asset_id(U256::ZERO)
            .side(Side::Buy)
            .price(Decimal::new(57, 2))
            .msg_type(PolymarketWsOrderMessageType::Cancellation)
            .outcome("YES".to_string())
            .original_size(Decimal::ONE)
            .size_matched(Decimal::ZERO)
            .timestamp(1_717_171_717_000)
            .build();
        let update = normalize_polymarket_ws_order_message(&message)
            .expect("normalize")
            .expect("mapped update");

        assert_eq!(update.status, OrderStatus::Canceled);
        assert_eq!(update.external_order_id, "pm_ord_1");
    }

    #[test]
    fn websocket_trade_message_generates_distinct_updates_per_order() {
        let maker_order = MakerOrder::builder()
            .asset_id(U256::ZERO)
            .matched_amount(Decimal::ONE)
            .order_id("pm_ord_maker".to_string())
            .outcome("YES".to_string())
            .owner(Uuid::nil())
            .price(Decimal::new(57, 2))
            .build();
        let message = PolymarketWsTradeMessage::builder()
            .id("pm_trade_1".to_string())
            .market(B256::ZERO)
            .asset_id(U256::ZERO)
            .side(Side::Buy)
            .size(Decimal::ONE)
            .price(Decimal::new(57, 2))
            .status(PolymarketWsTradeMessageStatus::Matched)
            .last_update(1_717_171_717_100)
            .matchtime(1_717_171_717_100)
            .timestamp(1_717_171_717_100)
            .outcome("YES".to_string())
            .taker_order_id("pm_ord_taker".to_string())
            .maker_orders(vec![maker_order])
            .fee_rate_bps(Decimal::new(25, 0))
            .build();
        let updates =
            normalize_polymarket_ws_trade_message(&message, "acct_polymarket").expect("normalize");

        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].external_order_id, "pm_ord_taker");
        assert_eq!(updates[1].external_order_id, "pm_ord_maker");
        assert_eq!(updates[0].external_trade_id, "pm_trade_1:pm_ord_taker");
        assert_eq!(updates[1].external_trade_id, "pm_trade_1:pm_ord_maker");
    }
}
