fn fair_value_test_books(now: OffsetDateTime) -> HashMap<String, RewardOrderBook> {
    [
        RewardOrderBook {
            token_id: "yes_fair".to_string(),
            bids: vec![RewardBookLevel {
                price: decimal("0.54"),
                size: decimal("100"),
            }],
            asks: vec![RewardBookLevel {
                price: decimal("0.56"),
                size: decimal("100"),
            }],
            observed_at: now,
            confirmed_at: now,
        },
        RewardOrderBook {
            token_id: "no_fair".to_string(),
            bids: vec![RewardBookLevel {
                price: decimal("0.44"),
                size: decimal("100"),
            }],
            asks: vec![RewardBookLevel {
                price: decimal("0.46"),
                size: decimal("100"),
            }],
            observed_at: now,
            confirmed_at: now,
        },
    ]
    .into_iter()
    .map(|book| (book.token_id.clone(), book))
    .collect()
}

fn fair_value_test_plan(price: Decimal, now: OffsetDateTime) -> RewardQuotePlan {
    RewardQuotePlan {
        condition_id: "cond_fair".to_string(),
        market_slug: "fair-market".to_string(),
        question: "Fair value test?".to_string(),
        score: decimal("90"),
        selection_score: Decimal::ZERO,
        eligible: true,
        pre_ai_eligible: true,
        quote_readiness: RewardQuoteReadiness::ReadyToQuote,
        reason: "ready".to_string(),
        strategy_bucket: RewardStrategyBucket::Standard,
        strategy_profile: RewardStrategyProfile::Standard,
        latest_run_id: None,
        quote_mode: RewardPlanQuoteMode::SingleYes,
        recommended_quote_mode: Some(RewardPlanQuoteMode::SingleYes),
        book_metrics: None,
        opportunity_metrics: None,
        selection_metrics: None,
        fair_value: None,
        ai_advisory: None,
        info_risk: None,
        event_window: None,
        midpoint: Some(decimal("0.55")),
        live_skip_until: None,
        live_skip_reason: None,
        first_quote_observed_at: None,
        ai_advisory_pending_since: None,
        info_risk_pending_since: None,
        total_daily_rate: decimal("10"),
        rewards_max_spread: decimal("3"),
        rewards_min_size: decimal("20"),
        orderbook_token_ids: vec!["yes_fair".to_string(), "no_fair".to_string()],
        legs: vec![RewardQuoteLeg {
            token_id: "yes_fair".to_string(),
            outcome: "Yes".to_string(),
            side: RewardOrderSide::Buy,
            price,
            size: decimal("20"),
            notional_usd: (price * decimal("20")).round_dp(4),
        }],
        updated_at: now,
    }
}

fn fair_value_test_config() -> RewardBotConfig {
    RewardBotConfig {
        fair_value_enabled: true,
        fair_value_min_confidence: decimal("0.5"),
        fair_value_min_raw_edge_cents: decimal("0.25"),
        fair_value_min_effective_edge_cents: decimal("0.75"),
        fair_value_uncertainty_buffer_cents: Decimal::ZERO,
        fair_value_rebate_haircut: Decimal::ZERO,
        fair_value_max_reward_rebate_cents: Decimal::ZERO,
        fair_value_history_window_sec: 0,
        fair_value_min_history_samples: 0,
        stale_book_ms: 60_000,
        ..RewardBotConfig::default()
    }
}

#[test]
fn fair_value_gate_accepts_positive_edge_quote() {
    let now = OffsetDateTime::now_utc();
    let books = fair_value_test_books(now);
    let mut plan = fair_value_test_plan(decimal("0.53"), now);

    let estimate = apply_reward_fair_value_to_quote_plan(
        &mut plan,
        &books,
        &HashMap::new(),
        &fair_value_test_config(),
        now,
    )
    .expect("fair value estimate");

    let decision = plan.fair_value.as_ref().expect("fair value decision");
    assert_eq!(estimate.fair_yes, decimal("0.55"));
    assert!(decision.passed);
    assert!(plan.eligible);
    assert!(decision.edges[0].raw_edge_cents >= decimal("1.9"));
}

