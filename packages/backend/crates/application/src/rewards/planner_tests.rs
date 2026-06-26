use super::*;

fn test_market(rewards_min_size: Decimal) -> RewardMarket {
    RewardMarket {
        condition_id: "cond_budget".to_string(),
        question: "Budget allocation market".to_string(),
        market_slug: "budget-allocation-market".to_string(),
        event_slug: "budget-allocation-event".to_string(),
        category: "politics".to_string(),
        image: String::new(),
        rewards_max_spread: decimal("8"),
        rewards_min_size,
        total_daily_rate: decimal("50"),
        liquidity_usd: decimal("10000"),
        volume_24h_usd: decimal("25000"),
        market_spread_cents: decimal("2"),
        end_at: Some(OffsetDateTime::now_utc() + TimeDuration::days(30)),
        ambiguity_level: "low".to_string(),
        market_synced_at: Some(OffsetDateTime::now_utc()),
        tokens: vec![
            RewardToken {
                token_id: "yes_budget".to_string(),
                outcome: "Yes".to_string(),
                price: None,
            },
            RewardToken {
                token_id: "no_budget".to_string(),
                outcome: "No".to_string(),
                price: None,
            },
        ],
        active: true,
        updated_at: OffsetDateTime::now_utc(),
    }
}

fn test_books() -> HashMap<String, RewardOrderBook> {
    let now = OffsetDateTime::now_utc();
    [
        RewardOrderBook {
            token_id: "yes_budget".to_string(),
            bids: vec![RewardBookLevel {
                price: decimal("0.77"),
                size: decimal("1000"),
            }],
            asks: vec![RewardBookLevel {
                price: decimal("0.78"),
                size: decimal("1000"),
            }],
            observed_at: now,
            confirmed_at: now,
        },
        RewardOrderBook {
            token_id: "no_budget".to_string(),
            bids: vec![RewardBookLevel {
                price: decimal("0.22"),
                size: decimal("1000"),
            }],
            asks: vec![RewardBookLevel {
                price: decimal("0.23"),
                size: decimal("1000"),
            }],
            observed_at: now,
            confirmed_at: now,
        },
    ]
    .into_iter()
    .map(|book| (book.token_id.clone(), book))
    .collect()
}

fn dominant_yes_books() -> HashMap<String, RewardOrderBook> {
    let now = OffsetDateTime::now_utc();
    [
        RewardOrderBook {
            token_id: "yes_budget".to_string(),
            bids: vec![
                RewardBookLevel {
                    price: decimal("0.91"),
                    size: decimal("1000"),
                },
                RewardBookLevel {
                    price: decimal("0.90"),
                    size: decimal("500"),
                },
                RewardBookLevel {
                    price: decimal("0.89"),
                    size: decimal("500"),
                },
            ],
            asks: vec![RewardBookLevel {
                price: decimal("0.93"),
                size: decimal("1000"),
            }],
            observed_at: now,
            confirmed_at: now,
        },
        RewardOrderBook {
            token_id: "no_budget".to_string(),
            bids: vec![RewardBookLevel {
                price: decimal("0.07"),
                size: decimal("1000"),
            }],
            asks: vec![RewardBookLevel {
                price: decimal("0.08"),
                size: decimal("1000"),
            }],
            observed_at: now,
            confirmed_at: now,
        },
    ]
    .into_iter()
    .map(|book| (book.token_id.clone(), book))
    .collect()
}

#[test]
fn quote_materialization_ignores_config_market_and_leg_budgets() {
    let config = RewardBotConfig {
        per_market_usd: decimal("10"),
        quote_size_usd: Decimal::ZERO,
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };

    let plan = build_reward_quote_plan(&test_market(decimal("20")), &test_books(), &config);
    let materialized =
        materialize_reward_quote_plan_for_live_orderbook(&plan, &test_books(), &config)
            .expect("live materialization");

    assert!(plan.eligible, "{}", plan.reason);
    assert_eq!(plan.legs.len(), 2);
    assert_eq!(plan.quote_mode, RewardPlanQuoteMode::Double);
    assert!(materialized.legs.iter().all(|leg| leg.size >= decimal("20")));
    assert!(
        materialized
            .legs
            .iter()
            .fold(Decimal::ZERO, |sum, leg| sum + leg.price * leg.size)
        > config.per_market_usd
    );
}

