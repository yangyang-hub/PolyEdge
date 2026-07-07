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

#[test]
fn reward_fast_reconcile_external_sync_policy_throttles_heavy_calls() {
    let config = RewardBotConfig {
        reconcile_interval_sec: 1,
        min_scoring_check_sec: 1,
        ..RewardBotConfig::default()
    }
    .normalized();
    let mut throttle = RewardExternalSyncThrottle::default();
    let started_at = Instant::now();

    let first = throttle.fast_reconcile_policy(&config, started_at);
    assert!(first.order_statuses);
    assert!(first.reward_earnings);
    assert!(first.managed_scoring);
    assert!(first.open_orders);
    assert!(first.account_snapshot);

    throttle.mark_fast_reconcile(first, started_at);
    let one_second_later =
        throttle.fast_reconcile_policy(&config, started_at + Duration::from_secs(1));
    assert!(!one_second_later.order_statuses);
    assert!(!one_second_later.reward_earnings);
    assert!(!one_second_later.managed_scoring);
    assert!(!one_second_later.open_orders);
    assert!(!one_second_later.account_snapshot);

    let five_seconds_later =
        throttle.fast_reconcile_policy(&config, started_at + Duration::from_secs(5));
    assert!(five_seconds_later.order_statuses);
    assert!(!five_seconds_later.open_orders);
    assert!(!five_seconds_later.managed_scoring);
    assert!(!five_seconds_later.reward_earnings);
    assert!(!five_seconds_later.account_snapshot);

    let fifteen_seconds_later =
        throttle.fast_reconcile_policy(&config, started_at + Duration::from_secs(15));
    assert!(fifteen_seconds_later.open_orders);
    assert!(fifteen_seconds_later.managed_scoring);
    assert!(!fifteen_seconds_later.reward_earnings);
    assert!(!fifteen_seconds_later.account_snapshot);

    let sixty_seconds_later =
        throttle.fast_reconcile_policy(&config, started_at + Duration::from_secs(60));
    assert!(sixty_seconds_later.reward_earnings);
    assert!(sixty_seconds_later.account_snapshot);
}

#[test]
fn reward_live_action_orderbook_tokens_cover_orders_and_eligible_plans() {
    let now = OffsetDateTime::now_utc();
    let mut plan = live_test_plan(now);
    plan.orderbook_token_ids = vec![
        "yes_live".to_string(),
        "no_live".to_string(),
        "extra_live".to_string(),
    ];

    let mut inactive_plan = live_test_plan(now);
    inactive_plan.eligible = false;
    inactive_plan.orderbook_token_ids = vec!["blocked_live".to_string()];

    let mut cancelled_order = live_test_open_order("cancelled_live");
    cancelled_order.status = ManagedRewardOrderStatus::Cancelled;

    let tokens = reward_live_action_orderbook_tokens(
        &[plan, inactive_plan],
        &[live_test_open_order("open_live"), cancelled_order],
    );

    assert_eq!(
        tokens,
        vec![
            "open_live".to_string(),
            "yes_live".to_string(),
            "no_live".to_string(),
            "extra_live".to_string(),
        ]
    );
}

#[test]
fn live_buy_submission_last_look_tokens_keep_single_order_token() {
    let order = live_test_open_order("yes_live");

    let token_ids = live_buy_submission_last_look_token_ids(&order, None);

    assert_eq!(token_ids, vec!["yes_live".to_string()]);
}

#[test]
fn live_buy_submission_last_look_tokens_include_current_plan_tokens() {
    let now = OffsetDateTime::now_utc();
    let order = live_test_open_order("yes_live");
    let plan = live_test_plan(now);

    let token_ids = live_buy_submission_last_look_token_ids(&order, Some(&plan));

    assert_eq!(token_ids, vec!["yes_live".to_string(), "no_live".to_string()]);
}

#[test]
fn live_buy_submission_last_look_missing_token_is_fail_closed() {
    let now = OffsetDateTime::now_utc();
    let token_ids = vec!["yes_live".to_string(), "no_live".to_string()];
    let books = HashMap::from([(
        "yes_live".to_string(),
        live_test_book("yes_live", now),
    )]);

    assert_eq!(
        missing_live_buy_submission_last_look_token(&token_ids, &books),
        Some("no_live")
    );
}

#[test]
fn live_buy_submission_last_look_reprices_to_current_target() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        stale_book_ms: 0,
        max_global_position_usd: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let plan = live_test_plan(now);
    let mut order = live_test_open_order("yes_live");
    order.external_order_id = None;
    order.status = ManagedRewardOrderStatus::Planned;
    order.price = reward_decimal("0.49");
    let open_orders = vec![order.clone()];
    let plans = HashMap::from([(plan.condition_id.as_str(), &plan)]);
    let book_history = HashMap::new();
    let account = live_test_account(Decimal::from(100_u64));
    let context = LiveBuySubmitRiskContext {
        config: &config,
        plans: &plans,
        book_history: &book_history,
        open_orders: &open_orders,
        positions: &[],
        account: &account,
        kill_switch: false,
    };
    let books = HashMap::from([
        ("yes_live".to_string(), live_test_book("yes_live", now)),
        ("no_live".to_string(), live_test_book("no_live", now)),
    ]);

    let last_look_plan =
        live_buy_submission_last_look_plan(&order, context, &books).expect("last-look plan");
    let event = live_buy_submission_last_look_reprice(&mut order, &last_look_plan, context)
        .expect("last-look reprice")
        .expect("reprice event");

    assert_eq!(order.price, reward_decimal("0.48"));
    assert_eq!(
        event.event_type,
        "reward_live_order_pre_submit_last_look_repriced"
    );
}

#[test]
fn live_buy_submission_last_look_checks_ai_cap_without_price_change() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        stale_book_ms: 0,
        max_global_position_usd: Decimal::ZERO,
        ai_strategy_hint_enabled: true,
        ai_strategy_hint_min_confidence: reward_decimal("0.75"),
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let mut plan = live_test_plan(now);
    plan.ai_advisory = Some(live_test_allow_advisory_with_metrics(
        &plan.condition_id,
        json!({
            "strategy_hint": {
                "quote_mode": "double",
                "bid_rank": 1,
                "max_condition_notional_usd": "1"
            }
        }),
        now,
    ));
    let mut order = live_test_open_order("yes_live");
    order.external_order_id = None;
    order.status = ManagedRewardOrderStatus::Planned;
    order.price = reward_decimal("0.49");
    order.size = reward_decimal("5");
    let open_orders = vec![order.clone()];
    let plans = HashMap::from([(plan.condition_id.as_str(), &plan)]);
    let book_history = HashMap::new();
    let account = live_test_account(Decimal::from(100_u64));
    let context = LiveBuySubmitRiskContext {
        config: &config,
        plans: &plans,
        book_history: &book_history,
        open_orders: &open_orders,
        positions: &[],
        account: &account,
        kill_switch: false,
    };

    assert!(!live_buy_submission_last_look_reprice_allowed(
        &order,
        &plan,
        order.price,
        context
    ));
}

#[test]
fn reward_provider_refresh_batch_orderbooks_include_only_advisory_conditions() {
    let advisory_conditions = HashSet::from(["ai_condition".to_string()]);
    let info_only_batch = vec!["info_condition".to_string()];
    let mixed_batch = vec![
        "info_condition".to_string(),
        "ai_condition".to_string(),
    ];

    assert!(reward_provider_refresh_batch_orderbook_conditions(
        &info_only_batch,
        &advisory_conditions,
    )
    .is_empty());
    assert_eq!(
        reward_provider_refresh_batch_orderbook_conditions(&mixed_batch, &advisory_conditions),
        vec!["ai_condition".to_string()]
    );
}

