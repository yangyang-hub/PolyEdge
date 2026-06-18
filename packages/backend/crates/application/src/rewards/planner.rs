pub fn select_reward_book_token_ids(markets: &[RewardMarket]) -> Vec<String> {
    let mut selected = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for market in markets {
        let Some((yes_token, no_token)) = reward_quote_tokens(market) else {
            continue;
        };
        for token in [yes_token, no_token] {
            if token.token_id.trim().is_empty() || !seen.insert(token.token_id.clone()) {
                continue;
            }
            selected.push(token.token_id.clone());
        }
    }

    selected
}

pub fn select_reward_quote_candidate_markets(
    markets: &[RewardMarket],
    config: &RewardBotConfig,
) -> Vec<RewardMarket> {
    if config.max_markets == 0
        || config.max_open_orders == 0
        || config.quote_size_usd <= Decimal::ZERO
    {
        return Vec::new();
    }

    markets
        .iter()
        .filter(|market| reward_market_prefilter_reason(market, config).is_none())
        .cloned()
        .collect()
}

fn reward_market_prefilter_reason(
    market: &RewardMarket,
    config: &RewardBotConfig,
) -> Option<&'static str> {
    let now = OffsetDateTime::now_utc();
    if !market.active {
        return Some("reward market is inactive");
    }
    if reward_quote_tokens(market).is_none() {
        return Some("missing quoteable reward tokens");
    }
    if market.total_daily_rate < config.min_daily_reward {
        return Some("daily reward is below threshold");
    }
    if market.liquidity_usd < config.min_market_liquidity_usd {
        return Some("market liquidity is below threshold");
    }
    if market.volume_24h_usd < config.min_market_volume_24h_usd {
        return Some("market 24h volume is below threshold");
    }
    if market.market_spread_cents > config.max_market_spread_cents {
        return Some("market top-of-book spread is too wide");
    }
    if market.ambiguity_level == "high" {
        return Some("market resolution metadata is high risk");
    }
    if reward_market_event_risk_reason(market).is_some() {
        return Some("market has high event/tail-risk characteristics");
    }
    if market
        .end_at
        .is_none_or(|end_at| end_at < now + TimeDuration::hours(config.min_hours_to_end as i64))
    {
        return Some("market settlement is unknown or too close");
    }
    if market.market_synced_at.is_none_or(|synced_at| {
        synced_at < now - TimeDuration::minutes(config.max_market_data_age_minutes as i64)
            || synced_at > now + TimeDuration::minutes(5)
    }) {
        return Some("market metadata is stale");
    }
    if normalize_reward_spread_cents(market.rewards_max_spread) <= Decimal::ZERO {
        return Some("invalid rewards spread setting");
    }
    None
}

fn reward_market_event_risk_reason(market: &RewardMarket) -> Option<&'static str> {
    let text = format!(
        "{} {} {} {}",
        market.question, market.market_slug, market.event_slug, market.category
    )
    .to_ascii_lowercase();
    let has_any = |needles: &[&str]| needles.iter().any(|needle| text.contains(needle));

    if has_any(&["one day after launch", "fdv above", "fully diluted valuation"]) {
        return Some("launch valuation market has high jump risk");
    }
    if has_any(&["launch a token", "token launch", "launch token", "airdrop"]) {
        return Some("token launch market has high announcement risk");
    }
    if has_any(&["official result", "announced by", "will be listed", "listing by"]) {
        return Some("official-result market has high announcement risk");
    }
    None
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
    let Some((yes_token, no_token)) = reward_quote_tokens(market) else {
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
    let metrics = build_market_book_metrics(yes_token, no_token, books, midpoint, config);
    let quote_mode = selected_reward_quote_mode_for_planning(config, midpoint);

    if quote_mode == RewardPlanQuoteMode::None {
        return empty_plan_with_metrics(
            market,
            "dominant probability is beyond configured single-side cap",
            now,
            Some(midpoint),
            metrics,
        );
    }

    let midpoint_is_in_double_range =
        midpoint >= config.min_midpoint && midpoint <= config.max_midpoint;
    if !midpoint_is_in_double_range
        && !matches!(
            quote_mode,
            RewardPlanQuoteMode::SingleYes | RewardPlanQuoteMode::SingleNo
        )
    {
        return empty_plan_with_metrics(
            market,
            "midpoint is too close to 0/1 for the first rewards strategy",
            now,
            Some(midpoint),
            metrics,
        );
    }

    if market.total_daily_rate < config.min_daily_reward {
        return empty_plan_with_metrics(
            market,
            "daily reward is below threshold",
            now,
            Some(midpoint),
            metrics,
        );
    }

    let max_spread_cents = Decimal::min(
        normalize_reward_spread_cents(market.rewards_max_spread),
        config.max_spread_cents,
    );
    if max_spread_cents <= Decimal::ZERO {
        return empty_plan_with_metrics(
            market,
            "invalid rewards spread setting",
            now,
            Some(midpoint),
            metrics,
        );
    }

    if config.quote_size_usd <= Decimal::ZERO {
        return empty_plan_with_metrics(
            market,
            "quote size is zero",
            now,
            Some(midpoint),
            metrics,
        );
    }

    let legs = placeholder_quote_legs(yes_token, no_token, quote_mode);

    build_ready_quote_plan(
        market,
        quote_mode,
        metrics,
        midpoint,
        max_spread_cents,
        legs,
        config,
        now,
    )
}

