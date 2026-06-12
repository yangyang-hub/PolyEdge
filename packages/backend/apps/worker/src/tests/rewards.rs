fn reward_decimal(value: &str) -> Decimal {
    Decimal::from_str_exact(value).expect("decimal")
}

#[test]
fn rewards_account_sync_prefers_funding_wallet_address() {
    assert_eq!(
        polymarket_funding_wallet_address(
            "0x0000000000000000000000000000000000000001",
            Some(" 0x0000000000000000000000000000000000000002 "),
        )
        .as_deref(),
        Some("0x0000000000000000000000000000000000000002"),
    );
    assert_eq!(
        polymarket_funding_wallet_address(" 0x0000000000000000000000000000000000000001 ", None,)
            .as_deref(),
        Some("0x0000000000000000000000000000000000000001"),
    );
}

fn live_test_plan(now: OffsetDateTime) -> RewardQuotePlan {
    RewardQuotePlan {
        condition_id: "cond_live".to_string(),
        market_slug: "live-market".to_string(),
        question: "Will the live event happen?".to_string(),
        score: reward_decimal("50"),
        eligible: true,
        reason: "eligible".to_string(),
        midpoint: Some(reward_decimal("0.50")),
        total_daily_rate: reward_decimal("25"),
        rewards_max_spread: reward_decimal("8"),
        rewards_min_size: reward_decimal("5"),
        legs: vec![
            polyedge_application::RewardQuoteLeg {
                token_id: "yes_live".to_string(),
                outcome: "YES".to_string(),
                side: RewardOrderSide::Buy,
                price: reward_decimal("0.49"),
                size: reward_decimal("20"),
                notional_usd: reward_decimal("9.8"),
            },
            polyedge_application::RewardQuoteLeg {
                token_id: "no_live".to_string(),
                outcome: "NO".to_string(),
                side: RewardOrderSide::Buy,
                price: reward_decimal("0.49"),
                size: reward_decimal("20"),
                notional_usd: reward_decimal("9.8"),
            },
        ],
        updated_at: now,
    }
}

fn live_test_book(token_id: &str, observed_at: OffsetDateTime) -> RewardOrderBook {
    RewardOrderBook {
        token_id: token_id.to_string(),
        bids: vec![RewardBookLevel {
            price: reward_decimal("0.48"),
            size: reward_decimal("100"),
        }],
        asks: vec![RewardBookLevel {
            price: reward_decimal("0.52"),
            size: reward_decimal("100"),
        }],
        observed_at,
    }
}

fn live_test_open_order(token_id: &str) -> ManagedRewardOrder {
    let now = OffsetDateTime::now_utc();
    ManagedRewardOrder {
        id: format!("rewlive_seed_{token_id}"),
        account_id: "reward_live".to_string(),
        condition_id: "cond_live".to_string(),
        token_id: token_id.to_string(),
        outcome: "YES".to_string(),
        side: RewardOrderSide::Buy,
        price: reward_decimal("0.49"),
        size: reward_decimal("20"),
        external_order_id: Some(format!("pm_{token_id}")),
        status: ManagedRewardOrderStatus::Open,
        scoring: true,
        reason: "seed live order".to_string(),
        filled_size: Decimal::ZERO,
        reward_earned: Decimal::ZERO,
        last_scored_at: None,
        created_at: now,
        updated_at: now,
    }
}

fn live_test_trade_update(
    external_order_id: &str,
    external_trade_id: &str,
    size: Decimal,
) -> ConnectorTradeFillUpdate {
    ConnectorTradeFillUpdate {
        event_id: format!("evt_{external_trade_id}"),
        connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
        external_order_id: external_order_id.to_string(),
        account_id: "reward_live".to_string(),
        external_trade_id: external_trade_id.to_string(),
        fill_price: Probability::new(reward_decimal("0.49")).expect("fill price"),
        filled_quantity: Quantity::new(size).expect("fill size"),
        fee: polyedge_domain::UsdAmount::new(Decimal::ZERO).expect("fee"),
    }
}

