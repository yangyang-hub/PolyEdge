use super::*;

fn strategy_test_market(condition_id: &str, now: OffsetDateTime) -> RewardMarket {
    RewardMarket {
        condition_id: condition_id.to_string(),
        question: format!("{condition_id} market"),
        market_slug: format!("{condition_id}-slug"),
        event_slug: format!("{condition_id}-event"),
        category: "test".to_string(),
        image: String::new(),
        rewards_max_spread: decimal("8"),
        rewards_min_size: decimal("50"),
        total_daily_rate: decimal("50"),
        liquidity_usd: decimal("10000"),
        volume_24h_usd: decimal("25000"),
        market_spread_cents: decimal("2"),
        end_at: Some(now + TimeDuration::days(30)),
        ambiguity_level: "low".to_string(),
        market_synced_at: Some(now),
        tokens: vec![
            RewardToken {
                token_id: format!("yes_{condition_id}"),
                outcome: "Yes".to_string(),
                price: None,
            },
            RewardToken {
                token_id: format!("no_{condition_id}"),
                outcome: "No".to_string(),
                price: None,
            },
        ],
        active: true,
        updated_at: now,
    }
}

fn strategy_test_book(token_id: &str, now: OffsetDateTime) -> RewardOrderBook {
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

fn strategy_test_plan(condition_id: &str, now: OffsetDateTime) -> RewardQuotePlan {
    RewardQuotePlan {
        condition_id: condition_id.to_string(),
        market_slug: format!("{condition_id}-slug"),
        question: format!("{condition_id} market"),
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
        orderbook_token_ids: vec![format!("yes_{condition_id}"), format!("no_{condition_id}")],
        legs: vec![
            RewardQuoteLeg {
                token_id: format!("yes_{condition_id}"),
                outcome: "Yes".to_string(),
                side: RewardOrderSide::Buy,
                price: decimal("0.48"),
                size: decimal("50"),
                notional_usd: decimal("24"),
            },
            RewardQuoteLeg {
                token_id: format!("no_{condition_id}"),
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

pub(super) fn strategy_test_snapshot(
    now: OffsetDateTime,
    force_orders: bool,
) -> RewardStrategyInput {
    let market = strategy_test_market("cond_strategy", now);
    let plan = strategy_test_plan("cond_strategy", now);
    RewardStrategyInput {
        now,
        force_orders,
        config: RewardBotConfig {
            account_id: "reward_strategy".to_string(),
            enabled: false,
            ..RewardBotConfig::default()
        },
        candidate_markets: vec![RewardCandidateMarket {
            market,
            strategy_bucket: RewardStrategyBucket::Standard,
            strategy_profile: RewardStrategyProfile::Standard,
        }],
        plans: vec![plan.clone()],
        previous_plans: vec![plan],
        pre_ai_eligible_condition_ids: vec!["cond_strategy".to_string()],
        books: [
            strategy_test_book("yes_cond_strategy", now),
            strategy_test_book("no_cond_strategy", now),
        ]
        .into_iter()
        .map(|book| (book.token_id.clone(), book))
        .collect(),
        book_history: [(
            "yes_cond_strategy".to_string(),
            vec![BookSnapshot {
                bids: vec![RewardBookLevel {
                    price: decimal("0.47"),
                    size: decimal("80"),
                }],
                asks: vec![RewardBookLevel {
                    price: decimal("0.53"),
                    size: decimal("80"),
                }],
                observed_at: now,
            }],
        )]
        .into_iter()
        .collect(),
        account: RewardAccountState::fresh("reward_strategy", decimal("1000"), now),
        open_orders: Vec::new(),
        positions: Vec::new(),
        event_windows: Vec::new(),
    }
}

#[test]
fn reward_strategy_input_round_trips_through_serde() {
    let snapshot = strategy_test_snapshot(OffsetDateTime::now_utc(), false);
    let serialized = serde_json::to_value(&snapshot).expect("serialize snapshot");
    let deserialized: RewardStrategyInput =
        serde_json::from_value(serialized).expect("deserialize snapshot");
    assert_eq!(deserialized, snapshot);
}

#[test]
fn reward_live_cycle_from_strategy_input_maps_all_fields() {
    let now = OffsetDateTime::now_utc();
    let snapshot = strategy_test_snapshot(now, false);
    let cycle = RewardLiveCycle::from_strategy_input(&snapshot);

    assert_eq!(cycle.config, snapshot.config);
    assert_eq!(cycle.account, snapshot.account);
    assert_eq!(cycle.plans, snapshot.plans);
    assert_eq!(cycle.previous_plans, snapshot.previous_plans);
    assert_eq!(
        cycle.pre_ai_eligible_condition_ids,
        snapshot.pre_ai_eligible_condition_ids
    );
    assert_eq!(cycle.open_orders, snapshot.open_orders);
    assert_eq!(cycle.positions, snapshot.positions);
    // markets are projected 1:1 from candidate markets.
    let expected_markets = snapshot
        .candidate_markets
        .iter()
        .map(|candidate| candidate.market.clone())
        .collect::<Vec<_>>();
    assert_eq!(cycle.markets, expected_markets);
    // should_execute derives from config.enabled || force_orders.
    assert_eq!(cycle.should_execute, snapshot.config.enabled || snapshot.force_orders);
}

#[test]
fn reward_live_cycle_from_strategy_input_derives_should_execute() {
    let now = OffsetDateTime::now_utc();

    // Disabled config + normal poll -> does not execute.
    let mut snapshot = strategy_test_snapshot(now, false);
    snapshot.config.enabled = false;
    assert!(!RewardLiveCycle::from_strategy_input(&snapshot).should_execute);

    // Disabled config but forced run -> executes.
    snapshot.force_orders = true;
    assert!(RewardLiveCycle::from_strategy_input(&snapshot).should_execute);

    // Enabled config, normal poll -> executes.
    snapshot.config.enabled = true;
    snapshot.force_orders = false;
    assert!(RewardLiveCycle::from_strategy_input(&snapshot).should_execute);
}