fn build_ready_quote_plan(
    market: &RewardMarket,
    quote_mode: RewardPlanQuoteMode,
    metrics: Option<RewardMarketBookMetrics>,
    midpoint: Decimal,
    max_spread_cents: Decimal,
    legs: Vec<RewardQuoteLeg>,
    config: &RewardBotConfig,
    now: OffsetDateTime,
) -> RewardQuotePlan {
    let score = score_market(market, max_spread_cents, midpoint, &legs, config);
    let eligible = score >= config.min_market_score;

    RewardQuotePlan {
        condition_id: market.condition_id.clone(),
        market_slug: market.market_slug.clone(),
        question: market.question.clone(),
        score,
        eligible,
        reason: if eligible {
            format!(
                "eligible pending live orderbook validation for {} quotes",
                quote_mode.as_str()
            )
        } else {
            "score is below threshold".to_string()
        },
        quote_mode,
        recommended_quote_mode: metrics
            .as_ref()
            .map(|metrics| metrics.recommended_quote_mode),
        book_metrics: metrics,
        ai_advisory: None,
        info_risk: None,
        midpoint: Some(midpoint),
        live_skip_until: None,
        live_skip_reason: None,
        total_daily_rate: market.total_daily_rate,
        rewards_max_spread: market.rewards_max_spread,
        rewards_min_size: market.rewards_min_size,
        legs,
        updated_at: now,
    }
}

fn reward_quote_tokens(market: &RewardMarket) -> Option<(&RewardToken, &RewardToken)> {
    let mut tokens = market
        .tokens
        .iter()
        .filter(|token| !token.token_id.trim().is_empty());
    let (Some(first), Some(second)) = (tokens.next(), tokens.next()) else {
        return None;
    };
    if tokens.next().is_some() || first.token_id == second.token_id {
        return None;
    }
    if first.outcome.trim().eq_ignore_ascii_case("yes")
        && second.outcome.trim().eq_ignore_ascii_case("no")
    {
        Some((first, second))
    } else if first.outcome.trim().eq_ignore_ascii_case("no")
        && second.outcome.trim().eq_ignore_ascii_case("yes")
    {
        Some((second, first))
    } else {
        None
    }
}

fn placeholder_quote_legs(
    yes_token: &RewardToken,
    no_token: &RewardToken,
    quote_mode: RewardPlanQuoteMode,
) -> Vec<RewardQuoteLeg> {
    match quote_mode {
        RewardPlanQuoteMode::Double
        | RewardPlanQuoteMode::SingleYes
        | RewardPlanQuoteMode::SingleNo => vec![
            placeholder_quote_leg(yes_token),
            placeholder_quote_leg(no_token),
        ],
        RewardPlanQuoteMode::None => Vec::new(),
    }
}

