fn reward_ai_pricing_context(
    market: &RewardMarket,
    plan: &RewardQuotePlan,
    books: &HashMap<String, RewardOrderBook>,
    config: &RewardBotConfig,
    now: OffsetDateTime,
) -> Value {
    let mut token_items = Vec::new();
    let mut midpoint_sum = Decimal::ZERO;
    let mut midpoint_count = 0usize;
    let mut max_spread_cents: Option<Decimal> = None;
    let mut quote_crosses_or_touches_ask = false;
    let mut quote_negative_edge_count = 0usize;
    let mut stale_token_count = 0usize;
    let max_stale_book_ms = i64::try_from(config.stale_book_ms).unwrap_or(i64::MAX);

    for token in &market.tokens {
        let book = books.get(&token.token_id);
        let best_bid = book.and_then(|book| reward_ai_best_price(&book.bids));
        let best_ask = book.and_then(|book| reward_ai_best_price(&book.asks));
        let midpoint = match (best_bid, best_ask, token.price) {
            (Some(bid), Some(ask), _) => Some((bid + ask) / Decimal::from(2)),
            (_, _, Some(price)) if price > Decimal::ZERO => Some(price),
            _ => None,
        };
        if let Some(midpoint) = midpoint {
            midpoint_sum += midpoint;
            midpoint_count += 1;
        }

        let spread_cents = best_bid
            .zip(best_ask)
            .map(|(bid, ask)| (ask - bid) * decimal("100"));
        if let Some(spread) = spread_cents {
            max_spread_cents = Some(max_spread_cents.map_or(spread, |current| current.max(spread)));
        }

        let quote_leg = plan.legs.iter().find(|leg| leg.token_id == token.token_id);
        let quote_price = quote_leg.map(|leg| leg.price);
        let quote_edge_cents = quote_price
            .zip(midpoint)
            .map(|(price, midpoint)| (midpoint - price) * decimal("100"));
        if best_ask
            .zip(quote_price)
            .is_some_and(|(ask, price)| price >= ask)
        {
            quote_crosses_or_touches_ask = true;
        }
        if quote_edge_cents.is_some_and(|edge| edge < Decimal::ZERO) {
            quote_negative_edge_count += 1;
        }

        let confirmed_age_ms = book
            .map(|book| {
                let age_ms = (now - book.confirmed_at).whole_milliseconds().max(0);
                i64::try_from(age_ms).unwrap_or(i64::MAX)
            })
            .unwrap_or(i64::MAX);
        let stale_for_placement = confirmed_age_ms > max_stale_book_ms;
        if stale_for_placement {
            stale_token_count += 1;
        }

        token_items.push(json!({
            "token_id": token.token_id,
            "outcome": token.outcome,
            "catalog_price": token.price,
            "best_bid": best_bid,
            "best_ask": best_ask,
            "midpoint": midpoint,
            "spread_cents": spread_cents,
            "quote_side": quote_leg.map(|leg| leg.side),
            "quote_price": quote_price,
            "quote_size": quote_leg.map(|leg| leg.size),
            "quote_notional_usd": quote_leg.map(|leg| leg.notional_usd),
            "quote_edge_to_midpoint_cents": quote_edge_cents,
            "quote_crosses_or_touches_best_ask": best_ask
                .zip(quote_price)
                .is_some_and(|(ask, price)| price >= ask),
            "confirmed_age_ms": confirmed_age_ms,
            "stale_for_placement": stale_for_placement,
        }));
    }

    let binary_midpoint_sum_deviation_cents = if midpoint_count >= 2 {
        Some(reward_ai_abs_decimal(midpoint_sum - Decimal::ONE) * decimal("100"))
    } else {
        None
    };
    let binary_midpoint_sum_reasonable =
        binary_midpoint_sum_deviation_cents.is_none_or(|deviation| deviation <= decimal("3"));
    let current_max_spread_cents = max_spread_cents.unwrap_or(Decimal::ZERO);

    json!({
        "schema_version": 1,
        "pricing_time_utc": now,
        "max_stale_book_ms": config.stale_book_ms,
        "strategy_max_spread_cents": config.max_spread_cents,
        "market_rewards_max_spread_cents": market.rewards_max_spread,
        "deterministic_quote_mode": plan.quote_mode,
        "recommended_quote_mode": plan.recommended_quote_mode,
        "binary_midpoint_sum": if midpoint_count >= 2 { Some(midpoint_sum) } else { None },
        "binary_midpoint_sum_deviation_cents": binary_midpoint_sum_deviation_cents,
        "binary_midpoint_sum_reasonable": binary_midpoint_sum_reasonable,
        "current_max_spread_cents": current_max_spread_cents,
        "quote_crosses_or_touches_ask": quote_crosses_or_touches_ask,
        "quote_negative_edge_count": quote_negative_edge_count,
        "stale_token_count": stale_token_count,
        "all_quote_prices_resting_and_reasonable": !quote_crosses_or_touches_ask
            && quote_negative_edge_count == 0
            && stale_token_count == 0
            && binary_midpoint_sum_reasonable
            && current_max_spread_cents <= config.max_spread_cents,
        "tokens": token_items,
    })
}

