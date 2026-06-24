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
    let old = now - TimeDuration::seconds(40);
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
fn live_placement_uses_orderbook_confirmation_time_for_freshness() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        stale_book_ms: 45_000,
        max_markets: 1,
        max_open_orders: 2,
        max_global_position_usd: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let content_time = now - TimeDuration::minutes(10);
    let mut yes_book = live_test_book("yes_live", content_time);
    yes_book.confirmed_at = now;
    let mut no_book = live_test_book("no_live", content_time);
    no_book.confirmed_at = now;
    let books = HashMap::from([
        ("yes_live".to_string(), yes_book),
        ("no_live".to_string(), no_book),
    ]);
    let plan = live_test_plan(now);

    assert!(live_orderbook_placement_wait_reason(&config, &plan.legs, &books, now).is_none());
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
fn refresh_live_quote_plan_readiness_materializes_placeholder_plan() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        stale_book_ms: 45_000,
        max_markets: 1,
        max_open_orders: 2,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let mut plan = live_test_plan(now);
    plan.reason = "eligible pending live orderbook validation for double quotes".to_string();
    for leg in &mut plan.legs {
        leg.price = Decimal::ZERO;
        leg.size = Decimal::ZERO;
        leg.notional_usd = Decimal::ZERO;
    }
    let books = HashMap::from([
        ("yes_live".to_string(), live_test_book("yes_live", now)),
        ("no_live".to_string(), live_test_book("no_live", now)),
    ]);

    let mut plans = vec![plan];
    assert!(refresh_live_quote_plan_readiness(
        &config,
        &mut plans,
        &books
    ));

    assert!(plans[0].eligible);
    assert!(plans[0].reason.contains("eligible for live post-only"));
    assert!(plans[0].legs.iter().all(|leg| leg.price > Decimal::ZERO));
    assert!(plans[0].legs.iter().all(|leg| leg.size > Decimal::ZERO));
}

#[test]
fn refresh_live_quote_plan_readiness_waits_when_books_exceed_placement_headroom() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        stale_book_ms: 45_000,
        max_markets: 1,
        max_open_orders: 2,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let old = now - TimeDuration::seconds(40);
    let books = HashMap::from([
        ("yes_live".to_string(), live_test_book("yes_live", old)),
        ("no_live".to_string(), live_test_book("no_live", old)),
    ]);

    let mut plans = vec![live_test_plan(now)];
    assert!(refresh_live_quote_plan_readiness(
        &config,
        &mut plans,
        &books
    ));

    assert!(plans[0].eligible);
    assert!(plans[0].reason.contains("orderbook too close to stale"));
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
fn live_cancel_candidates_cancel_buy_that_would_touch_best_ask() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        stale_book_ms: 45_000,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let plan = live_test_plan(now);
    let mut order = live_test_open_order("yes_live");
    order.price = reward_decimal("0.49");
    let mut book = live_test_book("yes_live", now);
    book.asks[0].price = reward_decimal("0.49");
    let books = HashMap::from([("yes_live".to_string(), book)]);

    let candidates =
        live_cancel_candidates(&config, &[plan], &[order], &books, &HashMap::new(), false);

    assert_eq!(candidates.len(), 1);
    assert!(candidates[0].1.contains("post-only buy would touch best ask"));
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

#[test]
fn live_cancel_in_flight_guard_dedupes_concurrent_cancel_requests() {
    let external_order_id = format!("test_cancel_{}", new_trace_id());
    let first = RewardCancelInFlightGuard::try_acquire(&external_order_id);
    assert!(first.is_some());
    assert!(RewardCancelInFlightGuard::try_acquire(&external_order_id).is_none());
    drop(first);
    assert!(RewardCancelInFlightGuard::try_acquire(&external_order_id).is_some());
}

#[test]
fn event_cancel_fast_path_filters_to_updated_token_and_hard_risk() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        min_depth_usd: reward_decimal("100"),
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let plan = live_test_plan(now);
    let yes_order = live_test_open_order("yes_live");
    let no_order = live_test_open_order("no_live");
    let mut yes_book = live_test_book("yes_live", now);
    yes_book.bids = vec![RewardBookLevel {
        price: yes_order.price,
        size: reward_decimal("20"),
    }];
    let books = HashMap::from([
        ("yes_live".to_string(), yes_book),
        ("no_live".to_string(), live_test_book("no_live", now)),
    ]);
    let updated_tokens = HashSet::from(["yes_live".to_string()]);

    let candidates = live_event_hard_cancel_candidates(
        &config,
        &[plan],
        &[yes_order.clone(), no_order],
        &books,
        &HashMap::new(),
        &updated_tokens,
        false,
    );

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].0, yes_order.id);
    assert!(candidates[0].1.contains("external bid depth"));
}