fn placeholder_quote_leg(token: &RewardToken) -> RewardQuoteLeg {
    RewardQuoteLeg {
        token_id: token.token_id.clone(),
        outcome: if token.outcome.trim().is_empty() {
            token.token_id.clone()
        } else {
            token.outcome.clone()
        },
        side: RewardOrderSide::Buy,
        price: Decimal::ZERO,
        size: Decimal::ZERO,
        notional_usd: Decimal::ZERO,
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
        quote_mode: RewardPlanQuoteMode::None,
        recommended_quote_mode: None,
        book_metrics: None,
        ai_advisory: None,
        info_risk: None,
        midpoint,
        live_skip_until: None,
        live_skip_reason: None,
        total_daily_rate: market.total_daily_rate,
        rewards_max_spread: market.rewards_max_spread,
        rewards_min_size: market.rewards_min_size,
        legs: Vec::new(),
        updated_at: now,
    }
}

fn empty_plan_with_metrics(
    market: &RewardMarket,
    reason: impl Into<String>,
    now: OffsetDateTime,
    midpoint: Option<Decimal>,
    metrics: Option<RewardMarketBookMetrics>,
) -> RewardQuotePlan {
    let mut plan = empty_plan(market, reason, now, midpoint);
    plan.recommended_quote_mode = metrics.as_ref().map(|metrics| metrics.recommended_quote_mode);
    plan.book_metrics = metrics;
    plan
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
    let bid_prices = if fresh {
        book.map(distinct_bid_prices).unwrap_or_default()
    } else {
        Vec::new()
    };

    if let (Some(best_bid), Some(best_ask)) = (best_bid, best_ask)
        && best_bid > Decimal::ZERO
        && best_ask > Decimal::ZERO
    {
        return Some(TokenBookState {
            midpoint: (best_bid + best_ask) / decimal("2"),
            best_ask: Some(best_ask),
            bid_prices,
        });
    }

    if let Some(price) = token
        .price
        .filter(|price| *price > Decimal::ZERO && *price < Decimal::ONE)
    {
        return Some(TokenBookState {
            midpoint: price,
            best_ask: None,
            bid_prices,
        });
    }

    None
}

fn distinct_bid_prices(book: &RewardOrderBook) -> Vec<Decimal> {
    let mut prices = Vec::new();
    for level in &book.bids {
        if level.price <= Decimal::ZERO || prices.last() == Some(&level.price) {
            continue;
        }
        prices.push(level.price);
    }
    prices
}

fn quote_bid_price(state: &TokenBookState, rank: u16) -> Option<Decimal> {
    state
        .bid_prices
        .get(usize::from(rank.saturating_sub(1)))
        .copied()
        .map(|price| floor_to_tick(price, DEFAULT_TICK))
}

fn make_quote_legs(
    yes_token: &RewardToken,
    yes_price: Decimal,
    no_token: &RewardToken,
    no_price: Decimal,
    rewards_min_size: Decimal,
    config: &RewardBotConfig,
) -> Option<Vec<RewardQuoteLeg>> {
    let effective_quote_size = if config.account_capital_usd > Decimal::ZERO {
        Decimal::min(config.quote_size_usd, config.account_capital_usd)
    } else {
        config.quote_size_usd
    };

    let prices = [yes_price, no_price];
    let minimum_sizes =
        prices.map(|price| ceil_reward_size_for_cost_precision(price, rewards_min_size));
    let minimum_notionals = [
        prices[0] * minimum_sizes[0],
        prices[1] * minimum_sizes[1],
    ];
    let target_notionals =
        minimum_notionals.map(|minimum| Decimal::max(minimum, effective_quote_size));
    let allocated_notionals = if config.per_market_usd <= Decimal::ZERO {
        target_notionals
    } else {
        allocate_quote_notionals(
            minimum_notionals,
            target_notionals,
            config.per_market_usd,
        )?
    };

    let legs = [
        (yes_token, yes_price, allocated_notionals[0]),
        (no_token, no_price, allocated_notionals[1]),
    ]
    .into_iter()
    .map(|(token, price, notional)| make_leg(token, price, notional))
    .collect::<Vec<_>>();

    if rewards_min_size > Decimal::ZERO
        && legs.iter().any(|leg| leg.size < rewards_min_size)
    {
        return None;
    }

    let total_notional = legs
        .iter()
        .fold(Decimal::ZERO, |sum, leg| sum + leg.price * leg.size);
    if config.per_market_usd > Decimal::ZERO && total_notional > config.per_market_usd {
        return None;
    }

    Some(legs)
}

