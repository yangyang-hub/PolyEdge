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

#[test]
fn live_requote_drift_waits_for_order_cooldown() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        requote_drift_cents: reward_decimal("2"),
        requote_drift_confirm_sec: 60,
        requote_drift_cooldown_sec: 300,
        requote_drift_max_cancels_per_cycle: 1,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let mut plan = live_test_plan(now);
    plan.legs[0].price = reward_decimal("0.40");
    let mut order = live_test_open_order("yes_live");
    order.created_at = now - TimeDuration::seconds(299);
    order.updated_at = order.created_at;
    let books = HashMap::from([(
        "yes_live".to_string(),
        live_test_book("yes_live", now),
    )]);
    let history = HashMap::from([(
        "yes_live".to_string(),
        VecDeque::from([live_test_book_snapshot(
            reward_decimal("0.40"),
            now - TimeDuration::seconds(61),
        )]),
    )]);

    let candidates = live_cancel_candidates(&config, &[plan], &[order], &books, &history, false);

    assert!(candidates.is_empty());
}

#[test]
fn live_requote_drift_is_stable_and_limited_per_cycle() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        requote_drift_cents: reward_decimal("2"),
        requote_drift_confirm_sec: 60,
        requote_drift_cooldown_sec: 300,
        requote_drift_max_cancels_per_cycle: 1,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let mut plan = live_test_plan(now);
    plan.legs[0].price = reward_decimal("0.40");
    plan.legs[1].price = reward_decimal("0.40");
    let mut yes_order = live_test_open_order("yes_live");
    yes_order.created_at = now - TimeDuration::seconds(301);
    yes_order.updated_at = yes_order.created_at;
    let mut no_order = live_test_open_order("no_live");
    no_order.created_at = yes_order.created_at;
    no_order.updated_at = yes_order.created_at;
    let books = HashMap::from([
        ("yes_live".to_string(), live_test_book("yes_live", now)),
        ("no_live".to_string(), live_test_book("no_live", now)),
    ]);
    let history = HashMap::from([
        (
            "yes_live".to_string(),
            VecDeque::from([live_test_book_snapshot(
                reward_decimal("0.40"),
                now - TimeDuration::seconds(61),
            )]),
        ),
        (
            "no_live".to_string(),
            VecDeque::from([live_test_book_snapshot(
                reward_decimal("0.40"),
                now - TimeDuration::seconds(61),
            )]),
        ),
    ]);

    let candidates =
        live_cancel_candidates(&config, &[plan], &[yes_order, no_order], &books, &history, false);

    assert_eq!(candidates.len(), 1);
    assert!(candidates[0].1.contains("quote target moved"));
}

#[test]
fn live_requote_drift_limit_does_not_throttle_hard_cancels() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        requote_drift_max_cancels_per_cycle: 1,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let mut plan = live_test_plan(now);
    plan.eligible = false;
    let books = HashMap::from([
        ("yes_live".to_string(), live_test_book("yes_live", now)),
        ("no_live".to_string(), live_test_book("no_live", now)),
    ]);

    let candidates = live_cancel_candidates(
        &config,
        &[plan],
        &[live_test_open_order("yes_live"), live_test_open_order("no_live")],
        &books,
        &HashMap::new(),
        false,
    );

    assert_eq!(candidates.len(), 2);
    assert!(
        candidates
            .iter()
            .all(|(_, reason)| reason == "market dropped below eligibility threshold")
    );
}

fn live_test_book_snapshot(price: Decimal, observed_at: OffsetDateTime) -> BookSnapshot {
    BookSnapshot {
        bids: vec![RewardBookLevel {
            price,
            size: reward_decimal("100"),
        }],
        asks: vec![RewardBookLevel {
            price: reward_decimal("0.52"),
            size: reward_decimal("100"),
        }],
        observed_at,
    }
}
