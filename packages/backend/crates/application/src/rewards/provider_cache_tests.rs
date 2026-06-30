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
            confirmed_at: observed_at,
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
            confirmed_at: observed_at,
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
        unmanaged_external_buy_notional: decimal("0"),
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
        strategy_bucket: RewardStrategyBucket::Standard,
        strategy_profile: RewardStrategyProfile::Standard,
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

fn cache_test_candle(token_id: &str, outcome: &str, bucket_offset: i64, close: &str) -> RewardMarketCandle {
    let bucket_start = OffsetDateTime::from_unix_timestamp(1_785_000_000 + bucket_offset)
        .expect("valid timestamp");
    RewardMarketCandle {
        token_id: token_id.to_string(),
        condition_id: "cond_cache".to_string(),
        outcome: outcome.to_string(),
        interval_sec: REWARD_AI_CANDLE_SOURCE_INTERVAL_SEC,
        bucket_start,
        open: decimal("0.50"),
        high: decimal(close).max(decimal("0.50")),
        low: decimal(close).min(decimal("0.50")),
        close: decimal(close),
        best_bid_close: decimal("0.49"),
        best_ask_close: decimal("0.51"),
        spread_cents_close: decimal("2"),
        sample_count: 3,
        close_observed_at: bucket_start + TimeDuration::seconds(30),
        updated_at: bucket_start + TimeDuration::seconds(30),
    }
}

fn cache_test_ai_decision() -> RewardAiAdvisoryDecision {
    RewardAiAdvisoryDecision {
        suitability: RewardAiSuitability::Allow,
        quote_mode: RewardPlanQuoteMode::Double,
        exit_policy: PostFillStrategy::ExitAtMarkup,
        confidence: decimal("0.91"),
        reasons: vec!["stable enough for cache expiry test".to_string()],
        metrics: json!({"fixture": "ai"}),
    }
}

fn cache_test_info_risk_decision() -> RewardInfoRiskAssessmentDecision {
    RewardInfoRiskAssessmentDecision {
        risk_level: RewardInfoRiskLevel::Low,
        risk_type: RewardInfoRiskType::None,
        directional_risk: RewardInfoDirectionalRisk::Unclear,
        resolution_imminent: false,
        expected_event_at: None,
        confidence: decimal("0.93"),
        summary: "No current information-risk catalyst in fixture.".to_string(),
        sources: Vec::new(),
        metrics: json!({"fixture": "info_risk"}),
    }
}

fn assert_provider_cache_expiry_window(
    now: OffsetDateTime,
    ttl_sec: u64,
    expires_at: OffsetDateTime,
) {
    let base_expiry = now + TimeDuration::seconds(ttl_sec as i64);
    let max_expiry = base_expiry
        + TimeDuration::seconds(reward_provider_cache_jitter_window_sec(ttl_sec) as i64);
    assert!(expires_at >= base_expiry);
    assert!(expires_at <= max_expiry);
}

#[test]
fn reward_ai_advisory_expiry_uses_deterministic_jitter() {
    let now = OffsetDateTime::from_unix_timestamp(1_785_000_000).expect("valid timestamp");
    let ttl_sec = 3600;
    let request = RewardAiAdvisoryRequest {
        condition_id: "cond_cache".to_string(),
        provider: RewardAiProvider::OpenAi,
        request_format: RewardAiRequestFormat::OpenAiChatCompletions,
        model: "mimo-v2.5".to_string(),
        input_hash: "input-hash-cache".to_string(),
        payload: json!({}),
    };

    let first = cache_test_ai_decision()
        .into_advisory(&request, ttl_sec, now)
        .expires_at;
    let second = cache_test_ai_decision()
        .into_advisory(&request, ttl_sec, now)
        .expires_at;

    assert_eq!(first, second);
    assert_provider_cache_expiry_window(now, ttl_sec, first);

    let mut spread = None;
    for index in 0..32 {
        let mut other = request.clone();
        other.condition_id = format!("cond_cache_{index}");
        other.input_hash = format!("input-hash-cache-{index}");
        let other_expires_at = cache_test_ai_decision()
            .into_advisory(&other, ttl_sec, now)
            .expires_at;
        if other_expires_at != first {
            spread = Some(other_expires_at);
            break;
        }
    }
    assert!(spread.is_some());
}

