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

#[cfg(test)]
pub fn select_reward_quote_candidate_markets(
    markets: &[RewardMarket],
    config: &RewardBotConfig,
) -> Vec<RewardMarket> {
    select_reward_quote_candidate_market_profiles(
        markets,
        config,
        RewardStrategyBucket::Standard,
    )
    .into_iter()
    .map(|candidate| candidate.market)
    .collect()
}

pub fn select_reward_quote_candidate_market_profiles(
    markets: &[RewardMarket],
    config: &RewardBotConfig,
    strategy_bucket: RewardStrategyBucket,
) -> Vec<RewardCandidateMarket> {
    let effective_config = config.config_for_strategy_bucket(strategy_bucket);
    if reward_candidate_profile_is_disabled(config, &effective_config, strategy_bucket) {
        return Vec::new();
    }

    markets
        .iter()
        .filter(|market| reward_market_prefilter_reason(market, &effective_config).is_none())
        .map(|market| RewardCandidateMarket {
            market: market.clone(),
            strategy_bucket,
        })
        .collect()
}

fn reward_candidate_profile_is_disabled(
    base_config: &RewardBotConfig,
    effective_config: &RewardBotConfig,
    strategy_bucket: RewardStrategyBucket,
) -> bool {
    if effective_config.max_markets == 0 {
        return true;
    }
    match strategy_bucket {
        RewardStrategyBucket::Standard => effective_config.max_open_orders == 0,
        RewardStrategyBucket::LowCompetition => {
            base_config.low_competition_mode == RewardLowCompetitionMode::Enforce
                && effective_config.max_open_orders == 0
        }
        RewardStrategyBucket::None => true,
    }
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
    if has_any(&[
        "official result",
        "announced by",
        "will be announced",
        "will be listed",
        "listing by",
        "appointed by",
        "confirmed by",
        "certified by",
    ]) {
        return Some("official-result market has high announcement risk");
    }
    if has_any(&[
        "drop out before",
        "dropout before",
        "withdraw before",
        "suspend campaign",
        "resign before",
        "step down before",
        "removed before",
    ]) {
        return Some("personnel-change market has high event risk");
    }
    if has_any(&[
        "scheduled event",
        "before the deadline",
        "by the deadline",
        "before market close",
        "by market close",
    ]) {
        return Some("scheduled-event market has high jump risk");
    }
    None
}

pub fn apply_first_quote_entry_gates(
    plans: &mut [RewardQuotePlan],
    previous_plans: &[RewardQuotePlan],
    open_orders: &[ManagedRewardOrder],
    positions: &[RewardPosition],
    config: &RewardBotConfig,
    now: OffsetDateTime,
) -> bool {
    if !first_quote_entry_gate_enabled(config) || plans.is_empty() {
        return false;
    }

    let previous_by_condition = previous_plans
        .iter()
        .map(|plan| (plan.condition_id.as_str(), plan))
        .collect::<HashMap<_, _>>();
    let active_conditions = first_quote_active_conditions(open_orders, positions);
    let mut changed = false;

    for plan in plans.iter_mut().filter(|plan| plan.eligible) {
        if active_conditions.contains(plan.condition_id.as_str()) {
            continue;
        }
        if config.require_info_risk_before_first_quote && plan.info_risk.is_none() {
            changed |= block_first_quote_plan(
                plan,
                "info risk pending: first quote requires provider risk filter",
                previous_by_condition
                    .get(plan.condition_id.as_str())
                    .map(|previous| previous.updated_at)
                    .unwrap_or(now),
            );
            continue;
        }

        if config.first_quote_quarantine_sec == 0 {
            continue;
        }
        let first_seen_at = previous_by_condition
            .get(plan.condition_id.as_str())
            .map(|previous| previous.updated_at)
            .unwrap_or(now);
        let ready_at = first_seen_at + TimeDuration::seconds(config.first_quote_quarantine_sec as i64);
        if now < ready_at {
            let observed_sec = (now - first_seen_at).whole_seconds().max(0);
            changed |= block_first_quote_plan(
                plan,
                format!(
                    "first quote quarantine: market observed for {observed_sec}s; requires {}s before initial live quote",
                    config.first_quote_quarantine_sec
                ),
                first_seen_at,
            );
        }
    }

    changed
}