#[test]
fn fair_value_gate_blocks_quote_above_fair_value() {
    let now = OffsetDateTime::now_utc();
    let books = fair_value_test_books(now);
    let mut plan = fair_value_test_plan(decimal("0.565"), now);

    let config = RewardBotConfig {
        quote_bid_rank: 1,
        quote_max_bid_rank: 1,
        fair_value_min_effective_edge_cents: decimal("2"),
        ..fair_value_test_config()
    };
    apply_reward_fair_value_to_quote_plan(
        &mut plan,
        &books,
        &HashMap::new(),
        &config,
        now,
    )
    .expect("fair value estimate");

    let decision = plan.fair_value.as_ref().expect("fair value decision");
    assert!(!decision.passed);
    assert!(!plan.eligible);
    assert!(!plan.pre_ai_eligible);
    assert!(plan.reason.contains("fair value gate"));
    assert!(decision.edges[0].raw_edge_cents < Decimal::ZERO);
}

#[test]
fn upstream_event_block_marks_fair_value_not_evaluated_instead_of_failed() {
    let now = OffsetDateTime::now_utc();
    let books = fair_value_test_books(now);
    let mut plan = fair_value_test_plan(decimal("0.53"), now);
    plan.eligible = false;
    plan.pre_ai_eligible = false;
    plan.quote_mode = RewardPlanQuoteMode::None;
    plan.legs.clear();
    plan.reason = "event window blocked: event starts soon".to_string();
    plan.event_window = Some(RewardEventWindowAssessment {
        status: RewardEventWindowStatus::StopNewQuotes,
        reason: "event starts soon".to_string(),
        event_key: Some("event:test".to_string()),
        event_time_role: Some(RewardEventTimeRole::EventOccurrence),
        schedule_status: Some(RewardEventScheduleStatus::Scheduled),
        time_precision: Some(RewardEventTimePrecision::Exact),
        start_source_field: Some("manual.event_start_at".to_string()),
        end_policy: Some(RewardEventEndPolicy::Point),
        hard_gate_eligible: Some(true),
        producer_version: Some(1),
        source_updated_at: Some(now),
        observed_at: Some(now),
        expires_at: None,
        event_start_at: Some(now + TimeDuration::minutes(30)),
        event_end_at: None,
        source: Some("manual".to_string()),
        confidence: Some(RewardEventTimeConfidence::High),
        event_type: Some("sports".to_string()),
    });

    apply_reward_fair_value_to_quote_plan(
        &mut plan,
        &books,
        &HashMap::new(),
        &fair_value_test_config(),
        now,
    )
    .expect("fair value estimate");

    let decision = plan.fair_value.as_ref().expect("fair value decision");
    assert_eq!(
        decision.assessment_status,
        RewardFairValueAssessmentStatus::NotEvaluated
    );
    assert!(!decision.passed);
    assert!(decision.edges.is_empty());
    assert!(!reward_quote_plan_blocker_codes(&plan, "event_window")
        .iter()
        .any(|code| code == "fair_value"));
    assert_eq!(
        reward_strategy_decision_from_plan(1, 0, &plan, now).fair_value_passed,
        None
    );
}

#[test]
fn fair_value_uses_top_of_book_microprice_imbalance() {
    let now = OffsetDateTime::now_utc();
    let mut books = fair_value_test_books(now);
    books.get_mut("yes_fair").expect("YES book").bids[0].size = decimal("900");
    books.get_mut("yes_fair").expect("YES book").asks[0].size = decimal("100");
    let mut plan = fair_value_test_plan(decimal("0.53"), now);

    let estimate = apply_reward_fair_value_to_quote_plan(
        &mut plan,
        &books,
        &HashMap::new(),
        &fair_value_test_config(),
        now,
    )
    .expect("fair value estimate");

    assert!(estimate.fair_yes > decimal("0.55"));
    assert!(estimate.components.iter().any(|component| {
        component.source == "current_yes_microprice" && component.value > decimal("0.55")
    }));
    assert!(estimate.uncertainty_cents > Decimal::ZERO);
}

