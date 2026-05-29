#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copytrade_config_default_is_normalized() {
        let config = CopyTradeConfig::default().normalized();
        assert!(!config.enabled);
        assert_eq!(config.mode, CopyTradeMode::Paper);
        assert_eq!(config.sizing_mode, CopySizingMode::FixedUsd);
        assert!(config.account_capital_usd > Decimal::ZERO);
        assert!(config.min_price < config.max_price);
    }

    #[test]
    fn copytrade_config_patch_applies_selectively() {
        let base = CopyTradeConfig::default().normalized();
        let patch = CopyTradeConfigPatch {
            enabled: Some(true),
            fixed_usd_per_trade: Some(Decimal::from(50)),
            ..CopyTradeConfigPatch::default()
        };
        let next = base.apply_patch(patch);
        assert!(next.enabled);
        assert_eq!(next.fixed_usd_per_trade, Decimal::from(50));
        assert_eq!(next.mode, CopyTradeMode::Paper);
    }

    #[test]
    fn normalize_address_accepts_valid_hex() {
        let addr = "0xAbCdEf0123456789AbCdEf0123456789AbCdEf01";
        let normalized = normalize_address(addr).unwrap();
        assert_eq!(normalized, "0xabcdef0123456789abcdef0123456789abcdef01");
    }

    #[test]
    fn normalize_address_rejects_invalid() {
        assert!(normalize_address("not-an-address").is_none());
        assert!(normalize_address("0xabc").is_none());
        assert!(normalize_address("0xZZZZ").is_none());
    }

    #[test]
    fn fixed_usd_sizing_returns_fixed_amount() {
        let config = CopyTradeConfig {
            sizing_mode: CopySizingMode::FixedUsd,
            fixed_usd_per_trade: decimal("20"),
            ..CopyTradeConfig::default()
        };
        let source = SourceTrade {
            id: "st_1".into(),
            wallet_address: "0xabc".into(),
            condition_id: "cond1".into(),
            token_id: "tok1".into(),
            outcome: "Yes".into(),
            side: CopyOrderSide::Buy,
            price: decimal("0.5"),
            size: decimal("100"),
            usd_size: decimal("50"),
            title: "Test".into(),
            source_tx_hash: "0xtx1".into(),
            source_timestamp: OffsetDateTime::now_utc(),
            observed_at: OffsetDateTime::now_utc(),
            copied: false,
            decision_reason: String::new(),
        };
        let account = CopyAccountState::fresh("test", decimal("1000"), OffsetDateTime::now_utc());
        let decision = compute_copy_size(&config, &source, None, &account, None);
        assert!(decision.copy);
        // $20 / 0.5 = 40 tokens
        assert_eq!(decision.size, decimal("40"));
    }

    #[test]
    fn proportional_sizing_scales_to_source() {
        let config = CopyTradeConfig {
            sizing_mode: CopySizingMode::ProportionalToSource,
            proportional_factor: decimal("0.1"),
            ..CopyTradeConfig::default()
        };
        let source = SourceTrade {
            id: "st_1".into(),
            wallet_address: "0xabc".into(),
            condition_id: "cond1".into(),
            token_id: "tok1".into(),
            outcome: "Yes".into(),
            side: CopyOrderSide::Buy,
            price: decimal("0.6"),
            size: decimal("100"),
            usd_size: decimal("60"),
            title: "Test".into(),
            source_tx_hash: "0xtx1".into(),
            source_timestamp: OffsetDateTime::now_utc(),
            observed_at: OffsetDateTime::now_utc(),
            copied: false,
            decision_reason: String::new(),
        };
        let account = CopyAccountState::fresh("test", decimal("10000"), OffsetDateTime::now_utc());
        let decision = compute_copy_size(&config, &source, None, &account, None);
        assert!(decision.copy);
        // source_usd_size=60 * 0.1 = $6.  $6 / 0.6 = 10 tokens.
        assert_eq!(decision.size, decimal("10"));
    }

    #[test]
    fn skip_reasons_catch_below_min_usd() {
        let config = CopyTradeConfig {
            min_source_trade_usd: decimal("10"),
            ..CopyTradeConfig::default()
        };
        let source = SourceTrade {
            id: "st_1".into(),
            wallet_address: "0xabc".into(),
            condition_id: "cond1".into(),
            token_id: "tok1".into(),
            outcome: "Yes".into(),
            side: CopyOrderSide::Buy,
            price: decimal("0.5"),
            size: decimal("5"),
            usd_size: decimal("2.5"),
            title: "Test".into(),
            source_tx_hash: "0xtx1".into(),
            source_timestamp: OffsetDateTime::now_utc(),
            observed_at: OffsetDateTime::now_utc(),
            copied: false,
            decision_reason: String::new(),
        };
        let account = CopyAccountState::fresh("test", decimal("1000"), OffsetDateTime::now_utc());
        let reason = check_skip_reasons(&config, &source, &account, &[], &[], Decimal::ZERO, Decimal::ZERO);
        assert_eq!(reason, Some(CopySkipReason::BelowMinSize));
    }

    #[test]
    fn simulation_engine_fills_crossed_buy_order() {
        let config = CopyTradeConfig::default().normalized();
        let account = CopyAccountState::fresh("test", decimal("1000"), OffsetDateTime::now_utc());
        let new_order = CopyOrder {
            id: "ct_ord_1".into(),
            account_id: "test".into(),
            wallet_address: "0xabc".into(),
            source_trade_id: "st_1".into(),
            condition_id: "cond1".into(),
            token_id: "tok1".into(),
            outcome: "Yes".into(),
            side: CopyOrderSide::Buy,
            price: decimal("0.55"),
            size: decimal("20"),
            notional_usd: decimal("11"),
            external_order_id: None,
            status: CopyOrderStatus::Planned,
            reason: "fixed_usd".into(),
            filled_size: Decimal::ZERO,
            realized_pnl: Decimal::ZERO,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        };
        let mut books = HashMap::new();
        books.insert(
            "tok1".into(),
            CopyOrderBook {
                token_id: "tok1".into(),
                bids: vec![CopyBookLevel {
                    price: decimal("0.50"),
                    size: decimal("100"),
                }],
                asks: vec![CopyBookLevel {
                    price: decimal("0.53"),
                    size: decimal("100"),
                }],
                observed_at: OffsetDateTime::now_utc(),
            },
        );
        let outcome = run_copy_simulation_tick(
            &config,
            account,
            Vec::new(),
            Vec::new(),
            vec![new_order],
            &books,
            &[],
            60,
            "test_trace",
        );
        assert_eq!(outcome.report.orders_filled, 1);
        assert_eq!(outcome.fills.len(), 1);
        assert_eq!(outcome.fills[0].size, decimal("20"));
        assert_eq!(outcome.orders[0].status, CopyOrderStatus::Filled);
    }

    #[test]
    fn build_wallet_analysis_computes_stats() {
        let now = OffsetDateTime::now_utc();
        let activities = vec![
            WalletActivityInput {
                kind: "TRADE".into(),
                side: "BUY".into(),
                asset: "tok1".into(),
                condition_id: "c1".into(),
                outcome: "Yes".into(),
                title: "Market A".into(),
                slug: "market-a".into(),
                price: decimal("0.5"),
                size: decimal("100"),
                usdc_size: decimal("50"),
                transaction_hash: "0xa".into(),
                timestamp: now,
            },
            WalletActivityInput {
                kind: "TRADE".into(),
                side: "SELL".into(),
                asset: "tok1".into(),
                condition_id: "c1".into(),
                outcome: "Yes".into(),
                title: "Market A".into(),
                slug: "market-a".into(),
                price: decimal("0.6"),
                size: decimal("50"),
                usdc_size: decimal("30"),
                transaction_hash: "0xb".into(),
                timestamp: now,
            },
        ];
        let positions = vec![WalletPositionInput {
            asset: "tok1".into(),
            condition_id: "c1".into(),
            outcome: "Yes".into(),
            title: "Market A".into(),
            slug: "market-a".into(),
            size: decimal("50"),
            avg_price: decimal("0.5"),
            cur_price: decimal("0.6"),
            realized_pnl: decimal("5"),
            percent_pnl: decimal("20"),
        }];
        let stats = build_wallet_analysis(&activities, &positions);
        assert_eq!(stats.trades_window, 2);
        assert_eq!(stats.volume_window_usd, decimal("80"));
        assert_eq!(stats.markets_traded, 1);
        assert_eq!(stats.avg_trade_usd, decimal("40"));
        assert!(stats.win_rate > Decimal::ZERO);
    }
}
