#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn poly1271_signature_scheme_maps_to_sdk_signature_type() {
        let signature_type: SignatureType = PolymarketSignatureScheme::Poly1271.into();

        assert_eq!(signature_type, SignatureType::Poly1271);
    }

    #[test]
    fn clob_pagination_stops_at_documented_terminal_cursor() {
        assert!(clob_page_is_terminal(CLOB_TERMINAL_CURSOR, 1, None));
        assert!(clob_page_is_terminal("", 1, None));
        assert!(clob_page_is_terminal("next", 0, None));
        assert!(clob_page_is_terminal("next", 1, Some("next")));
        assert!(!clob_page_is_terminal("next", 1, None));
    }

    #[test]
    fn polygon_pusd_balance_hex_is_converted_to_decimal_usd() {
        let balance = erc20_hex_units_to_decimal(
            "0x00000000000000000000000000000000000000000000000000000000013125b5",
            6,
        )
        .expect("balance");

        assert_eq!(
            balance,
            Decimal::from_str_exact("19.9981").expect("decimal")
        );
    }

    #[test]
    fn polygon_wallet_address_requires_twenty_bytes() {
        let error =
            normalize_evm_address("wallet_address", "0x1234", "POLYGON_WALLET_ADDRESS_INVALID")
                .expect_err("short address");

        assert_eq!(error.code(), "POLYGON_WALLET_ADDRESS_INVALID");
    }

    #[test]
    fn successful_terminal_post_statuses_remain_accepted_for_reconciliation() {
        assert_eq!(
            accepted_order_status(&SdkOrderStatusType::Unmatched),
            PolymarketAcceptedOrderStatus::Unmatched
        );
        assert_eq!(
            accepted_order_status(&SdkOrderStatusType::Canceled),
            PolymarketAcceptedOrderStatus::Canceled
        );
        assert_eq!(
            accepted_order_status(&SdkOrderStatusType::Unknown("new_status".to_string())),
            PolymarketAcceptedOrderStatus::Unknown
        );
    }

    #[test]
    fn explicit_client_error_from_order_post_is_a_rejection() {
        let error = PolymarketSdkError::status(
            polymarket_client_sdk::error::StatusCode::BAD_REQUEST,
            polymarket_client_sdk::error::Method::POST,
            "/order".to_string(),
            r#"{"error":"the order signer address has to be the address of the API KEY"}"#,
        );

        let rejection = explicit_order_post_rejection(&error).expect("explicit rejection");

        assert_eq!(rejection.code, "POLYMARKET_ORDER_REJECTED");
        assert!(rejection.message.contains("HTTP 400 Bad Request"));
        assert!(rejection.message.contains("order signer address"));
    }

    #[test]
    fn server_error_from_order_post_remains_unknown() {
        let error = PolymarketSdkError::status(
            polymarket_client_sdk::error::StatusCode::INTERNAL_SERVER_ERROR,
            polymarket_client_sdk::error::Method::POST,
            "/order".to_string(),
            "upstream unavailable",
        );

        assert!(explicit_order_post_rejection(&error).is_none());
    }

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
    fn live_trade_reconciliation_only_books_confirmed_trades() {
        assert_eq!(
            live_trade_settlement(&SdkTradeStatusType::Confirmed),
            LivePolymarketTradeSettlement::Confirmed
        );
        assert_eq!(
            live_trade_settlement(&SdkTradeStatusType::Failed),
            LivePolymarketTradeSettlement::SettledWithoutFill
        );
        assert_eq!(
            live_trade_settlement(&SdkTradeStatusType::Matched),
            LivePolymarketTradeSettlement::Pending
        );
        assert_eq!(
            live_trade_settlement(&SdkTradeStatusType::Retrying),
            LivePolymarketTradeSettlement::Pending
        );
    }

    #[test]
    fn live_trade_reconciliation_waits_before_closing_terminal_fak_order() {
        let order = polymarket_client_sdk::clob::types::response::OpenOrderResponse::builder()
            .id("pm_fak_1")
            .status(SdkOrderStatusType::Matched)
            .owner(Uuid::nil())
            .maker_address(Address::ZERO)
            .market(B256::ZERO)
            .asset_id(U256::ZERO)
            .side(Side::Sell)
            .original_size(Decimal::from(20_u64))
            .size_matched(Decimal::from(7_u64))
            .price(Decimal::new(49, 2))
            .associate_trades(vec!["pm_trade_1".to_string()])
            .outcome("YES")
            .created_at("2024-01-15T12:34:56Z".parse().expect("created at"))
            .expiration("2024-01-15T12:34:56Z".parse().expect("expiration"))
            .order_type(OrderType::FAK)
            .build();

        assert!(reconciled_order_status_update(&order, false).is_none());
        assert_eq!(
            reconciled_order_status_update(&order, true)
                .expect("settled FAK match must be terminal")
                .status,
            OrderStatus::Filled
        );

        let mut live_order = order;
        live_order.status = SdkOrderStatusType::Live;
        assert_eq!(
            reconciled_order_status_update(&live_order, false)
                .expect("live status is safe before trade settlement")
                .status,
            OrderStatus::Open
        );

        live_order.status = SdkOrderStatusType::Unmatched;
        live_order.order_type = OrderType::GTC;
        assert_eq!(
            reconciled_order_status_update(&live_order, false)
                .expect("unmatched GTC order rests on the book")
                .status,
            OrderStatus::Open
        );
    }

    #[test]
    fn trade_order_fill_uses_order_specific_maker_amount() {
        let trade = TradeResponse::builder()
            .id("pm_trade_1")
            .taker_order_id("pm_taker")
            .market(B256::ZERO)
            .asset_id(U256::ZERO)
            .side(Side::Buy)
            .size(Decimal::new(125, 1))
            .fee_rate_bps(Decimal::new(25, 0))
            .price(Decimal::new(42, 2))
            .status(SdkTradeStatusType::Matched)
            .match_time("2024-01-15T12:34:56Z".parse().expect("match time"))
            .last_update("2024-01-15T12:35:30Z".parse().expect("last update"))
            .outcome("YES")
            .bucket_index(0)
            .owner(Uuid::nil())
            .maker_address(Address::ZERO)
            .maker_orders(vec![
                polymarket_client_sdk::clob::types::response::MakerOrder::builder()
                    .order_id("pm_maker_1")
                    .owner(Uuid::nil())
                    .maker_address(Address::ZERO)
                    .matched_amount(Decimal::new(50, 1))
                    .price(Decimal::new(43, 2))
                    .fee_rate_bps(Decimal::new(10, 0))
                    .asset_id(U256::ZERO)
                    .outcome("YES")
                    .side(Side::Sell)
                    .build(),
                polymarket_client_sdk::clob::types::response::MakerOrder::builder()
                    .order_id("pm_maker_2")
                    .owner(Uuid::nil())
                    .maker_address(Address::ZERO)
                    .matched_amount(Decimal::new(75, 1))
                    .price(Decimal::new(44, 2))
                    .fee_rate_bps(Decimal::new(12, 0))
                    .asset_id(U256::ZERO)
                    .outcome("YES")
                    .side(Side::Sell)
                    .build(),
            ])
            .transaction_hash(B256::ZERO)
            .trader_side(polymarket_client_sdk::clob::types::TraderSide::Taker)
            .build();

        let taker_fill = trade_order_fill(&trade, "pm_taker").expect("taker fill");
        let maker_fill = trade_order_fill(&trade, "pm_maker_1").expect("maker fill");

        assert_eq!(taker_fill.size, Decimal::new(125, 1));
        assert_eq!(taker_fill.price, Decimal::new(42, 2));
        assert_eq!(maker_fill.size, Decimal::new(50, 1));
        assert_eq!(maker_fill.price, Decimal::new(43, 2));
        assert_eq!(maker_fill.fee_rate_bps, Decimal::new(10, 0));

        let mut confirmed_trade = trade;
        confirmed_trade.status = SdkTradeStatusType::Confirmed;
        let reconciliation =
            reconcile_live_trade(&confirmed_trade, "pm_maker_1", "acct_polymarket")
                .expect("confirmed maker trade reconciliation");
        let LivePolymarketTradeReconciliation::Confirmed(update) = reconciliation else {
            panic!("confirmed matching trade must produce a fill update");
        };
        assert_eq!(update.external_order_id, "pm_maker_1");
        assert_eq!(update.filled_quantity.value(), Decimal::new(50, 1));
    }

    #[test]
    fn trade_order_fill_aggregates_repeated_maker_entries() {
        // The same maker order can appear more than once in a single trade when
        // multiple maker fills cross in one match event; all must be summed.
        let trade = TradeResponse::builder()
            .id("pm_trade_2")
            .taker_order_id("pm_taker")
            .market(B256::ZERO)
            .asset_id(U256::ZERO)
            .side(Side::Buy)
            .size(Decimal::new(100, 1))
            .fee_rate_bps(Decimal::new(25, 0))
            .price(Decimal::new(42, 2))
            .status(SdkTradeStatusType::Matched)
            .match_time("2024-01-15T12:34:56Z".parse().expect("match time"))
            .last_update("2024-01-15T12:35:30Z".parse().expect("last update"))
            .outcome("YES")
            .bucket_index(0)
            .owner(Uuid::nil())
            .maker_address(Address::ZERO)
            .maker_orders(vec![
                polymarket_client_sdk::clob::types::response::MakerOrder::builder()
                    .order_id("pm_maker_1")
                    .owner(Uuid::nil())
                    .maker_address(Address::ZERO)
                    .matched_amount(Decimal::new(40, 1)) // 4.0 @ 0.40
                    .price(Decimal::new(40, 2))
                    .fee_rate_bps(Decimal::new(10, 0))
                    .asset_id(U256::ZERO)
                    .outcome("YES")
                    .side(Side::Sell)
                    .build(),
                polymarket_client_sdk::clob::types::response::MakerOrder::builder()
                    .order_id("pm_maker_1")
                    .owner(Uuid::nil())
                    .maker_address(Address::ZERO)
                    .matched_amount(Decimal::new(60, 1)) // 6.0 @ 0.50
                    .price(Decimal::new(50, 2))
                    .fee_rate_bps(Decimal::new(10, 0))
                    .asset_id(U256::ZERO)
                    .outcome("YES")
                    .side(Side::Sell)
                    .build(),
            ])
            .transaction_hash(B256::ZERO)
            .trader_side(polymarket_client_sdk::clob::types::TraderSide::Taker)
            .build();

        let maker_fill = trade_order_fill(&trade, "pm_maker_1").expect("maker fill");
        // Total size = 4.0 + 6.0 = 10.0; notional = 1.6 + 3.0 = 4.6 → price 0.46.
        assert_eq!(maker_fill.size, Decimal::new(100, 1));
        assert_eq!(maker_fill.price, Decimal::new(46, 2));
        assert_eq!(maker_fill.fee_rate_bps, Decimal::new(10, 0));
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
            .matched_amount(Decimal::new(4, 0))
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
            .size(Decimal::new(10, 0))
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
        assert_eq!(updates[0].filled_quantity.value(), Decimal::new(10, 0));
        assert_eq!(updates[1].filled_quantity.value(), Decimal::new(4, 0));
    }
}