#[test]
fn reward_info_risk_expiry_jitter_and_refresh_windows_are_separate() {
    let now = OffsetDateTime::from_unix_timestamp(1_785_000_000).expect("valid timestamp");
    let ttl_sec = 3600;
    let request = RewardInfoRiskAssessmentRequest {
        condition_id: "cond_cache".to_string(),
        provider: RewardAiProvider::OpenAi,
        request_format: RewardAiRequestFormat::OpenAiChatCompletions,
        model: "mimo-v2.5".to_string(),
        query: "cache expiry risk fixture".to_string(),
        query_hash: "query-hash-cache".to_string(),
        input_hash: "input-hash-cache".to_string(),
        payload: json!({}),
    };

    let expires_at = cache_test_info_risk_decision()
        .into_info_risk(&request, ttl_sec, now)
        .expires_at;
    assert_provider_cache_expiry_window(now, ttl_sec, expires_at);

    let jitter_window = reward_provider_cache_jitter_window_sec(ttl_sec);
    let refresh_window = reward_provider_cache_refresh_window_sec(ttl_sec);
    assert_eq!(jitter_window, 720);
    assert_eq!(refresh_window, 60);
    assert!(!reward_provider_cache_refresh_due(
        expires_at,
        ttl_sec,
        expires_at - TimeDuration::seconds(refresh_window as i64 + 1),
    ));
    assert!(reward_provider_cache_refresh_due(
        expires_at,
        ttl_sec,
        expires_at - TimeDuration::seconds(refresh_window as i64),
    ));
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
        &[],
        &config,
        config.ai_advisory_ttl_sec,
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
        &[],
        &config,
        config.ai_advisory_ttl_sec,
        RewardAiProvider::OpenAi,
        RewardAiRequestFormat::OpenAiChatCompletions,
        "mimo-v2.5",
    )
    .expect("second request");

    assert_eq!(first.input_hash, second.input_hash);
    assert_ne!(first.payload, second.payload);
    assert!(first.payload.get("pricing_context").is_some());
    assert_eq!(
        first
            .payload
            .pointer("/provider_cache_policy/ttl_sec")
            .and_then(Value::as_u64),
        Some(config.ai_advisory_ttl_sec)
    );
}

#[test]
fn reward_ai_advisory_cache_key_ignores_in_progress_source_candle_changes() {
    let market = cache_test_market();
    let books = cache_test_books(
        OffsetDateTime::from_unix_timestamp(1_785_000_000).expect("valid timestamp"),
        "0.54",
    );
    let plan = cache_test_plan(&market, &books);
    let account = cache_test_account("100", 1);
    let config = RewardBotConfig::default();
    let first_candles = vec![
        cache_test_candle("token_yes_cache", "Yes", 0, "0.51"),
        cache_test_candle("token_no_cache", "No", 0, "0.49"),
    ];
    let second_candles = vec![
        cache_test_candle("token_yes_cache", "Yes", 0, "0.51"),
        cache_test_candle("token_yes_cache", "Yes", 300, "0.56"),
        cache_test_candle("token_no_cache", "No", 0, "0.49"),
        cache_test_candle("token_no_cache", "No", 300, "0.44"),
    ];

    let first = build_reward_ai_advisory_request(
        &market,
        &plan,
        &account,
        &[],
        &[],
        &books,
        &first_candles,
        &config,
        config.ai_advisory_ttl_sec,
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
        &second_candles,
        &config,
        config.ai_advisory_ttl_sec,
        RewardAiProvider::OpenAi,
        RewardAiRequestFormat::OpenAiChatCompletions,
        "mimo-v2.5",
    )
    .expect("second request");

    assert_eq!(first.input_hash, second.input_hash);
    assert_ne!(first.payload, second.payload);
    assert!(first.payload.get("candles").is_some());
    assert!(first.payload.get("candle_summary").is_some());
    let interval_sec = first
        .payload
        .get("candle_summary")
        .and_then(|summary| summary.get("interval_sec"))
        .and_then(Value::as_i64);
    assert_eq!(interval_sec, Some(i64::from(REWARD_AI_CANDLE_INTERVAL_SEC)));
}

