fn test_market(rewards_min_size: Decimal) -> RewardMarket {
    RewardMarket {
        condition_id: "cond_low_comp".to_string(),
        question: "Low competition reward market".to_string(),
        market_slug: "low-competition-reward-market".to_string(),
        event_slug: "low-competition-event".to_string(),
        category: "politics".to_string(),
        image: String::new(),
        rewards_max_spread: decimal("8"),
        rewards_min_size,
        total_daily_rate: decimal("50"),
        liquidity_usd: decimal("500"),
        volume_24h_usd: decimal("150"),
        market_spread_cents: decimal("2"),
        end_at: Some(OffsetDateTime::now_utc() + TimeDuration::days(30)),
        ambiguity_level: "low".to_string(),
        market_synced_at: Some(OffsetDateTime::now_utc()),
        tokens: vec![
            RewardToken {
                token_id: "yes_low_comp".to_string(),
                outcome: "Yes".to_string(),
                price: None,
            },
            RewardToken {
                token_id: "no_low_comp".to_string(),
                outcome: "No".to_string(),
                price: None,
            },
        ],
        active: true,
        updated_at: OffsetDateTime::now_utc(),
    }
}

fn test_books() -> HashMap<String, RewardOrderBook> {
    test_books_with_competition(decimal("4"), decimal("4"))
}

