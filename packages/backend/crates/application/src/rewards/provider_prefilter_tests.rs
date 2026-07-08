use super::*;

fn prefilter_test_plan(condition_id: &str, bucket: RewardStrategyBucket) -> RewardQuotePlan {
    let now = OffsetDateTime::now_utc();
    RewardQuotePlan {
        condition_id: condition_id.to_string(),
        market_slug: "market".to_string(),
        question: "Question?".to_string(),
        score: Decimal::ONE,
        selection_score: Decimal::ZERO,
        eligible: true,
        pre_ai_eligible: true,
        quote_readiness: RewardQuoteReadiness::Blocked,
        reason: String::new(),
        strategy_bucket: bucket,
        strategy_profile: RewardStrategyProfile::Standard,
        quote_mode: RewardPlanQuoteMode::Double,
        recommended_quote_mode: None,
        book_metrics: None,
        opportunity_metrics: None,
        selection_metrics: None,
        fair_value: None,
        ai_advisory: None,
        info_risk: None,
        event_window: None,
        midpoint: Some(decimal("0.5")),
        live_skip_until: None,
        live_skip_reason: None,
        first_quote_observed_at: None,
        ai_advisory_pending_since: None,
        info_risk_pending_since: None,
        total_daily_rate: decimal("5"),
        rewards_max_spread: decimal("3"),
        rewards_min_size: decimal("20"),
        orderbook_token_ids: vec!["yes".to_string(), "no".to_string()],
        legs: Vec::new(),
        updated_at: now,
    }
}

#[test]
fn provider_prefilter_bypasses_gate_when_condition_has_exposure() {
    let config = RewardBotConfig::default();
    let plan = prefilter_test_plan("cond_exposure", RewardStrategyBucket::None);

    assert!(reward_provider_plan_passes_pre_llm_gate(&plan, &config, true));
    assert_eq!(
        reward_provider_pre_llm_candidate_kind(&plan, &config, true),
        Some(RewardProviderPreLlmCandidateKind::ActiveExposure)
    );
}

#[test]
fn provider_prefilter_requires_eligible_standard_plan_without_exposure() {
    let config = RewardBotConfig::default();
    let eligible = prefilter_test_plan("cond_ok", RewardStrategyBucket::Standard);
    let mut ineligible = prefilter_test_plan("cond_skip", RewardStrategyBucket::Standard);
    ineligible.eligible = false;
    ineligible.pre_ai_eligible = false;

    assert!(reward_provider_plan_passes_pre_llm_gate(&eligible, &config, false));
    assert_eq!(
        reward_provider_pre_llm_candidate_kind(&eligible, &config, false),
        Some(RewardProviderPreLlmCandidateKind::Standard)
    );
    assert!(!reward_provider_plan_passes_pre_llm_gate(&ineligible, &config, false));
    assert_eq!(
        reward_provider_pre_llm_candidate_kind(&ineligible, &config, false),
        None
    );
}

#[test]
fn provider_prefilter_detects_active_exposure_from_orders_and_positions() {
    let now = OffsetDateTime::now_utc();
    let open_order = ManagedRewardOrder {
        id: "order-1".to_string(),
        account_id: "acct".to_string(),
        condition_id: "cond_order".to_string(),
        token_id: "token".to_string(),
        outcome: "YES".to_string(),
        side: RewardOrderSide::Buy,
        price: decimal("0.5"),
        size: decimal("10"),
        strategy_bucket: RewardStrategyBucket::Standard,
        strategy_profile: RewardStrategyProfile::Standard,
        exit_strategy_source: RewardExitStrategySource::Configured,
        exit_strategy_selected: None,
        exit_floor_price: None,
        exit_reselect_count: 0,
        exit_last_reselected_at: None,
        external_order_id: Some("ext-1".to_string()),
        status: ManagedRewardOrderStatus::Open,
        scoring: true,
        reason: String::new(),
        filled_size: Decimal::ZERO,
        reward_earned: Decimal::ZERO,
        last_scored_at: None,
        created_at: now,
        updated_at: now,
    };
    let position = RewardPosition {
        account_id: "acct".to_string(),
        condition_id: "cond_position".to_string(),
        token_id: "token".to_string(),
        outcome: "YES".to_string(),
        size: decimal("1"),
        avg_price: decimal("0.5"),
        realized_pnl: Decimal::ZERO,
        updated_at: now,
    };
    let empty_position = RewardPosition {
        size: Decimal::ZERO,
        ..position.clone()
    };

    assert!(reward_condition_has_active_exposure(
        "cond_order",
        &[open_order.clone()],
        &[]
    ));
    assert!(reward_condition_has_active_exposure(
        "cond_position",
        &[],
        &[position]
    ));
    assert!(!reward_condition_has_active_exposure(
        "cond_position",
        &[],
        &[empty_position]
    ));
}
