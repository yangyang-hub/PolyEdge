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
fn stale_missing_external_order_closes_after_timeout() {
    let mut order = live_test_open_order("yes_live");
    let external_order_id = order.external_order_id.clone().expect("external order id");
    order.scoring = false;
    order.reason = format!(
        "{LIVE_EXTERNAL_ORDER_NOT_FOUND_MARKER}; manual reconciliation required: {external_order_id}"
    );
    order.updated_at = OffsetDateTime::now_utc()
        - TimeDuration::seconds(LIVE_EXTERNAL_ORDER_NOT_FOUND_CLOSE_AFTER_SECS + 1);

    let (closed, event) = mark_live_external_order_not_found(order, &external_order_id)
        .expect("stale missing external order must be closed locally");

    assert_eq!(closed.status, ManagedRewardOrderStatus::Cancelled);
    assert!(!closed.scoring);
    assert!(closed.reason.contains("local order closed"));
    assert_eq!(
        event.event_type,
        "reward_live_external_order_not_found_closed"
    );
    assert!(!has_unresolved_live_reconciliation(&[closed]));
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
fn empty_missing_order_trade_scan_uses_data_api_fallback() {
    let empty_missing = LivePolymarketTradeSyncOutcome {
        updates: Vec::new(),
        order_status: None,
        order_not_found: true,
    };
    assert!(should_try_data_api_fallback_for_clob_outcome(
        &empty_missing
    ));

    let live_order = LivePolymarketTradeSyncOutcome {
        order_not_found: false,
        ..empty_missing
    };
    assert!(!should_try_data_api_fallback_for_clob_outcome(
        &live_order
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

    let (mut order, _) = apply_live_reward_status_update_to_order(
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
    order.updated_at = now - TimeDuration::seconds(16);
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

    assert!(!recovered.scoring);
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

fn open_snapshot_order(id: &str, token_id: &str) -> PolymarketOpenOrder {
    PolymarketOpenOrder {
        id: id.to_string(),
        market: "0x0000000000000000000000000000000000000000000000000000000000000000"
            .to_string(),
        asset_id: token_id.to_string(),
        side: PolymarketTokenOrderSide::Buy,
        original_size: reward_decimal("20"),
        size_matched: Decimal::ZERO,
        price: reward_decimal("0.49"),
        outcome: "YES".to_string(),
        status: "Live".to_string(),
        created_at: OffsetDateTime::now_utc(),
    }
}

#[test]
fn external_open_order_snapshot_closes_missing_managed_buy() {
    let present = live_test_open_order("yes_live");
    let mut missing = live_test_open_order("no_live");
    missing.outcome = "NO".to_string();
    let snapshot = vec![open_snapshot_order("pm_yes_live", "yes_live")];

    let updates = close_managed_orders_absent_from_open_snapshot(
        &[present, missing],
        &snapshot,
        "trc_open_snapshot",
    );

    assert_eq!(updates.len(), 1);
    let (closed, event) = &updates[0];
    assert_eq!(closed.id, "rewlive_seed_no_live");
    assert_eq!(closed.status, ManagedRewardOrderStatus::Cancelled);
    assert!(!closed.scoring);
    assert!(closed.reason.contains("no longer present"));
    assert_eq!(
        event.event_type,
        "reward_live_order_missing_from_open_orders_closed"
    );
    assert!(!has_unresolved_live_reconciliation(std::slice::from_ref(closed)));
}

#[test]
fn external_open_order_snapshot_preserves_stuck_or_sell_orders() {
    let mut pending_cancel = live_test_open_order("pending_cancel");
    pending_cancel.reason = "risk cancel; cancel accepted; awaiting final reconciliation".to_string();
    let mut missing_lock = live_test_open_order("missing_lock");
    missing_lock.reason =
        "external order lookup returned not found; manual reconciliation required".to_string();
    let mut exit = live_test_open_order("exit_live");
    exit.side = RewardOrderSide::Sell;
    exit.status = ManagedRewardOrderStatus::ExitPending;

    let updates = close_managed_orders_absent_from_open_snapshot(
        &[pending_cancel, missing_lock, exit],
        &[],
        "trc_open_snapshot",
    );

    assert!(updates.is_empty());
}

#[test]
fn external_reward_buy_open_order_can_be_adopted_from_snapshot() {
    let created_at = OffsetDateTime::now_utc() - TimeDuration::minutes(10);
    let mut snapshot = open_snapshot_order("pm_orphan_yes", "yes_live");
    snapshot.market = "cond_live".to_string();
    snapshot.created_at = created_at;
    let token_match = RewardOpenOrderTokenMatch {
        condition_id: "cond_live".to_string(),
        token_id: "yes_live".to_string(),
        outcome: "YES".to_string(),
    };

    let (order, event) = build_external_open_reward_buy_order_adoption(
        "reward_live",
        &token_match,
        &snapshot,
        None,
        OffsetDateTime::now_utc(),
        "trc_adopt",
    )
    .expect("external open reward buy order should be adopted");

    assert_eq!(order.account_id, "reward_live");
    assert_eq!(order.condition_id, "cond_live");
    assert_eq!(order.token_id, "yes_live");
    assert_eq!(order.external_order_id.as_deref(), Some("pm_orphan_yes"));
    assert_eq!(order.status, ManagedRewardOrderStatus::Open);
    assert_eq!(order.created_at, created_at);
    assert_eq!(
        event.event_type,
        "reward_live_external_open_order_adopted"
    );
}

#[test]
fn external_reward_buy_open_order_reopens_cancelled_local_order() {
    let mut existing = live_test_open_order("yes_live");
    existing.status = ManagedRewardOrderStatus::Cancelled;
    existing.external_order_id = Some("pm_yes_live".to_string());
    existing.reason = "closed locally".to_string();
    let mut snapshot = open_snapshot_order("pm_yes_live", "yes_live");
    snapshot.market = existing.condition_id.clone();
    let account_id = existing.account_id.clone();
    let token_match = RewardOpenOrderTokenMatch {
        condition_id: existing.condition_id.clone(),
        token_id: existing.token_id.clone(),
        outcome: existing.outcome.clone(),
    };

    let (order, event) = build_external_open_reward_buy_order_adoption(
        &account_id,
        &token_match,
        &snapshot,
        Some(existing),
        OffsetDateTime::now_utc(),
        "trc_reopen",
    )
    .expect("cancelled local order should be reopened when CLOB still reports it open");

    assert_eq!(order.id, "rewlive_seed_yes_live");
    assert_eq!(order.status, ManagedRewardOrderStatus::Open);
    assert_eq!(
        event.event_type,
        "reward_live_external_open_order_reopened"
    );
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
    order.updated_at = now - TimeDuration::seconds(16);
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
        category: "politics".to_string(),
        image: String::new(),
        rewards_max_spread: reward_decimal("8"),
        rewards_min_size: reward_decimal("5"),
        total_daily_rate: reward_decimal("25"),
        liquidity_usd: reward_decimal("10000"),
        volume_24h_usd: reward_decimal("25000"),
        market_spread_cents: reward_decimal("2"),
        end_at: Some(now + time::Duration::days(30)),
        ambiguity_level: "low".to_string(),
        market_synced_at: Some(now),
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

    let mut plans = vec![plan];
    let (orders, _) = live_placement_orders(
        &config,
        &live_test_account(Decimal::from(100_u64)),
        &mut plans,
        &books,
        &HashMap::new(),
        &[],
        &positions,
        false,
        "trc_live_test",
    );

    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].token_id, "no_live");
}
