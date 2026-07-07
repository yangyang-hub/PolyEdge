fn market_maker_config(strategy_mode: RewardStrategyMode) -> RewardBotConfig {
    RewardBotConfig {
        strategy_mode,
        market_maker_enabled: strategy_mode.market_maker_enabled(),
        market_maker_min_total_ev_cents: decimal("0.5"),
        market_maker_min_pricing_edge_cents: decimal("0.5"),
        market_maker_max_reward_subsidized_negative_edge_cents: decimal("0.5"),
        market_maker_min_fair_value_confidence: decimal("0.60"),
        market_maker_max_uncertainty_cents: decimal("8"),
        market_maker_max_condition_inventory_usd: decimal("1000"),
        market_maker_max_category_inventory_usd: decimal("1000"),
        market_maker_max_global_inventory_usd: decimal("1000"),
        opportunity_max_entry_exit_slippage_cents: Decimal::ZERO,
        opportunity_max_midpoint_range_cents: Decimal::ZERO,
        opportunity_max_top_of_book_flip_count: 10,
        ..RewardBotConfig::default()
    }
    .normalized()
}

fn market_maker_test_market() -> RewardMarket {
    RewardMarket {
        condition_id: "cond_mm".to_string(),
        question: "Market maker test?".to_string(),
        market_slug: "market-maker-test".to_string(),
        event_slug: "market-maker-event".to_string(),
        category: "politics".to_string(),
        image: String::new(),
        rewards_max_spread: decimal("8"),
        rewards_min_size: decimal("10"),
        total_daily_rate: decimal("20"),
        liquidity_usd: decimal("1000"),
        volume_24h_usd: decimal("1000"),
        market_spread_cents: decimal("1"),
        end_at: Some(OffsetDateTime::now_utc() + TimeDuration::days(30)),
        ambiguity_level: "low".to_string(),
        market_synced_at: Some(OffsetDateTime::now_utc()),
        tokens: vec![
            RewardToken {
                token_id: "yes_mm".to_string(),
                outcome: "Yes".to_string(),
                price: None,
            },
            RewardToken {
                token_id: "no_mm".to_string(),
                outcome: "No".to_string(),
                price: None,
            },
        ],
        active: true,
        updated_at: OffsetDateTime::now_utc(),
    }
}

fn market_maker_test_plan(price: Decimal) -> RewardQuotePlan {
    let now = OffsetDateTime::now_utc();
    RewardQuotePlan {
        condition_id: "cond_mm".to_string(),
        market_slug: "market-maker-test".to_string(),
        question: "Market maker test?".to_string(),
        score: decimal("10"),
        eligible: true,
        pre_ai_eligible: true,
        quote_readiness: RewardQuoteReadiness::ReadyToQuote,
        reason: "eligible".to_string(),
        strategy_bucket: RewardStrategyBucket::Standard,
        strategy_profile: RewardStrategyProfile::Standard,
        quote_mode: RewardPlanQuoteMode::SingleYes,
        recommended_quote_mode: None,
        book_metrics: None,
        opportunity_metrics: Some(RewardOpportunityMetrics {
            planned_notional_usd: decimal("10"),
            probe_notional_usd: decimal("10"),
            qualified_competition_usd: Decimal::ZERO,
            competition_share_bps: Decimal::ZERO,
            competition_multiple: Decimal::ZERO,
            estimated_reward_per_100_usd_day: decimal("2"),
            competition_density: Decimal::ZERO,
            account_effective_available_usd: decimal("1000"),
            open_buy_notional_usd: Decimal::ZERO,
            open_buy_notional_usd_after_plan: Decimal::ZERO,
            condition_buy_notional_usd_after_plan: Decimal::ZERO,
            account_allocation_bps: Decimal::ZERO,
            market_allocation_bps: Decimal::ZERO,
            exit_depth_usd: decimal("100"),
            exit_slippage_cents: Some(Decimal::ZERO),
            bad_fill_recovery_days: Some(Decimal::ZERO),
            midpoint_range_cents: Some(Decimal::ZERO),
            top_of_book_flip_count: Some(0),
            sample_count: 30,
            reward_score: Decimal::ZERO,
            competition_score: Decimal::ZERO,
            exit_score: Decimal::ZERO,
            stability_score: Decimal::ZERO,
            opportunity_score: Decimal::ZERO,
            score_adjustment: Decimal::ZERO,
            warnings: Vec::new(),
        }),
        market_maker: None,
        low_competition_metrics: None,
        ai_advisory: None,
        info_risk: None,
        event_window: None,
        midpoint: Some(decimal("0.50")),
        live_skip_until: None,
        live_skip_reason: None,
        first_quote_observed_at: None,
        ai_advisory_pending_since: None,
        info_risk_pending_since: None,
        total_daily_rate: decimal("20"),
        rewards_max_spread: decimal("8"),
        rewards_min_size: decimal("10"),
        orderbook_token_ids: vec!["yes_mm".to_string(), "no_mm".to_string()],
        legs: vec![RewardQuoteLeg {
            token_id: "yes_mm".to_string(),
            outcome: "Yes".to_string(),
            side: RewardOrderSide::Buy,
            price,
            size: decimal("20"),
            notional_usd: (price * decimal("20")).round_dp(4),
        }],
        updated_at: now,
    }
}

