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
    assert!(polyedge_application::refresh_reward_live_quote_plan_readiness(
        &config, &mut plans, &books, now
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
    assert!(polyedge_application::refresh_reward_live_quote_plan_readiness(
        &config, &mut plans, &books, now
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
fn live_cancel_candidates_keep_stale_grace_independent_of_stop_new_state() {
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

    assert!(candidates.is_empty());
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
    assert!(
        candidates[0]
            .1
            .contains("post-only buy would touch best ask")
    );
}

#[test]
fn live_cancel_candidates_cancel_buy_when_live_token_spread_is_wide() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        stale_book_ms: 45_000,
        max_market_spread_cents: reward_decimal("10"),
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let plan = live_test_plan(now);
    let order = live_test_open_order("yes_live");
    let mut book = live_test_book("yes_live", now);
    book.asks[0].price = reward_decimal("0.70");
    let books = HashMap::from([("yes_live".to_string(), book)]);

    let candidates =
        live_cancel_candidates(&config, &[plan], &[order], &books, &HashMap::new(), false);

    assert_eq!(candidates.len(), 1);
    assert!(candidates[0].1.contains("live token spread"));
    assert!(candidates[0].1.contains("exceeds max market spread 10c"));
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
    plan.legs[0].price = reward_decimal("0.60");
    let mut order = live_test_open_order("yes_live");
    order.created_at = now - TimeDuration::seconds(299);
    order.updated_at = order.created_at;
    let books = HashMap::from([("yes_live".to_string(), live_test_book("yes_live", now))]);
    let history = HashMap::from([(
        "yes_live".to_string(),
        VecDeque::from([live_test_book_snapshot(
            reward_decimal("0.60"),
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
    plan.legs[0].price = reward_decimal("0.60");
    plan.legs[1].price = reward_decimal("0.60");
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
                reward_decimal("0.60"),
                now - TimeDuration::seconds(61),
            )]),
        ),
        (
            "no_live".to_string(),
            VecDeque::from([live_test_book_snapshot(
                reward_decimal("0.60"),
                now - TimeDuration::seconds(61),
            )]),
        ),
    ]);

    let candidates = live_cancel_candidates(
        &config,
        &[plan],
        &[yes_order, no_order],
        &books,
        &history,
        false,
    );

    assert_eq!(candidates.len(), 1);
    assert!(candidates[0].1.contains("quote target moved"));
}

#[test]
fn adverse_requote_bypasses_competitive_cooldown_and_cycle_limit() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        adverse_requote_drift_cents: reward_decimal("0.5"),
        adverse_requote_confirm_sec: 0,
        requote_drift_cooldown_sec: 3600,
        requote_drift_max_cancels_per_cycle: 1,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let mut plan = live_test_plan(now);
    plan.legs[0].price = reward_decimal("0.40");
    plan.legs[1].price = reward_decimal("0.40");
    let books = HashMap::from([
        ("yes_live".to_string(), live_test_book("yes_live", now)),
        ("no_live".to_string(), live_test_book("no_live", now)),
    ]);

    let yes_order = live_test_open_order("yes_live");
    let mut no_order = live_test_open_order("no_live");
    no_order.outcome = "NO".to_string();
    let candidates = live_cancel_candidates(
        &config,
        &[plan],
        &[yes_order, no_order],
        &books,
        &HashMap::new(),
        false,
    );

    assert_eq!(candidates.len(), 2);
    assert!(candidates
        .iter()
        .all(|(_, reason)| reason.starts_with("adverse quote target moved down")));
}

#[test]
fn adverse_requote_confirmation_tracks_dynamic_rank_offset() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        fair_value_enabled: false,
        quote_bid_rank: 1,
        quote_max_bid_rank: 3,
        adverse_requote_drift_cents: reward_decimal("0.5"),
        adverse_requote_confirm_sec: 60,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let mut plan = live_test_plan(now);
    plan.legs[0].price = reward_decimal("0.48");
    let mut order = live_test_open_order("yes_live");
    order.price = reward_decimal("0.50");

    let mut book = live_test_book("yes_live", now);
    book.bids[0].price = reward_decimal("0.50");
    book.asks[0].price = reward_decimal("0.54");
    let books = HashMap::from([("yes_live".to_string(), book)]);
    let history = HashMap::from([(
        "yes_live".to_string(),
        VecDeque::from([
            live_test_book_snapshot(
                reward_decimal("0.50"),
                now - TimeDuration::seconds(61),
            ),
            live_test_book_snapshot(reward_decimal("0.50"), now),
        ]),
    )]);

    let candidates = live_cancel_candidates(&config, &[plan], &[order], &books, &history, false);

    assert_eq!(candidates.len(), 1);
    assert!(candidates[0].1.starts_with("adverse quote target moved down"));
}

#[test]
fn ineligible_stop_new_plan_keeps_safe_resting_orders() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let mut plan = live_test_plan(now);
    plan.eligible = false;
    plan.reason = "AI advisory stop_new: structural concern".to_string();
    let books = HashMap::from([
        ("yes_live".to_string(), live_test_book("yes_live", now)),
        ("no_live".to_string(), live_test_book("no_live", now)),
    ]);

    let candidates = live_cancel_candidates(
        &config,
        &[plan],
        &[
            live_test_open_order("yes_live"),
            live_test_open_order("no_live"),
        ],
        &books,
        &HashMap::new(),
        false,
    );

    assert!(candidates.is_empty());
}

