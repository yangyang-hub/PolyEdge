#[cfg(test)]
mod fair_value_tests {
    use super::*;

    const NOW_SECONDS: i64 = 1_700_000_000;

    fn now() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(NOW_SECONDS).unwrap()
    }

    fn now_ms() -> i64 {
        NOW_SECONDS * 1_000
    }

    fn fv_config() -> HighProbabilityConfig {
        HighProbabilityConfig {
            fair_value_enabled: true,
            min_confidence: Decimal::new(60, 2),
            min_bucket_samples: 30,
            max_spread_cents: Decimal::new(3, 0),
            min_depth_usd: Decimal::new(50, 0),
            default_risk_margin: Decimal::new(2, 2),
            fair_value_max_uncertainty_cents: Decimal::new(8, 0),
            fair_value_target_sample_count: 200,
            fair_value_stale_book_ms: 60_000,
            fair_value_market_weight: Decimal::new(25, 2),
            fair_value_base_rate_weight: Decimal::new(75, 2),
            fair_value_ttl_sec: 300,
            excluded_risk_tags: vec!["ambiguous_rules".to_string()],
            ..HighProbabilityConfig::default()
        }
    }

    /// Build a bucket with explicit 5-dimension identity and a ±2c interval so
    /// the model error term stays inside the default uncertainty ceiling.
    fn bucket(
        market_type: &str,
        price_bucket: &str,
        time: &str,
        liquidity: &str,
        spread: &str,
        samples: u64,
        fair: Decimal,
    ) -> HighProbabilityBucketStats {
        HighProbabilityBucketStats {
            id: 0,
            model_version: "high_probability_bucket_v1".to_string(),
            bucket_key: format!(
                "type={market_type}|price={price_bucket}|time={time}|liquidity={liquidity}|spread={spread}"
            ),
            bucket_dimensions: json!({
                "market_type": market_type,
                "price_bucket": price_bucket,
                "time_to_resolution_bucket": time,
                "liquidity_bucket": liquidity,
                "spread_bucket": spread,
            }),
            sample_count: samples,
            win_count: 0,
            win_rate: Decimal::ZERO,
            fair_probability: fair,
            confidence_low: Some((fair - Decimal::new(2, 2)).max(Decimal::ZERO)),
            confidence_high: Some((fair + Decimal::new(2, 2)).min(Decimal::ONE)),
            expected_pnl: None,
            avg_max_drawdown_cents: None,
            break_70_rate: None,
            break_60_rate: None,
            break_50_rate: None,
            avg_hold_seconds: None,
            recommended_max_entry_price: None,
            computed_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    fn input(
        condition: &str,
        token: &str,
        outcome: &str,
        best_bid: Decimal,
        best_ask: Decimal,
    ) -> FairValuePricingInput {
        FairValuePricingInput {
            condition_id: condition.to_string(),
            token_id: token.to_string(),
            outcome: outcome.to_string(),
            reference_price: best_ask,
            reference_spread_cents: (best_ask - best_bid).max(Decimal::ZERO) * Decimal::from(100u64),
            best_bid: Some(best_bid),
            best_ask: Some(best_ask),
            ask_depth_usd: Some(Decimal::new(100, 0)),
            market_type: "sports".to_string(),
            liquidity_usd: Some(Decimal::new(20_000, 0)),
            end_at: Some(now() + time::Duration::hours(12)),
            observed_at: now(),
            risk_tags: Vec::new(),
            confirmed_at_ms: Some(now_ms()),
        }
    }

    #[test]
    fn global_prior_is_sample_weighted_mean() {
        let buckets = vec![
            bucket("sports", "0.80-0.85", "lte_1d", "high", "tight", 100, Decimal::new(80, 2)),
            bucket("sports", "0.85-0.90", "lte_1d", "high", "tight", 300, Decimal::new(90, 2)),
        ];
        // (0.80*100 + 0.90*300) / 400 = 0.875
        let prior = fair_value_global_prior(&buckets);
        assert_eq!(prior, Some(Decimal::new(875, 3)));
    }

    #[test]
    fn blend_normalizes_weights() {
        // Weights do not sum to 1.0; the blend normalizes internally.
        let mid = blend_fair_value_mid(
            Decimal::new(50, 2),
            Decimal::new(90, 2),
            Decimal::new(1, 0),
            Decimal::new(3, 0),
        );
        // (1*0.50 + 3*0.90) / 4 = 0.80
        assert_eq!(mid, Decimal::new(80, 2));
    }

    #[test]
    fn blend_falls_back_to_market_when_weights_zero() {
        let mid = blend_fair_value_mid(
            Decimal::new(40, 2),
            Decimal::new(90, 2),
            Decimal::ZERO,
            Decimal::ZERO,
        );
        assert_eq!(mid, Decimal::new(40, 2));
    }

    #[test]
    fn resolve_fallback_chain_widens_coarseness() {
        let index = FairValueBucketIndex::new(vec![
            bucket("sports", "0.80-0.85", "lte_1d", "high", "tight", 200, Decimal::new(85, 2)),
            // A coarser match (drops spread/liquidity/time) at the same price.
            bucket("sports", "0.80-0.85", "lte_7d", "medium", "wide", 50, Decimal::new(70, 2)),
        ]);

        // Exact 5-dim match → level 0, zero margin.
        let exact = input("c", "t", "yes", Decimal::new(79, 2), Decimal::new(80, 2));
        let resolved = resolve_fair_value_bucket(&exact, &index, 30).expect("exact match");
        assert_eq!(resolved.fallback_level, 0);
        assert_eq!(resolved.coarseness_margin_cents, Decimal::ZERO);
        assert_eq!(resolved.bucket.fair_probability, Decimal::new(85, 2));

        // An input that only matches on market_type + price → drops to level 3.
        let mut coarse = exact.clone();
        coarse.end_at = Some(now() + time::Duration::days(10)); // → gt_7d
        coarse.liquidity_usd = Some(Decimal::new(500, 0)); // → thin
        let resolved = resolve_fair_value_bucket(&coarse, &index, 30).expect("coarse match");
        assert_eq!(resolved.fallback_level, 3);
        assert!(resolved.coarseness_margin_cents > Decimal::ZERO);
    }

    #[test]
    fn resolve_falls_back_to_global_prior_then_none() {
        // No exact/coarse match, but buckets exist → global prior (level 5).
        let index = FairValueBucketIndex::new(vec![
            bucket("politics", "0.90-0.95", "gt_7d", "high", "tight", 100, Decimal::new(92, 2)),
        ]);
        let target = input("c", "t", "yes", Decimal::new(79, 2), Decimal::new(80, 2));
        let resolved = resolve_fair_value_bucket(&target, &index, 30).expect("global prior");
        assert_eq!(resolved.fallback_level, 5);
        assert_eq!(resolved.bucket.fair_probability, Decimal::new(92, 2));

        // No buckets at all → None.
        let empty = FairValueBucketIndex::new(Vec::new());
        assert!(resolve_fair_value_bucket(&target, &empty, 30).is_none());
    }

    #[test]
    fn uncertainty_is_monotonic_and_clamped() {
        let config = fv_config();
        let resolved = bucket("sports", "0.80-0.85", "lte_1d", "high", "tight", 200, Decimal::new(85, 2));
        let resolution = FairValueBucketResolution {
            bucket: resolved.clone(),
            fallback_level: 0,
            coarseness_margin_cents: Decimal::ZERO,
        };
        let tight = fair_value_uncertainty_cents(&config, Some(&resolution), Decimal::ONE, false, &[]);
        let wide = fair_value_uncertainty_cents(&config, Some(&resolution), Decimal::new(6, 0), false, &[]);
        let stale = fair_value_uncertainty_cents(&config, Some(&resolution), Decimal::ONE, true, &[]);
        assert!(wide > tight);
        assert!(stale > tight);
        // No resolution at all → clamps to the ceiling.
        let none = fair_value_uncertainty_cents(&config, None, Decimal::ONE, false, &[]);
        assert_eq!(none, Decimal::from(100u64));
    }

    #[test]
    fn confidence_is_bounded_and_increases_with_samples() {
        let config = fv_config();
        let mk = |samples: u64| {
            let bucket = bucket("sports", "0.80-0.85", "lte_1d", "high", "tight", samples, Decimal::new(85, 2));
            FairValueBucketResolution { bucket, fallback_level: 0, coarseness_margin_cents: Decimal::ZERO }
        };
        let low = fair_value_confidence(&config, Some(&mk(10)), Decimal::ONE, false, &[]);
        let high = fair_value_confidence(&config, Some(&mk(500)), Decimal::ONE, false, &[]);
        assert!(high > low);
        assert!(high <= Decimal::ONE);
        assert!(low >= Decimal::ZERO);
    }

    #[test]
    fn estimate_is_none_when_price_out_of_range() {
        let config = fv_config();
        let index = FairValueBucketIndex::new(vec![bucket(
            "sports", "0.80-0.85", "lte_1d", "high", "tight", 200, Decimal::new(85, 2),
        )]);
        // Price 0.40 is below the 0.55 research floor.
        let out_of_range = input("c", "t", "yes", Decimal::new(39, 2), Decimal::new(40, 2));
        assert!(build_fair_value_estimate(&config, &out_of_range, &index, now()).is_none());
    }

    #[test]
    fn estimate_produces_ordered_band_for_yes_side() {
        let config = fv_config();
        let index = FairValueBucketIndex::new(vec![bucket(
            "sports", "0.80-0.85", "lte_1d", "high", "tight", 200, Decimal::new(90, 2),
        )]);
        let estimate = build_fair_value_estimate(
            &config,
            &input("c", "t", "yes", Decimal::new(79, 2), Decimal::new(80, 2)),
            &index,
            now(),
        )
        .expect("estimate");

        assert_eq!(estimate.side_used, FairValueSide::Yes);
        assert!(estimate.fair_yes_low <= estimate.fair_yes_mid);
        assert!(estimate.fair_yes_mid <= estimate.fair_yes_high);
        assert!(estimate.fair_yes_low >= Decimal::ZERO);
        assert!(estimate.fair_yes_high <= Decimal::ONE);
        // Base rate dominates the blend, so mid should lean toward the 0.90 bucket.
        assert!(estimate.fair_yes_mid > Decimal::new(80, 2));
        assert_eq!(estimate.expires_at, now() + time::Duration::seconds(300));
    }

    #[test]
    fn estimate_complements_no_side_to_yes_scale() {
        let config = fv_config();
        let index = FairValueBucketIndex::new(vec![bucket(
            "sports", "0.80-0.85", "lte_1d", "high", "tight", 200, Decimal::new(90, 2),
        )]);
        // A NO token priced at 0.80 whose bucket fair (P[NO wins]) is 0.90 →
        // fair_yes base rate = 1 - 0.90 = 0.10.
        let estimate = build_fair_value_estimate(
            &config,
            &input("c", "t", "no", Decimal::new(79, 2), Decimal::new(80, 2)),
            &index,
            now(),
        )
        .expect("estimate");

        assert_eq!(estimate.side_used, FairValueSide::NoComplement);
        assert_eq!(estimate.base_rate, Decimal::new(10, 2));
    }

    #[test]
    fn live_eligible_passes_and_reports_reason() {
        let config = fv_config();
        let index = FairValueBucketIndex::new(vec![bucket(
            "sports", "0.80-0.85", "lte_1d", "high", "tight", 200, Decimal::new(90, 2),
        )]);
        let estimate = build_fair_value_estimate(
            &config,
            &input("c", "t", "yes", Decimal::new(79, 2), Decimal::new(80, 2)),
            &index,
            now(),
        )
        .expect("estimate");
        assert!(estimate.live_eligible);
        assert_eq!(estimate.reason_codes, vec!["eligible".to_string()]);
    }

    #[test]
    fn live_eligible_flags_excluded_risk_tag() {
        let config = fv_config();
        let index = FairValueBucketIndex::new(vec![bucket(
            "sports", "0.80-0.85", "lte_1d", "high", "tight", 200, Decimal::new(90, 2),
        )]);
        let mut input = input("c", "t", "yes", Decimal::new(79, 2), Decimal::new(80, 2));
        input.risk_tags = vec!["ambiguous_rules".to_string()];
        let estimate = build_fair_value_estimate(&config, &input, &index, now()).expect("estimate");
        assert!(!estimate.live_eligible);
        assert!(estimate.reason_codes.iter().any(|reason| reason == "excluded_risk_tag:ambiguous_rules"));
    }

    #[test]
    fn input_hash_is_deterministic_and_order_insensitive() {
        let hash_a = fair_value_input_hash(
            "high_probability_bucket_v1",
            "c1",
            "type=sports|price=0.80-0.85",
            FairValueSide::Yes,
            Decimal::new(80, 2),
            200,
            &["long_horizon".to_string(), "single_source_news".to_string()],
        );
        let hash_b = fair_value_input_hash(
            "high_probability_bucket_v1",
            "c1",
            "type=sports|price=0.80-0.85",
            FairValueSide::Yes,
            Decimal::new(80, 2),
            200,
            &["single_source_news".to_string(), "long_horizon".to_string()],
        );
        assert_eq!(hash_a, hash_b);

        // Different price → different hash.
        let hash_c = fair_value_input_hash(
            "high_probability_bucket_v1",
            "c1",
            "type=sports|price=0.80-0.85",
            FairValueSide::Yes,
            Decimal::new(81, 2),
            200,
            &["long_horizon".to_string()],
        );
        assert_ne!(hash_a, hash_c);
    }

    #[test]
    fn build_estimates_prefers_yes_and_groups_per_condition() {
        let config = fv_config();
        let index_buckets = vec![bucket(
            "sports", "0.80-0.85", "lte_1d", "high", "tight", 200, Decimal::new(90, 2),
        )];
        let mut by_condition = BTreeMap::new();
        by_condition.insert(
            "c1".to_string(),
            vec![
                input("c1", "yes-token", "yes", Decimal::new(79, 2), Decimal::new(80, 2)),
                input("c1", "no-token", "no", Decimal::new(19, 2), Decimal::new(20, 2)),
            ],
        );
        let estimates = build_fair_value_estimates(&config, &by_condition, &index_buckets, now());
        assert_eq!(estimates.len(), 1);
        assert_eq!(estimates[0].side_used, FairValueSide::Yes);
        assert_eq!(estimates[0].token_id, "yes-token");
    }
}
