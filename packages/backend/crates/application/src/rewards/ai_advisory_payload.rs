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
    let mut summary = reward_ai_candle_summary(market, candles);
    if let Value::Object(map) = &mut summary {
        map.insert(
            "cache_bucket_policy".to_string(),
            json!("completed_hourly_buckets_only"),
        );
    }
    summary
}

fn reward_ai_completed_candles(
    market: &RewardMarket,
    candles: &[RewardMarketCandle],
) -> Vec<RewardMarketCandle> {
    let mut completed = Vec::with_capacity(candles.len());
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
    completed
}
