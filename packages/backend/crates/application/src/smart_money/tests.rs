#[cfg(test)]
mod smart_money_tests {
    use super::*;

    fn profile() -> SmartWalletProfile {
        SmartWalletProfile {
            wallet_address: "0x0000000000000000000000000000000000000001".to_string(),
            trade_count: 100,
            settled_trade_count: 50,
            total_volume_usd: Decimal::from(20_000),
            realized_pnl_usd: Decimal::from(1_000),
            roi: Decimal::new(20, 2),
            win_rate: Decimal::new(65, 2),
            max_drawdown_usd: Decimal::from(200),
            avg_trade_usd: Decimal::from(200),
            median_trade_usd: Decimal::from(150),
            avg_hold_secs: Some(3600),
            active_days: 20,
            markets_traded: 30,
            category_concentration_score: Decimal::new(20, 2),
            market_concentration_score: Decimal::new(25, 2),
            low_liquidity_trade_ratio: Decimal::new(10, 2),
            stale_copy_window_ratio: Decimal::new(10, 2),
            last_trade_at: Some(OffsetDateTime::now_utc()),
            updated_at: OffsetDateTime::now_utc(),
        }
    }

    #[test]
    fn smart_wallet_score_promotes_copyable_wallet() {
        let score = build_smart_wallet_score(&SmartMoneyConfig::default(), &profile());
        assert!(score.total_score >= Decimal::new(60, 2));
        assert_ne!(score.tier, SmartWalletTier::Candidate);
    }

    #[test]
    fn smart_wallet_score_blocks_low_sample_from_watch() {
        let mut profile = profile();
        profile.trade_count = 3;
        let score = build_smart_wallet_score(&SmartMoneyConfig::default(), &profile);
        assert_eq!(score.tier, SmartWalletTier::Candidate);
    }

    #[test]
    fn smart_wallet_address_normalizes_lowercase() {
        let address = normalize_smart_wallet_address(
            "0xABCDEFabcdefABCDEFabcdefABCDEFabcdefABCD",
        )
        .expect("valid address");
        assert_eq!(address, "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd");
    }

    fn smart_trade() -> SmartWalletTrade {
        SmartWalletTrade {
            id: "trade-1".to_string(),
            wallet_address: "0x0000000000000000000000000000000000000001".to_string(),
            source: "test".to_string(),
            condition_id: "condition-1".to_string(),
            token_id: Some("token-1".to_string()),
            side: SmartMoneySide::Buy,
            outcome: Some("Yes".to_string()),
            price: Decimal::new(50, 2),
            size: Decimal::from(10),
            notional_usd: Decimal::from(5),
            tx_hash: None,
            source_timestamp: OffsetDateTime::now_utc(),
            discovered_at: OffsetDateTime::now_utc(),
            raw: json!({}),
        }
    }

    fn smart_quote(best_ask: Decimal, ask_depth_usd: Decimal) -> SmartSignalBookQuote {
        SmartSignalBookQuote {
            token_id: "token-1".to_string(),
            best_bid: Some(Decimal::new(49, 2)),
            best_ask: Some(best_ask),
            bid_depth_usd: Decimal::from(100),
            ask_depth_usd,
        }
    }

    #[test]
    fn smart_signal_gate_observes_passed_trade() {
        let config = SmartMoneyConfig::default();
        let trade = smart_trade();
        let signal = build_smart_signal_from_trade(
            &config,
            &trade,
            Some(&smart_quote(Decimal::new(51, 2), Decimal::from(100))),
            Some(Decimal::new(75, 2)),
            OffsetDateTime::now_utc(),
        );

        assert_eq!(signal.status, SmartSignalStatus::Observe);
        assert_eq!(signal.current_price, Some(Decimal::new(51, 2)));
        assert_eq!(signal.score, Decimal::new(75, 2));
    }

