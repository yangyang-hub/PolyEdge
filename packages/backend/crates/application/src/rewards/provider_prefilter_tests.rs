use super::*;

fn prefilter_test_plan(condition_id: &str, bucket: RewardStrategyBucket) -> RewardQuotePlan {
    let now = OffsetDateTime::now_utc();
    RewardQuotePlan {
        condition_id: condition_id.to_string(),
        market_slug: "market".to_string(),
        question: "Question?".to_string(),
        score: Decimal::ONE,
        eligible: true,
        pre_ai_eligible: true,
        quote_readiness: RewardQuoteReadiness::Blocked,
        reason: String::new(),
        strategy_bucket: bucket,
        strategy_profile: RewardStrategyProfile::Standard,
        quote_mode: RewardPlanQuoteMode::Double,
        recommended_quote_mode: None,
        book_metrics: None,
        low_competition_metrics: None,
        opportunity_metrics: None,
        ai_advisory: None,
        info_risk: None,
        event_window: None,
        midpoint: Some(decimal("0.5")),
        live_skip_until: None,
        live_skip_reason: None,
        total_daily_rate: decimal("5"),
        rewards_max_spread: decimal("3"),
        rewards_min_size: decimal("20"),
        orderbook_token_ids: vec!["yes".to_string(), "no".to_string()],
        legs: Vec::new(),
        updated_at: now,
    }
}

#[test]
fn provider_prefilter_treats_legacy_low_competition_bucket_as_standard() {
    let config = RewardBotConfig {
        low_competition_mode: RewardLowCompetitionMode::Observe,
        ..RewardBotConfig::default()
    };
    let plan = prefilter_test_plan("cond_legacy", RewardStrategyBucket::LowCompetition);

    assert!(reward_provider_plan_passes_pre_llm_gate(&plan, &config, false));
    assert_eq!(
        reward_provider_pre_llm_candidate_kind(&plan, &config, false),
        Some(RewardProviderPreLlmCandidateKind::Standard)
    );
}

#[test]
fn provider_prefilter_ignores_legacy_low_competition_enforce_gate() {
    let config = RewardBotConfig {
        low_competition_mode: RewardLowCompetitionMode::Enforce,
        ai_advisory_enabled: true,
        info_risk_enabled: true,
        info_risk_mode: RewardSelectionMode::Enforce,
        ..RewardBotConfig::default()
    };
    let plan = prefilter_test_plan("cond_enforce", RewardStrategyBucket::LowCompetition);

    assert!(reward_provider_plan_passes_pre_llm_gate(&plan, &config, false));
    assert_eq!(
        reward_provider_pre_llm_candidate_kind(&plan, &config, false),
        Some(RewardProviderPreLlmCandidateKind::Standard)
    );
}

#[test]
fn provider_prefilter_blocks_ineligible_legacy_low_competition_bucket() {
    let config = RewardBotConfig::default();
    let mut plan = prefilter_test_plan("cond_reject", RewardStrategyBucket::LowCompetition);
    plan.eligible = false;
    plan.pre_ai_eligible = false;

    assert!(!reward_provider_plan_passes_pre_llm_gate(&plan, &config, false));
}

#[test]
fn provider_prefilter_bypasses_gate_when_condition_has_exposure() {
    let config = RewardBotConfig {
        low_competition_mode: RewardLowCompetitionMode::Observe,
        ..RewardBotConfig::default()
    };
    let plan = prefilter_test_plan("cond_exposure", RewardStrategyBucket::LowCompetition);

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
