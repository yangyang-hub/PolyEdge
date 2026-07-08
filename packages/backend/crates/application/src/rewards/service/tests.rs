use super::*;

#[test]
fn worker_running_requires_enabled_config_and_fresh_heartbeat() {
    let now = OffsetDateTime::now_utc();
    let enabled = RewardBotConfig {
        enabled: true,
        ..RewardBotConfig::default()
    };

    assert!(reward_worker_is_running(
        &enabled,
        Some(now - TimeDuration::seconds(30)),
        now,
    ));
    assert!(!reward_worker_is_running(
        &enabled,
        Some(now - TimeDuration::minutes(3)),
        now,
    ));
    assert!(!reward_worker_is_running(
        &RewardBotConfig::default(),
        Some(now),
        now,
    ));
}

#[test]
fn transient_live_orderbook_skip_reasons_are_not_carried() {
    assert!(live_orderbook_skip_reason_is_transient(Some(
        "missing fresh orderbook midpoint for live quote",
    )));
    assert!(live_orderbook_skip_reason_is_transient(Some(
        "waiting for fresh orderbook data from subscription: YES orderbook unavailable",
    )));
    assert!(live_orderbook_skip_reason_is_transient(Some(
        "YES orderbook stale: age_ms=50000, max_age_ms=45000",
    )));
    assert!(live_orderbook_skip_reason_is_transient(Some(
        "quote plan missing YES token for live validation",
    )));
    assert!(!live_orderbook_skip_reason_is_transient(Some(
        "YES bid-3 is outside the rewards spread limit",
    )));
    assert!(!live_orderbook_skip_reason_is_transient(None));
}

#[test]
fn status_error_ignores_short_lived_final_reconciliation_pending() {
    let now = OffsetDateTime::now_utc();
    let mut order = ManagedRewardOrder {
        id: "rewlive_pending_cancel".to_string(),
        account_id: "reward_live".to_string(),
        condition_id: "cond_live".to_string(),
        token_id: "yes_token".to_string(),
        outcome: "Yes".to_string(),
        side: RewardOrderSide::Buy,
        price: Decimal::new(5, 1),
        size: Decimal::ONE,
        strategy_bucket: RewardStrategyBucket::Standard,
        strategy_profile: RewardStrategyProfile::Standard,
        exit_strategy_source: RewardExitStrategySource::Configured,
        exit_strategy_selected: None,
        exit_floor_price: None,
        exit_reselect_count: 0,
        exit_last_reselected_at: None,
        external_order_id: Some("pm_live".to_string()),
        status: ManagedRewardOrderStatus::Open,
        scoring: false,
        reason: "cancel accepted; awaiting final reconciliation".to_string(),
        filled_size: Decimal::ZERO,
        reward_earned: Decimal::ZERO,
        last_scored_at: None,
        created_at: now,
        updated_at: now,
    };

    assert!(!reward_order_has_active_reconciliation_error(&order));
    assert!(!reward_order_counts_as_external_open(&order));

    order.reason = "order confirmed live after cancellation attempt; cancellation must be retried"
        .to_string();
    assert!(reward_order_has_active_reconciliation_error(&order));
    assert!(reward_order_counts_as_external_open(&order));

    order.filled_size = order.size;
    assert!(!reward_order_counts_as_external_open(&order));
    order.filled_size = Decimal::ZERO;

    order.external_order_id = Some("rewlive_internal_id".to_string());
    assert!(!reward_order_counts_as_external_open(&order));
}

#[test]
fn waiting_orderbook_readiness_overrides_preserved_live_legs() {
    let now = OffsetDateTime::now_utc();
    let plan = RewardQuotePlan {
        condition_id: "cond_waiting_book".to_string(),
        market_slug: "waiting-book-market".to_string(),
        question: "Will this market keep old legs?".to_string(),
        score: Decimal::ONE,
        selection_score: Decimal::ZERO,
        eligible: true,
        pre_ai_eligible: true,
        quote_readiness: RewardQuoteReadiness::ReadyToQuote,
        reason: "waiting for fresh orderbook data from subscription: Yes orderbook too close to stale".to_string(),
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
        midpoint: Some(Decimal::new(5, 1)),
        live_skip_until: None,
        live_skip_reason: None,
        first_quote_observed_at: None,
        ai_advisory_pending_since: None,
        info_risk_pending_since: None,
        total_daily_rate: Decimal::ONE,
        rewards_max_spread: Decimal::ONE,
        rewards_min_size: Decimal::ONE,
        orderbook_token_ids: vec!["yes_token".to_string(), "no_token".to_string()],
        legs: vec![RewardQuoteLeg {
            token_id: "yes_token".to_string(),
            outcome: "Yes".to_string(),
            side: RewardOrderSide::Buy,
            price: Decimal::new(5, 1),
            size: Decimal::ONE,
            notional_usd: Decimal::new(5, 1),
        }],
        updated_at: now,
    };

    assert_eq!(
        reward_quote_plan_readiness(&plan),
        RewardQuoteReadiness::WaitingOrderbook
    );
}

#[test]
fn quote_plan_book_token_registration_uses_persisted_orderbook_tokens() {
    let now = OffsetDateTime::now_utc();
    let plan = RewardQuotePlan {
        condition_id: "cond_pre_ai".to_string(),
        market_slug: "pre-ai-market".to_string(),
        question: "Will this market need AI?".to_string(),
        score: Decimal::ONE,
        selection_score: Decimal::ZERO,
        eligible: false,
        pre_ai_eligible: true,
        quote_readiness: RewardQuoteReadiness::ProviderPending,
        reason: "AI advisory pending: market has not passed provider filter".to_string(),
        strategy_bucket: RewardStrategyBucket::Standard,
        strategy_profile: RewardStrategyProfile::Standard,
        latest_run_id: None,
        quote_mode: RewardPlanQuoteMode::None,
        recommended_quote_mode: Some(RewardPlanQuoteMode::Double),
        book_metrics: None,
        opportunity_metrics: None,
        selection_metrics: None,
        fair_value: None,
        ai_advisory: None,
        info_risk: None,
        event_window: None,
        midpoint: Some(Decimal::new(5, 1)),
        live_skip_until: None,
        live_skip_reason: None,
        first_quote_observed_at: None,
        ai_advisory_pending_since: None,
        info_risk_pending_since: None,
        total_daily_rate: Decimal::ONE,
        rewards_max_spread: Decimal::ONE,
        rewards_min_size: Decimal::ONE,
        orderbook_token_ids: vec!["yes_token".to_string(), "no_token".to_string()],
        legs: Vec::new(),
        updated_at: now,
    };
    let mut seen = HashSet::new();
    let mut token_ids = Vec::new();

    RewardBotService::push_reward_quote_plan_book_tokens(&plan, &mut seen, &mut token_ids);

    assert_eq!(
        token_ids,
        vec!["yes_token".to_string(), "no_token".to_string()]
    );
}