#[test]
fn event_cancel_fast_path_cancels_buy_that_would_touch_best_ask() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        stale_book_ms: 45_000,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let plan = live_test_plan(now);
    let mut order = live_test_open_order("yes_live");
    order.price = reward_decimal("0.49");
    let mut book = live_test_book("yes_live", now);
    book.asks[0].price = reward_decimal("0.49");
    let books = HashMap::from([("yes_live".to_string(), book)]);
    let updated_tokens = HashSet::from(["yes_live".to_string()]);

    let candidates = live_event_hard_cancel_candidates(
        &config,
        &[plan],
        &[order.clone()],
        &books,
        &HashMap::new(),
        &updated_tokens,
        false,
    );

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].0, order.id);
    assert!(candidates[0].1.contains("post-only buy would touch best ask"));
}

#[test]
fn live_cancel_candidates_recheck_low_competition_gate() {
    let config = low_competition_live_cancel_test_config();
    let now = OffsetDateTime::now_utc();
    let mut plan = live_test_plan(now);
    plan.strategy_bucket = RewardStrategyBucket::LowCompetition;
    let yes_order = live_test_open_order("yes_live");
    let books = HashMap::from([
        ("yes_live".to_string(), live_test_book("yes_live", now)),
        ("no_live".to_string(), live_test_book("no_live", now)),
    ]);
    let history = low_competition_live_book_history(
        &books,
        config.low_competition_min_book_samples.max(1),
    );

    let candidates = live_cancel_candidates_with_account(
        &config,
        &[plan],
        &[yes_order.clone()],
        &books,
        &history,
        &live_test_account(reward_decimal("1000")),
        false,
    );

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].0, yes_order.id);
    assert!(candidates[0].1.contains("low-competition cancel gate rejected"));
    assert!(candidates[0].1.contains("competition share"));
}

#[test]
fn event_cancel_fast_path_rechecks_low_competition_gate_with_companion_books() {
    let config = low_competition_live_cancel_test_config();
    let now = OffsetDateTime::now_utc();
    let mut plan = live_test_plan(now);
    plan.strategy_bucket = RewardStrategyBucket::LowCompetition;
    let yes_order = live_test_open_order("yes_live");
    let books = HashMap::from([
        ("yes_live".to_string(), live_test_book("yes_live", now)),
        ("no_live".to_string(), live_test_book("no_live", now)),
    ]);
    let history = low_competition_live_book_history(
        &books,
        config.low_competition_min_book_samples.max(1),
    );
    let updated_tokens = HashSet::from(["yes_live".to_string()]);

    let candidates = live_event_hard_cancel_candidates_with_account(
        &config,
        &[plan],
        &[yes_order.clone()],
        &books,
        &history,
        &live_test_account(reward_decimal("1000")),
        &updated_tokens,
        false,
    );

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].0, yes_order.id);
    assert!(candidates[0].1.contains("low-competition cancel gate rejected"));
}

#[test]
fn event_cancel_fast_path_rechecks_low_competition_gate_on_companion_token_update() {
    let config = low_competition_live_cancel_test_config();
    let now = OffsetDateTime::now_utc();
    let mut plan = live_test_plan(now);
    plan.strategy_bucket = RewardStrategyBucket::LowCompetition;
    let yes_order = live_test_open_order("yes_live");
    let books = HashMap::from([
        ("yes_live".to_string(), live_test_book("yes_live", now)),
        ("no_live".to_string(), live_test_book("no_live", now)),
    ]);
    let history = low_competition_live_book_history(
        &books,
        config.low_competition_min_book_samples.max(1),
    );
    let updated_tokens = HashSet::from(["no_live".to_string()]);

    let candidates = live_event_hard_cancel_candidates_with_account(
        &config,
        &[plan],
        &[yes_order.clone()],
        &books,
        &history,
        &live_test_account(reward_decimal("1000")),
        &updated_tokens,
        false,
    );

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].0, yes_order.id);
    assert!(candidates[0].1.contains("low-competition cancel gate rejected"));
}

#[test]
fn event_cancel_active_tokens_include_low_competition_companion_update() {
    let now = OffsetDateTime::now_utc();
    let mut plan = live_test_plan(now);
    plan.strategy_bucket = RewardStrategyBucket::LowCompetition;
    let yes_order = live_test_open_order("yes_live");
    let updated_tokens = HashSet::from(["no_live".to_string()]);

    let token_ids =
        reward_event_cancel_active_order_tokens(&[plan], &[yes_order], &updated_tokens);

    assert_eq!(
        token_ids,
        vec!["yes_live".to_string(), "no_live".to_string()]
    );
}

#[test]
fn live_cancel_candidates_skip_normal_depth_rule_for_low_competition_orders() {
    let config = low_competition_skip_normal_cancel_test_config();
    let now = OffsetDateTime::now_utc();
    let mut plan = live_test_plan(now);
    plan.strategy_bucket = RewardStrategyBucket::LowCompetition;
    let mut order = live_test_open_order("yes_live");
    order.strategy_bucket = RewardStrategyBucket::LowCompetition;
    let books = HashMap::from([
        ("yes_live".to_string(), live_test_book("yes_live", now)),
        ("no_live".to_string(), live_test_book("no_live", now)),
    ]);

    let candidates = live_cancel_candidates_with_account(
        &config,
        &[plan],
        &[order],
        &books,
        &HashMap::new(),
        &live_test_account(reward_decimal("1000")),
        false,
    );

    assert!(candidates.is_empty());
}