#[test]
fn lp_reward_cannot_rescue_a_quote_that_fails_trading_edge() {
    let now = OffsetDateTime::now_utc();
    let books = fair_value_test_books(now);
    let mut plan = fair_value_test_plan(decimal("0.545"), now);
    plan.opportunity_metrics = Some(RewardOpportunityMetrics {
        planned_notional_usd: decimal("10.9"),
        probe_notional_usd: decimal("10.9"),
        qualified_competition_usd: Decimal::ZERO,
        competition_share_bps: Decimal::ZERO,
        competition_multiple: Decimal::ZERO,
        estimated_reward_per_100_usd_day: decimal("5"),
        competition_density: Decimal::ZERO,
        account_effective_available_usd: decimal("100"),
        open_buy_notional_usd: Decimal::ZERO,
        open_buy_notional_usd_after_plan: decimal("10.9"),
        condition_buy_notional_usd_after_plan: decimal("10.9"),
        account_allocation_bps: decimal("1090"),
        market_allocation_bps: decimal("1090"),
        exit_depth_usd: decimal("100"),
        exit_slippage_cents: Some(Decimal::ZERO),
        bad_fill_recovery_days: Some(Decimal::ZERO),
        midpoint_range_cents: Some(Decimal::ZERO),
        top_of_book_flip_count: Some(0),
        sample_count: 10,
        reward_score: decimal("100"),
        competition_score: decimal("100"),
        exit_score: decimal("100"),
        stability_score: decimal("100"),
        opportunity_score: decimal("100"),
        score_adjustment: Decimal::ZERO,
        warnings: Vec::new(),
    });
    let config = RewardBotConfig {
        fair_value_rebate_haircut: Decimal::ONE,
        fair_value_max_reward_rebate_cents: decimal("10"),
        quote_bid_rank: 1,
        quote_max_bid_rank: 1,
        ..fair_value_test_config()
    };

    apply_reward_fair_value_to_quote_plan(
        &mut plan,
        &books,
        &HashMap::new(),
        &config,
        now,
    )
    .expect("fair value estimate");

    let edge = &plan.fair_value.as_ref().expect("decision").edges[0];
    assert!(edge.effective_edge_cents < config.fair_value_min_effective_edge_cents);
    assert!(edge.reward_adjusted_edge_cents > config.fair_value_min_effective_edge_cents);
    assert!(!edge.passed);
    assert!(!plan.eligible);
}

#[test]
fn provider_edge_buffer_is_included_in_final_edge_gate() {
    let now = OffsetDateTime::now_utc();
    let books = fair_value_test_books(now);
    let mut plan = fair_value_test_plan(decimal("0.53"), now);
    plan.ai_advisory = Some(RewardMarketAdvisory {
        condition_id: plan.condition_id.clone(),
        provider: RewardAiProvider::OpenAi,
        request_format: RewardAiRequestFormat::OpenAiResponses,
        model: "test-model".to_string(),
        input_hash: "test-hash".to_string(),
        action: RewardProviderAction::Reduce,
        size_multiplier: Decimal::ONE,
        edge_buffer_cents: decimal("2"),
        confidence: Decimal::ONE,
        reasons: vec!["structural uncertainty".to_string()],
        metrics: json!({}),
        created_at: now,
        expires_at: now + TimeDuration::hours(1),
    });
    let config = RewardBotConfig {
        ai_advisory_enabled: true,
        ai_risk_adjustment_enabled: true,
        ai_action_min_confidence: decimal("0.5"),
        ..fair_value_test_config()
    };

    apply_reward_fair_value_to_quote_plan(&mut plan, &books, &HashMap::new(), &config, now)
        .expect("fair value estimate");

    let edge = &plan.fair_value.as_ref().expect("decision").edges[0];
    assert_eq!(edge.uncertainty_cents, decimal("3"));
    assert!(!edge.passed);
    assert!(!plan.eligible);
}