fn reward_ai_best_price(levels: &[RewardBookLevel]) -> Option<Decimal> {
    levels
        .iter()
        .find(|level| level.price > Decimal::ZERO && level.size > Decimal::ZERO)
        .map(|level| level.price)
}

fn reward_ai_abs_decimal(value: Decimal) -> Decimal {
    if value < Decimal::ZERO {
        -value
    } else {
        value
    }
}

#[derive(Debug, Clone)]
struct RewardAiCandleBucket {
    token_id: String,
    condition_id: String,
    outcome: String,
    bucket_start: OffsetDateTime,
    open: Decimal,
    high: Decimal,
    low: Decimal,
    close: Decimal,
    best_bid_close: Decimal,
    best_ask_close: Decimal,
    spread_cents_close: Decimal,
    sample_count: i32,
    close_observed_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

impl RewardAiCandleBucket {
    fn new(candle: &RewardMarketCandle, bucket_start: OffsetDateTime) -> Self {
        Self {
            token_id: candle.token_id.clone(),
            condition_id: candle.condition_id.clone(),
            outcome: candle.outcome.clone(),
            bucket_start,
            open: candle.open,
            high: candle.high,
            low: candle.low,
            close: candle.close,
            best_bid_close: candle.best_bid_close,
            best_ask_close: candle.best_ask_close,
            spread_cents_close: candle.spread_cents_close,
            sample_count: candle.sample_count.max(0),
            close_observed_at: candle.close_observed_at,
            updated_at: candle.updated_at,
        }
    }

    fn push(&mut self, candle: &RewardMarketCandle) {
        self.high = self.high.max(candle.high);
        self.low = self.low.min(candle.low);
        self.close = candle.close;
        self.best_bid_close = candle.best_bid_close;
        self.best_ask_close = candle.best_ask_close;
        self.spread_cents_close = candle.spread_cents_close;
        self.sample_count = self.sample_count.saturating_add(candle.sample_count.max(0));
        self.close_observed_at = self.close_observed_at.max(candle.close_observed_at);
        self.updated_at = self.updated_at.max(candle.updated_at);
    }

