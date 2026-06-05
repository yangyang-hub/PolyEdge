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