#[test]
fn final_fair_value_gate_searches_deeper_rank_with_dynamic_uncertainty() {
    let now = OffsetDateTime::now_utc();
    let mut books = fair_value_test_books(now);
    books.get_mut("yes_fair").expect("YES book").bids = vec![
        RewardBookLevel {
            price: decimal("0.54"),
            size: decimal("100"),
        },
        RewardBookLevel {
            price: decimal("0.53"),
            size: decimal("100"),
        },
        RewardBookLevel {
            price: decimal("0.52"),
            size: decimal("100"),
        },
    ];
    let mut plan = fair_value_test_plan(decimal("0.54"), now);
    let config = RewardBotConfig {
        quote_bid_rank: 1,
        quote_max_bid_rank: 3,
        fair_value_min_raw_edge_cents: decimal("0.5"),
        fair_value_min_effective_edge_cents: decimal("1"),
        ..fair_value_test_config()
    };

    apply_reward_fair_value_to_quote_plan(&mut plan, &books, &HashMap::new(), &config, now)
        .expect("fair value estimate");

    assert_eq!(plan.legs[0].price, decimal("0.53"));
    assert!(plan.fair_value.as_ref().expect("decision").passed);
}

#[test]
fn fair_value_estimate_is_reused_across_strategy_profiles() {
    let now = OffsetDateTime::now_utc();
    let books = fair_value_test_books(now);
    let standard = fair_value_test_plan(decimal("0.53"), now);
    let mut balanced_merge = fair_value_test_plan(decimal("0.52"), now);
    balanced_merge.strategy_bucket = RewardStrategyBucket::Standard;
    balanced_merge.strategy_profile = RewardStrategyProfile::BalancedMerge;
    balanced_merge.ai_advisory = Some(RewardMarketAdvisory {
        condition_id: balanced_merge.condition_id.clone(),
        provider: RewardAiProvider::OpenAi,
        request_format: RewardAiRequestFormat::OpenAiResponses,
        model: "test-model".to_string(),
        input_hash: "balanced-hash".to_string(),
        action: RewardProviderAction::Reduce,
        size_multiplier: Decimal::ONE,
        edge_buffer_cents: decimal("0.5"),
        confidence: Decimal::ONE,
        reasons: vec!["profile-specific adjustment".to_string()],
        metrics: json!({}),
        created_at: now,
        expires_at: now + TimeDuration::hours(1),
    });
    let mut plans = vec![standard, balanced_merge];
    let config = RewardBotConfig {
        ai_advisory_enabled: true,
        ai_risk_adjustment_enabled: true,
        ai_action_min_confidence: decimal("0.5"),
        ..fair_value_test_config()
    };

    let estimates = apply_reward_fair_values_to_quote_plans(
        &mut plans,
        &books,
        &HashMap::new(),
        &config,
        now,
    );

    assert_eq!(estimates.len(), 1);
    assert_eq!(estimates[0].condition_id, "cond_fair");
    assert_eq!(
        plans[0].fair_value.as_ref().expect("standard decision").estimate,
        plans[1]
            .fair_value
            .as_ref()
            .expect("balanced decision")
            .estimate
    );
    assert_ne!(
        plans[0].fair_value.as_ref().expect("standard decision").edges,
        plans[1]
            .fair_value
            .as_ref()
            .expect("balanced decision")
            .edges
    );
}

#[test]
fn inconsistent_profile_token_mapping_fails_closed_for_the_condition() {
    let now = OffsetDateTime::now_utc();
    let books = fair_value_test_books(now);
    let standard = fair_value_test_plan(decimal("0.53"), now);
    let mut balanced_merge = fair_value_test_plan(decimal("0.52"), now);
    balanced_merge.strategy_bucket = RewardStrategyBucket::Standard;
    balanced_merge.strategy_profile = RewardStrategyProfile::BalancedMerge;
    balanced_merge.orderbook_token_ids = vec!["other_yes".to_string(), "other_no".to_string()];
    balanced_merge.legs[0].token_id = "other_yes".to_string();
    let mut plans = vec![standard, balanced_merge];

    let estimates = apply_reward_fair_values_to_quote_plans(
        &mut plans,
        &books,
        &HashMap::new(),
        &fair_value_test_config(),
        now,
    );

    assert_eq!(estimates.len(), 1);
    let reason = estimates[0]
        .do_not_quote_reason
        .as_deref()
        .expect("fail-closed reason");
    assert!(reason.contains("inconsistent outcome token mapping across strategy profiles"));
    for plan in plans {
        assert!(!plan.eligible);
        assert!(!plan.pre_ai_eligible);
        let decision = plan.fair_value.expect("fair-value decision");
        assert!(!decision.passed);
        assert_eq!(decision.estimate.do_not_quote_reason.as_deref(), Some(reason));
    }
}
