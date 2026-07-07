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
        eligible: true,
        pre_ai_eligible: true,
        quote_readiness: RewardQuoteReadiness::ReadyToQuote,
        reason: "ready".to_string(),
        strategy_bucket: RewardStrategyBucket::Standard,
        strategy_profile: RewardStrategyProfile::Standard,
        quote_mode: RewardPlanQuoteMode::SingleYes,
        recommended_quote_mode: Some(RewardPlanQuoteMode::SingleYes),
        book_metrics: None,
        opportunity_metrics: None,
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

    apply_reward_fair_value_to_quote_plan(
        &mut plan,
        &books,
        &HashMap::new(),
        &fair_value_test_config(),
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