fn live_test_plan(now: OffsetDateTime) -> RewardQuotePlan {
    RewardQuotePlan {
        condition_id: "cond_live".to_string(),
        market_slug: "live-market".to_string(),
        question: "Will the live event happen?".to_string(),
        score: reward_decimal("50"),
        eligible: true,
        pre_ai_eligible: true,
        quote_readiness: polyedge_application::RewardQuoteReadiness::ReadyToQuote,
        reason: "eligible".to_string(),
        strategy_bucket: polyedge_application::RewardStrategyBucket::None,
        strategy_profile: polyedge_application::RewardStrategyProfile::Standard,
        quote_mode: polyedge_application::RewardPlanQuoteMode::Double,
        recommended_quote_mode: Some(polyedge_application::RewardPlanQuoteMode::Double),
        book_metrics: None,
        opportunity_metrics: None,
        ai_advisory: None,
        info_risk: None,
        event_window: None,
        midpoint: Some(reward_decimal("0.50")),
        live_skip_until: None,
        live_skip_reason: None,
        first_quote_observed_at: None,
        ai_advisory_pending_since: None,
        info_risk_pending_since: None,
        total_daily_rate: reward_decimal("25"),
        rewards_max_spread: reward_decimal("8"),
        rewards_min_size: reward_decimal("5"),
        orderbook_token_ids: vec!["yes_live".to_string(), "no_live".to_string()],
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

fn live_test_allow_advisory_with_metrics(
    condition_id: &str,
    metrics: Value,
    now: OffsetDateTime,
) -> RewardMarketAdvisory {
    RewardMarketAdvisory {
        condition_id: condition_id.to_string(),
        provider: polyedge_application::RewardAiProvider::OpenAi,
        request_format: polyedge_application::RewardAiRequestFormat::OpenAiChatCompletions,
        model: "test-model".to_string(),
        input_hash: "hash".to_string(),
        suitability: polyedge_application::RewardAiSuitability::Allow,
        quote_mode: RewardPlanQuoteMode::Double,
        exit_policy: PostFillStrategy::ExitAtMarkup,
        confidence: Decimal::ONE,
        reasons: Vec::new(),
        metrics,
        created_at: now,
        expires_at: now + TimeDuration::hours(1),
    }
}

#[test]
fn reward_ai_advisory_candidates_include_open_orders_positions_and_eligible_plans() {
    let now = OffsetDateTime::now_utc();
    let config = RewardBotConfig {
        ai_request_format: polyedge_application::RewardAiRequestFormat::OpenAiChatCompletions,
        ..RewardBotConfig::default()
    };
    let mut eligible = live_test_plan(now);
    eligible.condition_id = "cond_eligible".to_string();
    eligible.strategy_bucket = RewardStrategyBucket::Standard;

    let mut open_order_plan = live_test_plan(now);
    open_order_plan.condition_id = "cond_open".to_string();
    open_order_plan.strategy_bucket = RewardStrategyBucket::Standard;
    open_order_plan.eligible = false;

    let mut position_plan = live_test_plan(now);
    position_plan.condition_id = "cond_position".to_string();
    position_plan.strategy_bucket = RewardStrategyBucket::Standard;
    position_plan.eligible = false;

    let mut rejected = live_test_plan(now);
    rejected.condition_id = "cond_rejected".to_string();
    rejected.strategy_bucket = RewardStrategyBucket::Standard;
    rejected.eligible = false;
    rejected.reason = "below initial score".to_string();

    let mut open_order = live_test_open_order("yes_live");
    open_order.condition_id = "cond_open".to_string();
    let position = RewardPosition {
        account_id: "reward_live".to_string(),
        condition_id: "cond_position".to_string(),
        token_id: "yes_live".to_string(),
        outcome: "YES".to_string(),
        size: reward_decimal("5"),
        avg_price: reward_decimal("0.50"),
        realized_pnl: Decimal::ZERO,
        updated_at: now,
    };

    let plans = vec![rejected, eligible, position_plan, open_order_plan];
    let pre_ai_eligible_condition_ids = vec![
        "cond_eligible".to_string(),
        "cond_open".to_string(),
        "cond_position".to_string(),
    ];
    let condition_ids = reward_ai_advisory_candidate_condition_ids(
        &plans,
        &[open_order],
        &[position],
        &pre_ai_eligible_condition_ids,
        &config,
        "mimo-v2.5-pro",
        None,
        now,
    );

    assert_eq!(condition_ids, vec!["cond_open", "cond_position", "cond_eligible"]);
}

#[test]
fn reward_ai_advisory_candidates_include_active_exposure_outside_pre_ai_set() {
    let now = OffsetDateTime::now_utc();
    let config = RewardBotConfig {
        ai_advisory_enabled: true,
        ai_request_format: polyedge_application::RewardAiRequestFormat::OpenAiChatCompletions,
        ..RewardBotConfig::default()
    };

    let mut missing = live_test_plan(now);
    missing.condition_id = "cond_missing".to_string();
    missing.strategy_bucket = RewardStrategyBucket::Standard;

    let mut already_admitted = live_test_plan(now);
    already_admitted.condition_id = "cond_admitted".to_string();
    already_admitted.strategy_bucket = RewardStrategyBucket::Standard;
    already_admitted.ai_advisory = Some(RewardMarketAdvisory {
        condition_id: already_admitted.condition_id.clone(),
        provider: polyedge_application::RewardAiProvider::OpenAi,
        request_format: config.ai_request_format,
        model: "mimo-v2.5-pro".to_string(),
        input_hash: "hash_admitted".to_string(),
        suitability: polyedge_application::RewardAiSuitability::Allow,
        quote_mode: polyedge_application::RewardPlanQuoteMode::Double,
        exit_policy: PostFillStrategy::ExitAtMarkup,
        confidence: reward_decimal("0.95"),
        reasons: vec!["cached approval".to_string()],
        metrics: serde_json::json!({}),
        created_at: now,
        expires_at: now + TimeDuration::hours(1),
    });

    let mut hard_rejected = live_test_plan(now);
    hard_rejected.condition_id = "cond_hard_rejected".to_string();
    hard_rejected.strategy_bucket = RewardStrategyBucket::Standard;
    hard_rejected.eligible = false;
    hard_rejected.reason = "market failed non-transient live validation".to_string();

    let mut open_order = live_test_open_order("yes_live");
    open_order.condition_id = "cond_hard_rejected".to_string();

    let plans = vec![missing, already_admitted, hard_rejected];
    let pre_ai_eligible_condition_ids =
        vec!["cond_missing".to_string(), "cond_admitted".to_string()];
    let condition_ids = reward_ai_advisory_candidate_condition_ids(
        &plans,
        &[open_order],
        &[],
        &pre_ai_eligible_condition_ids,
        &config,
        "mimo-v2.5-pro",
        None,
        now,
    );

    assert_eq!(condition_ids, vec!["cond_hard_rejected", "cond_missing"]);
}

#[test]
fn reward_ai_advisory_candidates_include_eligible_legacy_bucket() {
    let now = OffsetDateTime::now_utc();
    let config = RewardBotConfig {
        ai_advisory_enabled: true,
        ..RewardBotConfig::default()
    };
    let plan = legacy_bucket_provider_test_plan(now, "cond_legacy");

    let condition_ids = reward_ai_advisory_candidate_condition_ids(
        &[plan],
        &[],
        &[],
        &["cond_legacy".to_string()],
        &config,
        "mimo-v2.5-pro",
        None,
        now,
    );

    assert_eq!(condition_ids, vec!["cond_legacy"]);
}

#[test]
fn reward_ai_advisory_candidates_keep_ineligible_legacy_bucket_with_exposure() {
    let now = OffsetDateTime::now_utc();
    let config = RewardBotConfig {
        ai_advisory_enabled: true,
        ..RewardBotConfig::default()
    };
    let mut plan = legacy_bucket_provider_test_plan(now, "cond_legacy");
    plan.eligible = false;
    plan.pre_ai_eligible = false;
    let mut open_order = live_test_open_order("yes_live");
    open_order.condition_id = "cond_legacy".to_string();

    let condition_ids = reward_ai_advisory_candidate_condition_ids(
        &[plan],
        &[open_order],
        &[],
        &[],
        &config,
        "mimo-v2.5-pro",
        None,
        now,
    );

    assert_eq!(condition_ids, vec!["cond_legacy"]);
}

#[test]
fn reward_provider_refresh_candidates_prioritize_active_exposure_and_dedupe() {
    let now = OffsetDateTime::now_utc();
    let mut open_order = live_test_open_order("yes_live");
    open_order.condition_id = "cond_open".to_string();
    let position = RewardPosition {
        account_id: "reward_live".to_string(),
        condition_id: "cond_position".to_string(),
        token_id: "yes_position".to_string(),
        outcome: "YES".to_string(),
        size: reward_decimal("5"),
        avg_price: reward_decimal("0.50"),
        realized_pnl: Decimal::ZERO,
        updated_at: now,
    };
    let condition_ids = vec![
        "cond_eligible".to_string(),
        "cond_position".to_string(),
        "cond_candidate".to_string(),
        "cond_open".to_string(),
        "cond_eligible".to_string(),
    ];
    let mut eligible = live_test_plan(now);
    eligible.condition_id = "cond_eligible".to_string();
    eligible.strategy_bucket = RewardStrategyBucket::Standard;
    let mut candidate = live_test_plan(now);
    candidate.condition_id = "cond_candidate".to_string();
    candidate.strategy_bucket = RewardStrategyBucket::Standard;

    let ordered = reward_provider_refresh_candidate_condition_ids(
        &condition_ids,
        &[eligible, candidate],
        &[open_order],
        &[position],
        &RewardBotConfig::default(),
    );

    assert_eq!(
        ordered,
        vec![
            "cond_open",
            "cond_position",
            "cond_eligible",
            "cond_candidate",
        ],
    );
}

#[test]
fn reward_provider_refresh_candidates_keep_legacy_bucket_in_unified_order() {
    let now = OffsetDateTime::now_utc();
    let mut standard = live_test_plan(now);
    standard.condition_id = "cond_standard".to_string();
    standard.strategy_bucket = RewardStrategyBucket::Standard;
    let mut candidate = live_test_plan(now);
    candidate.condition_id = "cond_candidate".to_string();
    candidate.strategy_bucket = RewardStrategyBucket::Standard;

    let legacy = legacy_bucket_provider_test_plan(now, "cond_legacy");
    let ai_only_legacy = legacy_bucket_provider_test_plan(now, "cond_ai_only_legacy");
    let mut legacy_rejected = legacy_bucket_provider_test_plan(now, "cond_legacy_rejected");
    legacy_rejected.eligible = false;
    legacy_rejected.pre_ai_eligible = false;

    let mut open_order = live_test_open_order("yes_live");
    open_order.condition_id = "cond_open".to_string();
    let position = RewardPosition {
        account_id: "reward_live".to_string(),
        condition_id: "cond_position".to_string(),
        token_id: "yes_position".to_string(),
        outcome: "YES".to_string(),
        size: reward_decimal("5"),
        avg_price: reward_decimal("0.50"),
        realized_pnl: Decimal::ZERO,
        updated_at: now,
    };

    let condition_ids = vec![
        "cond_standard".to_string(),
        "cond_candidate".to_string(),
        "cond_legacy".to_string(),
        "cond_legacy_rejected".to_string(),
        "cond_ai_only_legacy".to_string(),
        "cond_open".to_string(),
        "cond_position".to_string(),
    ];
    let ordered = reward_provider_refresh_candidate_condition_ids(
        &condition_ids,
        &[standard, candidate, legacy, ai_only_legacy, legacy_rejected],
        &[open_order],
        &[position],
        &RewardBotConfig::default(),
    );

    assert_eq!(
        ordered,
        vec![
            "cond_open",
            "cond_position",
            "cond_standard",
            "cond_candidate",
            "cond_legacy",
            "cond_ai_only_legacy",
        ],
    );
}

#[test]
fn reward_provider_refresh_candidates_keep_input_order_after_active_exposure() {
    let now = OffsetDateTime::now_utc();
    let mut standard_a = live_test_plan(now);
    standard_a.condition_id = "cond_standard_a".to_string();
    standard_a.strategy_bucket = RewardStrategyBucket::Standard;
    let mut standard_b = live_test_plan(now);
    standard_b.condition_id = "cond_standard_b".to_string();
    standard_b.strategy_bucket = RewardStrategyBucket::Standard;
    let mut standard_c = live_test_plan(now);
    standard_c.condition_id = "cond_standard_c".to_string();
    standard_c.strategy_bucket = RewardStrategyBucket::Standard;
    let mut standard_d = live_test_plan(now);
    standard_d.condition_id = "cond_standard_d".to_string();
    standard_d.strategy_bucket = RewardStrategyBucket::Standard;
    let legacy_a = legacy_bucket_provider_test_plan(now, "cond_legacy_a");
    let legacy_b = legacy_bucket_provider_test_plan(now, "cond_legacy_b");
    let legacy_c = legacy_bucket_provider_test_plan(now, "cond_legacy_c");

    let condition_ids = vec![
        "cond_standard_a".to_string(),
        "cond_legacy_a".to_string(),
        "cond_standard_b".to_string(),
        "cond_legacy_b".to_string(),
        "cond_standard_c".to_string(),
        "cond_standard_d".to_string(),
        "cond_legacy_c".to_string(),
    ];
    let ordered = reward_provider_refresh_candidate_condition_ids(
        &condition_ids,
        &[
            standard_a, standard_b, standard_c, standard_d, legacy_a, legacy_b, legacy_c,
        ],
        &[],
        &[],
        &RewardBotConfig::default(),
    );

    assert_eq!(
        ordered,
        vec![
            "cond_standard_a",
            "cond_legacy_a",
            "cond_standard_b",
            "cond_legacy_b",
            "cond_standard_c",
            "cond_standard_d",
            "cond_legacy_c",
        ],
    );
}

#[test]
fn reward_info_risk_candidates_apply_pre_llm_gate_before_market_candidates() {
    let now = OffsetDateTime::now_utc();
    let config = RewardBotConfig::default();
    let mut standard = live_test_plan(now);
    standard.condition_id = "cond_standard".to_string();
    standard.strategy_bucket = RewardStrategyBucket::Standard;

    let legacy = legacy_bucket_provider_test_plan(now, "cond_legacy");
    let market_without_plan = reward_test_market(now, "cond_market_only");
    let markets = vec![
        reward_test_market(now, "cond_standard"),
        reward_test_market(now, "cond_legacy"),
        market_without_plan,
    ];

    let condition_ids = reward_info_risk_candidate_conditions(
        &markets,
        &[standard, legacy],
        &[],
        &[],
        &config,
        "test-model",
        None,
        now,
    );

    assert_eq!(condition_ids, vec!["cond_standard", "cond_legacy"]);
}

#[test]
fn reward_info_risk_candidates_keep_active_exposure_despite_unified_gate() {
    let now = OffsetDateTime::now_utc();
    let config = RewardBotConfig::default();
    let mut legacy = legacy_bucket_provider_test_plan(now, "cond_legacy");
    legacy.eligible = false;
    legacy.pre_ai_eligible = false;
    let mut open_order = live_test_open_order("yes_live");
    open_order.condition_id = "cond_legacy".to_string();

    let condition_ids = reward_info_risk_candidate_conditions(
        &[reward_test_market(now, "cond_legacy")],
        &[legacy],
        &[open_order],
        &[],
        &config,
        "test-model",
        None,
        now,
    );

    assert_eq!(condition_ids, vec!["cond_legacy"]);
}

fn legacy_bucket_provider_test_plan(
    now: OffsetDateTime,
    condition_id: &str,
) -> RewardQuotePlan {
    let mut plan = live_test_plan(now);
    plan.condition_id = condition_id.to_string();
    plan.strategy_bucket = RewardStrategyBucket::Standard;
    plan.eligible = true;
    plan.pre_ai_eligible = true;
    plan
}

fn reward_test_market(now: OffsetDateTime, condition_id: &str) -> RewardMarket {
    RewardMarket {
        condition_id: condition_id.to_string(),
        question: "Test market?".to_string(),
        market_slug: condition_id.to_string(),
        event_slug: "event".to_string(),
        category: "test".to_string(),
        image: String::new(),
        rewards_max_spread: reward_decimal("3"),
        rewards_min_size: reward_decimal("5"),
        total_daily_rate: reward_decimal("10"),
        liquidity_usd: reward_decimal("1000"),
        volume_24h_usd: reward_decimal("500"),
        market_spread_cents: reward_decimal("1"),
        end_at: None,
        ambiguity_level: String::new(),
        market_synced_at: Some(now),
        tokens: Vec::new(),
        active: true,
        updated_at: now,
    }
}

#[test]
fn reward_ai_advisory_incremental_apply_only_updates_matching_plan() {
    let now = OffsetDateTime::now_utc();
    let mut target = live_test_plan(now);
    target.condition_id = "cond_ai".to_string();
    let mut waiting = live_test_plan(now);
    waiting.condition_id = "cond_waiting".to_string();
    let config = RewardBotConfig {
        ai_advisory_enabled: true,
        ai_request_format: polyedge_application::RewardAiRequestFormat::OpenAiChatCompletions,
        ..RewardBotConfig::default()
    };
    let advisory = RewardMarketAdvisory {
        condition_id: target.condition_id.clone(),
        provider: polyedge_application::RewardAiProvider::OpenAi,
        request_format: config.ai_request_format,
        model: "mimo-v2.5-pro".to_string(),
        input_hash: "hash_ai".to_string(),
        suitability: polyedge_application::RewardAiSuitability::Allow,
        quote_mode: polyedge_application::RewardPlanQuoteMode::Double,
        exit_policy: PostFillStrategy::ExitAtMarkup,
        confidence: reward_decimal("0.95"),
        reasons: vec!["market passed advisory filter".to_string()],
        metrics: serde_json::json!({}),
        created_at: now,
        expires_at: now + TimeDuration::hours(1),
    };

    let mut plans = vec![target, waiting];
    let applied = apply_reward_ai_advisory_to_quote_plan(
        &mut plans,
        &config,
        "cond_ai",
        advisory,
        reward_decimal("0.65"),
    );

    assert!(applied);
    assert!(plans[0].ai_advisory.is_some());
    assert!(plans[0].eligible);
    assert!(plans[1].ai_advisory.is_none());
    assert!(plans[1].eligible);
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
        confirmed_at: observed_at,
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
        strategy_bucket: RewardStrategyBucket::Standard,
        strategy_profile: RewardStrategyProfile::Standard,
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

fn live_test_position(token_id: &str, size: &str, avg_price: &str) -> RewardPosition {
    RewardPosition {
        account_id: "reward_live".to_string(),
        condition_id: "cond_live".to_string(),
        token_id: token_id.to_string(),
        outcome: "YES".to_string(),
        size: reward_decimal(size),
        avg_price: reward_decimal(avg_price),
        realized_pnl: Decimal::ZERO,
        updated_at: OffsetDateTime::now_utc(),
    }
}

#[test]
fn external_inventory_sync_plans_original_price_sell_exit() {
    let position = live_test_position("yes_inventory", "12.345", "0.491");

    let updates = external_inventory_original_price_exit_updates(
        "reward_live",
        std::slice::from_ref(&position),
        &[],
        &[],
        "trc_inventory_exit",
    );

    assert_eq!(updates.len(), 1);
    let (order, event) = &updates[0];
    assert_eq!(order.account_id, "reward_live");
    assert_eq!(order.token_id, "yes_inventory");
    assert_eq!(order.side, RewardOrderSide::Sell);
    assert_eq!(order.status, ManagedRewardOrderStatus::ExitPending);
    assert_eq!(order.price, reward_decimal("0.50"));
    assert_eq!(order.size, reward_decimal("12.34"));
    assert!(order.external_order_id.is_none());
    assert_eq!(order.reason, "external inventory original-price exit");
    assert_eq!(event.event_type, "reward_live_inventory_exit_planned");
}

#[test]
fn external_inventory_sync_does_not_duplicate_existing_sell_exit() {
    let position = live_test_position("yes_inventory", "12.345", "0.49");
    let mut sell = live_test_open_order("yes_inventory");
    sell.side = RewardOrderSide::Sell;
    sell.status = ManagedRewardOrderStatus::ExitPending;
    sell.external_order_id = None;

    let updates = external_inventory_original_price_exit_updates(
        "reward_live",
        &[position],
        &[sell],
        &[],
        "trc_inventory_exit",
    );

    assert!(updates.is_empty());
}

fn live_test_account(available_usd: Decimal) -> RewardAccountState {
    let mut account = RewardAccountState::fresh(
        "reward_live",
        Decimal::from(100_u64),
        OffsetDateTime::now_utc(),
    );
    account.available_usd = available_usd;
    account
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

    let mut plans = vec![first_plan, second_plan];
    let (orders, _) = live_placement_orders(
        &config,
        &live_test_account(Decimal::from(5_u64)),
        &mut plans,
        &books,
        &HashMap::new(),
        &[],
        &[],
        false,
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
    let mut plan = live_test_plan(now);
    plan.rewards_min_size = reward_decimal("50");
    let books = HashMap::from([
        ("yes_live".to_string(), live_test_book("yes_live", now)),
        ("no_live".to_string(), live_test_book("no_live", now)),
    ]);

    let mut plans = vec![plan];
    let (orders, plans_changed) = live_placement_orders(
        &config,
        &live_test_account(reward_decimal("47.99")),
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
    assert!(!plans[0].eligible);
    assert_eq!(
        plans[0].quote_mode,
        polyedge_application::RewardPlanQuoteMode::None
    );
    assert!(plans[0]
        .reason
        .contains("live funding below rewards minimum"));
}

#[test]
fn live_funding_precheck_blocks_underfunded_new_condition_before_provider() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        stale_book_ms: 0,
        max_markets: 1,
        max_open_orders: 2,
        max_global_position_usd: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let mut plan = live_test_plan(now);
    plan.rewards_min_size = reward_decimal("50");
    let books = HashMap::from([
        ("yes_live".to_string(), live_test_book("yes_live", now)),
        ("no_live".to_string(), live_test_book("no_live", now)),
    ]);

    let mut plans = vec![plan];
    let blocked = apply_live_funding_precheck(
        &config,
        &live_test_account(reward_decimal("47.99")),
        &mut plans,
        &books,
        &[],
        &[],
    );
    let mut pre_ai_eligible = Vec::new();
    mark_pre_ai_eligible_quote_plans(&mut plans, &mut pre_ai_eligible);

    assert_eq!(blocked, 1);
    assert!(!plans[0].eligible);
    assert!(!plans[0].pre_ai_eligible);
    assert!(pre_ai_eligible.is_empty());
    assert!(plans[0]
        .reason
        .contains("live funding below rewards minimum"));
}

#[test]
fn live_funding_precheck_keeps_active_exposure_in_provider_queue() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        stale_book_ms: 0,
        max_markets: 1,
        max_open_orders: 2,
        max_global_position_usd: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let mut plan = live_test_plan(now);
    plan.rewards_min_size = reward_decimal("50");
    let books = HashMap::from([
        ("yes_live".to_string(), live_test_book("yes_live", now)),
        ("no_live".to_string(), live_test_book("no_live", now)),
    ]);
    let open_order = live_test_open_order("yes_live");

    let mut plans = vec![plan];
    let blocked = apply_live_funding_precheck(
        &config,
        &live_test_account(reward_decimal("47.99")),
        &mut plans,
        &books,
        std::slice::from_ref(&open_order),
        &[],
    );
    let mut pre_ai_eligible = Vec::new();
    mark_pre_ai_eligible_quote_plans(&mut plans, &mut pre_ai_eligible);

    assert_eq!(blocked, 0);
    assert!(plans[0].eligible);
    assert!(plans[0].pre_ai_eligible);
    assert_eq!(pre_ai_eligible, vec!["cond_live".to_string()]);
}

#[test]
fn live_placement_uses_wallet_balance_instead_of_config_quote_budgets() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        stale_book_ms: 0,
        max_markets: 1,
        max_open_orders: 2,
        per_market_usd: reward_decimal("1"),
        quote_size_usd: Decimal::ZERO,
        max_position_usd: reward_decimal("100"),
        max_global_position_usd: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let mut plan = live_test_plan(now);
    plan.rewards_min_size = reward_decimal("50");
    let books = HashMap::from([
        ("yes_live".to_string(), live_test_book("yes_live", now)),
        ("no_live".to_string(), live_test_book("no_live", now)),
    ]);

    let mut plans = vec![plan];
    let (orders, _) = live_placement_orders(
        &config,
        &live_test_account(reward_decimal("100")),
        &mut plans,
        &books,
        &HashMap::new(),
        &[],
        &[],
        false,
        "trc_live_test",
    );

    assert_eq!(orders.len(), 2);
    assert!(orders.iter().all(|order| order.size >= reward_decimal("50")));
}

#[test]
fn live_placement_hard_blocks_ai_notional_cap_below_minimum_quote() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        stale_book_ms: 0,
        max_markets: 1,
        max_open_orders: 2,
        max_global_position_usd: Decimal::ZERO,
        ai_strategy_hint_enabled: true,
        ai_strategy_hint_min_confidence: reward_decimal("0.75"),
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let mut plan = live_test_plan(now);
    plan.ai_advisory = Some(live_test_allow_advisory_with_metrics(
        &plan.condition_id,
        json!({
            "strategy_hint": {
                "quote_mode": "double",
                "bid_rank": 1,
                "max_condition_notional_usd": "1"
            }
        }),
        now,
    ));
    let books = HashMap::from([
        ("yes_live".to_string(), live_test_book("yes_live", now)),
        ("no_live".to_string(), live_test_book("no_live", now)),
    ]);

    let mut plans = vec![plan];
    let (orders, plans_changed) = live_placement_orders(
        &config,
        &live_test_account(reward_decimal("100")),
        &mut plans,
        &books,
        &HashMap::new(),
        &[],
        &[],
        false,
        "trc_ai_cap",
    );

    assert!(orders.is_empty());
    assert!(plans_changed);
    assert!(!plans[0].eligible);
    assert!(plans[0].reason.contains("AI notional cap below required rewards quote"));
}

#[test]
fn live_placement_profile_order_cap_only_skips_that_profile() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        stale_book_ms: 0,
        max_markets: 4,
        max_open_orders: 1,
        max_global_position_usd: Decimal::ZERO,
        balanced_merge_enabled: true,
        balanced_merge_max_markets: 1,
        balanced_merge_max_open_orders: 2,
        balanced_merge_min_edge_cents: Decimal::ZERO,
        balanced_merge_max_unpaired_position_usd: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let mut standard_plan = live_test_plan(now);
    standard_plan.condition_id = "cond_standard_cap".to_string();
    standard_plan.market_slug = "standard-cap".to_string();
    standard_plan.legs[0].token_id = "yes_standard_cap".to_string();
    standard_plan.legs[1].token_id = "no_standard_cap".to_string();
    standard_plan.orderbook_token_ids = vec![
        "yes_standard_cap".to_string(),
        "no_standard_cap".to_string(),
    ];
    let mut balanced_plan = live_test_plan(now);
    balanced_plan.condition_id = "cond_balanced_cap".to_string();
    balanced_plan.market_slug = "balanced-cap".to_string();
    balanced_plan.strategy_profile = RewardStrategyProfile::BalancedMerge;
    balanced_plan.legs[0].token_id = "yes_balanced_cap".to_string();
    balanced_plan.legs[1].token_id = "no_balanced_cap".to_string();
    balanced_plan.orderbook_token_ids = vec![
        "yes_balanced_cap".to_string(),
        "no_balanced_cap".to_string(),
    ];
    let books = HashMap::from([
        (
            "yes_standard_cap".to_string(),
            live_test_book("yes_standard_cap", now),
        ),
        (
            "no_standard_cap".to_string(),
            live_test_book("no_standard_cap", now),
        ),
        (
            "yes_balanced_cap".to_string(),
            live_test_book("yes_balanced_cap", now),
        ),
        (
            "no_balanced_cap".to_string(),
            live_test_book("no_balanced_cap", now),
        ),
    ]);
    let mut existing_standard = live_test_open_order("yes_existing_standard_cap");
    existing_standard.condition_id = "cond_existing_standard_cap".to_string();
    existing_standard.strategy_profile = RewardStrategyProfile::Standard;

    let mut plans = vec![standard_plan, balanced_plan];
    let (orders, _) = live_placement_orders(
        &config,
        &live_test_account(reward_decimal("100")),
        &mut plans,
        &books,
        &HashMap::new(),
        &[existing_standard],
        &[],
        false,
        "trc_profile_cap",
    );

    assert_eq!(orders.len(), 2);
    assert!(orders
        .iter()
        .all(|order| order.strategy_profile == RewardStrategyProfile::BalancedMerge));
}