fn first_quote_entry_gate_enabled(config: &RewardBotConfig) -> bool {
    config.info_risk_enabled
        && config.info_risk_mode == RewardSelectionMode::Enforce
        && (config.require_info_risk_before_first_quote || config.first_quote_quarantine_sec > 0)
}

fn first_quote_active_conditions<'a>(
    open_orders: &'a [ManagedRewardOrder],
    positions: &'a [RewardPosition],
) -> HashSet<&'a str> {
    let mut active = HashSet::new();
    for order in open_orders.iter().filter(|order| order.status.is_open_like()) {
        active.insert(order.condition_id.as_str());
    }
    for position in positions.iter().filter(|position| position.size > Decimal::ZERO) {
        active.insert(position.condition_id.as_str());
    }
    active
}

fn block_first_quote_plan(
    plan: &mut RewardQuotePlan,
    reason: impl Into<String>,
    first_seen_at: OffsetDateTime,
) -> bool {
    let reason = reason.into();
    let changed =
        plan.eligible || plan.quote_mode != RewardPlanQuoteMode::None || plan.reason != reason;
    plan.eligible = false;
    plan.quote_mode = RewardPlanQuoteMode::None;
    plan.legs.clear();
    plan.reason = reason;
    plan.updated_at = first_seen_at;
    changed
}

pub fn build_reward_quote_plans(
    markets: &[RewardMarket],
    books: &HashMap<String, RewardOrderBook>,
    config: &RewardBotConfig,
) -> Vec<RewardQuotePlan> {
    let candidates = markets
        .iter()
        .cloned()
        .map(|market| RewardCandidateMarket {
            market,
            strategy_bucket: RewardStrategyBucket::Standard,
        })
        .collect::<Vec<_>>();
    build_reward_quote_plans_for_candidates(&candidates, books, config)
}