#[test]
fn evidence_backed_directional_info_action_cancels_only_matching_outcome() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        info_risk_enabled: true,
        info_risk_mode: polyedge_application::RewardSelectionMode::Enforce,
        info_risk_min_confidence: reward_decimal("0.70"),
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let mut plan = live_test_plan(now);
    plan.info_risk = Some(RewardMarketInfoRisk {
        condition_id: plan.condition_id.clone(),
        provider: polyedge_application::RewardAiProvider::OpenAi,
        request_format: polyedge_application::RewardAiRequestFormat::OpenAiChatCompletions,
        model: "test-model".to_string(),
        query_hash: "query".to_string(),
        input_hash: "input".to_string(),
        action: polyedge_application::RewardProviderAction::CancelYes,
        risk_level: polyedge_application::RewardInfoRiskLevel::Critical,
        risk_type: polyedge_application::RewardInfoRiskType::OfficialResult,
        directional_risk: polyedge_application::RewardInfoDirectionalRisk::Yes,
        resolution_imminent: true,
        expected_event_at: Some(now),
        confidence: reward_decimal("0.95"),
        summary: "fresh evidence makes the YES resting BUY unsafe".to_string(),
        sources: vec![polyedge_application::RewardInfoRiskSource {
            url: "https://example.com/result".to_string(),
            title: "Official result".to_string(),
            published_at: Some(now),
            snippet: Some("YES BUY is exposed to adverse selection".to_string()),
        }],
        metrics: json!({}),
        created_at: now,
        expires_at: now + TimeDuration::hours(1),
    });
    let books = HashMap::from([
        ("yes_live".to_string(), live_test_book("yes_live", now)),
        ("no_live".to_string(), live_test_book("no_live", now)),
    ]);

    let yes_order = live_test_open_order("yes_live");
    let mut no_order = live_test_open_order("no_live");
    no_order.outcome = "NO".to_string();
    let candidates = live_cancel_candidates(
        &config,
        &[plan],
        &[yes_order, no_order],
        &books,
        &HashMap::new(),
        false,
    );

    assert_eq!(candidates.len(), 1);
    assert!(candidates[0].1.contains("cancel_yes"));
}