#[test]
fn live_placement_reuses_cash_across_markets() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        stale_book_ms: 0,
        max_markets: 2,
        max_open_orders: 4,
        max_global_position_usd: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let old = now - TimeDuration::hours(1);
    let first_plan = live_test_plan(now);
    let mut second_plan = live_test_plan(now);
    second_plan.condition_id = "cond_live_2".to_string();
    second_plan.market_slug = "live-market-2".to_string();
    second_plan.legs[0].token_id = "yes_live_2".to_string();
    second_plan.legs[1].token_id = "no_live_2".to_string();
    let books = HashMap::from([
        ("yes_live".to_string(), live_test_book("yes_live", old)),
        ("no_live".to_string(), live_test_book("no_live", old)),
        ("yes_live_2".to_string(), live_test_book("yes_live_2", old)),
        ("no_live_2".to_string(), live_test_book("no_live_2", old)),
    ]);

    let orders = live_placement_orders(
        &config,
        "reward_live",
        &[first_plan, second_plan],
        &books,
        &[],
        &[],
        Decimal::from(10_u64),
        "trc_live_test",
    );

    assert_eq!(orders.len(), 4);
    assert_eq!(
        orders
            .iter()
            .map(|order| order.condition_id.as_str())
            .collect::<HashSet<_>>(),
        HashSet::from(["cond_live", "cond_live_2"])
    );
    assert!(orders.iter().all(|order| {
        order.side == RewardOrderSide::Buy && order.status == ManagedRewardOrderStatus::Planned
    }));
}

#[test]
fn live_placement_requires_each_quote_to_fit_available_cash() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        stale_book_ms: 0,
        max_markets: 1,
        max_open_orders: 2,
        max_global_position_usd: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let plan = live_test_plan(now);
    let books = HashMap::from([
        ("yes_live".to_string(), live_test_book("yes_live", now)),
        ("no_live".to_string(), live_test_book("no_live", now)),
    ]);

    let orders = live_placement_orders(
        &config,
        "reward_live",
        &[plan],
        &books,
        &[],
        &[],
        reward_decimal("9.79"),
        "trc_live_test",
    );

    assert!(orders.is_empty());
}

#[test]
fn live_fill_update_clamps_multiple_updates_to_remaining_size() {
    let mut account = polyedge_application::RewardAccountState::fresh(
        "reward_live",
        Decimal::from(100_u64),
        OffsetDateTime::now_utc(),
    );
    let mut positions = HashMap::new();
    let mut order = live_test_open_order("yes_live");
    order.size = Decimal::from(20_u64);
    order.external_order_id = Some("pm_yes_live".to_string());

    let first = apply_live_reward_fill_update(
        order,
        &mut account,
        &mut positions,
        &live_test_trade_update("pm_yes_live", "pm_trade_1", Decimal::from(12_u64)),
        "rewfill_pm_trade_1_pm_yes_live",
        "trc_live_fill",
    )
    .expect("first fill");
    let first_fill_size = first.fill.size;

    let second = apply_live_reward_fill_update(
        first.order,
        &mut account,
        &mut positions,
        &live_test_trade_update("pm_yes_live", "pm_trade_2", Decimal::from(12_u64)),
        "rewfill_pm_trade_2_pm_yes_live",
        "trc_live_fill",
    )
    .expect("second fill");

    assert_eq!(first_fill_size, Decimal::from(12_u64));
    assert_eq!(second.fill.size, Decimal::from(8_u64));
    assert_eq!(second.order.filled_size, Decimal::from(20_u64));
    assert_eq!(second.order.status, ManagedRewardOrderStatus::Filled);
    assert_eq!(
        positions.get("yes_live").expect("position").size,
        Decimal::from(20_u64)
    );
}

#[test]
fn partial_live_fill_preserves_pending_cancellation_intent() {
    let mut account = polyedge_application::RewardAccountState::fresh(
        "reward_live",
        Decimal::from(100_u64),
        OffsetDateTime::now_utc(),
    );
    let mut positions = HashMap::new();
    let mut order = live_test_open_order("yes_live");
    order.reason = "risk cancel; cancel accepted; awaiting final reconciliation".to_string();

    let update = apply_live_reward_fill_update(
        order,
        &mut account,
        &mut positions,
        &live_test_trade_update("pm_yes_live", "pm_trade_partial", Decimal::from(5_u64)),
        "rewfill_pm_trade_partial_pm_yes_live",
        "trc_partial_cancel",
    )
    .expect("partial fill");

    assert!(
        update
            .order
            .reason
            .contains("awaiting final reconciliation")
    );
    assert!(
        update
            .order
            .reason
            .contains("partially filled on Polymarket")
    );
}

