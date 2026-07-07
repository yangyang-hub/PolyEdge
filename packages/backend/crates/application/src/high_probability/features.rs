// Research feature extraction for the high-probability pricing model. These are
// pure functions: they turn candle windows, orderbook quotes and metadata into a
// structured `HighProbabilityFeatureVector` that future explainable ML models
// (Phase 2) and fair-value diagnostics can consume. They never perform IO.
//
// Path features are computed only from the candle window *before* the sample
// point (at-sample-time information). Forward-looking labels
// (`min_future_close` / `max_future_close`) remain owned by the sample builder.

const HIGH_PROBABILITY_FEATURE_VERSION: &str = "high_probability_features_v1";

/// A risk-tag classifier: sets the matching presence flag on `RiskFeatures`.
type RiskTagClassifier = fn(&mut RiskFeatures);

const HIGH_PROBABILITY_RISK_TAG_TAXONOMY: &[(&str, RiskTagClassifier)] = &[
    ("ambiguous_rules", |features| features.ambiguous_rules = true),
    ("subjective_resolution", |features| {
        features.subjective_resolution = true
    }),
    ("regulatory_or_court_dependency", |features| {
        features.regulatory_or_court_dependency = true
    }),
    ("official_confirmation_pending", |features| {
        features.official_confirmation_pending = true
    }),
    ("single_source_news", |features| features.single_source_news = true),
    ("high_news_velocity", |features| features.high_news_velocity = true),
    ("source_conflict", |features| features.source_conflict = true),
    ("long_horizon", |features| features.long_horizon = true),
];

/// Compute price-path features from candles sorted ascending by `bucket_start`,
/// up to and including the sample point. `bucket_seconds` is the per-candle
/// duration (300 for the 5-minute reward candles).
#[must_use]
pub fn compute_price_path_features(
    past_candles: &[HighProbabilityRewardCandleSampleInput],
    bucket_seconds: i64,
) -> PricePathFeatures {
    let mut features = PricePathFeatures::default();
    if past_candles.is_empty() {
        return features;
    }
    let normalized: Vec<HighProbabilityRewardCandleSampleInput> = past_candles
        .iter()
        .cloned()
        .map(HighProbabilityRewardCandleSampleInput::normalized)
        .collect();
    let mut closes: Vec<Decimal> = normalized.iter().map(|candle| candle.close).collect();
    let sample_close = *closes.last().expect("non-empty");
    let last = closes.len() - 1;

    features.return_5m = window_return(&closes, last, 1);
    features.return_1h = window_return(&closes, last, 12);
    features.return_6h = window_return(&closes, last, 72);
    features.return_24h = window_return(&closes, last, 288);

    features.realized_volatility = realized_volatility(&closes);
    features.max_run_up_cents = max_run_up_cents(&closes);
    features.largest_prior_drawdown_cents = largest_prior_drawdown_cents(&closes);
    features.prior_bucket_crossings = prior_bucket_crossings(&closes);
    features.monotonic_trend_score = monotonic_trend_score(&closes);

    features.time_above_70_sec = time_above_threshold_sec(&closes, Decimal::new(70, 2), bucket_seconds);
    features.time_above_80_sec = time_above_threshold_sec(&closes, Decimal::new(80, 2), bucket_seconds);
    features.time_above_90_sec = time_above_threshold_sec(&closes, Decimal::new(90, 2), bucket_seconds);

    // Silence unused-mut when closes has a single element (no returns to mutate).
    let _ = &mut closes;
    let _ = sample_close;
    features
}

fn window_return(closes: &[Decimal], last: usize, candles_back: usize) -> Option<Decimal> {
    if candles_back == 0 || last < candles_back {
        return None;
    }
    let base = closes.get(last - candles_back)?;
    if *base == Decimal::ZERO {
        return None;
    }
    Some((closes[last] - *base) / *base)
}

fn realized_volatility(closes: &[Decimal]) -> Option<Decimal> {
    if closes.len() < 2 {
        return None;
    }
    let mut returns: Vec<f64> = Vec::with_capacity(closes.len() - 1);
    for window in closes.windows(2) {
        let prev = window[0];
        let curr = window[1];
        if prev == Decimal::ZERO {
            continue;
        }
        let ret = ((curr - prev) / prev).to_string();
        if let Ok(value) = ret.parse::<f64>() {
            returns.push(value);
        }
    }
    if returns.is_empty() {
        return None;
    }
    let mean = returns.iter().copied().sum::<f64>() / returns.len() as f64;
    let variance =
        returns.iter().map(|value| (value - mean).powi(2)).sum::<f64>() / returns.len() as f64;
    let stddev = variance.sqrt();
    if stddev.is_finite() {
        Some(Decimal::from_str(&format!("{stddev:.6}")).unwrap_or(Decimal::ZERO))
    } else {
        None
    }
}

fn max_run_up_cents(closes: &[Decimal]) -> Option<Decimal> {
    let first = closes.first()?;
    let peak = closes.iter().copied().max()?;
    Some(((peak - *first).max(Decimal::ZERO)) * Decimal::from(100u64))
}

fn largest_prior_drawdown_cents(closes: &[Decimal]) -> Option<Decimal> {
    let mut peak = *closes.first()?;
    let mut max_drawdown = Decimal::ZERO;
    for close in closes {
        peak = peak.max(*close);
        let drawdown = peak - *close;
        if drawdown > max_drawdown {
            max_drawdown = drawdown;
        }
    }
    Some(max_drawdown * Decimal::from(100u64))
}