#[test]
fn reward_ai_advisory_cache_key_tracks_completed_hourly_candle_changes() {
    let market = cache_test_market();
    let books = cache_test_books(
        OffsetDateTime::from_unix_timestamp(1_785_000_000).expect("valid timestamp"),
        "0.54",
    );
    let plan = cache_test_plan(&market, &books);
    let account = cache_test_account("100", 1);
    let config = RewardBotConfig::default();
    let first_candles = vec![
        cache_test_candle("token_yes_cache", "Yes", 0, "0.51"),
        cache_test_candle("token_yes_cache", "Yes", 3_600, "0.52"),
        cache_test_candle("token_no_cache", "No", 0, "0.49"),
        cache_test_candle("token_no_cache", "No", 3_600, "0.48"),
    ];
    let second_candles = vec![
        cache_test_candle("token_yes_cache", "Yes", 0, "0.56"),
        cache_test_candle("token_yes_cache", "Yes", 3_600, "0.52"),
        cache_test_candle("token_no_cache", "No", 0, "0.44"),
        cache_test_candle("token_no_cache", "No", 3_600, "0.48"),
    ];

    let first = build_reward_ai_advisory_request(
        &market,
        &plan,
        &account,
        &[],
        &[],
        &books,
        &first_candles,
        &config,
        config.ai_advisory_ttl_sec,
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
        &second_candles,
        &config,
        config.ai_advisory_ttl_sec,
        RewardAiProvider::OpenAi,
        RewardAiRequestFormat::OpenAiChatCompletions,
        "mimo-v2.5",
    )
    .expect("second request");

    assert_ne!(first.input_hash, second.input_hash);
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
        &[],
        &base_config,
        base_config.ai_advisory_ttl_sec,
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
        &[],
        &changed_config,
        changed_config.ai_advisory_ttl_sec,
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
    assert!(first.payload.get("evaluation_time_utc").is_some());
    assert!(first
        .payload
        .pointer("/imminent_resolution_policy/current_time_source")
        .is_some());
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

#[test]
fn reward_info_risk_cache_key_ignores_quote_mode_changes() {
    let market = cache_test_market();
    let books = cache_test_books(
        OffsetDateTime::from_unix_timestamp(1_785_000_000).expect("valid timestamp"),
        "0.54",
    );
    let mut plan = cache_test_plan(&market, &books);
    let account = cache_test_account("100", 1);
    let config = RewardBotConfig::default();

    let first = build_reward_info_risk_assessment_request(
        &market,
        Some(&plan),
        &account,
        &[],
        &[],
        &config,
        RewardAiProvider::OpenAi,
        RewardAiRequestFormat::OpenAiChatCompletions,
        "mimo-v2.5",
    )
    .expect("first request");

    // The materialized quote mode flips between double and single_no every tick
    // for markets on the funding boundary; the info-risk assessment must not be
    // invalidated by that oscillation (it caused eligible_markets to drop to 0
    // intermittently under enforce + require_info_risk_before_first_quote).
    plan.quote_mode = RewardPlanQuoteMode::SingleNo;
    plan.recommended_quote_mode = Some(RewardPlanQuoteMode::SingleNo);

    let second = build_reward_info_risk_assessment_request(
        &market,
        Some(&plan),
        &account,
        &[],
        &[],
        &config,
        RewardAiProvider::OpenAi,
        RewardAiRequestFormat::OpenAiChatCompletions,
        "mimo-v2.5",
    )
    .expect("second request");

    assert_eq!(first.input_hash, second.input_hash);
}

#[test]
fn reward_ai_advisory_cache_key_ignores_quote_mode_changes() {
    let market = cache_test_market();
    let books = cache_test_books(
        OffsetDateTime::from_unix_timestamp(1_785_000_000).expect("valid timestamp"),
        "0.54",
    );
    let mut plan = cache_test_plan(&market, &books);
    let account = cache_test_account("100", 1);
    let config = RewardBotConfig::default();

    let first = build_reward_ai_advisory_request(
        &market,
        &plan,
        &account,
        &[],
        &[],
        &books,
        &[],
        &config,
        config.ai_advisory_ttl_sec,
        RewardAiProvider::OpenAi,
        RewardAiRequestFormat::OpenAiChatCompletions,
        "mimo-v2.5",
    )
    .expect("first request");

    plan.quote_mode = RewardPlanQuoteMode::SingleNo;
    plan.recommended_quote_mode = Some(RewardPlanQuoteMode::SingleNo);

    let second = build_reward_ai_advisory_request(
        &market,
        &plan,
        &account,
        &[],
        &[],
        &books,
        &[],
        &config,
        config.ai_advisory_ttl_sec,
        RewardAiProvider::OpenAi,
        RewardAiRequestFormat::OpenAiChatCompletions,
        "mimo-v2.5",
    )
    .expect("second request");

    assert_eq!(first.input_hash, second.input_hash);
}

#[test]
fn reward_info_risk_cache_key_ignores_sync_driven_market_metadata() {
    let mut market = cache_test_market();
    let books = cache_test_books(
        OffsetDateTime::from_unix_timestamp(1_785_000_000).expect("valid timestamp"),
        "0.54",
    );
    let plan = cache_test_plan(&market, &books);
    let account = cache_test_account("100", 1);
    let config = RewardBotConfig::default();

    let first = build_reward_info_risk_assessment_request(
        &market,
        Some(&plan),
        &account,
        &[],
        &[],
        &config,
        RewardAiProvider::OpenAi,
        RewardAiRequestFormat::OpenAiChatCompletions,
        "mimo-v2.5",
    )
    .expect("first request");

    // `event_slug` (rewards-catalog sync) and `ambiguity_level` (Gamma sync)
    // are written by two independent sync loops that drift across cycles. The
    // info-risk cache key must not be invalidated by that drift, otherwise a
    // still-valid risk row is missed on alternate ticks and eligible drops to 0.
    market.event_slug = "different-event-slug".to_string();
    market.ambiguity_level = "high".to_string();

    let second = build_reward_info_risk_assessment_request(
        &market,
        Some(&plan),
        &account,
        &[],
        &[],
        &config,
        RewardAiProvider::OpenAi,
        RewardAiRequestFormat::OpenAiChatCompletions,
        "mimo-v2.5",
    )
    .expect("second request");

    assert_eq!(first.input_hash, second.input_hash);
}