#[test]
fn partial_live_exit_fill_preserves_post_only_retry_strategy() {
    let mut account = polyedge_application::RewardAccountState::fresh(
        "reward_live",
        Decimal::from(100_u64),
        OffsetDateTime::now_utc(),
    );
    let mut positions = HashMap::from([(
        "yes_live".to_string(),
        RewardPosition {
            account_id: "reward_live".to_string(),
            condition_id: "cond_live".to_string(),
            token_id: "yes_live".to_string(),
            outcome: "YES".to_string(),
            size: Decimal::from(20_u64),
            avg_price: reward_decimal("0.49"),
            realized_pnl: Decimal::ZERO,
            updated_at: OffsetDateTime::now_utc(),
        },
    )]);
    let mut order = live_test_open_order("yes_live");
    order.side = RewardOrderSide::Sell;
    order.status = ManagedRewardOrderStatus::ExitPending;
    order.reason = "live post-only rewards exit accepted".to_string();

    let update = apply_live_reward_fill_update(
        order,
        &mut account,
        &mut positions,
        &live_test_trade_update("pm_yes_live", "pm_trade_exit_partial", Decimal::from(5_u64)),
        "rewfill_pm_trade_exit_partial_pm_yes_live",
        "trc_partial_exit",
    )
    .expect("partial exit fill");

    assert!(deferred_live_exit_is_post_only(&update.order));
}

#[test]
fn post_fill_exit_is_planned_before_live_submission() {
    let entry = live_test_open_order("yes_live");
    let positions = HashMap::from([(
        "yes_live".to_string(),
        RewardPosition {
            account_id: "reward_live".to_string(),
            condition_id: "cond_live".to_string(),
            token_id: "yes_live".to_string(),
            outcome: "YES".to_string(),
            size: Decimal::from(5_u64),
            avg_price: reward_decimal("0.49"),
            realized_pnl: Decimal::ZERO,
            updated_at: OffsetDateTime::now_utc(),
        },
    )]);

    let updates = plan_live_post_fill_orders(
        &RewardBotConfig::default(),
        &entry,
        Decimal::from(5_u64),
        &positions,
        &HashMap::new(),
        "trc_exit_plan",
    );

    let LiveRewardOrderUpdate::Changed(exit, event) = &updates[0] else {
        panic!("post-fill exit must be a persisted order update");
    };
    assert_eq!(exit.status, ManagedRewardOrderStatus::ExitPending);
    assert!(exit.external_order_id.is_none());
    assert!(deferred_live_exit_is_post_only(exit));
    assert_eq!(event.event_type, "reward_live_exit_planned");
}

#[test]
fn reward_live_fill_id_includes_order_id_and_keeps_legacy_id() {
    let update = live_test_trade_update("pm_yes_live", "pm_trade_1", Decimal::ONE);

    assert_eq!(
        reward_live_fill_id(&update),
        "rewfill_pm_trade_1_pm_yes_live"
    );
    assert_eq!(reward_live_legacy_fill_id(&update), "rewfill_pm_trade_1");
}

#[test]
fn external_account_refresh_waits_when_order_sync_records_a_fill() {
    assert!(can_refresh_external_account_after_order_sync(
        &RewardBotRunReport::default()
    ));
    assert!(!can_refresh_external_account_after_order_sync(
        &RewardBotRunReport {
            filled_orders: 1,
            ..RewardBotRunReport::default()
        }
    ));
}

#[test]
fn external_account_sync_waits_for_recent_fill_grace_period() {
    let now = OffsetDateTime::now_utc();

    assert!(!account_sync_is_outside_fill_grace(
        Some(now - TimeDuration::seconds(119)),
        now,
    ));
    assert!(account_sync_is_outside_fill_grace(
        Some(now - TimeDuration::seconds(120)),
        now,
    ));
    assert!(account_sync_is_outside_fill_grace(None, now));
}