pub fn build_reward_quote_plans_for_candidates(
    candidates: &[RewardCandidateMarket],
    books: &HashMap<String, RewardOrderBook>,
    config: &RewardBotConfig,
) -> Vec<RewardQuotePlan> {
    let mut plans = candidates
        .iter()
        .map(|candidate| {
            let effective_config = config.config_for_strategy_bucket(candidate.strategy_bucket);
            build_reward_quote_plan_for_bucket(
                &candidate.market,
                books,
                &effective_config,
                candidate.strategy_bucket,
            )
        })
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

#[cfg(test)]
fn build_reward_quote_plan(
    market: &RewardMarket,
    books: &HashMap<String, RewardOrderBook>,
    config: &RewardBotConfig,
) -> RewardQuotePlan {
    build_reward_quote_plan_for_bucket(market, books, config, RewardStrategyBucket::Standard)
}

fn build_reward_quote_plan_for_bucket(
    market: &RewardMarket,
    books: &HashMap<String, RewardOrderBook>,
    config: &RewardBotConfig,
    strategy_bucket: RewardStrategyBucket,
) -> RewardQuotePlan {
    let now = OffsetDateTime::now_utc();
    let Some((yes_token, no_token)) = reward_quote_tokens(market) else {
        return empty_plan(market, "missing YES/NO token", now, None, strategy_bucket);
    };

    let yes_state = get_token_book_state(yes_token, books, config, now);
    let no_state = get_token_book_state(no_token, books, config, now);
    let midpoint = yes_state
        .as_ref()
        .map(|state| state.midpoint)
        .or_else(|| no_state.as_ref().map(|state| Decimal::ONE - state.midpoint));
    let Some(midpoint) = midpoint else {
        return empty_plan(
            market,
            "missing book or fallback token price",
            now,
            None,
            strategy_bucket,
        );
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
            strategy_bucket,
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
            strategy_bucket,
        );
    }

    if market.total_daily_rate < config.min_daily_reward {
        return empty_plan_with_metrics(
            market,
            "daily reward is below threshold",
            now,
            Some(midpoint),
            metrics,
            strategy_bucket,
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
            strategy_bucket,
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
        strategy_bucket,
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
    strategy_bucket: RewardStrategyBucket,
) -> RewardQuotePlan {
    let score = score_market(market, max_spread_cents, midpoint, &legs, config);
    let eligible = score >= config.min_market_score;
    let orderbook_token_ids = quote_leg_token_ids(&legs);

    RewardQuotePlan {
        condition_id: market.condition_id.clone(),
        market_slug: market.market_slug.clone(),
        question: market.question.clone(),
        score,
        eligible,
        pre_ai_eligible: eligible,
        quote_readiness: RewardQuoteReadiness::Blocked,
        reason: if eligible {
            format!(
                "eligible pending live orderbook validation for {} quotes",
                quote_mode.as_str()
            )
        } else {
            "score is below threshold".to_string()
        },
        strategy_bucket,
        quote_mode,
        recommended_quote_mode: metrics
            .as_ref()
            .map(|metrics| metrics.recommended_quote_mode),
        book_metrics: metrics,
        low_competition_metrics: None,
        ai_advisory: None,
        info_risk: None,
        midpoint: Some(midpoint),
        live_skip_until: None,
        live_skip_reason: None,
        total_daily_rate: market.total_daily_rate,
        rewards_max_spread: market.rewards_max_spread,
        rewards_min_size: market.rewards_min_size,
        orderbook_token_ids,
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

fn quote_leg_token_ids(legs: &[RewardQuoteLeg]) -> Vec<String> {
    let mut token_ids = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for leg in legs {
        if leg.token_id.trim().is_empty() || !seen.insert(leg.token_id.clone()) {
            continue;
        }
        token_ids.push(leg.token_id.clone());
    }
    token_ids
}

fn empty_plan(
    market: &RewardMarket,
    reason: impl Into<String>,
    now: OffsetDateTime,
    midpoint: Option<Decimal>,
    strategy_bucket: RewardStrategyBucket,
) -> RewardQuotePlan {
    RewardQuotePlan {
        condition_id: market.condition_id.clone(),
        market_slug: market.market_slug.clone(),
        question: market.question.clone(),
        score: Decimal::ZERO,
        eligible: false,
        pre_ai_eligible: false,
        quote_readiness: RewardQuoteReadiness::Blocked,
        reason: reason.into(),
        strategy_bucket,
        quote_mode: RewardPlanQuoteMode::None,
        recommended_quote_mode: None,
        book_metrics: None,
        low_competition_metrics: None,
        ai_advisory: None,
        info_risk: None,
        midpoint,
        live_skip_until: None,
        live_skip_reason: None,
        total_daily_rate: market.total_daily_rate,
        rewards_max_spread: market.rewards_max_spread,
        rewards_min_size: market.rewards_min_size,
        orderbook_token_ids: Vec::new(),
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
    strategy_bucket: RewardStrategyBucket,
) -> RewardQuotePlan {
    let mut plan = empty_plan(market, reason, now, midpoint, strategy_bucket);
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
                (now - book.confirmed_at)
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
    let price = if bid_prices_use_fine_tick(&state.bid_prices) {
        quote_fine_tick_bid_price(&state.bid_prices, rank)?
    } else {
        state
            .bid_prices
            .get(usize::from(rank.saturating_sub(1)))
            .copied()?
    };
    Some(floor_to_tick(price, inferred_bid_price_tick(&state.bid_prices)))
}

fn quote_fine_tick_bid_price(bid_prices: &[Decimal], rank: u16) -> Option<Decimal> {
    let best_bid = bid_prices.first().copied()?;
    let target = best_bid - DEFAULT_TICK * Decimal::from(rank.saturating_sub(1));
    bid_prices
        .iter()
        .copied()
        .find(|price| *price <= target)
}

fn bid_prices_use_fine_tick(bid_prices: &[Decimal]) -> bool {
    inferred_bid_price_tick(bid_prices) < DEFAULT_TICK
}

fn inferred_bid_price_tick(bid_prices: &[Decimal]) -> Decimal {
    bid_prices
        .windows(2)
        .filter_map(|window| {
            let diff = (window[0] - window[1]).abs();
            (diff > Decimal::ZERO).then_some(diff)
        })
        .min()
        .unwrap_or(DEFAULT_TICK)
        .min(DEFAULT_TICK)
}

fn make_quote_legs(
    yes_token: &RewardToken,
    yes_price: Decimal,
    no_token: &RewardToken,
    no_price: Decimal,
    rewards_min_size: Decimal,
) -> Option<Vec<RewardQuoteLeg>> {
    let prices = [yes_price, no_price];
    let minimum_sizes = prices.map(|price| minimum_live_quote_size(price, rewards_min_size));
    let minimum_notionals = [
        prices[0] * minimum_sizes[0],
        prices[1] * minimum_sizes[1],
    ];

    let legs = [
        (yes_token, yes_price, minimum_notionals[0]),
        (no_token, no_price, minimum_notionals[1]),
    ]
    .into_iter()
    .map(|(token, price, notional)| make_leg(token, price, notional))
    .collect::<Vec<_>>();

    if rewards_min_size > Decimal::ZERO
        && legs.iter().any(|leg| leg.size < rewards_min_size)
    {
        return None;
    }

    Some(legs)
}

fn minimum_live_quote_size(price: Decimal, rewards_min_size: Decimal) -> Decimal {
    if price <= Decimal::ZERO {
        return Decimal::ZERO;
    }
    let reward_size = ceil_reward_size_for_cost_precision(price, rewards_min_size);
    let venue_min_size =
        ceil_reward_size_for_cost_precision(price, MIN_POLYMARKET_ORDER_NOTIONAL_USD / price);
    Decimal::max(reward_size, venue_min_size)
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

pub fn scale_single_leg_for_budget(
    token: &RewardToken,
    price: Decimal,
    rewards_min_size: Decimal,
    available_usd: Decimal,
) -> RewardQuoteLeg {
    let minimum_size = minimum_live_quote_size(price, rewards_min_size);
    let budget_size = if price > Decimal::ZERO {
        floor_reward_size_for_cost_precision(
            price,
            (available_usd / price).round_dp_with_strategy(2, RoundingStrategy::ToZero),
        )
    } else {
        Decimal::ZERO
    };
    let size = Decimal::max(minimum_size, budget_size);
    make_leg(token, price, price * size)
}

pub fn scale_double_legs_for_budget(
    yes_token: &RewardToken,
    yes_price: Decimal,
    no_token: &RewardToken,
    no_price: Decimal,
    rewards_min_size: Decimal,
    available_usd: Decimal,
) -> Vec<RewardQuoteLeg> {
    let per_leg = (available_usd / decimal("2")).round_dp(4);
    let yes_minimum_size = minimum_live_quote_size(yes_price, rewards_min_size);
    let yes_budget_size = if yes_price > Decimal::ZERO {
        floor_reward_size_for_cost_precision(
            yes_price,
            (per_leg / yes_price).round_dp_with_strategy(2, RoundingStrategy::ToZero),
        )
    } else {
        Decimal::ZERO
    };
    let yes_size = Decimal::max(yes_minimum_size, yes_budget_size);

    let no_minimum_size = minimum_live_quote_size(no_price, rewards_min_size);
    let no_budget_size = if no_price > Decimal::ZERO {
        floor_reward_size_for_cost_precision(
            no_price,
            (per_leg / no_price).round_dp_with_strategy(2, RoundingStrategy::ToZero),
        )
    } else {
        Decimal::ZERO
    };
    let no_size = Decimal::max(no_minimum_size, no_budget_size);

    vec![
        make_leg(yes_token, yes_price, yes_price * yes_size),
        make_leg(no_token, no_price, no_price * no_size),
    ]
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