#[test]
fn live_placement_waits_for_fresh_orderbook_without_long_skip() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        stale_book_ms: 45_000,
        max_markets: 1,
        max_open_orders: 2,
        max_global_position_usd: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let plan = live_test_plan(now);

    let mut plans = vec![plan];
    let (orders, plans_changed) = live_placement_orders(
        &config,
        &live_test_account(Decimal::from(20_u64)),
        &mut plans,
        &HashMap::new(),
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
    assert!(plans[0].live_skip_reason.is_none());
    assert!(
        plans[0]
            .reason
            .contains("waiting for fresh orderbook data from subscription")
    );
}

#[test]
fn allocate_registration_buckets_keeps_eligible_when_active_overlaps() {
    // Regression: a shared cross-source `seen` set previously emptied
    // rewards_eligible whenever active positions overlapped eligible tokens,
    // which deleted the source and drove a WS-rebuild cancel/re-place
    // oscillation. Each source must now register its full set independently;
    // cross-source dedup is left to the orderbook registry aggregation layer.
    let buckets = allocate_registration_buckets(
        vec!["t1".to_string()],
        Vec::new(),
        vec!["t1".to_string(), "t2".to_string()],
        Vec::new(),
        10,
        10,
    );
    assert_eq!(buckets.active, vec!["t1".to_string()]);
    assert_eq!(
        buckets.eligible,
        vec!["t1".to_string(), "t2".to_string()]
    );
    assert!(buckets.exec.is_empty());
    assert!(buckets.candidate.is_empty());
}

