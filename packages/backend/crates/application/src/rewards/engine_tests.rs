fn engine_test_config() -> RewardBotConfig {
    RewardBotConfig {
        account_id: "reward_engine".to_string(),
        fair_value_enabled: false,
        stale_book_ms: 45_000,
        max_markets: 1,
        max_open_orders: 2,
        max_global_position_usd: Decimal::ZERO,
        quote_mode: RewardQuoteMode::Double,
        ..RewardBotConfig::default()
    }
}

fn engine_test_account(available_usd: Decimal, now: OffsetDateTime) -> RewardAccountState {
    let mut account = RewardAccountState::fresh("reward_engine", available_usd, now);
    account.available_usd = available_usd;
    account
}

fn engine_test_book(token_id: &str, now: OffsetDateTime) -> RewardOrderBook {
    RewardOrderBook {
        token_id: token_id.to_string(),
        bids: vec![RewardBookLevel {
            price: decimal("0.48"),
            size: decimal("100"),
        }],
        asks: vec![RewardBookLevel {
            price: decimal("0.52"),
            size: decimal("100"),
        }],
        observed_at: now,
        confirmed_at: now,
    }
}

fn engine_test_books(now: OffsetDateTime) -> HashMap<String, RewardOrderBook> {
    [
        engine_test_book("yes_engine", now),
        engine_test_book("no_engine", now),
    ]
    .into_iter()
    .map(|book| (book.token_id.clone(), book))
    .collect()
}

fn engine_test_plan(now: OffsetDateTime) -> RewardQuotePlan {
    RewardQuotePlan {
        condition_id: "cond_engine".to_string(),
        market_slug: "engine-market".to_string(),
        question: "Engine test?".to_string(),
        score: decimal("90"),
        selection_score: Decimal::ZERO,
        eligible: true,
        pre_ai_eligible: true,
        quote_readiness: RewardQuoteReadiness::ReadyToQuote,
        reason: "eligible".to_string(),
        strategy_bucket: RewardStrategyBucket::Standard,
        strategy_profile: RewardStrategyProfile::Standard,
        latest_run_id: None,
        quote_mode: RewardPlanQuoteMode::Double,
        recommended_quote_mode: Some(RewardPlanQuoteMode::Double),
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
        rewards_min_size: decimal("50"),
        orderbook_token_ids: vec!["yes_engine".to_string(), "no_engine".to_string()],
        legs: vec![
            RewardQuoteLeg {
                token_id: "yes_engine".to_string(),
                outcome: "Yes".to_string(),
                side: RewardOrderSide::Buy,
                price: decimal("0.48"),
                size: decimal("50"),
                notional_usd: decimal("24"),
            },
            RewardQuoteLeg {
                token_id: "no_engine".to_string(),
                outcome: "No".to_string(),
                side: RewardOrderSide::Buy,
                price: decimal("0.48"),
                size: decimal("50"),
                notional_usd: decimal("24"),
            },
        ],
        updated_at: now,
    }
}

fn engine_test_cycle(
    config: RewardBotConfig,
    account: RewardAccountState,
    plan: RewardQuotePlan,
) -> RewardLiveCycle {
    RewardLiveCycle {
        config,
        account,
        markets: Vec::new(),
        plans: vec![plan],
        previous_plans: Vec::new(),
        pre_ai_eligible_condition_ids: Vec::new(),
        open_orders: Vec::new(),
        positions: Vec::new(),
        should_execute: true,
    }
}

#[test]
fn decision_engine_blocks_underfunded_plan_before_provider() {
    let now = OffsetDateTime::now_utc();
    let books = engine_test_books(now);
    let history = HashMap::new();
    let cycle = engine_test_cycle(
        engine_test_config(),
        engine_test_account(decimal("47.99"), now),
        engine_test_plan(now),
    );

    let decisions = RewardDecisionEngine::evaluate_pre_provider(RewardStrategyInput {
        cycle,
        books: &books,
        book_history: &history,
        now,
    });

    assert_eq!(decisions.funding_precheck_blocked, 1);
    assert!(decisions.cycle.pre_ai_eligible_condition_ids.is_empty());
    assert!(!decisions.cycle.plans[0].eligible);
    assert_eq!(
        decisions.cycle.plans[0].quote_readiness,
        RewardQuoteReadiness::Blocked
    );
    assert!(decisions.cycle.plans[0]
        .reason
        .contains("live funding below rewards minimum"));
}

#[test]
fn decision_engine_refreshes_placeholder_plan_to_ready_quote() {
    let now = OffsetDateTime::now_utc();
    let books = engine_test_books(now);
    let history = HashMap::new();
    let mut plan = engine_test_plan(now);
    plan.reason = "eligible pending live orderbook validation for double quotes".to_string();
    for leg in &mut plan.legs {
        leg.price = Decimal::ZERO;
        leg.size = Decimal::ZERO;
        leg.notional_usd = Decimal::ZERO;
    }
    refresh_reward_quote_plan_readiness(&mut plan);

    let cycle = engine_test_cycle(
        engine_test_config(),
        engine_test_account(decimal("100"), now),
        plan,
    );
    let decisions = RewardDecisionEngine::refresh_snapshot(RewardStrategyInput {
        cycle,
        books: &books,
        book_history: &history,
        now,
    });

    assert!(decisions.readiness_changed);
    assert!(decisions.cycle.plans[0].eligible);
    assert_eq!(
        decisions.cycle.plans[0].quote_readiness,
        RewardQuoteReadiness::ReadyToQuote
    );
    assert!(decisions.cycle.plans[0]
        .legs
        .iter()
        .all(|leg| leg.price > Decimal::ZERO && leg.size > Decimal::ZERO));
}