    #[test]
    fn smart_signal_gate_records_observe_decision() {
        let config = SmartMoneyConfig::default();
        let trade = smart_trade();
        let mut signal = build_smart_signal_from_trade(
            &config,
            &trade,
            Some(&smart_quote(Decimal::new(51, 2), Decimal::from(100))),
            Some(Decimal::new(75, 2)),
            OffsetDateTime::now_utc(),
        );
        signal.id = 42;
        let decision = build_smart_signal_decision_for_gate(&signal, OffsetDateTime::now_utc())
            .expect("observe signal has a deterministic decision");

        assert_eq!(decision.signal_id, 42);
        assert_eq!(decision.decision, SmartSignalDecisionValue::Observe);
        assert_eq!(decision.stage, "deterministic_gate");
        assert_eq!(decision.mode, SmartMoneyMode::Observe);
        assert!(decision.rejection_reason.is_none());
    }

    #[test]
    fn smart_signal_gate_rejects_excessive_slippage() {
        let config = SmartMoneyConfig::default();
        let trade = smart_trade();
        let signal = build_smart_signal_from_trade(
            &config,
            &trade,
            Some(&smart_quote(Decimal::new(55, 2), Decimal::from(100))),
            None,
            OffsetDateTime::now_utc(),
        );

        assert_eq!(signal.status, SmartSignalStatus::Rejected);
        assert_eq!(signal.reason.as_deref(), Some("price slippage exceeded"));
    }

    #[test]
    fn smart_signal_advisory_hash_ignores_request_time() {
        let config = SmartMoneyConfig::default();
        let trade = smart_trade();
        let signal = build_smart_signal_from_trade(
            &config,
            &trade,
            Some(&smart_quote(Decimal::new(51, 2), Decimal::from(100))),
            Some(Decimal::new(75, 2)),
            OffsetDateTime::now_utc(),
        );
        let now = OffsetDateTime::now_utc();
        let profile = profile();
        let first = build_smart_signal_advisory_request(
            "openai",
            "openai_responses",
            "gpt-test",
            &config,
            &signal,
            SmartSignalAdvisoryContext {
                source_trade: Some(&trade),
                profile: Some(&profile),
                score: None,
                now,
                ttl_sec: 300,
            },
        )
        .expect("request builds");
        let second = build_smart_signal_advisory_request(
            "openai",
            "openai_responses",
            "gpt-test",
            &config,
            &signal,
            SmartSignalAdvisoryContext {
                source_trade: Some(&trade),
                profile: Some(&profile),
                score: None,
                now: now + time::Duration::minutes(5),
                ttl_sec: 900,
            },
        )
        .expect("request builds");

        assert_eq!(first.input_hash, second.input_hash);
        assert_ne!(first.payload, second.payload);
    }

    #[test]
    fn smart_signal_advisory_hash_tracks_risk_config() {
        let trade = smart_trade();
        let signal = build_smart_signal_from_trade(
            &SmartMoneyConfig::default(),
            &trade,
            Some(&smart_quote(Decimal::new(51, 2), Decimal::from(100))),
            Some(Decimal::new(75, 2)),
            OffsetDateTime::now_utc(),
        );
        let now = OffsetDateTime::now_utc();
        let mut stricter = SmartMoneyConfig::default();
        stricter.max_price_slippage_cents = Decimal::ONE;
        let first = build_smart_signal_advisory_request(
            "openai",
            "openai_responses",
            "gpt-test",
            &SmartMoneyConfig::default(),
            &signal,
            SmartSignalAdvisoryContext {
                source_trade: Some(&trade),
                profile: None,
                score: None,
                now,
                ttl_sec: 300,
            },
        )
        .expect("request builds");
        let second = build_smart_signal_advisory_request(
            "openai",
            "openai_responses",
            "gpt-test",
            &stricter,
            &signal,
            SmartSignalAdvisoryContext {
                source_trade: Some(&trade),
                profile: None,
                score: None,
                now,
                ttl_sec: 300,
            },
        )
        .expect("request builds");

        assert_ne!(first.input_hash, second.input_hash);
    }
}
