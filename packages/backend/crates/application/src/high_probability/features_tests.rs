#[cfg(test)]
mod features_tests {
    use super::*;

    fn candle(token: &str, close: i64, offset_minutes: i64) -> HighProbabilityRewardCandleSampleInput {
        HighProbabilityRewardCandleSampleInput {
            condition_id: "condition".to_string(),
            token_id: token.to_string(),
            outcome: "yes".to_string(),
            bucket_start: OffsetDateTime::UNIX_EPOCH + time::Duration::minutes(offset_minutes),
            close: Decimal::new(close, 2),
            spread_cents_close: Decimal::ONE,
            market_type: "sports".to_string(),
            liquidity_usd: Some(Decimal::new(20_000, 0)),
            resolved_at: Some(OffsetDateTime::UNIX_EPOCH + time::Duration::days(1)),
            outcome_status: HighProbabilityMarketOutcomeStatus::Unresolved,
            winning_token_id: None,
            risk_tags: Vec::new(),
        }
    }

    #[test]
    fn risk_features_map_taxonomy_and_count() {
        let features = compute_risk_features(&[
            "ambiguous_rules".to_string(),
            "  Long_Horizon".to_string(),
            "unknown_tag".to_string(),
        ]);
        assert!(features.ambiguous_rules);
        assert!(features.long_horizon);
        assert!(!features.source_conflict);
        assert_eq!(features.active_count(), 2);
    }

    #[test]
    fn path_features_empty_window_returns_defaults() {
        let features = compute_price_path_features(&[], 300);
        assert!(features.monotonic_trend_score.is_none());
        assert!(features.max_run_up_cents.is_none());
    }

    #[test]
    fn path_features_measure_run_up_drawdown_and_crossings() {
        // 0.60 → 0.70 → 0.65 → 0.80  (5m candles, 12 per hour).
        let candles = [
            candle("t", 60, 0),
            candle("t", 70, 5),
            candle("t", 65, 10),
            candle("t", 80, 15),
        ];
        let features = compute_price_path_features(&candles, 300);
        // Run-up from first (0.60) to peak (0.80) = 20c.
        assert_eq!(features.max_run_up_cents, Some(Decimal::new(20, 0)));
        // Largest drawdown: 0.70 → 0.65 = 5c.
        assert_eq!(features.largest_prior_drawdown_cents, Some(Decimal::new(5, 0)));
        // Three bucket transitions (0.55-0.60 → 0.65-0.70 → 0.60-0.65 → 0.75-0.80).
        assert_eq!(features.prior_bucket_crossings, Some(3));
        // More up steps than down → positive monotonic trend.
        assert!(features.monotonic_trend_score.is_some_and(|score| score > Decimal::ZERO));
        // Closes at or above 0.70: 0.70 and 0.80 → 2 candles * 300s.
        assert_eq!(features.time_above_70_sec, Some(600));
    }

    #[test]
    fn liquidity_and_time_features_are_point_in_time() {
        let quote = HighProbabilityOrderbookQuote {
            token_id: "t".to_string(),
            best_bid: Some(Decimal::new(79, 2)),
            best_ask: Some(Decimal::new(80, 2)),
            ask_depth_usd: Some(Decimal::new(120, 0)),
            confirmed_at_ms: Some(1_700_000_000_000),
        };
        let liquidity = compute_liquidity_features(Decimal::ONE, Some(&quote), Some(Decimal::new(20_000, 0)), 1_700_000_030_000);
        assert_eq!(liquidity.spread_cents, Some(Decimal::ONE));
        assert_eq!(liquidity.book_fresh_ms, Some(30_000));
        assert_eq!(liquidity.liquidity_bucket.as_deref(), Some("high"));

        let time = compute_time_features(
            OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
            Some(OffsetDateTime::from_unix_timestamp(1_700_010_000).unwrap()),
            None,
        );
        assert_eq!(time.time_to_resolution_bucket.as_deref(), Some("lte_6h"));
        assert!(time.market_age_bucket.is_none());
    }

    #[test]
    fn feature_vector_is_versioned_and_serialized() {
        let vector = compute_high_probability_feature_vector(
            &[candle("t", 80, 0)],
            300,
            Decimal::ONE,
            None,
            Some(Decimal::new(20_000, 0)),
            OffsetDateTime::UNIX_EPOCH,
            None,
            None,
            &["long_horizon".to_string()],
            0,
        );
        assert!(!vector.version.is_empty());
        assert!(vector.risk.long_horizon);
        let json = feature_vector_to_json(&vector);
        assert!(json.get("version").is_some());
    }
}