    fn into_candle(self) -> RewardMarketCandle {
        RewardMarketCandle {
            token_id: self.token_id,
            condition_id: self.condition_id,
            outcome: self.outcome,
            interval_sec: REWARD_AI_CANDLE_INTERVAL_SEC,
            bucket_start: self.bucket_start,
            open: self.open,
            high: self.high,
            low: self.low,
            close: self.close,
            best_bid_close: self.best_bid_close,
            best_ask_close: self.best_ask_close,
            spread_cents_close: self.spread_cents_close,
            sample_count: self.sample_count,
            close_observed_at: self.close_observed_at,
            updated_at: self.updated_at,
        }
    }
}

fn reward_ai_coarse_candles(candles: &[RewardMarketCandle]) -> Result<Vec<RewardMarketCandle>> {
    let mut sorted = candles.to_vec();
    sorted.sort_by(|left, right| {
        left.token_id
            .cmp(&right.token_id)
            .then_with(|| left.bucket_start.cmp(&right.bucket_start))
            .then_with(|| left.close_observed_at.cmp(&right.close_observed_at))
    });

    let mut buckets = Vec::new();
    let mut current: Option<RewardAiCandleBucket> = None;
    for candle in &sorted {
        let bucket_start =
            reward_candle_bucket_start(candle.bucket_start, REWARD_AI_CANDLE_INTERVAL_SEC)?;
        if let Some(bucket) = current.as_mut() {
            if bucket.token_id == candle.token_id && bucket.bucket_start == bucket_start {
                bucket.push(candle);
                continue;
            }
        }
        if let Some(bucket) = current.take() {
            buckets.push(bucket.into_candle());
        }
        current = Some(RewardAiCandleBucket::new(candle, bucket_start));
    }
    if let Some(bucket) = current {
        buckets.push(bucket.into_candle());
    }

    Ok(reward_ai_limit_candles_per_token(buckets))
}

fn reward_ai_limit_candles_per_token(candles: Vec<RewardMarketCandle>) -> Vec<RewardMarketCandle> {
    let limit = usize::from(REWARD_AI_CANDLE_LIMIT_PER_TOKEN);
    if limit == 0 || candles.is_empty() {
        return Vec::new();
    }

    let mut limited = Vec::with_capacity(candles.len().min(limit));
    let mut index = 0usize;
    while index < candles.len() {
        let token_id = candles[index].token_id.as_str();
        let mut end = index + 1;
        while end < candles.len() && candles[end].token_id == token_id {
            end += 1;
        }
        let start = end.saturating_sub(limit).max(index);
        limited.extend(candles[start..end].iter().cloned());
        index = end;
    }
    limited
}

fn reward_ai_candle_payload(market: &RewardMarket, candles: &[RewardMarketCandle]) -> Value {
    let mut groups = Vec::new();
    for token in &market.tokens {
        let mut items = candles
            .iter()
            .filter(|candle| candle.token_id == token.token_id)
            .cloned()
            .collect::<Vec<_>>();
        items.sort_by_key(|candle| candle.bucket_start);
        groups.push(json!({
            "token_id": token.token_id,
            "outcome": token.outcome,
            "interval_sec": REWARD_AI_CANDLE_INTERVAL_SEC,
            "items": items.into_iter().map(|candle| {
                json!({
                    "bucket_start": candle.bucket_start,
                    "open": candle.open,
                    "high": candle.high,
                    "low": candle.low,
                    "close": candle.close,
                    "best_bid_close": candle.best_bid_close,
                    "best_ask_close": candle.best_ask_close,
                    "spread_cents_close": candle.spread_cents_close,
                    "sample_count": candle.sample_count,
                })
            }).collect::<Vec<_>>(),
        }));
    }
    Value::Array(groups)
}

fn reward_ai_candle_summary(market: &RewardMarket, candles: &[RewardMarketCandle]) -> Value {
    let mut token_summaries = Vec::new();
    let mut latest_bucket: Option<OffsetDateTime> = None;
    let mut total_samples = 0i64;
    let mut missing_tokens = 0usize;

    for token in &market.tokens {
        let mut items = candles
            .iter()
            .filter(|candle| candle.token_id == token.token_id)
            .collect::<Vec<_>>();
        items.sort_by_key(|candle| candle.bucket_start);
        let Some(first) = items.first().copied() else {
            missing_tokens += 1;
            token_summaries.push(json!({
                "token_id": token.token_id,
                "outcome": token.outcome,
                "sample_count": 0,
                "missing": true,
            }));
            continue;
        };
        let last = items[items.len() - 1];
        latest_bucket = Some(latest_bucket.map_or(last.bucket_start, |current| {
            current.max(last.bucket_start)
        }));
        let min_low = items
            .iter()
            .map(|candle| candle.low)
            .min()
            .unwrap_or(first.low);
        let max_high = items
            .iter()
            .map(|candle| candle.high)
            .max()
            .unwrap_or(first.high);
        let max_spread = items
            .iter()
            .map(|candle| candle.spread_cents_close)
            .max()
            .unwrap_or(Decimal::ZERO);
        let sample_count = items
            .iter()
            .map(|candle| i64::from(candle.sample_count.max(0)))
            .sum::<i64>();
        total_samples += sample_count;
        token_summaries.push(json!({
            "token_id": token.token_id,
            "outcome": token.outcome,
            "interval_sec": REWARD_AI_CANDLE_INTERVAL_SEC,
            "first_bucket_start": first.bucket_start,
            "last_bucket_start": last.bucket_start,
            "open": first.open,
            "close": last.close,
            "return_cents": ((last.close - first.open) * decimal("100")).round_dp(8),
            "range_cents": ((max_high - min_low) * decimal("100")).round_dp(8),
            "max_spread_cents": max_spread,
            "sample_count": sample_count,
            "missing": false,
        }));
    }

    json!({
        "schema_version": 1,
        "source_interval_sec": REWARD_AI_CANDLE_SOURCE_INTERVAL_SEC,
        "interval_sec": REWARD_AI_CANDLE_INTERVAL_SEC,
        "limit_per_token": REWARD_AI_CANDLE_LIMIT_PER_TOKEN,
        "source_limit_per_token": REWARD_AI_CANDLE_SOURCE_LIMIT_PER_TOKEN,
        "latest_bucket_start": latest_bucket,
        "token_count": market.tokens.len(),
        "missing_token_count": missing_tokens,
        "sample_count": total_samples,
        "stale": missing_tokens > 0,
        "tokens": token_summaries,
    })
}

fn reward_ai_candle_cache_summary(market: &RewardMarket, candles: &[RewardMarketCandle]) -> Value {
    let mut completed = Vec::new();
    for token in &market.tokens {
        let mut token_candles = candles
            .iter()
            .filter(|candle| candle.token_id == token.token_id)
            .cloned()
            .collect::<Vec<_>>();
        token_candles.sort_by_key(|candle| candle.bucket_start);
        if token_candles.len() <= 1 {
            continue;
        }
        let completed_len = token_candles.len() - 1;
        completed.extend(token_candles.into_iter().take(completed_len));
    }

    let mut summary = reward_ai_candle_summary(market, &completed);
    if let Value::Object(map) = &mut summary {
        map.insert(
            "cache_bucket_policy".to_string(),
            json!("completed_hourly_buckets_only"),
        );
    }
    summary
}
