#[cfg(test)]
mod high_probability_tests {
    use super::*;

    fn sample(
        outcome: HighProbabilitySampleOutcome,
        price: i64,
        drawdown: i64,
    ) -> HighProbabilitySample {
        HighProbabilitySample {
            id: 0,
            condition_id: "condition".to_string(),
            token_id: format!("token-{price}-{drawdown}"),
            side: "yes".to_string(),
            sampled_at: OffsetDateTime::UNIX_EPOCH,
            trigger_kind: HighProbabilityTriggerKind::FirstTouch,
            executable_price: Decimal::new(price, 2),
            price_bucket: "0.80-0.85".to_string(),
            market_type: "sports".to_string(),
            time_to_resolution_bucket: Some("1d".to_string()),
            liquidity_bucket: Some("high".to_string()),
            spread_bucket: Some("tight".to_string()),
            path_features: json!({}),
            risk_tags: Vec::new(),
            outcome,
            settlement_pnl: Some(if outcome == HighProbabilitySampleOutcome::Win {
                Decimal::ONE - Decimal::new(price, 2)
            } else {
                -Decimal::new(price, 2)
            }),
            max_drawdown_cents: Some(Decimal::new(drawdown, 0)),
            hold_seconds: Some(3_600),
            created_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    fn candle_input(
        token_id: &str,
        close: i64,
        offset_hours: i64,
    ) -> HighProbabilityRewardCandleSampleInput {
        HighProbabilityRewardCandleSampleInput {
            condition_id: "condition".to_string(),
            token_id: token_id.to_string(),
            outcome: "Yes".to_string(),
            bucket_start: OffsetDateTime::UNIX_EPOCH + time::Duration::hours(offset_hours),
            close: Decimal::new(close, 2),
            spread_cents_close: Decimal::ONE,
            market_type: "sports".to_string(),
            liquidity_usd: Some(Decimal::new(20_000, 0)),
            resolved_at: Some(OffsetDateTime::UNIX_EPOCH + time::Duration::days(1)),
            outcome_status: HighProbabilityMarketOutcomeStatus::Resolved,
            winning_token_id: Some("token-yes".to_string()),
            risk_tags: Vec::new(),
        }
    }

    fn timed_sample(
        outcome: HighProbabilitySampleOutcome,
        price: i64,
        offset_hours: i64,
    ) -> HighProbabilitySample {
        let mut sample = sample(outcome, price, 0);
        sample.token_id = format!("token-{price}-{offset_hours}");
        sample.sampled_at = OffsetDateTime::UNIX_EPOCH + time::Duration::hours(offset_hours);
        sample.price_bucket = high_probability_price_bucket(sample.executable_price)
            .unwrap_or("unknown")
            .to_string();
        sample
    }

    #[test]
    fn bucket_stats_skip_small_buckets() {
        let config = HighProbabilityConfig {
            min_bucket_samples: 3,
            ..HighProbabilityConfig::default()
        };
        let samples = vec![
            sample(HighProbabilitySampleOutcome::Win, 80, 2),
            sample(HighProbabilitySampleOutcome::Loss, 80, 30),
        ];

        let stats = build_high_probability_bucket_stats(&config, &samples);

        assert!(stats.is_empty());
    }

    #[test]
    fn bucket_stats_compute_conservative_probability() {
        let config = HighProbabilityConfig {
            min_bucket_samples: 3,
            min_required_edge: Decimal::new(3, 2),
            fee_buffer: Decimal::new(5, 3),
            default_risk_margin: Decimal::new(2, 2),
            ..HighProbabilityConfig::default()
        };
        let samples = vec![
            sample(HighProbabilitySampleOutcome::Win, 80, 2),
            sample(HighProbabilitySampleOutcome::Win, 81, 3),
            sample(HighProbabilitySampleOutcome::Loss, 82, 25),
        ];

        let stats = build_high_probability_bucket_stats(&config, &samples);

        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].sample_count, 3);
        assert_eq!(stats[0].win_count, 2);
        assert_eq!(stats[0].fair_probability, Decimal::new(60, 2));
        assert_eq!(
            stats[0].break_60_rate,
            Some(Decimal::ONE / Decimal::from(3u64))
        );
    }