#[test]
fn pre_ai_eligible_plan_keeps_orderbook_tokens_after_ai_gate_clears_legs() {
    let now = OffsetDateTime::now_utc();
    let mut plans = vec![live_test_plan(now)];
    plans[0].orderbook_token_ids.clear();

    let mut pre_ai_condition_ids = Vec::new();
    mark_pre_ai_eligible_quote_plans(&mut plans, &mut pre_ai_condition_ids);

    assert_eq!(pre_ai_condition_ids, vec!["cond_live".to_string()]);
    assert!(plans[0].pre_ai_eligible);
    assert_eq!(
        plans[0].orderbook_token_ids,
        vec!["yes_live".to_string(), "no_live".to_string()]
    );

    plans[0].eligible = false;
    plans[0].quote_mode = polyedge_application::RewardPlanQuoteMode::None;
    plans[0].legs.clear();

    let buckets = allocate_registration_buckets(
        Vec::new(),
        Vec::new(),
        plans[0].orderbook_token_ids.clone(),
        Vec::new(),
        10,
        0,
    );
    assert_eq!(
        buckets.eligible,
        vec!["yes_live".to_string(), "no_live".to_string()]
    );
    assert!(buckets.candidate.is_empty());
}

#[test]
fn allocate_registration_buckets_caps_each_source_independently() {
    let active = (0..5).map(|i| format!("a{i}")).collect::<Vec<_>>();
    let eligible = (0..5).map(|i| format!("e{i}")).collect::<Vec<_>>();
    let buckets = allocate_registration_buckets(active, Vec::new(), eligible, Vec::new(), 3, 10);
    assert_eq!(buckets.active.len(), 3);
    assert_eq!(buckets.eligible.len(), 3);
}