#[test]
fn planner_freshness_uses_orderbook_confirmation_time() {
    let config = RewardBotConfig {
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let mut books = test_books();
    for book in books.values_mut() {
        book.observed_at = now - TimeDuration::minutes(10);
        book.confirmed_at = now;
    }

    let plan = build_reward_quote_plan(&test_market(decimal("5")), &books, &config);
    let materialized = materialize_reward_quote_plan_for_live_orderbook(&plan, &books, &config)
        .expect("live materialization should accept recently confirmed books");

    assert!(plan.eligible, "{}", plan.reason);
    assert_eq!(materialized.legs.len(), 2);
}

#[test]
fn planner_rejects_books_with_stale_confirmation_time() {
    let config = RewardBotConfig {
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let mut books = test_books();
    for book in books.values_mut() {
        book.observed_at = now;
        book.confirmed_at = now - TimeDuration::minutes(10);
    }

    let plan = build_reward_quote_plan(&test_market(decimal("5")), &books, &config);

    assert!(!plan.eligible);
    assert!(plan.reason.contains("missing book or fallback token price"));
}

#[test]
fn auto_enforce_quotes_only_dominant_yes_side() {
    let config = RewardBotConfig {
        quote_mode: RewardQuoteMode::Auto,
        selection_mode: RewardSelectionMode::Enforce,
        dominant_single_side_enabled: true,
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };

    let plan = build_reward_quote_plan(&test_market(decimal("5")), &dominant_yes_books(), &config);
    let materialized =
        materialize_reward_quote_plan_for_live_orderbook(&plan, &dominant_yes_books(), &config)
            .expect("live materialization");

    assert!(plan.eligible, "{}", plan.reason);
    assert_eq!(plan.quote_mode, RewardPlanQuoteMode::SingleYes);
    assert_eq!(plan.recommended_quote_mode, Some(RewardPlanQuoteMode::SingleYes));
    assert_eq!(plan.legs.len(), 2);
    assert_eq!(materialized.legs.len(), 1);
    assert_eq!(materialized.legs[0].outcome, "Yes");
    assert_eq!(materialized.legs[0].price, decimal("0.91"));
}

#[test]
fn auto_enforce_concentration_is_checked_during_live_materialization() {
    let config = RewardBotConfig {
        quote_mode: RewardQuoteMode::Auto,
        selection_mode: RewardSelectionMode::Enforce,
        dominant_single_side_enabled: true,
        max_top1_depth_share: decimal("0.40"),
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };

    let plan = build_reward_quote_plan(&test_market(decimal("5")), &dominant_yes_books(), &config);
    let error =
        materialize_reward_quote_plan_for_live_orderbook(&plan, &dominant_yes_books(), &config)
            .expect_err("live materialization should reject concentrated book");

    assert!(plan.eligible, "{}", plan.reason);
    assert_eq!(plan.quote_mode, RewardPlanQuoteMode::SingleYes);
    assert!(error.contains("top-1 depth share"));
    assert_eq!(plan.recommended_quote_mode, Some(RewardPlanQuoteMode::None));
}

#[test]
fn ai_enforce_can_filter_double_plan_to_single_side() {
    let config = RewardBotConfig {
        ai_advisory_enabled: true,
        quote_mode: RewardQuoteMode::Auto,
        selection_mode: RewardSelectionMode::Enforce,
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let mut plans = vec![build_reward_quote_plan(
        &test_market(decimal("5")),
        &test_books(),
        &config,
    )];
    assert_eq!(plans[0].legs.len(), 2);

    let advisory = test_advisory(
        RewardAiSuitability::Allow,
        RewardPlanQuoteMode::SingleNo,
        decimal("0.80"),
    );
    let advisories = HashMap::from([(advisory.condition_id.clone(), advisory)]);

    apply_reward_ai_advisories(&mut plans, &advisories, &config, decimal("0.65"));

    assert!(plans[0].eligible);
    assert_eq!(plans[0].quote_mode, RewardPlanQuoteMode::SingleNo);
    assert_eq!(plans[0].legs.len(), 2);
    let materialized = materialize_reward_quote_plan_for_live_orderbook(
        &plans[0],
        &test_books(),
        &config,
    )
    .expect("live materialization");
    assert_eq!(materialized.legs.len(), 1);
    assert_eq!(materialized.legs[0].outcome, "No");
    assert!(plans[0].ai_advisory.is_some());
}

#[test]
fn ai_enforce_avoid_rejects_plan_without_relaxing_checks() {
    let config = RewardBotConfig {
        ai_advisory_enabled: true,
        selection_mode: RewardSelectionMode::Enforce,
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let mut plans = vec![build_reward_quote_plan(
        &test_market(decimal("5")),
        &test_books(),
        &config,
    )];
    let advisory = test_advisory(
        RewardAiSuitability::Avoid,
        RewardPlanQuoteMode::Double,
        decimal("0.90"),
    );
    let advisories = HashMap::from([(advisory.condition_id.clone(), advisory)]);

    apply_reward_ai_advisories(&mut plans, &advisories, &config, decimal("0.65"));

    assert!(!plans[0].eligible);
    assert!(plans[0].legs.is_empty());
    assert_eq!(plans[0].quote_mode, RewardPlanQuoteMode::None);
    assert!(plans[0].reason.contains("AI advisory avoid"));
}

#[test]
fn low_competition_requires_high_confidence_ai_allow() {
    let config = RewardBotConfig {
        ai_advisory_enabled: true,
        low_competition_mode: RewardLowCompetitionMode::Enforce,
        low_competition_require_ai_allow: true,
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let mut plan = build_reward_quote_plan(&test_market(decimal("5")), &test_books(), &config);
    plan.strategy_bucket = RewardStrategyBucket::LowCompetition;
    let advisory = test_advisory(
        RewardAiSuitability::Watch,
        RewardPlanQuoteMode::Double,
        decimal("0.90"),
    );
    let advisories = HashMap::from([(advisory.condition_id.clone(), advisory)]);
    let mut plans = vec![plan];

    apply_reward_ai_advisories(&mut plans, &advisories, &config, decimal("0.65"));

    assert!(!plans[0].eligible);
    assert_eq!(plans[0].quote_mode, RewardPlanQuoteMode::None);
    assert!(plans[0].reason.contains("requires high-confidence allow"));
}

#[test]
fn ai_enabled_rejects_eligible_plan_without_provider_decision() {
    let config = RewardBotConfig {
        ai_advisory_enabled: true,
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let mut plans = vec![build_reward_quote_plan(
        &test_market(decimal("5")),
        &test_books(),
        &config,
    )];

    apply_reward_ai_advisories(&mut plans, &HashMap::new(), &config, decimal("0.65"));

    assert!(!plans[0].eligible);
    assert!(plans[0].legs.is_empty());
    assert_eq!(plans[0].quote_mode, RewardPlanQuoteMode::None);
    assert!(plans[0].reason.contains("AI advisory pending"));
}

#[test]
fn ai_prepare_apply_existing_does_not_reject_missing_provider_decision() {
    let config = RewardBotConfig {
        ai_advisory_enabled: true,
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let mut plans = vec![build_reward_quote_plan(
        &test_market(decimal("5")),
        &test_books(),
        &config,
    )];

    apply_existing_reward_ai_advisories(&mut plans, &HashMap::new(), &config, decimal("0.65"));

    assert!(plans[0].eligible);
    assert_eq!(plans[0].quote_mode, RewardPlanQuoteMode::Double);
    assert_eq!(plans[0].legs.len(), 2);
    assert!(plans[0].ai_advisory.is_none());
}

#[test]
fn info_risk_enforce_rejects_eligible_plan_without_provider_decision() {
    let config = RewardBotConfig {
        info_risk_enabled: true,
        info_risk_mode: RewardSelectionMode::Enforce,
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let mut plans = vec![build_reward_quote_plan(
        &test_market(decimal("5")),
        &test_books(),
        &config,
    )];

    apply_reward_info_risks(&mut plans, &HashMap::new(), &config, decimal("0.65"));

    assert!(!plans[0].eligible);
    assert!(plans[0].legs.is_empty());
    assert_eq!(plans[0].quote_mode, RewardPlanQuoteMode::None);
    assert!(plans[0].reason.contains("info risk pending"));
}

#[test]
fn info_risk_enforce_keeps_non_imminent_high_risk_as_advisory() {
    let config = RewardBotConfig {
        info_risk_enabled: true,
        info_risk_mode: RewardSelectionMode::Enforce,
        info_risk_avoid_level: RewardInfoRiskLevel::High,
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let mut plans = vec![build_reward_quote_plan(
        &test_market(decimal("5")),
        &test_books(),
        &config,
    )];
    let risk = test_info_risk(
        RewardInfoRiskLevel::High,
        RewardInfoRiskType::ScheduledEvent,
        false,
    );
    let risks = HashMap::from([(risk.condition_id.clone(), risk)]);

    apply_reward_info_risks(&mut plans, &risks, &config, decimal("0.65"));

    assert!(plans[0].eligible);
    assert_eq!(plans[0].quote_mode, RewardPlanQuoteMode::Double);
    assert_eq!(plans[0].legs.len(), 2);
    assert!(plans[0].info_risk.is_some());
}

#[test]
fn low_competition_uses_stricter_info_risk_avoid_level() {
    let config = RewardBotConfig {
        info_risk_enabled: true,
        info_risk_mode: RewardSelectionMode::Enforce,
        info_risk_avoid_level: RewardInfoRiskLevel::High,
        low_competition_mode: RewardLowCompetitionMode::Enforce,
        low_competition_info_risk_avoid_level: RewardInfoRiskLevel::Medium,
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let mut plan = build_reward_quote_plan(&test_market(decimal("5")), &test_books(), &config);
    plan.strategy_bucket = RewardStrategyBucket::LowCompetition;
    let risk = test_info_risk(
        RewardInfoRiskLevel::Medium,
        RewardInfoRiskType::ScheduledEvent,
        false,
    );
    let risks = HashMap::from([(risk.condition_id.clone(), risk)]);
    let mut plans = vec![plan];

    apply_reward_info_risks(&mut plans, &risks, &config, decimal("0.65"));

    assert!(!plans[0].eligible);
    assert_eq!(plans[0].quote_mode, RewardPlanQuoteMode::None);
    assert!(plans[0].reason.contains("info risk medium"));
}

#[test]
fn info_risk_enforce_keeps_imminent_type_without_imminent_flag_as_advisory() {
    let config = RewardBotConfig {
        info_risk_enabled: true,
        info_risk_mode: RewardSelectionMode::Enforce,
        info_risk_avoid_level: RewardInfoRiskLevel::High,
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let mut plans = vec![build_reward_quote_plan(
        &test_market(decimal("5")),
        &test_books(),
        &config,
    )];
    let risk = test_info_risk(
        RewardInfoRiskLevel::High,
        RewardInfoRiskType::ImminentResolution,
        false,
    );
    let risks = HashMap::from([(risk.condition_id.clone(), risk)]);

    apply_reward_info_risks(&mut plans, &risks, &config, decimal("0.65"));

    assert!(plans[0].eligible);
    assert_eq!(plans[0].quote_mode, RewardPlanQuoteMode::Double);
    assert_eq!(plans[0].legs.len(), 2);
    assert!(plans[0].info_risk.is_some());
}

#[test]
fn info_risk_enforce_rejects_critical_risk() {
    let config = RewardBotConfig {
        info_risk_enabled: true,
        info_risk_mode: RewardSelectionMode::Enforce,
        info_risk_avoid_level: RewardInfoRiskLevel::High,
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let mut plans = vec![build_reward_quote_plan(
        &test_market(decimal("5")),
        &test_books(),
        &config,
    )];
    let risk = test_info_risk(
        RewardInfoRiskLevel::Critical,
        RewardInfoRiskType::ScheduledEvent,
        false,
    );
    let risks = HashMap::from([(risk.condition_id.clone(), risk)]);

    apply_reward_info_risks(&mut plans, &risks, &config, decimal("0.65"));

    assert!(!plans[0].eligible);
    assert!(plans[0].legs.is_empty());
    assert_eq!(plans[0].quote_mode, RewardPlanQuoteMode::None);
    assert!(plans[0].reason.contains("info risk critical"));
}

#[test]
fn info_risk_enforce_rejects_imminent_high_risk() {
    let config = RewardBotConfig {
        info_risk_enabled: true,
        info_risk_mode: RewardSelectionMode::Enforce,
        info_risk_avoid_level: RewardInfoRiskLevel::High,
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let mut plans = vec![build_reward_quote_plan(
        &test_market(decimal("5")),
        &test_books(),
        &config,
    )];
    let risk = test_info_risk(
        RewardInfoRiskLevel::High,
        RewardInfoRiskType::ImminentResolution,
        true,
    );
    let risks = HashMap::from([(risk.condition_id.clone(), risk)]);

    apply_reward_info_risks(&mut plans, &risks, &config, decimal("0.65"));

    assert!(!plans[0].eligible);
    assert!(plans[0].legs.is_empty());
    assert_eq!(plans[0].quote_mode, RewardPlanQuoteMode::None);
    assert!(plans[0].reason.contains("info risk high"));
}

#[test]
fn quote_plan_counts_classify_provider_and_blocker_reasons() {
    let config = RewardBotConfig {
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let base = build_reward_quote_plan(&test_market(decimal("5")), &test_books(), &config);
    let mut ai_pending = base.clone();
    ai_pending.pre_ai_eligible = true;
    ai_pending.eligible = false;
    ai_pending.quote_mode = RewardPlanQuoteMode::None;
    ai_pending.legs.clear();
    ai_pending.reason = "AI advisory pending: market has not passed provider filter".to_string();

    let mut info_risk = base.clone();
    info_risk.eligible = false;
    info_risk.quote_mode = RewardPlanQuoteMode::None;
    info_risk.legs.clear();
    info_risk.reason = "info risk critical: imminent official result".to_string();

    let mut funding = base.clone();
    funding.eligible = false;
    funding.quote_mode = RewardPlanQuoteMode::None;
    funding.legs.clear();
    funding.reason = "live funding below rewards minimum: available 1".to_string();

    let mut live_validation = base.clone();
    live_validation.eligible = false;
    live_validation.quote_mode = RewardPlanQuoteMode::None;
    live_validation.legs.clear();
    live_validation.reason =
        "live orderbook validation skipped until 2026-06-23T00:00:00Z: no viable leg"
            .to_string();

    let counts = RewardQuotePlanCounts::from_plans([
        &base,
        &ai_pending,
        &info_risk,
        &funding,
        &live_validation,
    ]);

    assert_eq!(counts.total, 5);
    assert_eq!(counts.eligible, 1);
    assert_eq!(counts.provider_pending, 1);
    assert_eq!(counts.blockers.ai_pending, 1);
    assert_eq!(counts.blockers.info_risk, 1);
    assert_eq!(counts.blockers.funding, 1);
    assert_eq!(counts.blockers.live_validation, 1);
    assert_eq!(counts.blockers.other, 0);
}

#[test]
fn ai_enabled_keeps_low_confidence_allow_decision_as_deterministic_plan() {
    let config = RewardBotConfig {
        ai_advisory_enabled: true,
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let mut plans = vec![build_reward_quote_plan(
        &test_market(decimal("5")),
        &test_books(),
        &config,
    )];
    let advisory = test_advisory(
        RewardAiSuitability::Allow,
        RewardPlanQuoteMode::Double,
        decimal("0.40"),
    );
    let advisories = HashMap::from([(advisory.condition_id.clone(), advisory)]);

    apply_reward_ai_advisories(&mut plans, &advisories, &config, decimal("0.65"));

    assert!(plans[0].eligible);
    assert_eq!(plans[0].quote_mode, RewardPlanQuoteMode::Double);
    assert_eq!(plans[0].legs.len(), 2);
    assert!(plans[0].ai_advisory.is_some());
}

#[test]
fn ai_enabled_keeps_watch_decision_as_deterministic_plan() {
    let config = RewardBotConfig {
        ai_advisory_enabled: true,
        selection_mode: RewardSelectionMode::Enforce,
        quote_mode: RewardQuoteMode::Auto,
        dominant_single_side_enabled: true,
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let mut plans = vec![build_reward_quote_plan(
        &test_market(decimal("5")),
        &test_books(),
        &config,
    )];
    let advisory = test_advisory(
        RewardAiSuitability::Watch,
        RewardPlanQuoteMode::None,
        decimal("0.90"),
    );
    let advisories = HashMap::from([(advisory.condition_id.clone(), advisory)]);

    apply_reward_ai_advisories(&mut plans, &advisories, &config, decimal("0.65"));

    assert!(plans[0].eligible);
    assert_ne!(plans[0].quote_mode, RewardPlanQuoteMode::None);
    assert!(!plans[0].legs.is_empty());
    assert!(plans[0].ai_advisory.is_some());
}

#[test]
fn ai_enabled_allows_high_confidence_provider_pass_in_observe_mode() {
    let config = RewardBotConfig {
        ai_advisory_enabled: true,
        selection_mode: RewardSelectionMode::Observe,
        quote_mode: RewardQuoteMode::Auto,
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let mut plans = vec![build_reward_quote_plan(
        &test_market(decimal("5")),
        &test_books(),
        &config,
    )];
    let advisory = test_advisory(
        RewardAiSuitability::Allow,
        RewardPlanQuoteMode::SingleNo,
        decimal("0.80"),
    );
    let advisories = HashMap::from([(advisory.condition_id.clone(), advisory)]);

    apply_reward_ai_advisories(&mut plans, &advisories, &config, decimal("0.65"));

    assert!(plans[0].eligible);
    assert_eq!(plans[0].quote_mode, RewardPlanQuoteMode::Double);
    assert_eq!(plans[0].legs.len(), 2);
    assert!(plans[0].ai_advisory.is_some());
}

#[test]
fn ai_advisory_carry_forward_reuses_unexpired_matching_snapshot_decision() {
    let config = RewardBotConfig {
        ai_advisory_enabled: true,
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let now = OffsetDateTime::now_utc();
    let mut previous_plan = build_reward_quote_plan(&test_market(decimal("5")), &test_books(), &config);
    previous_plan.ai_advisory = Some(test_advisory(
        RewardAiSuitability::Allow,
        RewardPlanQuoteMode::Double,
        decimal("0.80"),
    ));

    let carried =
        reward_ai_advisories_from_quote_plans(&[previous_plan.clone()], &config, "test-model", now);
    assert_eq!(carried.len(), 1);
    assert!(reward_ai_advisories_from_quote_plans(
        &[previous_plan],
        &config,
        "other-model",
        now,
    )
    .is_empty());

    let mut next_plans = vec![build_reward_quote_plan(
        &test_market(decimal("5")),
        &test_books(),
        &config,
    )];
    apply_reward_ai_advisories(&mut next_plans, &carried, &config, decimal("0.65"));

    assert!(next_plans[0].eligible);
    assert!(next_plans[0].ai_advisory.is_some());
    assert_eq!(next_plans[0].legs.len(), 2);
}

#[test]
fn static_event_risk_filter_blocks_personnel_deadline_markets() {
    let mut market = test_market(decimal("20"));
    market.question = "Will the next UK Prime Minister be appointed by July 19?".to_string();
    market.market_slug =
        "will-the-next-uk-prime-minister-be-appointed-by-july-19".to_string();
    let config = RewardBotConfig {
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };

    let candidates = select_reward_quote_candidate_markets(&[market], &config);

    assert!(candidates.is_empty());
}

#[test]
fn first_quote_quarantine_blocks_new_market_until_observation_window_passes() {
    let now = OffsetDateTime::now_utc();
    let config = RewardBotConfig {
        info_risk_enabled: true,
        info_risk_mode: RewardSelectionMode::Enforce,
        require_info_risk_before_first_quote: true,
        first_quote_quarantine_sec: 300,
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let mut previous_plan = build_reward_quote_plan(&test_market(decimal("20")), &test_books(), &config);
    previous_plan.updated_at = now - TimeDuration::seconds(120);
    let mut plan = build_reward_quote_plan(&test_market(decimal("20")), &test_books(), &config);
    plan.info_risk = Some(test_info_risk(
        RewardInfoRiskLevel::Low,
        RewardInfoRiskType::Unknown,
        false,
    ));

    let changed = apply_first_quote_entry_gates(
        std::slice::from_mut(&mut plan),
        std::slice::from_ref(&previous_plan),
        &[],
        &[],
        &config,
        now,
    );

    assert!(changed);
    assert!(!plan.eligible);
    assert!(plan.reason.starts_with("first quote quarantine:"));
    assert_eq!(plan.updated_at, previous_plan.updated_at);

    plan = build_reward_quote_plan(&test_market(decimal("20")), &test_books(), &config);
    plan.info_risk = Some(test_info_risk(
        RewardInfoRiskLevel::Low,
        RewardInfoRiskType::Unknown,
        false,
    ));
    let changed = apply_first_quote_entry_gates(
        std::slice::from_mut(&mut plan),
        std::slice::from_ref(&previous_plan),
        &[],
        &[],
        &config,
        now + TimeDuration::seconds(301),
    );

    assert!(!changed);
    assert!(plan.eligible, "{}", plan.reason);
}

#[test]
fn quote_materialization_allows_minimum_sizes_above_config_market_budget() {
    let config = RewardBotConfig {
        per_market_usd: decimal("20"),
        quote_size_usd: decimal("10"),
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };

    let plan = build_reward_quote_plan(&test_market(decimal("50")), &test_books(), &config);
    let materialized =
        materialize_reward_quote_plan_for_live_orderbook(&plan, &test_books(), &config)
            .expect("live materialization");

    assert!(plan.eligible, "{}", plan.reason);
    assert_eq!(materialized.quote_mode, RewardPlanQuoteMode::Double);
    assert_eq!(materialized.legs.len(), 2);
    assert!(materialized.legs.iter().all(|leg| leg.size >= decimal("50")));
    assert!(
        materialized
            .legs
            .iter()
            .fold(Decimal::ZERO, |sum, leg| sum + leg.price * leg.size)
            > config.per_market_usd
    );
}

#[test]
fn auto_enforce_keeps_double_when_only_config_market_budget_would_fail() {
    let config = RewardBotConfig {
        quote_mode: RewardQuoteMode::Auto,
        selection_mode: RewardSelectionMode::Enforce,
        dominant_single_side_enabled: true,
        per_market_usd: decimal("20"),
        quote_size_usd: decimal("10"),
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };

    let plan = build_reward_quote_plan(&test_market(decimal("50")), &test_books(), &config);
    let materialized =
        materialize_reward_quote_plan_for_live_orderbook(&plan, &test_books(), &config)
            .expect("live materialization");

    assert!(plan.eligible, "{}", plan.reason);
    assert_eq!(plan.quote_mode, RewardPlanQuoteMode::Double);
    assert_eq!(materialized.quote_mode, RewardPlanQuoteMode::Double);
    assert_eq!(materialized.legs.len(), 2);
    assert!(materialized.legs.iter().all(|leg| leg.size >= decimal("50")));
}

fn test_advisory(
    suitability: RewardAiSuitability,
    quote_mode: RewardPlanQuoteMode,
    confidence: Decimal,
) -> RewardMarketAdvisory {
    let now = OffsetDateTime::now_utc();
    RewardMarketAdvisory {
        condition_id: "cond_budget".to_string(),
        provider: RewardAiProvider::OpenAi,
        request_format: RewardAiRequestFormat::OpenAiResponses,
        model: "test-model".to_string(),
        input_hash: "hash".to_string(),
        suitability,
        quote_mode,
        exit_policy: PostFillStrategy::ExitAtMarkup,
        confidence,
        reasons: vec!["test advisory".to_string()],
        metrics: json!({}),
        created_at: now,
        expires_at: now + TimeDuration::hours(1),
    }
}

fn test_info_risk(
    risk_level: RewardInfoRiskLevel,
    risk_type: RewardInfoRiskType,
    resolution_imminent: bool,
) -> RewardMarketInfoRisk {
    let now = OffsetDateTime::now_utc();
    RewardMarketInfoRisk {
        condition_id: "cond_budget".to_string(),
        provider: RewardAiProvider::OpenAi,
        request_format: RewardAiRequestFormat::OpenAiResponses,
        model: "test-model".to_string(),
        query_hash: "query-hash".to_string(),
        input_hash: "input-hash".to_string(),
        risk_level,
        risk_type,
        directional_risk: RewardInfoDirectionalRisk::Unclear,
        resolution_imminent,
        expected_event_at: None,
        confidence: decimal("0.90"),
        summary: "test info risk".to_string(),
        sources: Vec::new(),
        metrics: json!({}),
        created_at: now,
        expires_at: now + TimeDuration::hours(1),
    }
}

#[test]
fn quote_plan_accounts_for_clob_cost_precision_without_config_budget_rejection() {
    let config = RewardBotConfig {
        per_market_usd: decimal("20.50"),
        quote_size_usd: decimal("1"),
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };

    let plan = build_reward_quote_plan(&test_market(decimal("20.30")), &test_books(), &config);
    let materialized =
        materialize_reward_quote_plan_for_live_orderbook(&plan, &test_books(), &config)
            .expect("live materialization");

    assert!(plan.eligible, "{}", plan.reason);
    assert_eq!(materialized.legs[0].size, decimal("21"));
    assert_eq!(materialized.legs[1].size, decimal("20.5"));
    assert!(
        materialized
            .legs
            .iter()
            .fold(Decimal::ZERO, |sum, leg| sum + leg.price * leg.size)
            > config.per_market_usd
    );
}

#[test]
fn quote_plan_sizes_already_match_clob_cost_precision() {
    let config = RewardBotConfig {
        per_market_usd: decimal("25"),
        quote_size_usd: decimal("1"),
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };

    let plan = build_reward_quote_plan(&test_market(decimal("20.30")), &test_books(), &config);
    let materialized =
        materialize_reward_quote_plan_for_live_orderbook(&plan, &test_books(), &config)
            .expect("live materialization");

    assert!(plan.eligible, "{}", plan.reason);
    assert_eq!(materialized.legs[0].size, decimal("21"));
    assert_eq!(materialized.legs[1].size, decimal("20.5"));
    assert!(materialized
        .legs
        .iter()
        .all(|leg| leg.size >= decimal("20.30")));
}

#[test]
fn quote_bid_rank_selects_requested_distinct_bid_level() {
    let config = RewardBotConfig {
        quote_bid_rank: 2,
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let mut books = test_books();
    books.get_mut("yes_budget").expect("YES book").bids = vec![
        RewardBookLevel {
            price: decimal("0.77"),
            size: decimal("100"),
        },
        RewardBookLevel {
            price: decimal("0.77"),
            size: decimal("50"),
        },
        RewardBookLevel {
            price: decimal("0.76"),
            size: decimal("100"),
        },
    ];
    books.get_mut("no_budget").expect("NO book").bids = vec![
        RewardBookLevel {
            price: decimal("0.22"),
            size: decimal("100"),
        },
        RewardBookLevel {
            price: decimal("0.21"),
            size: decimal("100"),
        },
    ];

    let plan = build_reward_quote_plan(&test_market(decimal("5")), &books, &config);
    let materialized = materialize_reward_quote_plan_for_live_orderbook(&plan, &books, &config)
        .expect("live materialization");

    assert!(plan.eligible, "{}", plan.reason);
    assert_eq!(materialized.legs[0].price, decimal("0.76"));
    assert_eq!(materialized.legs[1].price, decimal("0.21"));
}

#[test]
fn quote_bid_rank_on_fine_tick_uses_cent_distance_from_best_bid() {
    let config = RewardBotConfig {
        quote_bid_rank: 3,
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    let mut books = test_books();
    books.get_mut("yes_budget").expect("YES book").bids = vec![
        RewardBookLevel {
            price: decimal("0.775"),
            size: decimal("100"),
        },
        RewardBookLevel {
            price: decimal("0.774"),
            size: decimal("100"),
        },
        RewardBookLevel {
            price: decimal("0.773"),
            size: decimal("100"),
        },
        RewardBookLevel {
            price: decimal("0.755"),
            size: decimal("100"),
        },
    ];
    books.get_mut("no_budget").expect("NO book").bids = vec![
        RewardBookLevel {
            price: decimal("0.205"),
            size: decimal("100"),
        },
        RewardBookLevel {
            price: decimal("0.204"),
            size: decimal("100"),
        },
        RewardBookLevel {
            price: decimal("0.203"),
            size: decimal("100"),
        },
        RewardBookLevel {
            price: decimal("0.185"),
            size: decimal("100"),
        },
    ];

    let plan = build_reward_quote_plan(&test_market(decimal("5")), &books, &config);
    let materialized = materialize_reward_quote_plan_for_live_orderbook(&plan, &books, &config)
        .expect("live materialization");

    assert!(plan.eligible, "{}", plan.reason);
    assert_eq!(materialized.legs[0].price, decimal("0.755"));
    assert_eq!(materialized.legs[1].price, decimal("0.185"));
}

#[test]
fn quote_bid_rank_depth_is_checked_during_live_materialization() {
    let config = RewardBotConfig {
        quote_bid_rank: 3,
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };

    let plan = build_reward_quote_plan(&test_market(decimal("5")), &test_books(), &config);
    let error = materialize_reward_quote_plan_for_live_orderbook(&plan, &test_books(), &config)
        .expect_err("live materialization should reject missing bid depth");

    assert!(plan.eligible, "{}", plan.reason);
    assert_eq!(error, "YES book does not have bid-3");
}

#[test]
fn quote_bid_rank_spread_is_checked_during_live_materialization() {
    let config = RewardBotConfig {
        quote_bid_rank: 2,
        min_market_score: Decimal::ZERO,
        max_spread_cents: decimal("2"),
        ..RewardBotConfig::default()
    };
    let mut books = test_books();
    books.get_mut("yes_budget").expect("YES book").bids.push(RewardBookLevel {
        price: decimal("0.70"),
        size: decimal("100"),
    });
    books.get_mut("no_budget").expect("NO book").bids.push(RewardBookLevel {
        price: decimal("0.15"),
        size: decimal("100"),
    });

    let plan = build_reward_quote_plan(&test_market(decimal("5")), &books, &config);
    let error = materialize_reward_quote_plan_for_live_orderbook(&plan, &books, &config)
        .expect_err("live materialization should reject out-of-spread bid");

    assert!(plan.eligible, "{}", plan.reason);
    assert_eq!(error, "YES bid-2 is outside the rewards spread limit");
}

#[test]
fn auto_enforce_falls_back_to_single_side_when_double_spread_fails() {
    let config = RewardBotConfig {
        quote_mode: RewardQuoteMode::Auto,
        selection_mode: RewardSelectionMode::Enforce,
        dominant_single_side_enabled: true,
        quote_bid_rank: 2,
        min_market_score: Decimal::ZERO,
        max_spread_cents: decimal("2"),
        ..RewardBotConfig::default()
    };
    let mut books = test_books();
    books.get_mut("yes_budget").expect("YES book").bids.push(RewardBookLevel {
        price: decimal("0.70"),
        size: decimal("100"),
    });
    books.get_mut("no_budget").expect("NO book").bids.push(RewardBookLevel {
        price: decimal("0.21"),
        size: decimal("100"),
    });

    let plan = build_reward_quote_plan(&test_market(decimal("5")), &books, &config);
    let materialized = materialize_reward_quote_plan_for_live_orderbook(&plan, &books, &config)
        .expect("live materialization should fall back to one valid side");

    assert!(plan.eligible, "{}", plan.reason);
    assert_eq!(plan.quote_mode, RewardPlanQuoteMode::Double);
    assert_eq!(materialized.quote_mode, RewardPlanQuoteMode::SingleNo);
    assert_eq!(materialized.legs.len(), 1);
    assert_eq!(materialized.legs[0].outcome, "No");
    assert_eq!(materialized.legs[0].price, decimal("0.21"));
}

#[test]
fn quote_bid_rank_is_limited_to_supported_levels() {
    assert_eq!(
        RewardBotConfig {
            quote_bid_rank: 0,
            ..RewardBotConfig::default()
        }
        .normalized()
        .quote_bid_rank,
        1
    );
    assert_eq!(
        RewardBotConfig {
            quote_bid_rank: 9,
            ..RewardBotConfig::default()
        }
        .normalized()
        .quote_bid_rank,
        3
    );
}

#[test]
fn rewards_spread_limit_is_bounded_to_probability_range() {
    assert_eq!(
        RewardBotConfig {
            max_spread_cents: decimal("1000"),
            ..RewardBotConfig::default()
        }
        .normalized()
        .max_spread_cents,
        decimal("99")
    );
}

#[test]
fn market_rewards_spread_is_already_expressed_in_cents() {
    assert_eq!(normalize_reward_spread_cents(decimal("99")), decimal("99"));
}

#[test]
fn candidate_prefilter_rejects_low_quality_or_near_expiry_markets() {
    let config = RewardBotConfig::default();
    let mut market = test_market(decimal("5"));

    market.liquidity_usd = config.min_market_liquidity_usd - Decimal::ONE;
    market.volume_24h_usd = config.min_market_volume_24h_usd - Decimal::ONE;
    assert!(select_reward_quote_candidate_markets(&[market.clone()], &config).is_empty());

    market.liquidity_usd = config.min_market_liquidity_usd;
    assert_eq!(
        select_reward_quote_candidate_markets(&[market.clone()], &config).len(),
        1
    );

    market.liquidity_usd = config.min_market_liquidity_usd - Decimal::ONE;
    market.volume_24h_usd = config.min_market_volume_24h_usd;
    assert_eq!(
        select_reward_quote_candidate_markets(&[market.clone()], &config).len(),
        1
    );

    market.end_at = Some(
        OffsetDateTime::now_utc()
            + TimeDuration::hours(config.min_hours_to_end.saturating_sub(1) as i64),
    );
    assert!(select_reward_quote_candidate_markets(&[market.clone()], &config).is_empty());

    market.end_at = Some(
        OffsetDateTime::now_utc() + TimeDuration::hours(config.min_hours_to_end as i64 + 1),
    );
    market.ambiguity_level = "high".to_string();
    assert!(select_reward_quote_candidate_markets(&[market], &config).is_empty());
}

#[test]
fn candidate_prefilter_rejects_implausible_future_sync_time() {
    let config = RewardBotConfig::default();
    let mut market = test_market(decimal("5"));
    market.market_synced_at = Some(OffsetDateTime::now_utc() + TimeDuration::minutes(6));

    assert!(select_reward_quote_candidate_markets(&[market], &config).is_empty());
}

#[test]
fn candidate_prefilter_requires_exactly_one_yes_and_one_no_token() {
    let config = RewardBotConfig::default();
    let mut market = test_market(decimal("5"));
    market.tokens.push(RewardToken {
        token_id: "extra_yes".to_string(),
        outcome: "Yes".to_string(),
        price: None,
    });
    assert!(select_reward_quote_candidate_markets(&[market.clone()], &config).is_empty());

    market.tokens.pop();
    market.tokens[1].outcome = "Maybe".to_string();
    assert!(select_reward_quote_candidate_markets(&[market], &config).is_empty());
}

#[test]
fn candidate_prefilter_rejects_high_event_risk_launch_markets() {
    let config = RewardBotConfig::default();
    let mut market = test_market(decimal("5"));
    market.question = "Extended FDV above $300M one day after launch?".to_string();
    market.market_slug = "extended-fdv-above-300m-one-day-after-launch".to_string();

    assert!(select_reward_quote_candidate_markets(&[market], &config).is_empty());

    let mut token_launch = test_market(decimal("5"));
    token_launch.question = "Will OpenSea launch a token by December 31, 2026?".to_string();

    assert!(select_reward_quote_candidate_markets(&[token_launch], &config).is_empty());
}

#[test]
fn market_quality_increases_quote_plan_score() {
    let config = RewardBotConfig::default();
    let books = test_books();
    let mut lower_quality = test_market(decimal("5"));
    lower_quality.condition_id = "lower_quality".to_string();
    lower_quality.total_daily_rate = decimal("1");
    lower_quality.liquidity_usd = decimal("1000");
    lower_quality.volume_24h_usd = decimal("1000");
    lower_quality.end_at = Some(OffsetDateTime::now_utc() + TimeDuration::days(3));

    let mut higher_quality = lower_quality.clone();
    higher_quality.condition_id = "higher_quality".to_string();
    higher_quality.total_daily_rate = decimal("25");
    higher_quality.liquidity_usd = decimal("100000");
    higher_quality.volume_24h_usd = decimal("500000");
    higher_quality.end_at = Some(OffsetDateTime::now_utc() + TimeDuration::days(90));

    let plans = build_reward_quote_plans(&[lower_quality, higher_quality], &books, &config);
    let high = plans
        .iter()
        .find(|plan| plan.condition_id == "higher_quality")
        .expect("higher-quality plan");
    let low = plans
        .iter()
        .find(|plan| plan.condition_id == "lower_quality")
        .expect("lower-quality plan");
    assert!(high.score > low.score);
}

#[test]
fn cancel_bid_rank_cannot_cancel_a_new_quote_immediately() {
    let config = RewardBotConfig {
        quote_bid_rank: 2,
        cancel_bid_rank: 2,
        ..RewardBotConfig::default()
    }
    .normalized();
    assert_eq!(config.cancel_bid_rank, 1);

    let best_bid_config = RewardBotConfig {
        quote_bid_rank: 1,
        cancel_bid_rank: 1,
        ..RewardBotConfig::default()
    }
    .normalized();
    assert_eq!(best_bid_config.cancel_bid_rank, 0);
}
