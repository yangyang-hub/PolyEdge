pub fn select_reward_book_token_ids(markets: &[RewardMarket]) -> Vec<String> {
    let mut selected = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for market in markets {
        for token in market.tokens.iter().take(2) {
            if token.token_id.trim().is_empty() || !seen.insert(token.token_id.clone()) {
                continue;
            }
            selected.push(token.token_id.clone());
        }
    }

    selected
}

pub fn build_reward_quote_plans(
    markets: &[RewardMarket],
    books: &HashMap<String, RewardOrderBook>,
    config: &RewardBotConfig,
) -> Vec<RewardQuotePlan> {
    let mut plans = markets
        .iter()
        .map(|market| build_reward_quote_plan(market, books, config))
        .collect::<Vec<_>>();
    plans.sort_by(|left, right| {
        right
            .eligible
            .cmp(&left.eligible)
            .then_with(|| right.score.cmp(&left.score))
            .then_with(|| right.total_daily_rate.cmp(&left.total_daily_rate))
    });
    plans
}

fn build_reward_quote_plan(
    market: &RewardMarket,
    books: &HashMap<String, RewardOrderBook>,
    config: &RewardBotConfig,
) -> RewardQuotePlan {
    let now = OffsetDateTime::now_utc();
    let yes_token = market
        .tokens
        .iter()
        .find(|token| token.outcome.to_lowercase().contains("yes"))
        .or_else(|| market.tokens.first());
    let no_token = market
        .tokens
        .iter()
        .find(|token| token.outcome.to_lowercase().contains("no"))
        .or_else(|| market.tokens.get(1));
    let (Some(yes_token), Some(no_token)) = (yes_token, no_token) else {
        return empty_plan(market, "missing YES/NO token", now, None);
    };

    let yes_state = get_token_book_state(yes_token, books, config, now);
    let no_state = get_token_book_state(no_token, books, config, now);
    let midpoint = yes_state
        .as_ref()
        .map(|state| state.midpoint)
        .or_else(|| no_state.as_ref().map(|state| Decimal::ONE - state.midpoint));
    let Some(midpoint) = midpoint else {
        return empty_plan(market, "missing book or fallback token price", now, None);
    };

    if config.mode == RewardBotMode::Live
        && (!yes_state.as_ref().is_some_and(|state| state.fresh)
            || !no_state.as_ref().is_some_and(|state| state.fresh))
    {
        return empty_plan(
            market,
            "live mode requires fresh YES and NO books",
            now,
            Some(midpoint),
        );
    }

    if midpoint < config.min_midpoint || midpoint > config.max_midpoint {
        return empty_plan(
            market,
            "midpoint is too close to 0/1 for the first rewards strategy",
            now,
            Some(midpoint),
        );
    }

    if market.total_daily_rate < config.min_daily_reward {
        return empty_plan(
            market,
            "daily reward is below threshold",
            now,
            Some(midpoint),
        );
    }

    let max_spread_cents = Decimal::min(
        normalize_reward_spread_cents(market.rewards_max_spread),
        config.max_spread_cents,
    );
    if max_spread_cents <= Decimal::ZERO {
        return empty_plan(
            market,
            "invalid rewards spread setting",
            now,
            Some(midpoint),
        );
    }

    let quote_edge = Decimal::min(config.quote_edge_cents, max_spread_cents) / decimal("100");
    let safety = config.safety_margin_cents / decimal("100");
    let yes_bid = floor_to_tick(
        Decimal::max(decimal("0.01"), midpoint - quote_edge),
        DEFAULT_TICK,
    );
    let no_mid = Decimal::ONE - midpoint;
    let no_bid = floor_to_tick(
        Decimal::max(decimal("0.01"), no_mid - quote_edge),
        DEFAULT_TICK,
    );

    if yes_state
        .as_ref()
        .and_then(|state| state.best_ask)
        .is_some_and(|best_ask| yes_bid >= best_ask)
    {
        return empty_plan(market, "YES bid would touch best ask", now, Some(midpoint));
    }

    if no_state
        .as_ref()
        .and_then(|state| state.best_ask)
        .is_some_and(|best_ask| no_bid >= best_ask)
    {
        return empty_plan(market, "NO bid would touch best ask", now, Some(midpoint));
    }

    if yes_bid + no_bid > Decimal::ONE - safety {
        return empty_plan(
            market,
            "YES/NO bids do not leave enough safety margin",
            now,
            Some(midpoint),
        );
    }

    let legs = vec![
        make_leg(
            &yes_token.token_id,
            &yes_token.outcome,
            yes_bid,
            market.rewards_min_size,
            config,
        ),
        make_leg(
            &no_token.token_id,
            &no_token.outcome,
            no_bid,
            market.rewards_min_size,
            config,
        ),
    ];

    if market.rewards_min_size > Decimal::ZERO
        && legs.iter().any(|leg| leg.size < market.rewards_min_size)
    {
        return empty_plan(
            market,
            "per-market budget cannot satisfy rewards minimum size",
            now,
            Some(midpoint),
        );
    }

    let score = score_market(market, max_spread_cents, midpoint, &legs);
    let eligible = score >= config.min_market_score;

    RewardQuotePlan {
        condition_id: market.condition_id.clone(),
        market_slug: market.market_slug.clone(),
        question: market.question.clone(),
        score,
        eligible,
        reason: if eligible {
            "eligible for simulated post-only quotes".to_string()
        } else {
            "score is below threshold".to_string()
        },
        midpoint: Some(midpoint),
        total_daily_rate: market.total_daily_rate,
        rewards_max_spread: market.rewards_max_spread,
        rewards_min_size: market.rewards_min_size,
        legs,
        updated_at: now,
    }
}

