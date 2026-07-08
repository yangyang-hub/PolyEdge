#[test]
fn selection_score_prefers_maker_quality_over_base_market_quality() {
    let now = OffsetDateTime::now_utc();
    let mut crowded = selection_test_plan("crowded", decimal("90"), now);
    crowded.opportunity_metrics = Some(selection_test_opportunity_metrics(
        decimal("35"),
        decimal("95"),
        decimal("10"),
        decimal("20"),
        decimal("35"),
        decimal("2500"),
    ));
    crowded.fair_value = Some(selection_test_fair_value(decimal("0.25"), true, now));

    let mut maker_quality = selection_test_plan("maker_quality", decimal("60"), now);
    maker_quality.opportunity_metrics = Some(selection_test_opportunity_metrics(
        decimal("90"),
        decimal("85"),
        decimal("92"),
        decimal("88"),
        decimal("90"),
        decimal("200"),
    ));
    maker_quality.fair_value = Some(selection_test_fair_value(decimal("2.5"), true, now));

    let mut plans = vec![crowded, maker_quality];
    apply_reward_market_selection_to_quote_plans(&mut plans);

    assert_eq!(plans[0].condition_id, "maker_quality");
    assert!(plans[0].selection_score > plans[1].selection_score);
    assert!(plans[0].selection_metrics.is_some());
}

fn selection_test_plan(
    condition_id: &str,
    score: Decimal,
    now: OffsetDateTime,
) -> RewardQuotePlan {
    RewardQuotePlan {
        condition_id: condition_id.to_string(),
        market_slug: condition_id.to_string(),
        question: "Selection test?".to_string(),
        score,
        selection_score: Decimal::ZERO,
        eligible: true,
        pre_ai_eligible: true,
        quote_readiness: RewardQuoteReadiness::ReadyToQuote,
        reason: "eligible".to_string(),
        strategy_bucket: RewardStrategyBucket::Standard,
        strategy_profile: RewardStrategyProfile::Standard,
        quote_mode: RewardPlanQuoteMode::SingleYes,
        recommended_quote_mode: Some(RewardPlanQuoteMode::SingleYes),
        book_metrics: None,
        opportunity_metrics: None,
        selection_metrics: None,
        fair_value: None,
        ai_advisory: None,
        info_risk: None,
        event_window: None,
        midpoint: Some(decimal("0.5")),
        live_skip_until: None,
        live_skip_reason: None,
        first_quote_observed_at: None,
        ai_advisory_pending_since: None,
        info_risk_pending_since: None,
        total_daily_rate: decimal("10"),
        rewards_max_spread: decimal("3"),
        rewards_min_size: decimal("20"),
        orderbook_token_ids: vec!["yes".to_string(), "no".to_string()],
        legs: vec![RewardQuoteLeg {
            token_id: "yes".to_string(),
            outcome: "Yes".to_string(),
            side: RewardOrderSide::Buy,
            price: decimal("0.49"),
            size: decimal("20"),
            notional_usd: decimal("9.8"),
        }],
        updated_at: now,
    }
}

fn selection_test_opportunity_metrics(
    reward_score: Decimal,
    competition_score: Decimal,
    exit_score: Decimal,
    stability_score: Decimal,
    opportunity_score: Decimal,
    account_allocation_bps: Decimal,
) -> RewardOpportunityMetrics {
    RewardOpportunityMetrics {
        planned_notional_usd: decimal("10"),
        probe_notional_usd: decimal("10"),
        qualified_competition_usd: decimal("100"),
        competition_share_bps: decimal("100"),
        competition_multiple: decimal("1"),
        estimated_reward_per_100_usd_day: decimal("1"),
        competition_density: decimal("0.01"),
        account_effective_available_usd: decimal("1000"),
        open_buy_notional_usd: Decimal::ZERO,
        open_buy_notional_usd_after_plan: decimal("10"),
        condition_buy_notional_usd_after_plan: decimal("10"),
        account_allocation_bps,
        market_allocation_bps: account_allocation_bps,
        exit_depth_usd: decimal("100"),
        exit_slippage_cents: Some(decimal("0.5")),
        bad_fill_recovery_days: Some(decimal("1")),
        midpoint_range_cents: Some(decimal("1")),
        top_of_book_flip_count: Some(1),
        sample_count: 100,
        reward_score,
        competition_score,
        exit_score,
        stability_score,
        opportunity_score,
        score_adjustment: Decimal::ZERO,
        warnings: Vec::new(),
    }
}

fn selection_test_fair_value(
    effective_edge_cents: Decimal,
    passed: bool,
    now: OffsetDateTime,
) -> RewardFairValueDecision {
    RewardFairValueDecision {
        estimate: RewardFairValueEstimate {
            condition_id: "selection".to_string(),
            source: "test".to_string(),
            fair_yes: decimal("0.5"),
            fair_no: decimal("0.5"),
            market_midpoint_yes: Some(decimal("0.5")),
            confidence: decimal("0.9"),
            uncertainty_cents: decimal("0.5"),
            midpoint_deviation_cents: Some(Decimal::ZERO),
            sample_count: 10,
            components: Vec::new(),
            do_not_quote_reason: None,
            observed_at: now,
            expires_at: now + TimeDuration::minutes(5),
        },
        edges: vec![RewardQuoteEdge {
            token_id: "yes".to_string(),
            outcome: "Yes".to_string(),
            side: RewardOrderSide::Buy,
            quote_price: decimal("0.49"),
            fair_price: decimal("0.5"),
            raw_edge_cents: effective_edge_cents,
            expected_reward_rebate_cents: Decimal::ZERO,
            uncertainty_cents: Decimal::ZERO,
            effective_edge_cents,
            min_raw_edge_cents: Decimal::ZERO,
            min_effective_edge_cents: Decimal::ZERO,
            passed,
            reason: "test".to_string(),
        }],
        expected_reward_rebate_cents: Decimal::ZERO,
        passed,
        reason: "test".to_string(),
    }
}