#[test]
fn allocate_registration_buckets_caps_candidate_by_candidate_cap() {
    let candidate = (0..50).map(|i| format!("c{i}")).collect::<Vec<_>>();
    let buckets =
        allocate_registration_buckets(Vec::new(), Vec::new(), Vec::new(), candidate, 100, 10);
    assert_eq!(buckets.candidate.len(), 10);

    let candidate = (0..5).map(|i| format!("c{i}")).collect::<Vec<_>>();
    let buckets =
        allocate_registration_buckets(Vec::new(), Vec::new(), Vec::new(), candidate, 100, 0);
    assert!(buckets.candidate.is_empty());
}

#[test]
fn allocate_registration_buckets_dedupes_within_source_and_handles_empty() {
    let buckets = allocate_registration_buckets(
        vec![
            "a".to_string(),
            "a".to_string(),
            "  ".to_string(),
            "b".to_string(),
        ],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        10,
        10,
    );
    assert_eq!(buckets.active, vec!["a".to_string(), "b".to_string()]);

    let buckets =
        allocate_registration_buckets(Vec::new(), Vec::new(), Vec::new(), Vec::new(), 10, 10);
    assert!(buckets.active.is_empty());
    assert!(buckets.exec.is_empty());
    assert!(buckets.eligible.is_empty());
    assert!(buckets.candidate.is_empty());
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

    let mut plans = vec![plan];
    let (orders, _) = live_placement_orders(
        &config,
        &live_test_account(reward_decimal("12.19")),
        &mut plans,
        &books,
        &HashMap::new(),
        &[existing],
        &[],
        false,
        "trc_live_test",
    );

    assert!(orders.is_empty());
}

#[test]
fn live_placement_reserves_unmanaged_external_buy_notional() {
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
    let mut account = live_test_account(Decimal::from(25_u64));
    account.external_buy_notional = Decimal::from(21_u64);
    // Funding reads the snapshot-frozen `unmanaged_external_buy_notional`
    // directly (see account_sync.rs). Simulate a snapshot that observed 21 of
    // external (non-managed) buy occupancy so funding reserves against it.
    account.unmanaged_external_buy_notional = Decimal::from(21_u64);

    let mut plans = vec![plan];
    let (orders, _) = live_placement_orders(
        &config,
        &account,
        &mut plans,
        &books,
        &HashMap::new(),
        &[],
        &[],
        false,
        "trc_live_test",
    );

    assert!(orders.is_empty());
}