#[test]
fn directional_info_cancel_keeps_complementary_placement_budget() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        info_risk_enabled: true,
        info_risk_mode: polyedge_application::RewardSelectionMode::Enforce,
        info_risk_min_confidence: reward_decimal("0.70"),
        fair_value_enabled: false,
        max_markets: 1,
        max_open_orders: 2,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let mut plan = live_test_plan(now);
    plan.quote_mode = RewardPlanQuoteMode::SingleNo;
    plan.recommended_quote_mode = Some(RewardPlanQuoteMode::SingleNo);
    plan.info_risk = Some(RewardMarketInfoRisk {
        condition_id: plan.condition_id.clone(),
        provider: polyedge_application::RewardAiProvider::OpenAi,
        request_format: polyedge_application::RewardAiRequestFormat::OpenAiChatCompletions,
        model: "test-model".to_string(),
        query_hash: "query".to_string(),
        input_hash: "input".to_string(),
        action: polyedge_application::RewardProviderAction::CancelYes,
        risk_level: polyedge_application::RewardInfoRiskLevel::Critical,
        risk_type: polyedge_application::RewardInfoRiskType::OfficialResult,
        directional_risk: polyedge_application::RewardInfoDirectionalRisk::Yes,
        resolution_imminent: true,
        expected_event_at: Some(now),
        confidence: reward_decimal("0.95"),
        summary: "fresh evidence makes the YES resting BUY unsafe".to_string(),
        sources: vec![polyedge_application::RewardInfoRiskSource {
            url: "https://example.com/result".to_string(),
            title: "Official result".to_string(),
            published_at: Some(now),
            snippet: Some("YES BUY is exposed to adverse selection".to_string()),
        }],
        metrics: json!({}),
        created_at: now,
        expires_at: now + TimeDuration::hours(1),
    });
    let books = HashMap::from([
        ("yes_live".to_string(), live_test_book("yes_live", now)),
        ("no_live".to_string(), live_test_book("no_live", now)),
    ]);
    let mut plans = vec![plan];

    let (placements, _) = live_placement_orders(
        &config,
        &live_test_account(reward_decimal("100")),
        &mut plans,
        &books,
        &HashMap::new(),
        &[],
        &[],
        false,
        "trc_directional_complement",
    );

    assert_eq!(placements.len(), 1);
    assert_eq!(placements[0].token_id, "no_live");
}

#[test]
fn live_requote_drift_limit_does_not_throttle_hard_cancels() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        requote_drift_max_cancels_per_cycle: 1,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let plan = live_test_plan(now);
    let books = HashMap::from([
        ("yes_live".to_string(), live_test_book("yes_live", now)),
        ("no_live".to_string(), live_test_book("no_live", now)),
    ]);

    let candidates = live_cancel_candidates(
        &config,
        &[plan],
        &[
            live_test_open_order("yes_live"),
            live_test_open_order("no_live"),
        ],
        &books,
        &HashMap::new(),
        true,
    );

    assert_eq!(candidates.len(), 2);
    assert!(
        candidates
            .iter()
            .all(|(_, reason)| reason == "global kill switch is active")
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
    assert!(
        candidates[0]
            .1
            .contains("post-only buy would touch best ask")
    );
}

#[test]
fn event_cancel_fast_path_cancels_buy_when_live_token_spread_is_wide() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        stale_book_ms: 45_000,
        max_market_spread_cents: reward_decimal("10"),
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let plan = live_test_plan(now);
    let order = live_test_open_order("yes_live");
    let mut book = live_test_book("yes_live", now);
    book.asks[0].price = reward_decimal("0.70");
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
    assert!(candidates[0].1.contains("live token spread"));
}

#[test]
fn event_cancel_active_tokens_only_include_updated_order_token() {
    let now = OffsetDateTime::now_utc();
    let plan = live_test_plan(now);
    let yes_order = live_test_open_order("yes_live");
    let unrelated_updated_tokens = HashSet::from(["no_live".to_string()]);
    let matching_updated_tokens = HashSet::from(["yes_live".to_string()]);

    let unrelated = reward_event_cancel_active_order_tokens(
        &[plan.clone()],
        &[yes_order.clone()],
        &unrelated_updated_tokens,
    );
    let matching =
        reward_event_cancel_active_order_tokens(&[plan], &[yes_order], &matching_updated_tokens);

    assert!(unrelated.is_empty());
    assert_eq!(matching, vec!["yes_live".to_string()]);
}

#[test]
fn event_cancel_fast_path_uses_normal_depth_rule_for_all_buy_orders() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        min_depth_usd: reward_decimal("100"),
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let plan = live_test_plan(now);
    let order = live_test_open_order("yes_live");
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

    assert_eq!(candidates.len(), 1);
    assert!(candidates[0].1.contains("depth"));
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
    plan.legs[0].price = reward_decimal("0.60");
    let mut order = live_test_open_order("yes_live");
    order.created_at = now - TimeDuration::seconds(10);
    order.updated_at = order.created_at;
    let books = HashMap::from([("yes_live".to_string(), live_test_book("yes_live", now))]);
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
