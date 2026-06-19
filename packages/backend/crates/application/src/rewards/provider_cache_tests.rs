use super::*;

fn cache_test_market() -> RewardMarket {
    let now = OffsetDateTime::from_unix_timestamp(1_785_000_000).expect("valid timestamp");
    RewardMarket {
        condition_id: "cond_cache".to_string(),
        question: "Will the cache key stay stable?".to_string(),
        market_slug: "cache-key-stability".to_string(),
        event_slug: "cache-key-event".to_string(),
        category: "politics".to_string(),
        image: String::new(),
        rewards_max_spread: decimal("4.5"),
        rewards_min_size: decimal("100"),
        total_daily_rate: decimal("50"),
        liquidity_usd: decimal("10000"),
        volume_24h_usd: decimal("25000"),
        market_spread_cents: decimal("1"),
        end_at: Some(now + TimeDuration::days(30)),
        ambiguity_level: "low".to_string(),
        market_synced_at: Some(now),
        tokens: vec![
            RewardToken {
                token_id: "token_yes_cache".to_string(),
                outcome: "Yes".to_string(),
                price: Some(decimal("0.55")),
            },
            RewardToken {
                token_id: "token_no_cache".to_string(),
                outcome: "No".to_string(),
                price: Some(decimal("0.45")),
            },
        ],
        active: true,
        updated_at: now,
    }
}

fn cache_test_books(observed_at: OffsetDateTime, yes_bid: &str) -> HashMap<String, RewardOrderBook> {
    [
        RewardOrderBook {
            token_id: "token_yes_cache".to_string(),
            bids: vec![RewardBookLevel {
                price: decimal(yes_bid),
                size: decimal("1000"),
            }],
            asks: vec![RewardBookLevel {
                price: decimal("0.56"),
                size: decimal("1000"),
            }],
            observed_at,
        },
        RewardOrderBook {
            token_id: "token_no_cache".to_string(),
            bids: vec![RewardBookLevel {
                price: decimal("0.44"),
                size: decimal("1000"),
            }],
            asks: vec![RewardBookLevel {
                price: decimal("0.45"),
                size: decimal("1000"),
            }],
            observed_at,
        },
    ]
    .into_iter()
    .map(|book| (book.token_id.clone(), book))
    .collect()
}

fn cache_test_plan(market: &RewardMarket, books: &HashMap<String, RewardOrderBook>) -> RewardQuotePlan {
    let config = RewardBotConfig {
        min_market_score: Decimal::ZERO,
        ..RewardBotConfig::default()
    };
    build_reward_quote_plans(std::slice::from_ref(market), books, &config)
        .into_iter()
        .next()
        .expect("quote plan")
}

fn cache_test_account(available_usd: &str, tick_index: i64) -> RewardAccountState {
    let now = OffsetDateTime::from_unix_timestamp(1_785_000_000 + tick_index)
        .expect("valid timestamp");
    RewardAccountState {
        account_id: "reward_bot".to_string(),
        wallet_address: Some("0x0000000000000000000000000000000000000001".to_string()),
        capital_usd: decimal("1000"),
        available_usd: decimal(available_usd),
        external_buy_notional: decimal("0"),
        reserved_usd: Decimal::ZERO,
        realized_pnl: Decimal::ZERO,
        reward_earned_usd: Decimal::ZERO,
        fees_paid: Decimal::ZERO,
        tick_index,
        updated_at: now,
    }
}

fn cache_test_order(updated_at: OffsetDateTime) -> ManagedRewardOrder {
    ManagedRewardOrder {
        id: "order_cache".to_string(),
        account_id: "reward_bot".to_string(),
        condition_id: "cond_cache".to_string(),
        token_id: "token_yes_cache".to_string(),
        outcome: "Yes".to_string(),
        side: RewardOrderSide::Buy,
        price: decimal("0.55"),
        size: decimal("10"),
        external_order_id: Some("external_cache".to_string()),
        status: ManagedRewardOrderStatus::Open,
        scoring: false,
        reason: "test".to_string(),
        filled_size: Decimal::ZERO,
        reward_earned: Decimal::ZERO,
        last_scored_at: None,
        created_at: updated_at,
        updated_at,
    }
}

fn cache_test_position(updated_at: OffsetDateTime) -> RewardPosition {
    RewardPosition {
        account_id: "reward_bot".to_string(),
        condition_id: "cond_cache".to_string(),
        token_id: "token_yes_cache".to_string(),
        outcome: "Yes".to_string(),
        size: decimal("5"),
        avg_price: decimal("0.52"),
        realized_pnl: Decimal::ZERO,
        updated_at,
    }
}

