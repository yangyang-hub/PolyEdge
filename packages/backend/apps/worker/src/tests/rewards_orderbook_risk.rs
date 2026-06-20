#[test]
fn live_placement_waits_when_orderbook_is_too_close_to_stale() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        stale_book_ms: 45_000,
        max_markets: 1,
        max_open_orders: 2,
        max_global_position_usd: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let old = now - TimeDuration::seconds(20);
    let plan = live_test_plan(now);
    let books = HashMap::from([
        ("yes_live".to_string(), live_test_book("yes_live", old)),
        ("no_live".to_string(), live_test_book("no_live", old)),
    ]);

    let mut plans = vec![plan];
    let (orders, plans_changed) = live_placement_orders(
        &config,
        &live_test_account(Decimal::from(20_u64)),
        &mut plans,
        &books,
        &HashMap::new(),
        &[],
        &[],
        false,
        "trc_live_test",
    );

    assert!(orders.is_empty());
    assert!(plans_changed);
    assert!(plans[0].eligible);
    assert!(plans[0].live_skip_until.is_none());
    assert!(plans[0].reason.contains("orderbook too close to stale"));
}

#[test]
fn live_placement_headroom_scales_for_short_stale_window() {
    let config = RewardBotConfig {
        stale_book_ms: 10_000,
        ..RewardBotConfig::default()
    };

    assert_eq!(live_orderbook_max_placement_age_ms(&config), 5_000);
}

#[test]
fn live_cancel_candidates_grace_recent_live_buy_with_stale_orderbook() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        stale_book_ms: 45_000,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let plan = live_test_plan(now);
    let mut order = live_test_open_order("yes_live");
    order.created_at = now - TimeDuration::seconds(30);
    order.updated_at = order.created_at;
    let books = HashMap::from([(
        "yes_live".to_string(),
        live_test_book("yes_live", now - TimeDuration::seconds(50)),
    )]);

    let candidates =
        live_cancel_candidates(&config, &[plan], &[order], &books, &HashMap::new(), false);

    assert!(candidates.is_empty());
}

#[test]
fn live_cancel_candidates_do_not_grace_stale_book_when_plan_is_ineligible() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        stale_book_ms: 45_000,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let mut plan = live_test_plan(now);
    plan.eligible = false;
    let mut order = live_test_open_order("yes_live");
    order.created_at = now - TimeDuration::seconds(30);
    order.updated_at = order.created_at;
    let books = HashMap::from([(
        "yes_live".to_string(),
        live_test_book("yes_live", now - TimeDuration::seconds(50)),
    )]);

    let candidates =
        live_cancel_candidates(&config, &[plan], &[order], &books, &HashMap::new(), false);

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].1, "market dropped below eligibility threshold");
}

#[test]
fn live_cancel_candidates_cancel_old_live_buy_with_stale_orderbook() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        stale_book_ms: 45_000,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let plan = live_test_plan(now);
    let mut order = live_test_open_order("yes_live");
    order.created_at = now - live_stale_orderbook_cancel_grace(&config) - TimeDuration::seconds(1);
    order.updated_at = order.created_at;
    let books = HashMap::from([(
        "yes_live".to_string(),
        live_test_book("yes_live", now - TimeDuration::seconds(50)),
    )]);

    let candidates =
        live_cancel_candidates(&config, &[plan], &[order], &books, &HashMap::new(), false);

    assert_eq!(candidates.len(), 1);
    assert!(candidates[0].1.contains("orderbook stale for live order"));
}