#[test]
fn live_cancel_candidates_cancel_when_orderbook_missing() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        ..RewardBotConfig::default()
    };
    let plan = live_test_plan(OffsetDateTime::now_utc());
    let order = live_test_open_order("yes_live");

    let candidates = live_cancel_candidates(
        &config,
        &[plan],
        &[order],
        &HashMap::new(),
        &HashMap::new(),
        false,
    );

    assert_eq!(candidates.len(), 1);
    assert!(candidates[0].1.contains("orderbook unavailable"));
}

#[test]
fn live_cancel_candidates_keep_local_deferred_exit_without_orderbook() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        ..RewardBotConfig::default()
    };
    let plan = live_test_plan(OffsetDateTime::now_utc());
    let mut order = live_test_open_order("yes_live");
    order.side = RewardOrderSide::Sell;
    order.status = ManagedRewardOrderStatus::ExitPending;
    order.external_order_id = None;
    order.reason = "flatten deferred until bid liquidity is observed".to_string();

    let candidates = live_cancel_candidates(
        &config,
        &[plan],
        &[order],
        &HashMap::new(),
        &HashMap::new(),
        false,
    );

    assert!(candidates.is_empty());
}

#[test]
fn live_cancel_candidates_cancel_buys_when_global_kill_switch_is_active() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let plan = live_test_plan(now);
    let order = live_test_open_order("yes_live");
    let books = HashMap::from([("yes_live".to_string(), live_test_book("yes_live", now))]);

    let candidates =
        live_cancel_candidates(&config, &[plan], &[order], &books, &HashMap::new(), true);

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].1, "global kill switch is active");
}

#[test]
fn live_cancel_candidates_do_not_repeat_pending_cancel() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        ..RewardBotConfig::default()
    };
    let plan = live_test_plan(OffsetDateTime::now_utc());
    let mut order = live_test_open_order("yes_live");
    order.reason = "risk cancel; cancel accepted; awaiting final reconciliation".to_string();

    let candidates = live_cancel_candidates(
        &config,
        &[plan],
        &[order],
        &HashMap::new(),
        &HashMap::new(),
        false,
    );

    assert!(candidates.is_empty());
}

#[test]
fn live_cancel_candidates_keep_unknown_submission_locked() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        ..RewardBotConfig::default()
    };
    let plan = live_test_plan(OffsetDateTime::now_utc());
    let mut order = live_test_open_order("yes_live");
    order.external_order_id = None;
    order.status = ManagedRewardOrderStatus::Planned;
    order.reason = format!(
        "quote intent; {LIVE_SUBMISSION_ATTEMPTED_MARKER}; {LIVE_SUBMISSION_UNKNOWN_MARKER}"
    );

    let candidates = live_cancel_candidates(
        &config,
        &[plan],
        &[order],
        &HashMap::new(),
        &HashMap::new(),
        true,
    );

    assert!(candidates.is_empty());
}

#[test]
fn sibling_cancel_retry_preserves_unknown_submission_marker() {
    let mut order = live_test_open_order("yes_live");
    order.external_order_id = None;
    order.status = ManagedRewardOrderStatus::Planned;
    order.reason = format!(
        "quote intent; {LIVE_SUBMISSION_ATTEMPTED_MARKER}; {LIVE_SUBMISSION_UNKNOWN_MARKER}"
    );

    let retry = mark_sibling_cancel_for_retry(order);

    assert!(live_submission_was_attempted(&retry));
    assert!(live_submission_result_is_unknown(&retry));
    assert!(
        retry
            .reason
            .contains("sibling cancellation must be retried")
    );
}