#[test]
fn reward_ai_advisory_cache_key_ignores_runtime_context() {
    let market = cache_test_market();
    let base_time = OffsetDateTime::from_unix_timestamp(1_785_000_000).expect("valid timestamp");
    let books = cache_test_books(base_time, "0.54");
    let plan = cache_test_plan(&market, &books);
    let config = RewardBotConfig::default();

    let first = build_reward_ai_advisory_request(
        &market,
        &plan,
        &cache_test_account("33.27", 1),
        &[],
        &[],
        &books,
        &config,
        RewardAiProvider::OpenAi,
        RewardAiRequestFormat::OpenAiChatCompletions,
        "mimo-v2.5",
    )
    .expect("first request");

    let later_time = base_time + TimeDuration::seconds(60);
    let second = build_reward_ai_advisory_request(
        &market,
        &plan,
        &cache_test_account("250.00", 2),
        &[cache_test_position(later_time)],
        &[cache_test_order(later_time)],
        &cache_test_books(later_time, "0.53"),
        &config,
        RewardAiProvider::OpenAi,
        RewardAiRequestFormat::OpenAiChatCompletions,
        "mimo-v2.5",
    )
    .expect("second request");

    assert_eq!(first.input_hash, second.input_hash);
    assert_ne!(first.payload, second.payload);
}

#[test]
fn reward_ai_advisory_cache_key_tracks_strategy_changes() {
    let market = cache_test_market();
    let books = cache_test_books(
        OffsetDateTime::from_unix_timestamp(1_785_000_000).expect("valid timestamp"),
        "0.54",
    );
    let plan = cache_test_plan(&market, &books);
    let account = cache_test_account("100", 1);
    let base_config = RewardBotConfig::default();
    let changed_config = RewardBotConfig {
        quote_bid_rank: 2,
        ..RewardBotConfig::default()
    };

    let first = build_reward_ai_advisory_request(
        &market,
        &plan,
        &account,
        &[],
        &[],
        &books,
        &base_config,
        RewardAiProvider::OpenAi,
        RewardAiRequestFormat::OpenAiChatCompletions,
        "mimo-v2.5",
    )
    .expect("first request");
    let second = build_reward_ai_advisory_request(
        &market,
        &plan,
        &account,
        &[],
        &[],
        &books,
        &changed_config,
        RewardAiProvider::OpenAi,
        RewardAiRequestFormat::OpenAiChatCompletions,
        "mimo-v2.5",
    )
    .expect("second request");

    assert_ne!(first.input_hash, second.input_hash);
}

#[test]
fn reward_info_risk_cache_key_ignores_runtime_context() {
    let market = cache_test_market();
    let books = cache_test_books(
        OffsetDateTime::from_unix_timestamp(1_785_000_000).expect("valid timestamp"),
        "0.54",
    );
    let mut plan = cache_test_plan(&market, &books);
    let config = RewardBotConfig::default();

    let first = build_reward_info_risk_assessment_request(
        &market,
        Some(&plan),
        &cache_test_account("33.27", 1),
        &[],
        &[],
        &config,
        RewardAiProvider::OpenAi,
        RewardAiRequestFormat::OpenAiChatCompletions,
        "mimo-v2.5",
    )
    .expect("first request");

    plan.reason = "runtime gate changed this reason".to_string();
    plan.score += decimal("10");
    let later_time = OffsetDateTime::from_unix_timestamp(1_785_000_060).expect("valid timestamp");
    let second = build_reward_info_risk_assessment_request(
        &market,
        Some(&plan),
        &cache_test_account("250.00", 2),
        &[cache_test_position(later_time)],
        &[cache_test_order(later_time)],
        &config,
        RewardAiProvider::OpenAi,
        RewardAiRequestFormat::OpenAiChatCompletions,
        "mimo-v2.5",
    )
    .expect("second request");

    assert_eq!(first.input_hash, second.input_hash);
    assert_ne!(first.payload, second.payload);
}

#[test]
fn reward_info_risk_cache_key_tracks_risk_policy_changes() {
    let market = cache_test_market();
    let books = cache_test_books(
        OffsetDateTime::from_unix_timestamp(1_785_000_000).expect("valid timestamp"),
        "0.54",
    );
    let plan = cache_test_plan(&market, &books);
    let account = cache_test_account("100", 1);
    let base_config = RewardBotConfig::default();
    let changed_config = RewardBotConfig {
        info_risk_avoid_level: RewardInfoRiskLevel::Medium,
        ..RewardBotConfig::default()
    };

    let first = build_reward_info_risk_assessment_request(
        &market,
        Some(&plan),
        &account,
        &[],
        &[],
        &base_config,
        RewardAiProvider::OpenAi,
        RewardAiRequestFormat::OpenAiChatCompletions,
        "mimo-v2.5",
    )
    .expect("first request");
    let second = build_reward_info_risk_assessment_request(
        &market,
        Some(&plan),
        &account,
        &[],
        &[],
        &changed_config,
        RewardAiProvider::OpenAi,
        RewardAiRequestFormat::OpenAiChatCompletions,
        "mimo-v2.5",
    )
    .expect("second request");

    assert_ne!(first.input_hash, second.input_hash);
}
