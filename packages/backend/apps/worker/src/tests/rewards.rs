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
        quote_mode: polyedge_application::RewardPlanQuoteMode::Double,
        recommended_quote_mode: Some(polyedge_application::RewardPlanQuoteMode::Double),
        book_metrics: None,
        ai_advisory: None,
        info_risk: None,
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
        Decimal::from(20_u64),
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
fn live_placement_requires_the_whole_market_to_fit_available_cash() {
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
        reward_decimal("19.59"),
        "trc_live_test",
    );

    assert!(orders.is_empty());
}

#[test]
fn live_placement_counts_existing_same_market_buys_against_cash() {
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
    let existing = live_test_open_order("yes_live");

    let orders = live_placement_orders(
        &config,
        "reward_live",
        &[plan],
        &books,
        &[existing],
        &[],
        reward_decimal("15"),
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
        false,
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
        false,
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
fn data_api_fill_does_not_double_apply_an_external_account_snapshot() {
    let now = OffsetDateTime::now_utc();
    let mut account = polyedge_application::RewardAccountState::fresh(
        "reward_live",
        Decimal::from(80_u64),
        now,
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
            updated_at: now,
        },
    )]);
    let available_before = account.available_usd;
    let mut order = live_test_open_order("yes_live");
    order.size = Decimal::from(20_u64);

    assert!(external_snapshot_covers_buy_fill(
        &account,
        &positions.values().cloned().collect::<Vec<_>>(),
        &order,
        Decimal::from(20_u64),
        now,
    ));

    let update = apply_live_reward_fill_update(
        order,
        &mut account,
        &mut positions,
        &live_test_trade_update("pm_yes_live", "data_api:tx_1", Decimal::from(20_u64)),
        "rewfill_data_api_tx_1_pm_yes_live",
        "trc_data_api_fill",
        true,
    )
    .expect("Data API fill");

    assert_eq!(account.available_usd, available_before);
    assert_eq!(positions["yes_live"].size, Decimal::from(20_u64));
    assert_eq!(update.order.status, ManagedRewardOrderStatus::Filled);
    assert_eq!(update.fill.size, Decimal::from(20_u64));
}