#[test]
fn unresolved_live_reconciliation_blocks_new_buy_submission() {
    let mut unknown = live_test_open_order("yes_live");
    unknown.external_order_id = None;
    unknown.status = ManagedRewardOrderStatus::Planned;
    unknown.reason = format!(
        "quote intent; {LIVE_SUBMISSION_ATTEMPTED_MARKER}; {LIVE_SUBMISSION_UNKNOWN_MARKER}"
    );

    assert!(has_unresolved_live_reconciliation(&[unknown]));

    let mut missing = live_test_open_order("no_live");
    missing.reason =
        "external order lookup returned not found; manual reconciliation required".to_string();
    assert!(has_unresolved_live_reconciliation(&[missing]));

    let mut pending_cancel = live_test_open_order("pending_cancel");
    pending_cancel.reason = "cancel accepted; awaiting final reconciliation".to_string();
    assert!(has_unresolved_live_reconciliation(&[pending_cancel]));
}

#[test]
fn missing_external_order_stays_open_for_trade_reconciliation() {
    let order = live_test_open_order("yes_live");
    let external_order_id = order.external_order_id.clone().expect("external order id");

    let (locked, event) = mark_live_external_order_not_found(order, &external_order_id)
        .expect("missing external order must create a reconciliation lock");

    assert!(locked.status.is_open_like());
    assert!(!locked.scoring);
    assert!(locked.reason.contains(LIVE_EXTERNAL_ORDER_NOT_FOUND_MARKER));
    assert_eq!(event.event_type, "reward_live_external_order_not_found");
    assert!(has_unresolved_live_reconciliation(&[locked]));
}

#[test]
fn missing_order_fallback_trade_query_failure_does_not_abort_reconciliation() {
    let error = AppError::internal(
        "POLYMARKET_MISSING_ORDER_TRADE_QUERY_FAILED",
        "malformed fallback trade response",
    );

    assert!(is_missing_external_order_reconciliation_error(&error));
    assert!(!is_missing_external_order_reconciliation_error(
        &AppError::internal("POLYMARKET_TRADE_QUERY_FAILED", "regular trade query failed")
    ));
}

#[test]
fn scoring_sync_skips_reconciliation_locks_and_preserves_state_age() {
    let now = OffsetDateTime::now_utc();
    let interval = TimeDuration::seconds(45);
    let mut healthy = live_test_open_order("healthy_scoring");
    let state_updated_at = healthy.updated_at;

    assert!(should_check_managed_reward_scoring(
        &healthy, now, interval
    ));
    assert!(apply_managed_reward_scoring_observation(
        &mut healthy,
        false,
        now
    ));
    assert_eq!(healthy.last_scored_at, Some(now));
    assert_eq!(healthy.updated_at, state_updated_at);

    let mut stuck = live_test_open_order("stuck_scoring");
    stuck.reason = format!(
        "{LIVE_EXTERNAL_ORDER_NOT_FOUND_MARKER}; manual reconciliation required: pm_stuck_scoring"
    );
    assert!(!should_check_managed_reward_scoring(&stuck, now, interval));
}

#[test]
fn live_status_after_pending_cancel_requires_retry() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let plan = live_test_plan(now);
    let mut order = live_test_open_order("yes_live");
    order.reason = "risk cancel; cancel accepted; awaiting final reconciliation".to_string();
    let external_order_id = order.external_order_id.clone().expect("external order id");

    let (order, _) = apply_live_reward_status_update_to_order(
        order,
        ConnectorOrderStatusUpdate {
            event_id: "evt_live_after_cancel".to_string(),
            connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
            external_order_id,
            status: OrderStatus::Open,
        },
        "trc_live_after_cancel",
    )
    .expect("live order after cancellation attempt must require retry");
    let books = HashMap::from([("yes_live".to_string(), live_test_book("yes_live", now))]);

    let candidates =
        live_cancel_candidates(&config, &[plan], &[order], &books, &HashMap::new(), false);

    assert_eq!(
        candidates,
        vec![(
            "rewlive_seed_yes_live".to_string(),
            "previous cancellation attempt left the order live".to_string()
        )]
    );
}