    #[test]
    fn reward_candle_sample_builder_uses_first_touch_and_outcome_label() {
        let inputs = vec![
            candle_input("token-yes", 54, 0),
            candle_input("token-yes", 80, 1),
            candle_input("token-yes", 82, 2),
            candle_input("token-yes", 72, 3),
        ];

        let samples = build_high_probability_samples_from_reward_candles(&inputs);

        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].price_bucket, "0.80-0.85");
        assert_eq!(samples[0].outcome, HighProbabilitySampleOutcome::Win);
        assert_eq!(samples[0].settlement_pnl, Some(Decimal::new(20, 2)));
        assert_eq!(samples[0].max_drawdown_cents, Some(Decimal::new(8, 0)));
        assert_eq!(samples[1].price_bucket, "0.70-0.75");
    }

    #[test]
    fn research_report_summarizes_samples_and_buckets() {
        let config = HighProbabilityConfig {
            min_bucket_samples: 2,
            min_confidence: Decimal::new(50, 2),
            ..HighProbabilityConfig::default()
        };
        let samples = vec![
            sample(HighProbabilitySampleOutcome::Win, 80, 2),
            sample(HighProbabilitySampleOutcome::Loss, 81, 20),
            sample(HighProbabilitySampleOutcome::Unknown, 82, 0),
        ];
        let bucket_stats = vec![
            HighProbabilityBucketStats {
                id: 1,
                model_version: config.model_version.clone(),
                bucket_key: "a".to_string(),
                bucket_dimensions: json!({}),
                sample_count: 2,
                win_count: 1,
                win_rate: Decimal::new(50, 2),
                fair_probability: Decimal::new(50, 2),
                confidence_low: None,
                confidence_high: None,
                expected_pnl: Some(Decimal::new(5, 2)),
                avg_max_drawdown_cents: None,
                break_70_rate: Some(Decimal::new(25, 2)),
                break_60_rate: None,
                break_50_rate: None,
                avg_hold_seconds: None,
                recommended_max_entry_price: Some(Decimal::new(44, 2)),
                computed_at: OffsetDateTime::UNIX_EPOCH,
            },
            HighProbabilityBucketStats {
                id: 2,
                model_version: config.model_version.clone(),
                bucket_key: "b".to_string(),
                bucket_dimensions: json!({}),
                sample_count: 1,
                win_count: 1,
                win_rate: Decimal::ONE,
                fair_probability: Decimal::new(66, 2),
                confidence_low: None,
                confidence_high: None,
                expected_pnl: Some(Decimal::new(-10, 2)),
                avg_max_drawdown_cents: None,
                break_70_rate: Some(Decimal::ZERO),
                break_60_rate: None,
                break_50_rate: None,
                avg_hold_seconds: None,
                recommended_max_entry_price: Some(Decimal::new(60, 2)),
                computed_at: OffsetDateTime::UNIX_EPOCH,
            },
        ];

        let report = build_high_probability_research_report(&config, &samples, &bucket_stats, 100);

        assert_eq!(report.samples_scanned, 3);
        assert_eq!(report.settled_samples, 2);
        assert_eq!(report.win_samples, 1);
        assert_eq!(report.loss_samples, 1);
        assert_eq!(report.unknown_samples, 1);
        assert_eq!(report.bucket_count, 2);
        assert_eq!(report.qualified_bucket_count, 1);
        assert_eq!(report.positive_expected_pnl_bucket_count, 1);
        assert_eq!(
            report.weighted_win_rate,
            Some(Decimal::from(2u64) / Decimal::from(3u64))
        );
        assert_eq!(report.weighted_expected_pnl, Some(Decimal::ZERO));
        assert_eq!(
            report
                .best_bucket
                .as_ref()
                .map(|bucket| bucket.bucket_key.as_str()),
            Some("a")
        );
        assert!(
            report
                .notes
                .contains(&"contains_unsettled_or_voided_samples".to_string())
        );
    }

    #[test]
    fn backtest_report_uses_earlier_samples_for_bucket_training() {
        let config = HighProbabilityConfig {
            min_bucket_samples: 2,
            min_confidence: Decimal::new(50, 2),
            min_required_edge: Decimal::new(1, 2),
            fee_buffer: Decimal::ZERO,
            default_risk_margin: Decimal::ZERO,
            ..HighProbabilityConfig::default()
        };
        let samples = vec![
            timed_sample(HighProbabilitySampleOutcome::Win, 55, 0),
            timed_sample(HighProbabilitySampleOutcome::Win, 55, 1),
            timed_sample(HighProbabilitySampleOutcome::Win, 55, 2),
            timed_sample(HighProbabilitySampleOutcome::Loss, 55, 3),
        ];

        let report = build_high_probability_backtest_report(&config, &samples, 100);

        assert_eq!(report.train_sample_count, 2);
        assert_eq!(report.test_sample_count, 2);
        assert_eq!(report.candidate_count, 2);
        assert_eq!(report.trade_count, 2);
        assert_eq!(report.win_trades, 1);
        assert_eq!(report.loss_trades, 1);
        assert_eq!(report.win_rate, Some(Decimal::new(50, 2)));
        assert_eq!(report.total_pnl, Decimal::new(-10, 2));
        assert_eq!(report.total_entry_cost, Decimal::new(110, 2));
        assert_eq!(report.max_drawdown, Decimal::new(55, 2));
        assert_eq!(report.exit_rule_reports.len(), 5);
        assert!(
            report.exit_rule_reports.iter().any(|rule| {
                rule.rule_key == "take_profit_90"
                    && rule
                        .notes
                        .contains(&"missing_path_features_fallback_to_settlement".to_string())
            })
        );
        assert!(report.notes.is_empty());
    }

    #[test]
    fn backtest_result_keeps_trade_audit_rows() {
        let config = HighProbabilityConfig {
            min_bucket_samples: 2,
            min_confidence: Decimal::new(50, 2),
            min_required_edge: Decimal::new(1, 2),
            fee_buffer: Decimal::ZERO,
            default_risk_margin: Decimal::ZERO,
            ..HighProbabilityConfig::default()
        };
        let mut samples = vec![
            timed_sample(HighProbabilitySampleOutcome::Win, 55, 0),
            timed_sample(HighProbabilitySampleOutcome::Win, 55, 1),
            timed_sample(HighProbabilitySampleOutcome::Win, 55, 2),
            timed_sample(HighProbabilitySampleOutcome::Loss, 55, 3),
        ];
        for (index, sample) in samples.iter_mut().enumerate() {
            sample.id = i64::try_from(index + 1).unwrap_or(i64::MAX);
        }

        let result = build_high_probability_backtest_result(&config, &samples, 100);

        assert_eq!(result.run.report.trade_count, 2);
        assert_eq!(result.trades.len(), 2);
        assert_eq!(result.trades[0].sample_id, 3);
        assert_eq!(result.trades[0].settlement_pnl, Decimal::new(45, 2));
        assert_eq!(result.trades[0].cumulative_pnl, Decimal::new(45, 2));
        assert_eq!(result.trades[0].drawdown, Decimal::ZERO);
        assert_eq!(result.trades[1].sample_id, 4);
        assert_eq!(result.trades[1].settlement_pnl, Decimal::new(-55, 2));
        assert_eq!(result.trades[1].cumulative_pnl, Decimal::new(-10, 2));
        assert_eq!(result.trades[1].drawdown, Decimal::new(55, 2));
    }

    #[test]
    fn backtest_exit_rules_use_path_features_when_available() {
        let mut winning_sample = sample(HighProbabilitySampleOutcome::Win, 80, 0);
        winning_sample.path_features = json!({
            "min_future_close": "0.60",
            "max_future_close": "0.95",
        });
        let mut losing_sample = sample(HighProbabilitySampleOutcome::Loss, 80, 0);
        losing_sample.path_features = json!({
            "min_future_close": "0.60",
            "max_future_close": "0.82",
        });

        let (take_profit_95_pnl, take_profit_missing) =
            exit_rule_pnl("take_profit_95", &winning_sample, Decimal::new(20, 2));
        let (stop_loss_70_pnl, stop_loss_missing) =
            exit_rule_pnl("stop_loss_70", &losing_sample, Decimal::new(-80, 2));
        let (settlement_pnl, settlement_missing) =
            exit_rule_pnl("settlement", &losing_sample, Decimal::new(-80, 2));

        assert_eq!(take_profit_95_pnl, Decimal::new(15, 2));
        assert_eq!(stop_loss_70_pnl, Decimal::new(-10, 2));
        assert_eq!(settlement_pnl, Decimal::new(-80, 2));
        assert!(!take_profit_missing);
        assert!(!stop_loss_missing);
        assert!(!settlement_missing);
    }

    #[test]
    fn observe_candidates_allow_when_bucket_edge_and_orderbook_pass() {
        let config = HighProbabilityConfig {
            min_bucket_samples: 2,
            min_confidence: Decimal::new(50, 2),
            min_required_edge: Decimal::new(2, 2),
            fee_buffer: Decimal::ZERO,
            default_risk_margin: Decimal::ZERO,
            max_spread_cents: Decimal::new(3, 0),
            min_depth_usd: Decimal::new(10, 0),
            max_single_trade_usd: Decimal::new(100, 0),
            conservative_kelly_multiplier: Decimal::new(10, 2),
            ..HighProbabilityConfig::default()
        };
        let mut train_a = timed_sample(HighProbabilitySampleOutcome::Win, 55, 0);
        train_a.time_to_resolution_bucket = Some("lte_1d".to_string());
        let mut train_b = timed_sample(HighProbabilitySampleOutcome::Win, 55, 1);
        train_b.time_to_resolution_bucket = Some("lte_1d".to_string());
        let bucket_stats = build_high_probability_bucket_stats(&config, &[train_a, train_b]);
        let candidates = vec![HighProbabilityObserveCandidate {
            condition_id: "condition".to_string(),
            token_id: "token-yes".to_string(),
            outcome: "yes".to_string(),
            observed_at: OffsetDateTime::UNIX_EPOCH,
            reference_price: Decimal::new(55, 2),
            reference_spread_cents: Decimal::ONE,
            market_type: "sports".to_string(),
            liquidity_usd: Some(Decimal::new(20_000, 0)),
            end_at: Some(OffsetDateTime::UNIX_EPOCH + time::Duration::hours(12)),
            risk_tags: Vec::new(),
        }];
        let quotes = vec![HighProbabilityOrderbookQuote {
            token_id: "token-yes".to_string(),
            best_bid: Some(Decimal::new(54, 2)),
            best_ask: Some(Decimal::new(55, 2)),
            ask_depth_usd: Some(Decimal::new(50, 0)),
            confirmed_at_ms: Some(1),
        }];

        let observations =
            build_high_probability_observations(&config, &candidates, &quotes, &bucket_stats);

        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].decision, HighProbabilityDecision::Allow);
        assert_eq!(observations[0].fair_probability, Some(Decimal::new(75, 2)));
        assert_eq!(observations[0].net_edge, Some(Decimal::new(20, 2)));
        assert!(
            observations[0]
                .reasons
                .contains(&"edge_gate_passed".to_string())
        );
        assert!(observations[0].recommended_size_usd.is_some());
    }

    #[test]
    fn observe_candidates_skip_when_orderbook_quote_missing() {
        let config = HighProbabilityConfig::default();
        let candidates = vec![HighProbabilityObserveCandidate {
            condition_id: "condition".to_string(),
            token_id: "token-yes".to_string(),
            outcome: "yes".to_string(),
            observed_at: OffsetDateTime::UNIX_EPOCH,
            reference_price: Decimal::new(80, 2),
            reference_spread_cents: Decimal::ONE,
            market_type: "sports".to_string(),
            liquidity_usd: None,
            end_at: None,
            risk_tags: Vec::new(),
        }];

        let observations = build_high_probability_observations(&config, &candidates, &[], &[]);

        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].decision, HighProbabilityDecision::Skip);
        assert_eq!(observations[0].executable_price, Decimal::new(80, 2));
        assert!(
            observations[0]
                .reasons
                .contains(&"orderbook_missing".to_string())
        );
    }
}