#[test]
fn data_api_trade_fallback_requires_one_matching_local_order() {
    let order = live_test_open_order("yes_live");
    let activity = PolymarketWalletActivity {
        proxy_wallet: "0x0000000000000000000000000000000000000001".to_string(),
        kind: "TRADE".to_string(),
        side: "BUY".to_string(),
        asset: order.token_id.clone(),
        condition_id: order.condition_id.clone(),
        outcome: order.outcome.clone(),
        outcome_index: 0,
        title: "test".to_string(),
        slug: "test".to_string(),
        transaction_hash: "0xtx1".to_string(),
        price: order.price,
        size: Decimal::from(20_u64),
        usdc_size: order.price * Decimal::from(20_u64),
        timestamp: order.created_at + TimeDuration::seconds(1),
    };

    assert!(data_api_activity_matches_reward_order(
        &activity,
        &order,
        std::slice::from_ref(&order),
    ));

    let mut duplicate = order.clone();
    duplicate.id = "duplicate_order".to_string();
    assert!(!data_api_activity_matches_reward_order(
        &activity,
        &order,
        &[order.clone(), duplicate],
    ));
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
        false,
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
        false,
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
        &[],
        &entry,
        Decimal::from(5_u64),
        &positions,
        &HashMap::new(),
        Decimal::ZERO,
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
fn transient_order_rejection_checks_status_code_and_message() {
    assert!(is_transient_order_rejection(&PolymarketOrderRejection {
        code: "HTTP_429".to_string(),
        message: "rate limited".to_string(),
    }));
    assert!(is_transient_order_rejection(&PolymarketOrderRejection {
        code: "temporary".to_string(),
        message: "Order manager not ready, please retry".to_string(),
    }));
    assert!(!is_transient_order_rejection(&PolymarketOrderRejection {
        code: "INVALID_ORDER".to_string(),
        message: "price is invalid".to_string(),
    }));
}

#[test]
fn exit_markup_price_rounds_up_to_the_exchange_tick() {
    assert_eq!(
        ceil_reward_price_to_tick(reward_decimal("0.515")),
        reward_decimal("0.52")
    );
    assert_eq!(
        ceil_reward_price_to_tick(reward_decimal("0.999")),
        reward_decimal("0.99")
    );
}

#[test]
fn rejected_exit_retries_use_bounded_backoff() {
    let now = OffsetDateTime::now_utc();
    let mut order = live_test_open_order("yes_live");
    order.side = RewardOrderSide::Sell;
    order.status = ManagedRewardOrderStatus::ExitPending;
    order.reason = "retryable live exit rejected [3/10] (post_only=true)".to_string();
    order.updated_at = now;

    assert!(!live_exit_retry_due(&order, now + TimeDuration::seconds(19)));
    assert!(live_exit_retry_due(&order, now + TimeDuration::seconds(20)));
}

#[test]
fn exit_min_notional_pre_submit_failure_uses_retry_backoff_marker() {
    let mut order = live_test_open_order("yes_live");
    order.side = RewardOrderSide::Sell;
    order.status = ManagedRewardOrderStatus::ExitPending;
    order.reason = "post-fill flatten immediately".to_string();
    let error = AppError::invalid_input(
        "POLYMARKET_NOTIONAL_INVALID",
        "polymarket live connector requires notional >= 1.00 USD",
    );

    let (reason, severity) =
        live_exit_pre_submit_failure(&order, &error, false, "post-fill flatten immediately")
            .expect("exit notional failure should use bounded retry state");

    assert_eq!(severity, RewardRiskSeverity::Warning);
    assert!(reason.contains("retryable live exit rejected [1/10]"));

    order.reason = reason;
    let now = OffsetDateTime::now_utc();
    order.updated_at = now;
    assert!(!live_exit_retry_due(&order, now + TimeDuration::seconds(4)));
    assert!(live_exit_retry_due(&order, now + TimeDuration::seconds(5)));
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
fn minimum_depth_excludes_our_own_liquidity_at_the_order_price() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        min_depth_usd: reward_decimal("40.01"),
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let plan = live_test_plan(now);
    let order = live_test_open_order("yes_live");
    let mut book = live_test_book("yes_live", now);
    book.bids = vec![RewardBookLevel {
        price: order.price,
        size: reward_decimal("100"),
    }];
    let books = HashMap::from([("yes_live".to_string(), book)]);

    let candidates =
        live_cancel_candidates(&config, &[plan], &[order], &books, &HashMap::new(), false);

    assert_eq!(candidates.len(), 1);
    assert!(candidates[0].1.contains("external bid depth 39.2"));
}

#[test]
fn live_placement_does_not_add_inventory_while_exit_is_pending() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        max_markets: 1,
        max_open_orders: 4,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let plan = live_test_plan(now);
    let books = HashMap::from([
        ("yes_live".to_string(), live_test_book("yes_live", now)),
        ("no_live".to_string(), live_test_book("no_live", now)),
    ]);
    let mut exit = live_test_open_order("yes_live");
    exit.side = RewardOrderSide::Sell;
    exit.status = ManagedRewardOrderStatus::ExitPending;

    let placements = live_placement_orders(
        &config,
        "reward_live",
        &[plan],
        &books,
        &[exit],
        &[],
        reward_decimal("100"),
        "trc_exit_pending",
    );

    assert!(placements.iter().all(|order| order.token_id != "yes_live"));
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

    let mut restored_unknown = live_test_open_order("restored_unknown");
    restored_unknown.external_order_id = None;
    restored_unknown.status = ManagedRewardOrderStatus::Planned;
    restored_unknown.reason = LIVE_SUBMISSION_UNKNOWN_MARKER.to_string();
    assert!(has_unresolved_live_reconciliation(&[restored_unknown]));

    let mut missing = live_test_open_order("no_live");
    missing.reason =
        "external order lookup returned not found; manual reconciliation required".to_string();
    assert!(has_unresolved_live_reconciliation(&[missing]));

    let mut pending_cancel = live_test_open_order("pending_cancel");
    pending_cancel.reason = "cancel accepted; awaiting final reconciliation".to_string();
    assert!(has_unresolved_live_reconciliation(&[pending_cancel]));
}