fn empty_plan(
    market: &RewardMarket,
    reason: impl Into<String>,
    now: OffsetDateTime,
    midpoint: Option<Decimal>,
) -> RewardQuotePlan {
    RewardQuotePlan {
        condition_id: market.condition_id.clone(),
        market_slug: market.market_slug.clone(),
        question: market.question.clone(),
        score: Decimal::ZERO,
        eligible: false,
        reason: reason.into(),
        midpoint,
        total_daily_rate: market.total_daily_rate,
        rewards_max_spread: market.rewards_max_spread,
        rewards_min_size: market.rewards_min_size,
        legs: Vec::new(),
        updated_at: now,
    }
}

fn get_token_book_state(
    token: &RewardToken,
    books: &HashMap<String, RewardOrderBook>,
    config: &RewardBotConfig,
    now: OffsetDateTime,
) -> Option<TokenBookState> {
    let book = books.get(&token.token_id);
    let fresh = config.stale_book_ms == 0
        || book
            .and_then(|book| {
                (now - book.observed_at)
                    .whole_milliseconds()
                    .try_into()
                    .ok()
            })
            .is_some_and(|age_ms: u64| age_ms <= config.stale_book_ms);
    let (best_bid, best_ask) = if fresh {
        (
            book.and_then(|book| book.bids.first().map(|level| level.price)),
            book.and_then(|book| book.asks.first().map(|level| level.price)),
        )
    } else {
        (None, None)
    };

    if let (Some(best_bid), Some(best_ask)) = (best_bid, best_ask)
        && best_bid > Decimal::ZERO
        && best_ask > Decimal::ZERO
    {
        return Some(TokenBookState {
            midpoint: (best_bid + best_ask) / decimal("2"),
            best_ask: Some(best_ask),
            fresh: true,
        });
    }

    if config.mode != RewardBotMode::Live
        && let Some(price) = token
            .price
            .filter(|price| *price > Decimal::ZERO && *price < Decimal::ONE)
    {
        return Some(TokenBookState {
            midpoint: price,
            best_ask: None,
            fresh: false,
        });
    }

    None
}

fn make_leg(
    token_id: &str,
    outcome: &str,
    price: Decimal,
    rewards_min_size: Decimal,
    config: &RewardBotConfig,
) -> RewardQuoteLeg {
    let target_size = config.quote_size_usd / price;
    let max_leg_size = if config.per_market_usd == Decimal::ZERO {
        Decimal::MAX
    } else {
        config.per_market_usd / decimal("2") / price
    };
    let size = Decimal::min(Decimal::max(rewards_min_size, target_size), max_leg_size)
        .round_dp_with_strategy(2, RoundingStrategy::ToZero);

    RewardQuoteLeg {
        token_id: token_id.to_string(),
        outcome: if outcome.trim().is_empty() {
            token_id.to_string()
        } else {
            outcome.to_string()
        },
        side: RewardOrderSide::Buy,
        price,
        size,
        notional_usd: (price * size).round_dp(2),
    }
}

fn score_market(
    market: &RewardMarket,
    max_spread_cents: Decimal,
    midpoint: Decimal,
    legs: &[RewardQuoteLeg],
) -> Decimal {
    let reward_rate = decimal_to_f64(market.total_daily_rate).sqrt();
    let reward_score = f64::min(50.0, reward_rate * 12.0);
    let spread_score = f64::min(25.0, decimal_to_f64(max_spread_cents) * 4.0);
    let midpoint_score = f64::max(0.0, 15.0 - f64::abs(decimal_to_f64(midpoint) - 0.5) * 30.0);
    let notional = legs
        .iter()
        .fold(Decimal::ZERO, |sum, leg| sum + leg.notional_usd);
    let size_score = if notional > Decimal::ZERO { 10.0 } else { 0.0 };

    decimal_from_f64(reward_score + spread_score + midpoint_score + size_score).round_dp(2)
}