#[test]
fn successful_live_lookup_clears_external_order_not_found_lock() {
    let mut order = live_test_open_order("yes_live");
    order.scoring = false;
    order.reason = format!(
        "{LIVE_EXTERNAL_ORDER_NOT_FOUND_MARKER}; manual reconciliation required: pm_yes_live"
    );
    let external_order_id = order.external_order_id.clone().expect("external order id");

    let (recovered, event) = apply_live_reward_status_update_to_order(
        order,
        ConnectorOrderStatusUpdate {
            event_id: "evt_lookup_recovered".to_string(),
            connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
            external_order_id,
            status: OrderStatus::Open,
        },
        "trc_lookup_recovered",
    )
    .expect("successful lookup must clear not-found lock");

    assert!(recovered.scoring);
    assert!(
        !recovered
            .reason
            .contains(LIVE_EXTERNAL_ORDER_NOT_FOUND_MARKER)
    );
    assert_eq!(event.event_type, "reward_live_external_order_recovered");
    assert!(!has_unresolved_live_reconciliation(&[recovered]));
}

#[test]
fn sibling_cancel_targets_only_opposite_buy_quote() {
    let filled = live_test_open_order("yes_live");
    let mut opposite_buy = live_test_open_order("no_live");
    opposite_buy.outcome = "NO".to_string();
    let mut opposite_exit = opposite_buy.clone();
    opposite_exit.id = "rewexit_no_live".to_string();
    opposite_exit.side = RewardOrderSide::Sell;
    opposite_exit.status = ManagedRewardOrderStatus::ExitPending;

    assert!(is_sibling_live_buy_order(&opposite_buy, &filled));
    assert!(!is_sibling_live_buy_order(&opposite_exit, &filled));
}

#[test]
fn live_cancel_candidates_retry_rejected_post_only_violation_cancel() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let plan = live_test_plan(now);
    let mut order = live_test_open_order("yes_live");
    order.reason = "Polymarket returned matched for a post-only rewards quote and cancel was rejected; cancellation must be retried".to_string();
    let books = HashMap::from([("yes_live".to_string(), live_test_book("yes_live", now))]);

    let candidates =
        live_cancel_candidates(&config, &[plan], &[order], &books, &HashMap::new(), false);

    assert_eq!(
        candidates,
        vec![(
            "rewlive_seed_yes_live".to_string(),
            "post-only violation requires cancellation".to_string()
        )]
    );
}

#[test]
fn cancelled_live_exit_defers_remaining_position_for_retry() {
    let mut order = live_test_open_order("yes_live");
    order.side = RewardOrderSide::Sell;
    order.status = ManagedRewardOrderStatus::ExitPending;
    order.size = reward_decimal("20");
    order.filled_size = reward_decimal("7");
    order.reason = "live post-only rewards exit accepted; cancel requested because orderbook stale"
        .to_string();
    let position = RewardPosition {
        account_id: order.account_id.clone(),
        condition_id: order.condition_id.clone(),
        token_id: order.token_id.clone(),
        outcome: order.outcome.clone(),
        size: reward_decimal("10"),
        avg_price: reward_decimal("0.49"),
        realized_pnl: Decimal::ZERO,
        updated_at: OffsetDateTime::now_utc(),
    };

    let retry = deferred_live_exit_after_cancellation(&order, Some(&position), "trc_retry")
        .expect("remaining exit must be deferred");

    assert_eq!(retry.status, ManagedRewardOrderStatus::ExitPending);
    assert_eq!(retry.size, reward_decimal("10"));
    assert!(retry.external_order_id.is_none());
    assert!(retry.reason.contains("retry post-fill exit"));
    assert!(deferred_live_exit_is_post_only(&retry));
}

#[test]
fn cancelled_live_exit_does_not_retry_explicit_cancel_all() {
    let mut order = live_test_open_order("yes_live");
    order.side = RewardOrderSide::Sell;
    order.status = ManagedRewardOrderStatus::ExitPending;
    order.reason =
        "worker processed queued rewards live cancel-all command; cancel accepted".to_string();
    let position = RewardPosition {
        account_id: order.account_id.clone(),
        condition_id: order.condition_id.clone(),
        token_id: order.token_id.clone(),
        outcome: order.outcome.clone(),
        size: reward_decimal("10"),
        avg_price: reward_decimal("0.49"),
        realized_pnl: Decimal::ZERO,
        updated_at: OffsetDateTime::now_utc(),
    };

    assert!(deferred_live_exit_after_cancellation(&order, Some(&position), "trc_cancel").is_none());
}