fn make_single_side_budget_fallback_legs(
    yes_token: &RewardToken,
    yes_price: Decimal,
    no_token: &RewardToken,
    no_price: Decimal,
    rewards_min_size: Decimal,
    config: &RewardBotConfig,
) -> Option<(RewardPlanQuoteMode, Vec<RewardQuoteLeg>)> {
    if config.quote_mode != RewardQuoteMode::Auto
        || config.selection_mode != RewardSelectionMode::Enforce
        || !config.dominant_single_side_enabled
    {
        return None;
    }

    let yes = make_single_quote_leg(yes_token, yes_price, rewards_min_size, config)
        .map(|leg| (RewardPlanQuoteMode::SingleYes, leg));
    let no = make_single_quote_leg(no_token, no_price, rewards_min_size, config)
        .map(|leg| (RewardPlanQuoteMode::SingleNo, leg));

    let selected = match (yes, no) {
        (Some(yes), Some(no)) => {
            let yes_notional = yes.1.price * yes.1.size;
            let no_notional = no.1.price * no.1.size;
            if yes_notional <= no_notional { yes } else { no }
        }
        (Some(yes), None) => yes,
        (None, Some(no)) => no,
        (None, None) => return None,
    };

    Some((selected.0, vec![selected.1]))
}

fn allocate_quote_notionals(
    minimum_notionals: [Decimal; 2],
    target_notionals: [Decimal; 2],
    per_market_usd: Decimal,
) -> Option<[Decimal; 2]> {
    let minimum_total = minimum_notionals[0] + minimum_notionals[1];
    if minimum_total > per_market_usd {
        return None;
    }

    let target_total = target_notionals[0] + target_notionals[1];
    if target_total <= per_market_usd {
        return Some(target_notionals);
    }

    let extra_budget = per_market_usd - minimum_total;
    let gaps = [
        target_notionals[0] - minimum_notionals[0],
        target_notionals[1] - minimum_notionals[1],
    ];
    let total_gap = gaps[0] + gaps[1];
    if total_gap <= Decimal::ZERO {
        return Some(minimum_notionals);
    }

    Some([
        minimum_notionals[0] + extra_budget * gaps[0] / total_gap,
        minimum_notionals[1] + extra_budget * gaps[1] / total_gap,
    ])
}

fn make_leg(token: &RewardToken, price: Decimal, notional_usd: Decimal) -> RewardQuoteLeg {
    let size = floor_reward_size_for_cost_precision(
        price,
        (notional_usd / price).round_dp_with_strategy(2, RoundingStrategy::ToZero),
    );
    RewardQuoteLeg {
        token_id: token.token_id.clone(),
        outcome: if token.outcome.trim().is_empty() {
            token.token_id.clone()
        } else {
            token.outcome.clone()
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
    config: &RewardBotConfig,
) -> Decimal {
    let reward_rate = decimal_to_f64(market.total_daily_rate).sqrt();
    let reward_score = f64::min(35.0, reward_rate * 10.0);
    let liquidity_score = f64::min(
        20.0,
        decimal_to_f64(market.liquidity_usd).ln_1p() / 10_f64.ln() * 4.0,
    );
    let volume_score = f64::min(
        15.0,
        decimal_to_f64(market.volume_24h_usd).ln_1p() / 10_f64.ln() * 3.0,
    );
    let hours_to_end = market
        .end_at
        .map(|end_at| (end_at - OffsetDateTime::now_utc()).whole_hours().max(0) as f64)
        .unwrap_or_default();
    let duration_score = f64::min(10.0, (hours_to_end / 24.0).sqrt() * 2.0);
    let spread_score = f64::min(10.0, decimal_to_f64(max_spread_cents) * 1.25);
    let midpoint_score = f64::max(0.0, 5.0 - f64::abs(decimal_to_f64(midpoint) - 0.5) * 10.0);
    let notional = legs
        .iter()
        .fold(Decimal::ZERO, |sum, leg| sum + leg.notional_usd);
    let size_score = if notional > Decimal::ZERO { 5.0 } else { 0.0 };

    let base_score = decimal_from_f64(
        reward_score
            + liquidity_score
            + volume_score
            + duration_score
            + spread_score
            + midpoint_score
            + size_score,
    )
    .round_dp(2);
    (base_score + preferred_category_bonus(market, config)).round_dp(2)
}

#[cfg(test)]
mod planner_tests {
    include!("planner_tests.rs");
}
