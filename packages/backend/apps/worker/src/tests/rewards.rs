fn reward_decimal(value: &str) -> Decimal {
    Decimal::from_str_exact(value).expect("decimal")
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
fn live_placement_reuses_cash_and_allows_stale_book_age_check_to_be_disabled() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        stale_book_ms: 0,
        max_markets: 1,
        max_open_orders: 2,
        max_global_position_usd: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let old = now - TimeDuration::hours(1);
    let plan = live_test_plan(now);
    let books = HashMap::from([
        ("yes_live".to_string(), live_test_book("yes_live", old)),
        ("no_live".to_string(), live_test_book("no_live", old)),
    ]);

    let orders = live_placement_orders(
        &config,
        "reward_live",
        &[plan],
        &books,
        &[],
        &[],
        "trc_live_test",
    );

    assert_eq!(orders.len(), 2);
    assert!(orders.iter().all(|order| {
        order.side == RewardOrderSide::Buy && order.status == ManagedRewardOrderStatus::Planned
    }));
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

    assert!(update.order.reason.contains("awaiting final reconciliation"));
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
fn reward_live_fill_id_includes_order_id_and_keeps_legacy_id() {
    let update = live_test_trade_update("pm_yes_live", "pm_trade_1", Decimal::ONE);

    assert_eq!(
        reward_live_fill_id(&update),
        "rewfill_pm_trade_1_pm_yes_live"
    );
    assert_eq!(reward_live_legacy_fill_id(&update), "rewfill_pm_trade_1");
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
    let books = HashMap::from([(
        "yes_live".to_string(),
        live_test_book("yes_live", now),
    )]);

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

    let candidates =
        live_cancel_candidates(&config, &[plan], &[order], &HashMap::new(), &HashMap::new(), false);

    assert!(candidates.is_empty());
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
    let books = HashMap::from([(
        "yes_live".to_string(),
        live_test_book("yes_live", now),
    )]);

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
fn live_cancel_candidates_retry_rejected_post_only_violation_cancel() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let plan = live_test_plan(now);
    let mut order = live_test_open_order("yes_live");
    order.reason = "Polymarket returned matched for a post-only rewards quote and cancel was rejected; cancellation must be retried".to_string();
    let books = HashMap::from([(
        "yes_live".to_string(),
        live_test_book("yes_live", now),
    )]);

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
    order.reason =
        "live post-only rewards exit accepted; cancel requested because orderbook stale".to_string();
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
}

#[test]
fn cancelled_live_exit_does_not_retry_explicit_cancel_all() {
    let mut order = live_test_open_order("yes_live");
    order.side = RewardOrderSide::Sell;
    order.status = ManagedRewardOrderStatus::ExitPending;
    order.reason = "worker processed queued rewards live cancel-all command; cancel accepted"
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
    let state = test_state(SystemMode::ManualConfirm);
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
                account: RewardAccountState::fresh(
                    "reward_live",
                    Decimal::from(100_u64),
                    now,
                ),
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
        "trc_live_test",
    );

    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].token_id, "no_live");
}
