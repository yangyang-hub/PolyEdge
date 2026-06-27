fn opportunity_test_market(rewards_min_size: Decimal) -> RewardMarket {
    RewardMarket {
        condition_id: "cond_opportunity".to_string(),
        question: "Opportunity reward market".to_string(),
        market_slug: "opportunity-reward-market".to_string(),
        event_slug: "opportunity-event".to_string(),
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
                token_id: "yes_opportunity".to_string(),
                outcome: "Yes".to_string(),
                price: None,
            },
            RewardToken {
                token_id: "no_opportunity".to_string(),
                outcome: "No".to_string(),
                price: None,
            },
        ],
        active: true,
        updated_at: OffsetDateTime::now_utc(),
    }
}

fn opportunity_test_books(
    yes_competition_usd: Decimal,
    no_competition_usd: Decimal,
) -> HashMap<String, RewardOrderBook> {
    let now = OffsetDateTime::now_utc();
    let yes_price = decimal("0.77");
    let no_price = decimal("0.22");
    [
        RewardOrderBook {
            token_id: "yes_opportunity".to_string(),
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
            token_id: "no_opportunity".to_string(),
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

fn opportunity_book_history(
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

fn opportunity_test_account(available_usd: Decimal) -> RewardAccountState {
    RewardAccountState::fresh("acct", available_usd, OffsetDateTime::now_utc())
}

fn opportunity_config() -> RewardBotConfig {
    RewardBotConfig {
        opportunity_metrics_enabled: true,
        opportunity_probe_notional_usd: decimal("10"),
        opportunity_min_reward_per_100_usd_day: decimal("5"),
        opportunity_max_competition_multiple: decimal("1"),
        opportunity_min_exit_depth_usd: decimal("20"),
        opportunity_min_exit_depth_multiple: decimal("1"),
        opportunity_max_entry_exit_slippage_cents: decimal("40"),
        opportunity_max_bad_fill_recovery_days: decimal("30"),
        opportunity_min_book_samples: 3,
        opportunity_observation_window_sec: 300,
        opportunity_max_midpoint_range_cents: decimal("5"),
        opportunity_max_top_of_book_flip_count: 10,
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    }
}

#[test]
fn opportunity_metrics_force_unified_standard_bucket_and_clear_legacy_metrics() {
    let config = opportunity_config();
    let books = opportunity_test_books(decimal("4"), decimal("4"));
    let history = opportunity_book_history(&books, 3);
    let mut plan =
        build_reward_quote_plan(&opportunity_test_market(decimal("20")), &books, &config);
    plan.strategy_bucket = RewardStrategyBucket::LowCompetition;
    plan.low_competition_metrics = Some(empty_legacy_low_competition_metrics());
    let mut plans = vec![plan];

    apply_reward_opportunity_metrics_to_quote_plans(
        &mut plans,
        &books,
        &history,
        &[],
        &opportunity_test_account(decimal("1000")),
        &config,
    );

    assert_eq!(plans[0].strategy_bucket, RewardStrategyBucket::Standard);
    assert!(plans[0].low_competition_metrics.is_none());
    assert!(plans[0].opportunity_metrics.is_some());
}

#[test]
fn disabled_opportunity_metrics_still_clear_legacy_low_competition_bucket() {
    let config = RewardBotConfig {
        opportunity_metrics_enabled: false,
        ..opportunity_config()
    };
    let books = opportunity_test_books(decimal("4"), decimal("4"));
    let history = opportunity_book_history(&books, 3);
    let mut plan =
        build_reward_quote_plan(&opportunity_test_market(decimal("20")), &books, &config);
    plan.strategy_bucket = RewardStrategyBucket::LowCompetition;
    plan.low_competition_metrics = Some(empty_legacy_low_competition_metrics());
    let mut plans = vec![plan];

    apply_reward_opportunity_metrics_to_quote_plans(
        &mut plans,
        &books,
        &history,
        &[],
        &opportunity_test_account(decimal("1000")),
        &config,
    );

    assert_eq!(plans[0].strategy_bucket, RewardStrategyBucket::Standard);
    assert!(plans[0].low_competition_metrics.is_none());
    assert!(plans[0].opportunity_metrics.is_none());
}

#[test]
fn opportunity_metrics_penalize_crowded_reward_markets() {
    let config = RewardBotConfig {
        opportunity_reward_weight: Decimal::ZERO,
        opportunity_competition_weight: decimal("100"),
        opportunity_exit_weight: Decimal::ZERO,
        opportunity_stability_weight: Decimal::ZERO,
        ..opportunity_config()
    };
    let books = opportunity_test_books(decimal("100"), decimal("100"));
    let history = opportunity_book_history(&books, 3);
    let plan = build_reward_quote_plan(&opportunity_test_market(decimal("20")), &books, &config);
    let base_score = plan.score;
    let mut plans = vec![plan];

    apply_reward_opportunity_metrics_to_quote_plans(
        &mut plans,
        &books,
        &history,
        &[],
        &opportunity_test_account(decimal("1000")),
        &config,
    );

    let metrics = plans[0]
        .opportunity_metrics
        .as_ref()
        .expect("opportunity metrics");
    assert!(metrics.competition_multiple > config.opportunity_max_competition_multiple);
    assert!(metrics.score_adjustment < Decimal::ZERO);
    assert!(plans[0].score < base_score);
    assert!(
        metrics
            .warnings
            .iter()
            .any(|warning| warning.contains("competition multiple"))
    );
}

#[test]
fn opportunity_metrics_refresh_is_idempotent_and_does_not_promote_blocked_plan() {
    let config = RewardBotConfig {
        min_market_score: decimal("60"),
        opportunity_reward_weight: decimal("100"),
        opportunity_competition_weight: Decimal::ZERO,
        opportunity_exit_weight: Decimal::ZERO,
        opportunity_stability_weight: Decimal::ZERO,
        ..opportunity_config()
    };
    let books = opportunity_test_books(decimal("4"), decimal("4"));
    let history = opportunity_book_history(&books, 3);
    let mut plan =
        build_reward_quote_plan(&opportunity_test_market(decimal("20")), &books, &config);
    plan.score = decimal("55");
    plan.eligible = false;
    plan.pre_ai_eligible = false;
    plan.reason = "score is below threshold".to_string();
    let mut plans = vec![plan];

    refresh_reward_opportunity_metrics_for_quote_plans(
        &mut plans,
        &books,
        &history,
        &[],
        &opportunity_test_account(decimal("1000")),
        &config,
    );

    let first_score = plans[0].score;
    let first_adjustment = plans[0]
        .opportunity_metrics
        .as_ref()
        .expect("opportunity metrics")
        .score_adjustment;
    assert!(first_score >= config.min_market_score);
    assert!(first_adjustment > Decimal::ZERO);
    assert!(!plans[0].eligible);
    assert!(!plans[0].pre_ai_eligible);
    assert_eq!(plans[0].reason, "score is below threshold");

    refresh_reward_opportunity_metrics_for_quote_plans(
        &mut plans,
        &books,
        &history,
        &[],
        &opportunity_test_account(decimal("1000")),
        &config,
    );

    assert_eq!(plans[0].score, first_score);
    assert_eq!(
        plans[0]
            .opportunity_metrics
            .as_ref()
            .expect("opportunity metrics")
            .score_adjustment,
        first_adjustment
    );
}

#[test]
fn opportunity_metrics_use_snapshot_frozen_unmanaged_external_occupancy() {
    let mut account = opportunity_test_account(decimal("100"));
    account.unmanaged_external_buy_notional = decimal("25");

    assert_eq!(
        account_effective_available_after_unmanaged_external_buys(&account),
        decimal("75")
    );

    account.unmanaged_external_buy_notional = decimal("125");
    assert_eq!(
        account_effective_available_after_unmanaged_external_buys(&account),
        Decimal::ZERO
    );
}

fn empty_legacy_low_competition_metrics() -> RewardLowCompetitionMetrics {
    RewardLowCompetitionMetrics {
        planned_notional_usd: Decimal::ZERO,
        competition_probe_notional_usd: Decimal::ZERO,
        qualified_competition_usd: Decimal::ZERO,
        competition_share_bps: Decimal::ZERO,
        competition_multiple: Decimal::ZERO,
        estimated_reward_per_100_usd_day: Decimal::ZERO,
        competition_density: Decimal::ZERO,
        account_effective_available_usd: Decimal::ZERO,
        low_competition_open_buy_notional_usd: Decimal::ZERO,
        low_competition_open_buy_notional_usd_after_plan: Decimal::ZERO,
        condition_buy_notional_usd_after_plan: Decimal::ZERO,
        account_allocation_bps: Decimal::ZERO,
        market_allocation_bps: Decimal::ZERO,
        exit_depth_usd: Decimal::ZERO,
        exit_slippage_cents: None,
        bad_fill_recovery_days: None,
        midpoint_range_cents: None,
        top_of_book_flip_count: None,
        sample_count: 0,
        eligible_for_low_competition: false,
        rejection_reasons: Vec::new(),
        not_low_competition: false,
        not_low_competition_reason: None,
    }
}