fn market_maker_test_fair_value(now: OffsetDateTime) -> RewardMarketMakerFairValue {
    RewardMarketMakerFairValue {
        id: 42,
        condition_id: "cond_mm".to_string(),
        token_id: "yes_mm".to_string(),
        fair_yes_low: decimal("0.50"),
        fair_yes_mid: decimal("0.50"),
        fair_yes_high: decimal("0.50"),
        market_implied: decimal("0.50"),
        base_rate: decimal("0.50"),
        confidence: decimal("0.80"),
        uncertainty_cents: Decimal::ZERO,
        sample_count: 250,
        bucket_key: "bucket".to_string(),
        fallback_level: 0,
        model_version: "high_probability_bucket_v1".to_string(),
        input_hash: "hash".to_string(),
        reason_codes: Vec::new(),
        live_eligible: true,
        computed_at: now,
        expires_at: now + TimeDuration::minutes(5),
    }
}

#[test]
fn market_maker_default_mode_does_not_modify_quote_plans() {
    let mut plans = vec![market_maker_test_plan(decimal("0.49"))];
    plans[0].market_maker = Some(RewardMarketMakerPlanMetrics {
        strategy_mode: RewardStrategyMode::MarketMakerShadow,
        decision_status: RewardMarketMakerDecisionStatus::ShadowAllowed,
        best_total_ev_cents: Some(decimal("1")),
        best_pricing_edge_cents: Some(decimal("1")),
        best_reward_ev_cents: Some(Decimal::ZERO),
        fair_value: None,
        decisions: Vec::new(),
        reason_codes: Vec::new(),
        created_at: OffsetDateTime::now_utc(),
    });

    let decisions = apply_reward_market_maker_decisions_to_quote_plans(
        &mut plans,
        &[market_maker_test_market()],
        &[market_maker_test_fair_value(OffsetDateTime::now_utc())],
        &[],
        &[],
        &RewardBotConfig::default(),
        "trace",
        OffsetDateTime::now_utc(),
    );

    assert!(decisions.is_empty());
    assert!(plans[0].market_maker.is_none());
    assert_eq!(plans[0].legs[0].price, decimal("0.49"));
}

#[test]
fn market_maker_shadow_records_blocked_decision_without_mutating_plan() {
    let mut plans = vec![market_maker_test_plan(decimal("0.49"))];
    let decisions = apply_reward_market_maker_decisions_to_quote_plans(
        &mut plans,
        &[market_maker_test_market()],
        &[],
        &[],
        &[],
        &market_maker_config(RewardStrategyMode::MarketMakerShadow),
        "trace",
        OffsetDateTime::now_utc(),
    );

    assert_eq!(decisions.len(), 1);
    assert_eq!(
        decisions[0].decision_status,
        RewardMarketMakerDecisionStatus::ShadowBlocked
    );
    assert!(
        decisions[0]
            .reason_codes
            .contains(&"fair_value_unavailable".to_string())
    );
    assert_eq!(plans[0].legs[0].price, decimal("0.49"));
    assert_eq!(
        plans[0].market_maker.as_ref().map(|metrics| metrics.decision_status),
        Some(RewardMarketMakerDecisionStatus::ShadowBlocked)
    );
    assert!(plans[0].eligible);
}

#[test]
fn market_maker_shadow_can_audit_reward_subsidized_small_negative_edge() {
    let now = OffsetDateTime::now_utc();
    let mut plans = vec![market_maker_test_plan(decimal("0.504"))];
    let decisions = apply_reward_market_maker_decisions_to_quote_plans(
        &mut plans,
        &[market_maker_test_market()],
        &[market_maker_test_fair_value(now)],
        &[],
        &[],
        &market_maker_config(RewardStrategyMode::MarketMakerShadow),
        "trace",
        now,
    );

    assert_eq!(decisions.len(), 1);
    assert_eq!(
        decisions[0].decision_status,
        RewardMarketMakerDecisionStatus::ShadowAllowed
    );
    assert!(decisions[0].pricing_edge_cents < Decimal::ZERO);
    assert_eq!(decisions[0].target_price, Some(decimal("0.504")));
    assert_eq!(plans[0].legs[0].price, decimal("0.504"));
}

#[test]
fn market_maker_guarded_reprices_to_pricing_edge_floor_before_allowing_plan() {
    let now = OffsetDateTime::now_utc();
    let mut plans = vec![market_maker_test_plan(decimal("0.504"))];
    let decisions = apply_reward_market_maker_decisions_to_quote_plans(
        &mut plans,
        &[market_maker_test_market()],
        &[market_maker_test_fair_value(now)],
        &[],
        &[],
        &market_maker_config(RewardStrategyMode::MarketMakerGuarded),
        "trace",
        now,
    );

    assert_eq!(decisions.len(), 1);
    assert_eq!(
        decisions[0].decision_status,
        RewardMarketMakerDecisionStatus::Allowed
    );
    assert_eq!(decisions[0].target_price, Some(decimal("0.49")));
    assert_eq!(plans[0].legs[0].price, decimal("0.49"));
    assert_eq!(plans[0].quote_mode, RewardPlanQuoteMode::SingleYes);
    assert!(plans[0].eligible);
}