fn test_books_with_competition(
    yes_competition_usd: Decimal,
    no_competition_usd: Decimal,
) -> HashMap<String, RewardOrderBook> {
    let now = OffsetDateTime::now_utc();
    let yes_price = decimal("0.77");
    let no_price = decimal("0.22");
    [
        RewardOrderBook {
            token_id: "yes_low_comp".to_string(),
            bids: vec![
                RewardBookLevel {
                    price: yes_price,
                    size: (yes_competition_usd / yes_price).round_dp(4),
                },
                RewardBookLevel {
                    price: decimal("0.50"),
                    size: decimal("200"),
                },
            ],
            asks: vec![RewardBookLevel {
                price: decimal("0.78"),
                size: decimal("1000"),
            }],
            observed_at: now,
            confirmed_at: now,
        },
        RewardOrderBook {
            token_id: "no_low_comp".to_string(),
            bids: vec![
                RewardBookLevel {
                    price: no_price,
                    size: (no_competition_usd / no_price).round_dp(4),
                },
                RewardBookLevel {
                    price: decimal("0.05"),
                    size: decimal("1000"),
                },
            ],
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

fn test_account(available_usd: Decimal) -> RewardAccountState {
    RewardAccountState::fresh("acct", available_usd, OffsetDateTime::now_utc())
}

fn stable_book_history(
    books: &HashMap<String, RewardOrderBook>,
    samples: u64,
) -> HashMap<String, VecDeque<BookSnapshot>> {
    let now = OffsetDateTime::now_utc();
    books
        .values()
        .map(|book| {
            let snapshots = (0..samples)
                .map(|index| BookSnapshot {
                    bids: book.bids.clone(),
                    asks: book.asks.clone(),
                    observed_at: now - TimeDuration::seconds((samples - index) as i64 * 10),
                })
                .collect::<VecDeque<_>>();
            (book.token_id.clone(), snapshots)
        })
        .collect()
}

fn low_competition_plan_config(mode: RewardLowCompetitionMode) -> RewardBotConfig {
    RewardBotConfig {
        low_competition_mode: mode,
        low_competition_max_markets: 1,
        low_competition_max_open_orders: 2,
        low_competition_max_competition_usd: decimal("2000"),
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    }
}

fn low_competition_plans(config: &RewardBotConfig) -> Vec<RewardQuotePlan> {
    build_reward_quote_plans_for_candidates(
        &[RewardCandidateMarket {
            market: test_market(decimal("5")),
            strategy_bucket: RewardStrategyBucket::LowCompetition,
        }],
        &test_books(),
        config,
    )
}

#[test]
fn observe_records_metrics_without_live_eligibility() {
    let config = low_competition_plan_config(RewardLowCompetitionMode::Observe);
    let books = test_books();
    let history = stable_book_history(&books, config.low_competition_min_book_samples);
    let mut plans = low_competition_plans(&config);

    apply_low_competition_metrics_to_quote_plans(
        &mut plans,
        &books,
        &history,
        &[],
        &test_account(decimal("1000")),
        &config,
    );

    assert_eq!(plans[0].strategy_bucket, RewardStrategyBucket::LowCompetition);
    assert!(!plans[0].eligible);
    assert!(plans[0].reason.contains("observe only"));
    let metrics = plans[0]
        .low_competition_metrics
        .as_ref()
        .expect("low competition metrics");
    assert!(metrics.planned_notional_usd > Decimal::ZERO);
    assert!(metrics.eligible_for_low_competition);
    assert!(metrics.qualified_competition_usd > Decimal::ZERO);
    assert!(metrics.estimated_reward_per_100_usd_day > Decimal::ZERO);
}

#[test]
fn enforce_requires_ai_and_info_risk_gates() {
    let config = low_competition_plan_config(RewardLowCompetitionMode::Enforce);
    let books = test_books();
    let history = stable_book_history(&books, config.low_competition_min_book_samples);
    let mut plans = low_competition_plans(&config);

    apply_low_competition_metrics_to_quote_plans(
        &mut plans,
        &books,
        &history,
        &[],
        &test_account(decimal("1000")),
        &config,
    );

    assert!(!plans[0].eligible);
    assert_eq!(plans[0].quote_mode, RewardPlanQuoteMode::None);
    let metrics = plans[0]
        .low_competition_metrics
        .as_ref()
        .expect("low competition metrics");
    assert!(!metrics.eligible_for_low_competition);
    assert!(metrics
        .rejection_reasons
        .iter()
        .any(|reason| reason.contains("requires AI advisory")));
    assert!(metrics
        .rejection_reasons
        .iter()
        .any(|reason| reason.contains("requires info-risk enforce")));
}

#[test]
fn enforce_preserves_quote_metadata_when_orderbook_data_is_unavailable() {
    let config = low_competition_plan_config(RewardLowCompetitionMode::Enforce);
    let mut plans = low_competition_plans(&config);
    let initial_tokens = plans[0].orderbook_token_ids.clone();
    let initial_quote_mode = plans[0].quote_mode;

    apply_low_competition_metrics_to_quote_plans(
        &mut plans,
        &HashMap::new(),
        &HashMap::new(),
        &[],
        &test_account(decimal("1000")),
        &config,
    );

    assert!(!plans[0].eligible);
    assert_eq!(plans[0].quote_mode, initial_quote_mode);
    assert_eq!(plans[0].orderbook_token_ids, initial_tokens);
    assert_eq!(plans[0].legs.len(), 2);
    assert!(plans[0].reason.contains("low-competition data unavailable"));
    let metrics = plans[0]
        .low_competition_metrics
        .as_ref()
        .expect("low competition metrics");
    assert!(metrics
        .rejection_reasons
        .iter()
        .any(|reason| reason.contains("missing fresh orderbook midpoint")));
}

#[test]
fn enforce_can_pass_pre_provider_metrics() {
    let config = RewardBotConfig {
        ai_advisory_enabled: true,
        info_risk_enabled: true,
        info_risk_mode: RewardSelectionMode::Enforce,
        ..low_competition_plan_config(RewardLowCompetitionMode::Enforce)
    };
    let books = test_books();
    let history = stable_book_history(&books, config.low_competition_min_book_samples);
    let mut plans = low_competition_plans(&config);

    apply_low_competition_metrics_to_quote_plans(
        &mut plans,
        &books,
        &history,
        &[],
        &test_account(decimal("1000")),
        &config,
    );

    assert!(plans[0].eligible, "{}", plans[0].reason);
    assert!(plans[0].reason.contains("pending AI and info-risk gates"));
    let metrics = plans[0]
        .low_competition_metrics
        .as_ref()
        .expect("low competition metrics");
    assert!(metrics.eligible_for_low_competition);
}

#[test]
fn live_cancel_keeps_low_competition_order_when_metrics_still_pass() {
    let config = RewardBotConfig {
        ai_advisory_enabled: true,
        info_risk_enabled: true,
        info_risk_mode: RewardSelectionMode::Enforce,
        low_competition_probe_notional_usd: decimal("10"),
        low_competition_min_competition_share_bps: 5_000,
        low_competition_max_competition_multiple: decimal("1"),
        ..low_competition_plan_config(RewardLowCompetitionMode::Enforce)
    };
    let books = test_books_with_competition(decimal("4"), decimal("4"));
    let history = stable_book_history(&books, config.low_competition_min_book_samples);
    let plans = low_competition_plans(&config);

    let reason = low_competition_live_cancel_reason(
        &config,
        &plans[0],
        &books,
        &history,
        &[],
        &test_account(decimal("1000")),
    );

    assert!(reason.is_none());
}

#[test]
fn live_cancel_rejects_low_competition_order_when_competition_worsens() {
    let config = RewardBotConfig {
        ai_advisory_enabled: true,
        info_risk_enabled: true,
        info_risk_mode: RewardSelectionMode::Enforce,
        low_competition_probe_notional_usd: decimal("10"),
        low_competition_min_competition_share_bps: 5_000,
        low_competition_max_competition_multiple: decimal("1"),
        ..low_competition_plan_config(RewardLowCompetitionMode::Enforce)
    };
    let books = test_books_with_competition(decimal("12.5"), decimal("12.5"));
    let history = stable_book_history(&books, config.low_competition_min_book_samples);
    let plans = low_competition_plans(&config);

    let reason = low_competition_live_cancel_reason(
        &config,
        &plans[0],
        &books,
        &history,
        &[],
        &test_account(decimal("1000")),
    )
    .expect("competition worsening should cancel");

    assert!(reason.contains("low-competition cancel gate rejected"));
    assert!(reason.contains("competition share"));
}

#[test]
fn live_cancel_waits_for_competition_confirmation_before_rejecting() {
    let config = RewardBotConfig {
        ai_advisory_enabled: true,
        info_risk_enabled: true,
        info_risk_mode: RewardSelectionMode::Enforce,
        low_competition_probe_notional_usd: decimal("10"),
        low_competition_min_competition_share_bps: 5_000,
        low_competition_max_competition_multiple: decimal("1"),
        ..low_competition_plan_config(RewardLowCompetitionMode::Enforce)
    };
    let books = test_books_with_competition(decimal("12.5"), decimal("12.5"));
    let plans = low_competition_plans(&config);

    let reason = low_competition_live_cancel_reason(
        &config,
        &plans[0],
        &books,
        &HashMap::new(),
        &[],
        &test_account(decimal("1000")),
    );

    assert!(reason.is_none());
}

#[test]
fn live_cancel_does_not_reject_only_for_unavailable_low_competition_history() {
    let config = RewardBotConfig {
        ai_advisory_enabled: true,
        info_risk_enabled: true,
        info_risk_mode: RewardSelectionMode::Enforce,
        low_competition_probe_notional_usd: decimal("10"),
        low_competition_min_competition_share_bps: 5_000,
        low_competition_max_competition_multiple: decimal("1"),
        ..low_competition_plan_config(RewardLowCompetitionMode::Enforce)
    };
    let books = test_books_with_competition(decimal("4"), decimal("4"));
    let plans = low_competition_plans(&config);

    let reason = low_competition_live_cancel_reason(
        &config,
        &plans[0],
        &books,
        &HashMap::new(),
        &[],
        &test_account(decimal("1000")),
    );

    assert!(reason.is_none());
}

#[test]
fn live_cancel_rejects_low_competition_order_when_allocation_cap_is_exceeded() {
    let config = RewardBotConfig {
        ai_advisory_enabled: true,
        info_risk_enabled: true,
        info_risk_mode: RewardSelectionMode::Enforce,
        low_competition_probe_notional_usd: decimal("10"),
        low_competition_max_account_allocation_bps: 50,
        low_competition_max_market_allocation_bps: 10_000,
        ..low_competition_plan_config(RewardLowCompetitionMode::Enforce)
    };
    let books = test_books_with_competition(decimal("4"), decimal("4"));
    let plans = low_competition_plans(&config);

    let reason = low_competition_live_cancel_reason(
        &config,
        &plans[0],
        &books,
        &HashMap::new(),
        &[],
        &test_account(decimal("100")),
    )
    .expect("allocation cap should cancel immediately");

    assert!(reason.contains("low-competition cancel gate rejected"));
    assert!(reason.contains("account allocation"));
}

#[test]
fn ten_usd_probe_passes_when_it_is_most_of_competition_pool() {
    let config = RewardBotConfig {
        ai_advisory_enabled: true,
        info_risk_enabled: true,
        info_risk_mode: RewardSelectionMode::Enforce,
        low_competition_probe_notional_usd: decimal("10"),
        low_competition_min_competition_share_bps: 5_000,
        low_competition_max_competition_multiple: decimal("1"),
        ..low_competition_plan_config(RewardLowCompetitionMode::Enforce)
    };
    let books = test_books_with_competition(decimal("4"), decimal("4"));
    let history = stable_book_history(&books, config.low_competition_min_book_samples);
    let mut plans = low_competition_plans(&config);

    apply_low_competition_metrics_to_quote_plans(
        &mut plans,
        &books,
        &history,
        &[],
        &test_account(decimal("1000")),
        &config,
    );

    let metrics = plans[0]
        .low_competition_metrics
        .as_ref()
        .expect("low competition metrics");
    assert!(metrics.eligible_for_low_competition, "{metrics:?}");
    assert_eq!(metrics.competition_probe_notional_usd, decimal("10"));
    assert!(metrics.competition_share_bps >= decimal("5000"));
    assert!(metrics.competition_multiple <= decimal("1"));
}

#[test]
fn ten_usd_probe_fails_when_competition_pool_is_too_large() {
    let config = RewardBotConfig {
        ai_advisory_enabled: true,
        info_risk_enabled: true,
        info_risk_mode: RewardSelectionMode::Enforce,
        low_competition_probe_notional_usd: decimal("10"),
        low_competition_min_competition_share_bps: 5_000,
        low_competition_max_competition_multiple: decimal("1"),
        ..low_competition_plan_config(RewardLowCompetitionMode::Enforce)
    };
    let books = test_books_with_competition(decimal("12.5"), decimal("12.5"));
    let history = stable_book_history(&books, config.low_competition_min_book_samples);
    let mut plans = low_competition_plans(&config);

    apply_low_competition_metrics_to_quote_plans(
        &mut plans,
        &books,
        &history,
        &[],
        &test_account(decimal("1000")),
        &config,
    );

    let metrics = plans[0]
        .low_competition_metrics
        .as_ref()
        .expect("low competition metrics");
    assert!(!metrics.eligible_for_low_competition);
    assert!(metrics.competition_share_bps < decimal("5000"));
    assert!(metrics
        .rejection_reasons
        .iter()
        .any(|reason| reason.contains("competition share")));
}

#[test]
fn account_allocation_cap_blocks_low_competition_plan() {
    let config = RewardBotConfig {
        ai_advisory_enabled: true,
        info_risk_enabled: true,
        info_risk_mode: RewardSelectionMode::Enforce,
        low_competition_max_account_allocation_bps: 500,
        low_competition_max_market_allocation_bps: 10_000,
        ..low_competition_plan_config(RewardLowCompetitionMode::Enforce)
    };
    let books = test_books();
    let history = stable_book_history(&books, config.low_competition_min_book_samples);
    let mut plans = low_competition_plans(&config);

    apply_low_competition_metrics_to_quote_plans(
        &mut plans,
        &books,
        &history,
        &[],
        &test_account(decimal("50")),
        &config,
    );

    let metrics = plans[0]
        .low_competition_metrics
        .as_ref()
        .expect("low competition metrics");
    assert!(!metrics.eligible_for_low_competition);
    assert!(metrics.account_allocation_bps > decimal("500"));
    assert!(metrics
        .rejection_reasons
        .iter()
        .any(|reason| reason.contains("account allocation")));
}

#[test]
fn market_allocation_cap_blocks_low_competition_plan() {
    let config = RewardBotConfig {
        ai_advisory_enabled: true,
        info_risk_enabled: true,
        info_risk_mode: RewardSelectionMode::Enforce,
        low_competition_max_account_allocation_bps: 10_000,
        low_competition_max_market_allocation_bps: 500,
        ..low_competition_plan_config(RewardLowCompetitionMode::Enforce)
    };
    let books = test_books();
    let history = stable_book_history(&books, config.low_competition_min_book_samples);
    let mut plans = low_competition_plans(&config);

    apply_low_competition_metrics_to_quote_plans(
        &mut plans,
        &books,
        &history,
        &[],
        &test_account(decimal("50")),
        &config,
    );

    let metrics = plans[0]
        .low_competition_metrics
        .as_ref()
        .expect("low competition metrics");
    assert!(!metrics.eligible_for_low_competition);
    assert!(metrics.market_allocation_bps > decimal("500"));
    assert!(metrics
        .rejection_reasons
        .iter()
        .any(|reason| reason.contains("condition allocation")));
}

#[test]
fn observations_keep_planned_notional_after_provider_gate_clears_legs() {
    let config = RewardBotConfig {
        ai_advisory_enabled: true,
        info_risk_enabled: true,
        info_risk_mode: RewardSelectionMode::Enforce,
        ..low_competition_plan_config(RewardLowCompetitionMode::Enforce)
    };
    let books = test_books();
    let history = stable_book_history(&books, config.low_competition_min_book_samples);
    let mut plans = low_competition_plans(&config);

    apply_low_competition_metrics_to_quote_plans(
        &mut plans,
        &books,
        &history,
        &[],
        &test_account(decimal("1000")),
        &config,
    );

    let expected_notional = plans[0]
        .low_competition_metrics
        .as_ref()
        .expect("low competition metrics")
        .planned_notional_usd;
    assert!(expected_notional > Decimal::ZERO);

    plans[0].eligible = false;
    plans[0].quote_mode = RewardPlanQuoteMode::None;
    plans[0].legs.clear();
    plans[0].reason = "AI advisory blocked market".to_string();

    let observations =
        build_low_competition_observations("acct", &plans, &config, OffsetDateTime::now_utc());

    assert_eq!(observations.len(), 1);
    assert_eq!(observations[0].planned_notional_usd, expected_notional);
    assert!(observations[0].ai_blocked);
}

#[test]
fn shadow_report_recommends_only_after_sufficient_healthy_observations() {
    let config = RewardBotConfig {
        low_competition_min_reward_per_100_usd_day: decimal("0.25"),
        low_competition_min_exit_depth_multiple: decimal("3"),
        low_competition_max_midpoint_range_cents: decimal("2"),
        ..low_competition_plan_config(RewardLowCompetitionMode::Observe)
    };
    let now = OffsetDateTime::now_utc();
    let observations = (0..20)
        .map(|index| low_competition_test_observation(index, now, true))
        .collect::<Vec<_>>();

    let report = build_low_competition_shadow_report(&observations, 24, &config, now);

    assert!(report.should_consider_enforce);
    assert_eq!(report.observations, 20);
    assert_eq!(report.unique_markets, 3);
    assert_eq!(report.gate_pass_count, 20);
    assert_eq!(report.estimated_reward_per_100_usd_day_median, Some(decimal("0.50")));
    assert_eq!(report.exit_depth_multiple_median, Some(decimal("5.0000")));
}

#[test]
fn shadow_report_blocks_recommendation_on_sparse_or_unstable_samples() {
    let config = low_competition_plan_config(RewardLowCompetitionMode::Observe);
    let now = OffsetDateTime::now_utc();
    let observations = (0..5)
        .map(|index| low_competition_test_observation(index, now, index % 2 == 0))
        .collect::<Vec<_>>();

    let report = build_low_competition_shadow_report(&observations, 24, &config, now);

    assert!(!report.should_consider_enforce);
    assert!(report
        .recommendation_reasons
        .iter()
        .any(|reason| reason.contains("observations")));
    assert!(report.sample_insufficient_ratio > Decimal::ZERO);
}

fn low_competition_test_observation(
    index: usize,
    observed_at: OffsetDateTime,
    healthy: bool,
) -> RewardLowCompetitionObservation {
    let condition_id = format!("cond_low_comp_{}", index % 3);
    RewardLowCompetitionObservation {
        id: format!("obs_{index}"),
        account_id: "acct".to_string(),
        condition_id,
        market_slug: format!("low-comp-{index}"),
        question: "Low competition report test".to_string(),
        observed_at,
        mode: RewardLowCompetitionMode::Observe,
        planned_notional_usd: decimal("10"),
        competition_probe_notional_usd: decimal("10"),
        qualified_competition_usd: decimal("8"),
        competition_share_bps: decimal("5555.56"),
        competition_multiple: decimal("0.8"),
        estimated_reward_per_100_usd_day: if healthy {
            decimal("0.50")
        } else {
            decimal("0.10")
        },
        competition_density: decimal("0.16"),
        account_effective_available_usd: decimal("1000"),
        low_competition_open_buy_notional_usd: Decimal::ZERO,
        low_competition_open_buy_notional_usd_after_plan: decimal("10"),
        condition_buy_notional_usd_after_plan: decimal("10"),
        account_allocation_bps: decimal("100"),
        market_allocation_bps: decimal("100"),
        exit_depth_usd: if healthy { decimal("50") } else { decimal("5") },
        exit_slippage_cents: Some(decimal("0.25")),
        midpoint_range_cents: Some(if healthy { decimal("1") } else { decimal("3") }),
        top_of_book_flip_count: Some(0),
        sample_count: if healthy { 20 } else { 2 },
        sample_insufficient: !healthy,
        eligible_for_low_competition: healthy,
        final_eligible: false,
        ai_blocked: false,
        info_risk_blocked: false,
        standard_plan_overlap: false,
        not_low_competition: false,
        rejection_reasons: if healthy {
            Vec::new()
        } else {
            vec!["book history samples 2 below required 20".to_string()]
        },
        created_at: observed_at,
    }
}

#[test]
fn not_low_competition_flags_high_competition_candidates() {
    // 生产诊断：候选 competition_multiple 中位 174×，盘口竞争极激烈但流动性低被归为低竞争。
    // 候选早期剔除阈值默认 20；构造盘口竞争深度让 multiple 远超 20，验证标签生效。
    let config = low_competition_plan_config(RewardLowCompetitionMode::Observe);
    let books = test_books_with_competition(decimal("250"), decimal("250"));
    let history = stable_book_history(&books, config.low_competition_min_book_samples);
    let mut plans = build_reward_quote_plans_for_candidates(
        &[RewardCandidateMarket {
            market: test_market(decimal("5")),
            strategy_bucket: RewardStrategyBucket::LowCompetition,
        }],
        &books,
        &config,
    );

    apply_low_competition_metrics_to_quote_plans(
        &mut plans,
        &books,
        &history,
        &[],
        &test_account(decimal("1000")),
        &config,
    );

    let metrics = plans[0]
        .low_competition_metrics
        .as_ref()
        .expect("low competition metrics");
    assert!(
        metrics.competition_multiple > decimal("20"),
        "competition multiple should exceed candidate threshold: {:?}",
        metrics.competition_multiple
    );
    assert!(
        metrics.not_low_competition,
        "high-competition candidate should be flagged as not_low_competition"
    );
    assert!(
        metrics
            .not_low_competition_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("early-exclusion")),
        "flag reason should mention early-exclusion: {:?}",
        metrics.not_low_competition_reason
    );
    // 关键约束：分类标签绝不污染正式 gate 的 rejection_reasons
    assert!(
        !metrics
            .rejection_reasons
            .iter()
            .any(|reason| reason.contains("early-exclusion")
                || reason.contains("not_low_competition")),
        "early-exclusion label must not leak into rejection_reasons: {:?}",
        metrics.rejection_reasons
    );
    // eligible_for_low_competition 仍由正式 gate 独立决定（multiple > 正式阈值 1 → 不通过）
    assert!(
        !metrics.eligible_for_low_competition,
        "formal gate must still reject high-competition candidate independently"
    );
}

#[test]
fn not_low_competition_respects_configured_candidate_threshold() {
    // 把候选早期剔除阈值调到 1000，同样的高竞争盘口不再被打标签
    // （但正式 gate 仍独立拦截），证明阈值可配且标签与正式 gate 解耦。
    let config = RewardBotConfig {
        low_competition_candidate_max_competition_multiple: decimal("1000"),
        ..low_competition_plan_config(RewardLowCompetitionMode::Observe)
    };
    let books = test_books_with_competition(decimal("250"), decimal("250"));
    let history = stable_book_history(&books, config.low_competition_min_book_samples);
    let mut plans = build_reward_quote_plans_for_candidates(
        &[RewardCandidateMarket {
            market: test_market(decimal("5")),
            strategy_bucket: RewardStrategyBucket::LowCompetition,
        }],
        &books,
        &config,
    );

    apply_low_competition_metrics_to_quote_plans(
        &mut plans,
        &books,
        &history,
        &[],
        &test_account(decimal("1000")),
        &config,
    );

    let metrics = plans[0]
        .low_competition_metrics
        .as_ref()
        .expect("low competition metrics");
    assert!(
        !metrics.not_low_competition,
        "multiple below configured 1000 threshold should not be flagged"
    );
    assert!(metrics.not_low_competition_reason.is_none());
    assert!(
        !metrics.eligible_for_low_competition,
        "formal gate still rejects via competition_multiple > 1"
    );
}

#[test]
fn shadow_report_counts_not_low_competition_observations() {
    let config = low_competition_plan_config(RewardLowCompetitionMode::Observe);
    let now = OffsetDateTime::now_utc();
    let observations = (0..10)
        .map(|index| {
            let mut observation = low_competition_test_observation(index, now, true);
            // 标记前 7 个为“高竞争混入”
            observation.not_low_competition = index < 7;
            observation
        })
        .collect::<Vec<_>>();

    let report = build_low_competition_shadow_report(&observations, 24, &config, now);

    assert_eq!(report.not_low_competition_count, 7);
    assert_eq!(report.not_low_competition_ratio, decimal("0.7"));
}