fn prior_bucket_crossings(closes: &[Decimal]) -> Option<i64> {
    let mut last_bucket: Option<&'static str> = None;
    let mut crossings = 0i64;
    for close in closes {
        let bucket = high_probability_price_bucket(*close).unwrap_or("out_of_range");
        if last_bucket.is_some_and(|previous| previous != bucket) {
            crossings += 1;
        }
        last_bucket = Some(bucket);
    }
    Some(crossings)
}

fn monotonic_trend_score(closes: &[Decimal]) -> Option<Decimal> {
    if closes.len() < 2 {
        return None;
    }
    let mut up = 0i64;
    let mut down = 0i64;
    for window in closes.windows(2) {
        match window[1].cmp(&window[0]) {
            std::cmp::Ordering::Greater => up += 1,
            std::cmp::Ordering::Less => down += 1,
            std::cmp::Ordering::Equal => {}
        }
    }
    let total = up + down;
    if total == 0 {
        return Some(Decimal::ZERO);
    }
    Some((Decimal::from(up) - Decimal::from(down)) / Decimal::from(total))
}

fn time_above_threshold_sec(closes: &[Decimal], threshold: Decimal, bucket_seconds: i64) -> Option<i64> {
    let count = closes.iter().filter(|close| **close >= threshold).count();
    Some(i64::try_from(count).unwrap_or(i64::MAX) * bucket_seconds)
}

/// Point-in-time liquidity / book features. `now_ms` is the caller's reference
/// time (epoch millis) used to derive book freshness.
#[must_use]
pub fn compute_liquidity_features(
    spread_cents: Decimal,
    quote: Option<&HighProbabilityOrderbookQuote>,
    liquidity_usd: Option<Decimal>,
    now_ms: i64,
) -> LiquidityFeatures {
    let top_ask_depth_usd = quote.and_then(|quote| {
        quote
            .best_ask
            .zip(quote_first_size(quote, true))
            .map(|(price, size)| (price * size).max(Decimal::ZERO))
    });
    let top_bid_depth_usd = quote.and_then(|quote| {
        quote
            .best_bid
            .zip(quote_first_size(quote, false))
            .map(|(price, size)| (price * size).max(Decimal::ZERO))
    });
    let book_fresh_ms = quote.and_then(|quote| {
        quote
            .confirmed_at_ms
            .filter(|confirmed| *confirmed > 0)
            .map(|confirmed| (now_ms - confirmed).max(0))
    });
    LiquidityFeatures {
        spread_cents: Some(spread_cents.max(Decimal::ZERO)),
        top_ask_depth_usd,
        top_bid_depth_usd,
        book_fresh_ms,
        liquidity_bucket: high_probability_liquidity_bucket(liquidity_usd),
    }
}

fn quote_first_size(quote: &HighProbabilityOrderbookQuote, ask: bool) -> Option<Decimal> {
    // The observe quote only exposes top-of-book depth via `ask_depth_usd`;
    // there is no bid depth field, so we expose what is available and leave the
    // other side as `None` rather than fabricating a size.
    if ask {
        quote
            .ask_depth_usd
            .and_then(|depth| quote.best_ask.map(|price| depth / price))
    } else {
        None
    }
}

/// Time-to-resolution and market-age features.
#[must_use]
pub fn compute_time_features(
    sampled_at: OffsetDateTime,
    end_at: Option<OffsetDateTime>,
    created_at: Option<OffsetDateTime>,
) -> TimeFeatures {
    TimeFeatures {
        time_to_resolution_bucket: high_probability_time_to_resolution_bucket(sampled_at, end_at),
        market_age_bucket: created_at.map(|created| {
            let seconds = (sampled_at - created).whole_seconds().max(0);
            let bucket = if seconds <= 86_400 {
                "lte_1d"
            } else if seconds <= 604_800 {
                "lte_7d"
            } else if seconds <= 2_592_000 {
                "lte_30d"
            } else {
                "gt_30d"
            };
            bucket.to_string()
        }),
    }
}

/// Risk-tag presence flags over the documented taxonomy.
#[must_use]
pub fn compute_risk_features(risk_tags: &[String]) -> RiskFeatures {
    let mut features = RiskFeatures::default();
    for tag in risk_tags {
        let normalized = tag.trim().to_ascii_lowercase();
        if let Some((_, apply)) = HIGH_PROBABILITY_RISK_TAG_TAXONOMY
            .iter()
            .find(|(key, _)| *key == normalized)
        {
            apply(&mut features);
        }
    }
    features
}

/// Aggregate the four feature groups into a versioned vector.
#[must_use]
pub fn compute_high_probability_feature_vector(
    past_candles: &[HighProbabilityRewardCandleSampleInput],
    bucket_seconds: i64,
    spread_cents: Decimal,
    quote: Option<&HighProbabilityOrderbookQuote>,
    liquidity_usd: Option<Decimal>,
    sampled_at: OffsetDateTime,
    end_at: Option<OffsetDateTime>,
    created_at: Option<OffsetDateTime>,
    risk_tags: &[String],
    now_ms: i64,
) -> HighProbabilityFeatureVector {
    let sampled_close = past_candles.last().map(|candle| candle.close);
    let path = match sampled_close {
        Some(_) => compute_price_path_features(past_candles, bucket_seconds),
        None => PricePathFeatures::default(),
    };
    HighProbabilityFeatureVector {
        version: HIGH_PROBABILITY_FEATURE_VERSION.to_string(),
        path,
        liquidity: compute_liquidity_features(spread_cents, quote, liquidity_usd, now_ms),
        time: compute_time_features(sampled_at, end_at, created_at),
        risk: compute_risk_features(risk_tags),
    }
}

/// Serialize the feature vector into the `path_features` JSONB shape.
#[must_use]
pub fn feature_vector_to_json(features: &HighProbabilityFeatureVector) -> Value {
    serde_json::to_value(features).unwrap_or_else(|_| json!({}))
}