#[test]
fn terminal_match_closes_partial_live_exit_for_remaining_retry() {
    let mut order = live_test_open_order("yes_live");
    order.side = RewardOrderSide::Sell;
    order.status = ManagedRewardOrderStatus::ExitPending;
    order.size = reward_decimal("20");
    order.filled_size = reward_decimal("7");
    let external_order_id = order.external_order_id.clone().expect("external order id");

    let (closed, event) = apply_live_reward_status_update_to_order(
        order,
        ConnectorOrderStatusUpdate {
            event_id: "evt_terminal_match".to_string(),
            connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
            external_order_id,
            status: OrderStatus::Filled,
        },
        "trc_terminal_match",
    )
    .expect("terminal match must close the unfilled remainder");

    assert_eq!(closed.status, ManagedRewardOrderStatus::Cancelled);
    assert_eq!(closed.filled_size, reward_decimal("7"));
    assert_eq!(event.event_type, "reward_live_order_status_terminal_match");
}

#[tokio::test]
async fn live_tick_persistence_keeps_market_catalog_and_blocks_active_account_change() {
    let state = test_state(SystemMode::LiveAuto);
    state
        .reward_bot_service
        .update_config(RewardBotConfigPatch {
            account_id: Some("reward_live".to_string()),
            ..RewardBotConfigPatch::default()
        })
        .await
        .expect("set live account");
    let now = OffsetDateTime::now_utc();
    let market = RewardMarket {
        condition_id: "cond_catalog".to_string(),
        question: "Catalog market".to_string(),
        market_slug: "catalog-market".to_string(),
        event_slug: "catalog-event".to_string(),
        image: String::new(),
        rewards_max_spread: reward_decimal("8"),
        rewards_min_size: reward_decimal("5"),
        total_daily_rate: reward_decimal("25"),
        tokens: Vec::new(),
        active: true,
        updated_at: now,
    };
    state
        .reward_bot_service
        .upsert_reward_markets(&[market])
        .await
        .expect("seed reward catalog");
    state
        .reward_bot_service
        .apply_live_tick_outcome(
            &RewardTickOutcome {
                account: RewardAccountState::fresh("reward_live", Decimal::from(100_u64), now),
                markets: Vec::new(),
                plans: Vec::new(),
                orders: vec![live_test_open_order("yes_live")],
                positions: Vec::new(),
                fills: Vec::new(),
                events: Vec::new(),
                report: RewardBotRunReport::default(),
            },
            "trc_live_persist",
        )
        .await
        .expect("persist live tick");

    assert_eq!(
        state
            .reward_bot_service
            .list_active_reward_markets()
            .await
            .expect("list reward catalog")
            .len(),
        1
    );
    let error = state
        .reward_bot_service
        .update_config(RewardBotConfigPatch {
            account_id: Some("reward_other".to_string()),
            ..RewardBotConfigPatch::default()
        })
        .await
        .expect_err("account change must be blocked");
    assert_eq!(error.code(), "REWARD_ACCOUNT_CHANGE_BLOCKED");
}

#[test]
fn live_placement_counts_candidate_notional_against_position_cap() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        max_markets: 1,
        max_open_orders: 2,
        max_position_usd: Decimal::from(20_u64),
        max_global_position_usd: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let plan = live_test_plan(now);
    let books = HashMap::from([
        ("yes_live".to_string(), live_test_book("yes_live", now)),
        ("no_live".to_string(), live_test_book("no_live", now)),
    ]);
    let positions = vec![RewardPosition {
        account_id: "reward_live".to_string(),
        condition_id: "cond_live".to_string(),
        token_id: "yes_live".to_string(),
        outcome: "Yes".to_string(),
        size: Decimal::from(38_u64),
        avg_price: Decimal::from_parts(50, 0, 0, false, 2),
        realized_pnl: Decimal::ZERO,
        updated_at: now,
    }];

    let orders = live_placement_orders(
        &config,
        "reward_live",
        &[plan],
        &books,
        &[],
        &positions,
        Decimal::from(100_u64),
        "trc_live_test",
    );

    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].token_id, "no_live");
}