#[test]
fn event_cancel_fast_path_skips_normal_depth_rule_for_low_competition_orders() {
    let config = low_competition_skip_normal_cancel_test_config();
    let now = OffsetDateTime::now_utc();
    let mut plan = live_test_plan(now);
    plan.strategy_bucket = RewardStrategyBucket::LowCompetition;
    let mut order = live_test_open_order("yes_live");
    order.strategy_bucket = RewardStrategyBucket::LowCompetition;
    let books = HashMap::from([
        ("yes_live".to_string(), live_test_book("yes_live", now)),
        ("no_live".to_string(), live_test_book("no_live", now)),
    ]);
    let updated_tokens = HashSet::from(["yes_live".to_string()]);

    let candidates = live_event_hard_cancel_candidates_with_account(
        &config,
        &[plan],
        &[order],
        &books,
        &HashMap::new(),
        &live_test_account(reward_decimal("1000")),
        &updated_tokens,
        false,
    );

    assert!(candidates.is_empty());
}

#[test]
fn event_cancel_fast_path_ignores_requote_only_reasons() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        requote_drift_cents: reward_decimal("2"),
        requote_drift_confirm_sec: 0,
        requote_drift_cooldown_sec: 0,
        requote_drift_max_cancels_per_cycle: 1,
        requote_interval_sec: 1,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let mut plan = live_test_plan(now);
    plan.legs[0].price = reward_decimal("0.40");
    let mut order = live_test_open_order("yes_live");
    order.created_at = now - TimeDuration::seconds(10);
    order.updated_at = order.created_at;
    let books = HashMap::from([(
        "yes_live".to_string(),
        live_test_book("yes_live", now),
    )]);
    let updated_tokens = HashSet::from(["yes_live".to_string()]);

    let regular = live_cancel_candidates(
        &config,
        &[plan.clone()],
        &[order.clone()],
        &books,
        &HashMap::new(),
        false,
    );
    let event_only = live_event_hard_cancel_candidates(
        &config,
        &[plan],
        &[order],
        &books,
        &HashMap::new(),
        &updated_tokens,
        false,
    );

    assert_eq!(regular.len(), 1);
    assert!(regular[0].1.contains("quote target moved"));
    assert!(event_only.is_empty());
}

fn low_competition_skip_normal_cancel_test_config() -> RewardBotConfig {
    RewardBotConfig {
        account_id: "reward_live".to_string(),
        min_depth_usd: reward_decimal("100"),
        low_competition_mode: polyedge_application::RewardLowCompetitionMode::Enforce,
        low_competition_max_markets: 1,
        low_competition_max_open_orders: 2,
        low_competition_probe_notional_usd: reward_decimal("10"),
        low_competition_min_competition_share_bps: 0,
        low_competition_max_competition_multiple: Decimal::ZERO,
        low_competition_max_account_allocation_bps: 10_000,
        low_competition_max_market_allocation_bps: 10_000,
        ai_advisory_enabled: true,
        info_risk_enabled: true,
        info_risk_mode: polyedge_application::RewardSelectionMode::Enforce,
        ..RewardBotConfig::default()
    }
}

fn low_competition_live_cancel_test_config() -> RewardBotConfig {
    RewardBotConfig {
        account_id: "reward_live".to_string(),
        low_competition_mode: polyedge_application::RewardLowCompetitionMode::Enforce,
        low_competition_max_markets: 1,
        low_competition_max_open_orders: 2,
        low_competition_probe_notional_usd: reward_decimal("10"),
        low_competition_min_competition_share_bps: 5_000,
        low_competition_max_competition_multiple: reward_decimal("1"),
        low_competition_max_competition_usd: reward_decimal("2000"),
        low_competition_min_book_samples: 2,
        ai_advisory_enabled: true,
        info_risk_enabled: true,
        info_risk_mode: polyedge_application::RewardSelectionMode::Enforce,
        ..RewardBotConfig::default()
    }
}

fn low_competition_live_book_history(
    books: &HashMap<String, RewardOrderBook>,
    samples: u64,
) -> HashMap<String, VecDeque<BookSnapshot>> {
    let now = OffsetDateTime::now_utc();
    books
        .values()
        .map(|book| {
            let snapshots = (0..samples)
                .map(|index| BookSnapshot {
                    bids: book.bids.clone(),
                    asks: book.asks.clone(),
                    observed_at: now - TimeDuration::seconds((samples - index) as i64 * 40),
                })
                .collect::<VecDeque<_>>();
            (book.token_id.clone(), snapshots)
        })
        .collect()
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