#[test]
fn live_placement_does_not_double_reserve_managed_external_buys() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        stale_book_ms: 0,
        max_markets: 1,
        max_open_orders: 2,
        max_global_position_usd: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let mut plan = live_test_plan(now);
    plan.quote_mode = polyedge_application::RewardPlanQuoteMode::SingleNo;
    let books = HashMap::from([
        ("yes_live".to_string(), live_test_book("yes_live", now)),
        ("no_live".to_string(), live_test_book("no_live", now)),
    ]);
    let existing = live_test_open_order("yes_live");
    let mut account = live_test_account(Decimal::from(25_u64));
    account.external_buy_notional = reward_decimal("9.8");

    let mut plans = vec![plan];
    let (orders, _) = live_placement_orders(
        &config,
        &account,
        &mut plans,
        &books,
        &HashMap::new(),
        &[existing],
        &[],
        false,
        "trc_live_test",
    );

    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].token_id, "no_live");
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
fn balanced_merge_post_fill_does_not_plan_sell_exit() {
    let mut entry = live_test_open_order("yes_live");
    entry.strategy_profile = RewardStrategyProfile::BalancedMerge;
    let positions = HashMap::from([(
        "yes_live".to_string(),
        RewardPosition {
            account_id: entry.account_id.clone(),
            condition_id: entry.condition_id.clone(),
            token_id: entry.token_id.clone(),
            outcome: entry.outcome.clone(),
            size: Decimal::from(5_u64),
            avg_price: entry.price,
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
        "trc_balanced_merge",
    );

    assert!(updates.is_empty());
}

#[tokio::test]
async fn balanced_merge_fill_creates_merge_intent_for_paired_positions() {
    let state = test_state(SystemMode::LiveAuto);
    let now = OffsetDateTime::now_utc();
    let mut entry = live_test_open_order("yes_live");
    entry.strategy_profile = RewardStrategyProfile::BalancedMerge;
    entry.outcome = "Yes".to_string();
    let fill = RewardFill {
        id: "rewfill_pm_trade_pair_pm_yes_live".to_string(),
        order_id: entry.id.clone(),
        account_id: entry.account_id.clone(),
        condition_id: entry.condition_id.clone(),
        token_id: entry.token_id.clone(),
        outcome: entry.outcome.clone(),
        side: RewardOrderSide::Buy,
        price: entry.price,
        size: Decimal::from(5_u64),
        notional_usd: reward_decimal("2.45"),
        role: RewardFillRole::Maker,
        realized_pnl: Decimal::ZERO,
        reason: "test fill".to_string(),
        trace_id: "trc_balanced_merge".to_string(),
        created_at: now,
    };
    let positions = HashMap::from([
        (
            "yes_live".to_string(),
            RewardPosition {
                account_id: entry.account_id.clone(),
                condition_id: entry.condition_id.clone(),
                token_id: "yes_live".to_string(),
                outcome: "Yes".to_string(),
                size: Decimal::from(5_u64),
                avg_price: reward_decimal("0.49"),
                realized_pnl: Decimal::ZERO,
                updated_at: now,
            },
        ),
        (
            "no_live".to_string(),
            RewardPosition {
                account_id: entry.account_id.clone(),
                condition_id: entry.condition_id.clone(),
                token_id: "no_live".to_string(),
                outcome: "No".to_string(),
                size: Decimal::from(7_u64),
                avg_price: reward_decimal("0.48"),
                realized_pnl: Decimal::ZERO,
                updated_at: now,
            },
        ),
    ]);

    let (intents, events) = plan_live_balanced_merge_intent(
        &state,
        &RewardBotConfig::default(),
        &entry,
        &fill,
        &positions,
        "trc_balanced_merge",
    )
    .await
    .expect("balanced merge intent");

    assert_eq!(intents.len(), 1);
    assert_eq!(intents[0].id, "rewmerge_rewfill_pm_trade_pair_pm_yes_live");
    assert_eq!(intents[0].status, RewardMergeIntentStatus::Unsupported);
    assert_eq!(intents[0].merge_size, Decimal::from(5_u64));
    assert_eq!(intents[0].yes_token_id, "yes_live");
    assert_eq!(intents[0].no_token_id, "no_live");
    assert_eq!(events[0].event_type, "reward_live_balanced_merge_intent_created");
}

#[tokio::test]
async fn balanced_merge_auto_discovers_existing_paired_positions() {
    let state = test_state(SystemMode::LiveAuto);
    let now = OffsetDateTime::now_utc();
    let config = RewardBotConfig {
        balanced_merge_enabled: true,
        ..RewardBotConfig::default()
    };
    let positions = vec![
        RewardPosition {
            account_id: "reward_live".to_string(),
            condition_id: "cond_live".to_string(),
            token_id: "yes_live".to_string(),
            outcome: "Yes".to_string(),
            size: Decimal::from(4_u64),
            avg_price: reward_decimal("0.49"),
            realized_pnl: Decimal::ZERO,
            updated_at: now,
        },
        RewardPosition {
            account_id: "reward_live".to_string(),
            condition_id: "cond_live".to_string(),
            token_id: "no_live".to_string(),
            outcome: "No".to_string(),
            size: Decimal::from(6_u64),
            avg_price: reward_decimal("0.48"),
            realized_pnl: Decimal::ZERO,
            updated_at: now,
        },
    ];

    let (intents, events) = plan_live_balanced_merge_intents_for_positions(
        &state,
        &config,
        &positions,
        "trc_balanced_merge_auto",
    )
    .await
    .expect("auto balanced merge intent");

    assert_eq!(intents.len(), 1);
    assert!(intents[0].id.starts_with("rewmerge_auto_"));
    assert_eq!(intents[0].status, RewardMergeIntentStatus::Unsupported);
    assert_eq!(intents[0].merge_size, Decimal::from(4_u64));
    assert_eq!(intents[0].yes_token_id, "yes_live");
    assert_eq!(intents[0].no_token_id, "no_live");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, "reward_live_balanced_merge_intent_created");
}

#[test]
fn configured_post_fill_exit_ignores_ai_hold_policy_and_uses_entry_price() {
    let now = OffsetDateTime::now_utc();
    let mut entry = live_test_open_order("yes_live");
    entry.price = reward_decimal("0.49");
    let config = RewardBotConfig {
        ai_advisory_enabled: true,
        selection_mode: polyedge_application::RewardSelectionMode::Enforce,
        post_fill_strategy: PostFillStrategy::ExitAtMarkup,
        exit_markup_cents: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let mut plan = live_test_plan(now);
    plan.ai_advisory = Some(RewardMarketAdvisory {
        condition_id: entry.condition_id.clone(),
        provider: polyedge_application::RewardAiProvider::OpenAi,
        request_format: polyedge_application::RewardAiRequestFormat::OpenAiChatCompletions,
        model: "test-model".to_string(),
        input_hash: "hash".to_string(),
        suitability: polyedge_application::RewardAiSuitability::Allow,
        quote_mode: RewardPlanQuoteMode::Double,
        exit_policy: PostFillStrategy::HoldAndRequote,
        confidence: Decimal::ONE,
        reasons: Vec::new(),
        metrics: json!({}),
        created_at: now,
        expires_at: now + TimeDuration::hours(1),
    });
    let positions = HashMap::from([(
        "yes_live".to_string(),
        RewardPosition {
            account_id: entry.account_id.clone(),
            condition_id: entry.condition_id.clone(),
            token_id: entry.token_id.clone(),
            outcome: entry.outcome.clone(),
            size: Decimal::from(5_u64),
            avg_price: reward_decimal("0.52"),
            realized_pnl: Decimal::ZERO,
            updated_at: now,
        },
    )]);

    let updates = plan_live_post_fill_orders(
        &config,
        &[plan],
        &entry,
        Decimal::from(5_u64),
        &positions,
        &HashMap::new(),
        Decimal::ZERO,
        "trc_exit_plan",
    );

    let LiveRewardOrderUpdate::Changed(exit, _) = &updates[0] else {
        panic!("configured post-fill exit must create a sell intent");
    };
    assert_eq!(exit.status, ManagedRewardOrderStatus::ExitPending);
    assert_eq!(exit.price, entry.price);
    assert_eq!(exit.size, Decimal::from(5_u64));
}

#[test]
fn hold_and_requote_plans_original_price_post_only_exit() {
    let now = OffsetDateTime::now_utc();
    let mut entry = live_test_open_order("yes_live");
    entry.price = reward_decimal("0.49");
    let config = RewardBotConfig {
        post_fill_strategy: PostFillStrategy::HoldAndRequote,
        exit_markup_cents: reward_decimal("5"),
        ..RewardBotConfig::default()
    };
    let positions = HashMap::from([(
        "yes_live".to_string(),
        RewardPosition {
            account_id: entry.account_id.clone(),
            condition_id: entry.condition_id.clone(),
            token_id: entry.token_id.clone(),
            outcome: entry.outcome.clone(),
            size: Decimal::from(5_u64),
            avg_price: reward_decimal("0.52"),
            realized_pnl: Decimal::ZERO,
            updated_at: now,
        },
    )]);

    let updates = plan_live_post_fill_orders(
        &config,
        &[],
        &entry,
        Decimal::from(5_u64),
        &positions,
        &HashMap::new(),
        Decimal::ZERO,
        "trc_hold_requote",
    );

    let LiveRewardOrderUpdate::Changed(exit, event) = &updates[0] else {
        panic!("hold-and-requote must create an original-price sell intent");
    };
    assert_eq!(exit.status, ManagedRewardOrderStatus::ExitPending);
    assert_eq!(exit.price, entry.price);
    assert_eq!(exit.size, Decimal::from(5_u64));
    assert!(deferred_live_exit_is_post_only(exit));
    assert_eq!(event.event_type, "reward_live_hold_requote_exit_planned");
}

#[test]
fn post_fill_post_only_exit_uses_best_ask_submission_when_best_bid_crosses_floor() {
    let now = OffsetDateTime::now_utc();
    let mut entry = live_test_open_order("yes_live");
    entry.price = reward_decimal("0.64");
    let config = RewardBotConfig {
        post_fill_strategy: PostFillStrategy::HoldAndRequote,
        ..RewardBotConfig::default()
    };
    let mut book = live_test_book("yes_live", now);
    book.bids[0].price = reward_decimal("0.65");
    book.asks[0].price = reward_decimal("0.66");
    let positions = HashMap::from([(
        "yes_live".to_string(),
        RewardPosition {
            account_id: entry.account_id.clone(),
            condition_id: entry.condition_id.clone(),
            token_id: entry.token_id.clone(),
            outcome: entry.outcome.clone(),
            size: Decimal::from(5_u64),
            avg_price: entry.price,
            realized_pnl: Decimal::ZERO,
            updated_at: now,
        },
    )]);
    let books = HashMap::from([("yes_live".to_string(), book)]);

    let updates = plan_live_post_fill_orders(
        &config,
        &[],
        &entry,
        Decimal::from(5_u64),
        &positions,
        &books,
        Decimal::ZERO,
        "trc_hold_requote",
    );

    let LiveRewardOrderUpdate::Changed(exit, _) = &updates[0] else {
        panic!("hold-and-requote must create a sell intent");
    };
    assert_eq!(exit.price, reward_decimal("0.64"));
    assert!(deferred_live_exit_is_post_only(exit));
    assert_eq!(
        reward_post_only_exit_submission_price(exit, &books)
            .expect("best ask provides a resting maker price"),
        RewardPostOnlyExitSubmissionPrice {
            price: reward_decimal("0.66"),
            crossing_best_bid: Some(reward_decimal("0.65")),
            best_ask: Some(reward_decimal("0.66")),
        }
    );
}

#[test]
fn post_only_exit_keeps_floor_submission_when_floor_does_not_cross_bid() {
    let now = OffsetDateTime::now_utc();
    let mut book = live_test_book("yes_live", now);
    book.bids[0].price = reward_decimal("0.63");
    book.asks[0].price = reward_decimal("0.66");
    let mut order = live_test_open_order("yes_live");
    order.side = RewardOrderSide::Sell;
    order.status = ManagedRewardOrderStatus::ExitPending;
    order.price = reward_decimal("0.64");
    order.reason = "post-only hold-and-requote original-price exit".to_string();
    let books = HashMap::from([("yes_live".to_string(), book)]);

    assert_eq!(
        reward_post_only_exit_submission_price(&order, &books)
            .expect("floor should be a resting maker price"),
        RewardPostOnlyExitSubmissionPrice {
            price: reward_decimal("0.64"),
            crossing_best_bid: None,
            best_ask: Some(reward_decimal("0.66")),
        }
    );
}

#[test]
fn flatten_immediately_plans_non_post_only_exit_at_non_loss_bid() {
    let now = OffsetDateTime::now_utc();
    let mut entry = live_test_open_order("yes_live");
    entry.price = reward_decimal("0.49");
    let config = RewardBotConfig {
        post_fill_strategy: PostFillStrategy::FlattenImmediately,
        ..RewardBotConfig::default()
    };
    let mut book = live_test_book("yes_live", now);
    book.bids[0].price = reward_decimal("0.53");
    let positions = HashMap::from([(
        "yes_live".to_string(),
        RewardPosition {
            account_id: entry.account_id.clone(),
            condition_id: entry.condition_id.clone(),
            token_id: entry.token_id.clone(),
            outcome: entry.outcome.clone(),
            size: Decimal::from(5_u64),
            avg_price: reward_decimal("0.52"),
            realized_pnl: Decimal::ZERO,
            updated_at: now,
        },
    )]);
    let books = HashMap::from([("yes_live".to_string(), book)]);

    let updates = plan_live_post_fill_orders(
        &config,
        &[],
        &entry,
        Decimal::from(5_u64),
        &positions,
        &books,
        Decimal::ZERO,
        "trc_flatten",
    );

    let LiveRewardOrderUpdate::Changed(exit, event) = &updates[0] else {
        panic!("flatten must create a sell intent");
    };
    assert_eq!(exit.status, ManagedRewardOrderStatus::ExitPending);
    assert_eq!(exit.price, reward_decimal("0.52"));
    assert!(!deferred_live_exit_is_post_only(exit));
    assert_eq!(event.event_type, "reward_live_flatten_planned");
    assert_eq!(
        reward_flatten_submission_price(exit, &books).expect("best bid meets floor"),
        reward_decimal("0.53")
    );
}

#[test]
fn flatten_immediately_defers_when_best_bid_is_below_floor() {
    let now = OffsetDateTime::now_utc();
    let mut book = live_test_book("yes_live", now);
    book.bids[0].price = reward_decimal("0.51");
    let mut order = live_test_open_order("yes_live");
    order.side = RewardOrderSide::Sell;
    order.status = ManagedRewardOrderStatus::ExitPending;
    order.price = reward_decimal("0.52");
    order.reason = "flatten immediately at non-loss floor".to_string();
    let books = HashMap::from([("yes_live".to_string(), book)]);

    let reason = reward_flatten_submission_price(&order, &books)
        .expect_err("flatten should wait below non-loss floor");

    assert!(reason.contains(LIVE_EXIT_FLATTEN_DEFERRED_MARKER));
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
fn authoritative_cancel_rejection_closes_local_order() {
    assert!(polymarket_cancel_rejection_confirms_order_not_open(
        &PolymarketOrderRejection {
            code: "POLYMARKET_ORDER_CANCEL_REJECTED".to_string(),
            message: "order can't be found - already canceled or matched".to_string(),
        }
    ));
    assert!(!polymarket_cancel_rejection_confirms_order_not_open(
        &PolymarketOrderRejection {
            code: "POLYMARKET_ORDER_CANCEL_REJECTED".to_string(),
            message: "temporary cancel service error".to_string(),
        }
    ));
}

#[test]
fn active_remote_sell_balance_rejection_supersedes_local_exit() {
    assert!(polymarket_rejection_reports_balance_reserved_by_active_orders(
        &PolymarketOrderRejection {
            code: "POLYMARKET_ORDER_REJECTED".to_string(),
            message: "CLOB rejected order with HTTP 400 Bad Request: {\"error\":\"not enough balance / allowance: the balance is not enough -> balance: 5000000, sum of active orders: 5000000, sum of matched orders: 0, order amount (inc. fees): 5000000\"}".to_string(),
        }
    ));
    assert!(!polymarket_rejection_reports_balance_reserved_by_active_orders(
        &PolymarketOrderRejection {
            code: "POLYMARKET_ORDER_REJECTED".to_string(),
            message: "not enough balance / allowance: balance: 0, sum of active orders: 0, order amount (inc. fees): 5000000".to_string(),
        }
    ));
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
fn post_only_exit_crossing_uses_best_bid_not_midpoint() {
    let now = OffsetDateTime::now_utc();
    let mut book = live_test_book("yes_live", now);
    book.bids[0].price = reward_decimal("0.818");
    book.asks[0].price = reward_decimal("0.922");
    let mut order = live_test_open_order("yes_live");
    order.side = RewardOrderSide::Sell;
    order.status = ManagedRewardOrderStatus::ExitPending;
    order.price = reward_decimal("0.87");
    order.reason = "post-only hold-and-requote original-price exit".to_string();

    assert_eq!(
        reward_post_only_exit_crossing_bid(
            &order,
            &HashMap::from([("yes_live".to_string(), book)]),
        ),
        None
    );
}

#[test]
fn crossing_bid_does_not_bypass_post_only_rejection_cap() {
    let now = OffsetDateTime::now_utc();
    let mut book = live_test_book("yes_live", now);
    book.bids[0].price = reward_decimal("0.65");
    let mut order = live_test_open_order("yes_live");
    order.side = RewardOrderSide::Sell;
    order.status = ManagedRewardOrderStatus::ExitPending;
    order.price = reward_decimal("0.64");
    order.reason =
        "retryable live exit rejected [10/10] (post_only=true): order crosses book".to_string();
    order.updated_at = now;
    let books = HashMap::from([("yes_live".to_string(), book)]);

    assert!(!live_exit_retry_due(
        &order,
        OffsetDateTime::now_utc() + TimeDuration::hours(1)
    ));
    assert_eq!(
        reward_post_only_exit_crossing_bid(&order, &books),
        Some(reward_decimal("0.65"))
    );
    assert!(!live_exit_retry_due(&order, now + TimeDuration::hours(1)));
}

#[test]
fn flatten_exit_floor_uses_position_average_when_order_price_is_lower() {
    let mut order = live_test_open_order("yes_live");
    order.side = RewardOrderSide::Sell;
    order.status = ManagedRewardOrderStatus::ExitPending;
    order.price = reward_decimal("0.818");
    order.reason = "post-fill flatten immediately".to_string();
    let position = RewardPosition {
        account_id: order.account_id.clone(),
        condition_id: order.condition_id.clone(),
        token_id: order.token_id.clone(),
        outcome: order.outcome.clone(),
        size: Decimal::from(20_u64),
        avg_price: reward_decimal("0.87"),
        realized_pnl: Decimal::ZERO,
        updated_at: OffsetDateTime::now_utc(),
    };

    assert_eq!(
        reward_sell_exit_floor(&order, &[position]),
        reward_decimal("0.87")
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
fn external_inventory_original_price_exit_is_post_only() {
    let mut order = live_test_open_order("yes_live");
    order.side = RewardOrderSide::Sell;
    order.status = ManagedRewardOrderStatus::ExitPending;
    order.reason = "external inventory original-price exit".to_string();

    assert!(deferred_live_exit_is_post_only(&order));
}

#[test]
fn post_only_false_marker_keeps_flatten_exit_non_post_only() {
    let mut order = live_test_open_order("yes_live");
    order.side = RewardOrderSide::Sell;
    order.status = ManagedRewardOrderStatus::ExitPending;
    order.reason = "retryable live exit rejected [1/10] (post_only=false)".to_string();

    assert!(!deferred_live_exit_is_post_only(&order));
}

#[test]
fn post_only_crossing_deferred_uses_short_backoff() {
    let now = OffsetDateTime::now_utc();
    let mut order = live_test_open_order("yes_live");
    order.side = RewardOrderSide::Sell;
    order.status = ManagedRewardOrderStatus::ExitPending;
    order.reason = format!(
        "{LIVE_EXIT_POST_ONLY_CROSSING_DEFERRED_MARKER}: best bid 0.65 >= maker price 0.64"
    );
    order.updated_at = now;

    assert!(!live_exit_retry_due(&order, now + TimeDuration::seconds(29)));
    assert!(live_exit_retry_due(&order, now + TimeDuration::seconds(30)));
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
        live_exit_pre_submit_failure(&order, &error, true, "post-fill flatten immediately")
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
fn exit_min_notional_pre_submit_failure_increments_existing_retry_marker() {
    let mut order = live_test_open_order("yes_live");
    order.side = RewardOrderSide::Sell;
    order.status = ManagedRewardOrderStatus::ExitPending;
    order.reason =
        "retryable live exit rejected [1/10] (post_only=true): prior rejection".to_string();
    let error = AppError::invalid_input(
        "POLYMARKET_NOTIONAL_INVALID",
        "polymarket live connector requires notional >= 1.00 USD",
    );

    let (reason, severity) =
        live_exit_pre_submit_failure(&order, &error, true, &order.reason)
            .expect("exit notional failure should increment retry state");

    assert_eq!(severity, RewardRiskSeverity::Warning);
    assert!(reason.contains("retryable live exit rejected [2/10]"));

    order.reason = reason;
    let now = OffsetDateTime::now_utc();
    order.updated_at = now;
    assert!(!live_exit_retry_due(&order, now + TimeDuration::seconds(9)));
    assert!(live_exit_retry_due(&order, now + TimeDuration::seconds(10)));
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
fn live_placement_applies_min_depth_before_submission() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        min_depth_usd: reward_decimal("100"),
        max_markets: 1,
        max_open_orders: 2,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let plan = live_test_plan(now);
    let mut yes_book = live_test_book("yes_live", now);
    yes_book.bids = vec![RewardBookLevel {
        price: reward_decimal("0.49"),
        size: reward_decimal("10"),
    }];
    let mut no_book = live_test_book("no_live", now);
    no_book.bids = vec![RewardBookLevel {
        price: reward_decimal("0.49"),
        size: reward_decimal("1000"),
    }];
    let books = HashMap::from([
        ("yes_live".to_string(), yes_book),
        ("no_live".to_string(), no_book),
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
        "trc_depth",
    );

    assert_eq!(placements.len(), 1);
    assert_eq!(placements[0].token_id, "no_live");
}

#[test]
fn live_cancel_uses_strategy_profile_when_matching_quote_plan() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        balanced_merge_enabled: false,
        max_markets: 1,
        max_open_orders: 2,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let plan = live_test_plan(now);
    let mut order = live_test_open_order("yes_live");
    order.strategy_profile = RewardStrategyProfile::BalancedMerge;
    let books = HashMap::from([("yes_live".to_string(), live_test_book("yes_live", now))]);

    let candidates =
        live_cancel_candidates(&config, &[plan], &[order], &books, &HashMap::new(), false);

    assert_eq!(candidates.len(), 1);
    assert!(candidates[0].1.contains("balanced_merge"));
    assert!(candidates[0].1.contains("no longer appears"));
}

#[test]
fn balanced_merge_buy_ignores_global_cancel_bid_rank() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        quote_bid_rank: 2,
        cancel_bid_rank: 1,
        balanced_merge_enabled: true,
        balanced_merge_max_markets: 1,
        balanced_merge_max_open_orders: 2,
        ..RewardBotConfig::default()
    }
    .normalized();
    let now = OffsetDateTime::now_utc();
    let mut plan = live_test_plan(now);
    plan.strategy_profile = RewardStrategyProfile::BalancedMerge;
    let mut order = live_test_open_order("yes_live");
    order.strategy_profile = RewardStrategyProfile::BalancedMerge;
    order.price = reward_decimal("0.48");
    plan.legs[0].price = order.price;
    let books = HashMap::from([("yes_live".to_string(), live_test_book("yes_live", now))]);

    let candidates =
        live_cancel_candidates(&config, &[plan], &[order], &books, &HashMap::new(), false);

    assert!(candidates.is_empty());
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

    let mut plans = vec![plan];
    let (placements, _) = live_placement_orders(
        &config,
        &live_test_account(reward_decimal("100")),
        &mut plans,
        &books,
        &HashMap::new(),
        &[exit],
        &[],
        false,
        "trc_exit_pending",
    );

    assert!(placements.iter().all(|order| order.token_id != "yes_live"));
}

#[test]
fn external_inventory_exit_skips_token_with_remote_active_sell() {
    let position = RewardPosition {
        account_id: "reward_live".to_string(),
        condition_id: "condition_live".to_string(),
        token_id: "yes_live".to_string(),
        outcome: "Yes".to_string(),
        size: reward_decimal("20"),
        avg_price: reward_decimal("0.50"),
        realized_pnl: Decimal::ZERO,
        updated_at: OffsetDateTime::now_utc(),
    };
    let external_sell = PolymarketOpenOrder {
        id: "pm_sell".to_string(),
        market: "condition_live".to_string(),
        asset_id: "yes_live".to_string(),
        side: PolymarketTokenOrderSide::Sell,
        original_size: reward_decimal("20"),
        size_matched: Decimal::ZERO,
        price: reward_decimal("0.51"),
        outcome: "Yes".to_string(),
        status: "open".to_string(),
        created_at: OffsetDateTime::now_utc(),
    };

    let updates = external_inventory_original_price_exit_updates(
        "reward_live",
        &[position],
        &[],
        &[external_sell],
        "trc_remote_sell",
    );

    assert!(updates.is_empty());
}

#[test]
fn dust_exit_is_deferred_without_reason_growth() {
    let mut order = live_test_open_order("yes_live");
    order.side = RewardOrderSide::Sell;
    order.status = ManagedRewardOrderStatus::ExitPending;
    order.price = reward_decimal("0.02");
    order.size = reward_decimal("21");
    order.reason = "post-fill flatten immediately".to_string();

    let (reason, _) = live_exit_dust_deferred_at_price(&order, order.price).expect("dust exit");
    assert!(reason.contains(LIVE_EXIT_DUST_DEFERRED_MARKER));
    assert!(!reason.contains("post-fill flatten immediately"));

    order.reason = reason;
    let now = OffsetDateTime::now_utc();
    order.updated_at = now;
    assert!(!live_exit_retry_due(&order, now + TimeDuration::seconds(299)));
    assert!(live_exit_retry_due(&order, now + TimeDuration::seconds(300)));
}

#[test]
fn max_exit_rejections_stop_retrying() {
    let mut order = live_test_open_order("yes_live");
    order.side = RewardOrderSide::Sell;
    order.status = ManagedRewardOrderStatus::ExitPending;
    order.reason =
        "retryable live exit rejected [10/10] (post_only=true): prior rejection".to_string();

    assert!(!live_exit_retry_due(
        &order,
        OffsetDateTime::now_utc() + TimeDuration::hours(1)
    ));
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
fn live_cancel_candidates_retry_stale_post_only_pending_cancel() {
    let config = RewardBotConfig {
        account_id: "reward_live".to_string(),
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let plan = live_test_plan(now);
    let mut order = live_test_open_order("yes_live");
    order.reason = "Polymarket returned matched for a post-only rewards quote; cancel accepted; awaiting final reconciliation".to_string();
    order.updated_at = now - TimeDuration::seconds(31);
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

#[test]
fn reward_orderbook_remote_refresh_treats_old_local_books_as_stale() {
    let book = CachedOrderBook {
        token_id: "token_stale".to_string(),
        bids: Vec::new(),
        asks: Vec::new(),
        observed_at: 1_000,
        confirmed_at: 1_000,
        source: BookSource::Poll,
    };

    assert!(reward_orderbook_book_is_stale(&book, 50_001, 45_000));
    assert!(!reward_orderbook_book_is_stale(&book, 46_000, 45_000));
    assert!(!reward_orderbook_book_is_stale(&book, 90_000, 0));

    let future_book = CachedOrderBook {
        confirmed_at: 100_000,
        ..book
    };
    assert!(reward_orderbook_book_is_stale(&future_book, 90_000, 45_000));
}

#[test]
fn reward_orderbook_remote_refresh_uses_confirmation_time_not_content_time() {
    let book = CachedOrderBook {
        token_id: "quiet_token".to_string(),
        bids: Vec::new(),
        asks: Vec::new(),
        observed_at: 1_000,
        confirmed_at: 40_000,
        source: BookSource::Poll,
    };

    assert!(!reward_orderbook_book_is_stale(&book, 50_000, 45_000));
    assert!(!reward_orderbook_book_needs_remote_refresh(
        &book,
        46_000,
        45_000,
        15_000,
    ));
}

#[test]
fn reward_orderbook_remote_refresh_uses_live_placement_headroom() {
    let config = RewardBotConfig {
        stale_book_ms: 45_000,
        ..RewardBotConfig::default()
    };
    let max_placement_age_ms = live_orderbook_max_placement_age_ms(&config);
    assert_eq!(max_placement_age_ms, 35_000);
    assert_eq!(
        reward_orderbook_remote_refresh_age_ms(max_placement_age_ms),
        25_000
    );

    let book = CachedOrderBook {
        token_id: "token_near_stale".to_string(),
        bids: Vec::new(),
        asks: Vec::new(),
        observed_at: 1_000,
        confirmed_at: 1_000,
        source: BookSource::Poll,
    };

    assert!(!reward_orderbook_book_is_stale(&book, 21_000, 45_000));
    assert!(!reward_orderbook_book_needs_remote_refresh(
        &book,
        21_000,
        config.stale_book_ms,
        max_placement_age_ms,
    ));
    assert!(!reward_orderbook_book_needs_remote_refresh(
        &book,
        8_000,
        config.stale_book_ms,
        max_placement_age_ms,
    ));
    assert!(!reward_orderbook_book_needs_remote_refresh(
        &book,
        10_000,
        config.stale_book_ms,
        max_placement_age_ms,
    ));
    assert!(reward_orderbook_book_needs_remote_refresh(
        &book,
        30_000,
        config.stale_book_ms,
        max_placement_age_ms,
    ));
}
