// Fair value provider — a conservative, auditable probability pricing model.
//
// Given the persisted bucket statistics (historical base rate), the current
// orderbook (market-implied probability) and risk tags, this module produces a
// `FairValueEstimate` per condition: a `fair_yes_low/mid/high` band with a
// confidence, an uncertainty in cents, the bucket it was derived from (with the
// coarseness fallback level reached), reason codes and a stable input hash.
//
// The provider never quotes, sizes, places orders or calls the live connector.
// `live_eligible` is a pure assertion about whether the estimate is good enough
// for the (future) Rewards market maker to consume; the market maker remains
// the sole authority over quoting/sizing/exit.

use sha2::{Digest, Sha256};

const HIGH_PROBABILITY_FAIR_VALUE_HASH_VERSION: &str = "high_probability_fair_value_v1";

/// Indexed bucket statistics used for fair-value resolution. Buckets are parsed
/// once into their 5 dimensions so the fallback chain can compare progressively
/// coarser keys without re-deriving them per candidate.
pub struct FairValueBucketIndex {
    indexed: Vec<IndexedBucket>,
    global_prior: Option<Decimal>,
}

struct IndexedBucket {
    stats: HighProbabilityBucketStats,
    dims: BucketDims,
}

#[derive(Clone, Debug)]
struct BucketDims {
    market_type: String,
    price_bucket: String,
    time: String,
    liquidity: String,
    spread: String,
}

/// Matcher predicate for one coarseness level of the bucket fallback chain.
type BucketDimMatcher = fn(&BucketDims, &BucketDims) -> bool;

impl FairValueBucketIndex {
    #[must_use]
    pub fn new(buckets: Vec<HighProbabilityBucketStats>) -> Self {
        let indexed = buckets
            .iter()
            .filter_map(|stats| {
                parse_bucket_dims(&stats.bucket_dimensions)
                    .map(|dims| IndexedBucket { stats: stats.clone(), dims })
            })
            .collect::<Vec<_>>();
        let global_prior = fair_value_global_prior(&buckets);
        Self { indexed, global_prior }
    }

    fn matching(
        &self,
        target: &BucketDims,
        matcher: BucketDimMatcher,
    ) -> Option<&HighProbabilityBucketStats> {
        self.indexed
            .iter()
            .filter(|bucket| matcher(&bucket.dims, target))
            .max_by_key(|bucket| bucket.stats.sample_count)
            .map(|bucket| &bucket.stats)
    }
}

/// A resolved bucket plus the coarseness level it was found at.
#[derive(Clone, Debug)]
pub struct FairValueBucketResolution {
    pub bucket: HighProbabilityBucketStats,
    pub fallback_level: u8,
    pub coarseness_margin_cents: Decimal,
}

fn parse_bucket_dims(dimensions: &Value) -> Option<BucketDims> {
    let object = dimensions.as_object()?;
    Some(BucketDims {
        market_type: object.get("market_type")?.as_str()?.to_string(),
        price_bucket: object.get("price_bucket")?.as_str()?.to_string(),
        time: object
            .get("time_to_resolution_bucket")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown")
            .to_string(),
        liquidity: object
            .get("liquidity_bucket")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown")
            .to_string(),
        spread: object
            .get("spread_bucket")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown")
            .to_string(),
    })
}

fn fair_value_target_dims(input: &FairValuePricingInput) -> Option<BucketDims> {
    let price = input.executable_price();
    let price_bucket = high_probability_price_bucket(price)?.to_string();
    let spread_cents = fair_value_spread_cents(input);
    Some(BucketDims {
        market_type: non_empty_or(input.market_type.clone(), "unknown"),
        price_bucket,
        time: high_probability_time_to_resolution_bucket(input.observed_at, input.end_at)
            .unwrap_or_else(|| "unknown".to_string()),
        liquidity: high_probability_liquidity_bucket(input.liquidity_usd)
            .unwrap_or_else(|| "unknown".to_string()),
        spread: high_probability_spread_bucket(spread_cents),
    })
}

fn dims_match_exact(bucket: &BucketDims, target: &BucketDims) -> bool {
    bucket.market_type == target.market_type
        && bucket.price_bucket == target.price_bucket
        && bucket.time == target.time
        && bucket.liquidity == target.liquidity
        && bucket.spread == target.spread
}

fn dims_match_drop_spread(bucket: &BucketDims, target: &BucketDims) -> bool {
    bucket.market_type == target.market_type
        && bucket.price_bucket == target.price_bucket
        && bucket.time == target.time
        && bucket.liquidity == target.liquidity
}

fn dims_match_drop_liquidity(bucket: &BucketDims, target: &BucketDims) -> bool {
    bucket.market_type == target.market_type
        && bucket.price_bucket == target.price_bucket
        && bucket.time == target.time
}

fn dims_match_drop_time(bucket: &BucketDims, target: &BucketDims) -> bool {
    bucket.market_type == target.market_type && bucket.price_bucket == target.price_bucket
}

fn dims_match_price_only(bucket: &BucketDims, target: &BucketDims) -> bool {
    bucket.price_bucket == target.price_bucket
}

/// Resolve the best bucket for an input by walking the coarseness chain.
/// Returns `None` only when the price is outside the research range or there is
/// no global prior (no historical buckets at all).
#[must_use]
pub fn resolve_fair_value_bucket(
    input: &FairValuePricingInput,
    index: &FairValueBucketIndex,
    _min_samples: u64,
) -> Option<FairValueBucketResolution> {
    let target = fair_value_target_dims(input)?;
    let levels: &[(u8, Decimal, BucketDimMatcher)] = &[
        (0, Decimal::ZERO, dims_match_exact),
        (1, Decimal::ONE, dims_match_drop_spread),
        (2, Decimal::from(2u64), dims_match_drop_liquidity),
        (3, Decimal::from(3u64), dims_match_drop_time),
        (4, Decimal::from(4u64), dims_match_price_only),
    ];
    for (level, margin, matcher) in levels {
        if let Some(bucket) = index.matching(&target, *matcher) {
            return Some(FairValueBucketResolution {
                bucket: bucket.clone(),
                fallback_level: *level,
                coarseness_margin_cents: *margin,
            });
        }
    }

    // Final fallback: sample-weighted global prior across all buckets.
    let prior = index.global_prior?;
    let total_samples = index.indexed.iter().map(|bucket| bucket.stats.sample_count).sum::<u64>();
    let model_version = index
        .indexed
        .first()
        .map(|bucket| bucket.stats.model_version.clone())
        .unwrap_or_default();
    let computed_at = index
        .indexed
        .iter()
        .map(|bucket| bucket.stats.computed_at)
        .max()
        .unwrap_or(OffsetDateTime::UNIX_EPOCH);
    let synthetic = HighProbabilityBucketStats {
        id: 0,
        model_version,
        bucket_key: "global_prior".to_string(),
        bucket_dimensions: json!({ "fallback": "global_prior" }),
        sample_count: total_samples,
        win_count: 0,
        win_rate: prior,
        fair_probability: prior,
        confidence_low: None,
        confidence_high: None,
        expected_pnl: None,
        avg_max_drawdown_cents: None,
        break_70_rate: None,
        break_60_rate: None,
        break_50_rate: None,
        avg_hold_seconds: None,
        recommended_max_entry_price: None,
        computed_at,
    };
    Some(FairValueBucketResolution {
        bucket: synthetic,
        fallback_level: 5,
        coarseness_margin_cents: Decimal::from(6u64),
    })
}

/// Sample-weighted mean of every bucket's conservative `fair_probability`.
#[must_use]
pub fn fair_value_global_prior(buckets: &[HighProbabilityBucketStats]) -> Option<Decimal> {
    let mut weighted = Decimal::ZERO;
    let mut total_samples = 0u64;
    for bucket in buckets {
        weighted += bucket.fair_probability * Decimal::from(bucket.sample_count);
        total_samples += bucket.sample_count;
    }
    (total_samples > 0).then(|| weighted / Decimal::from(total_samples))
}

/// Conservative weighted blend of the market-implied and historical base-rate
/// probabilities. Weights are clamped to `[0, ∞)` and normalized so callers do
/// not need to hit exactly 1.0; if both collapse to zero the market-implied
/// value is returned unchanged.
#[must_use]
pub fn blend_fair_value_mid(
    market_implied: Decimal,
    base_rate: Decimal,
    w_market: Decimal,
    w_base_rate: Decimal,
) -> Decimal {
    let w_market = w_market.max(Decimal::ZERO);
    let w_base_rate = w_base_rate.max(Decimal::ZERO);
    let sum = w_market + w_base_rate;
    if sum == Decimal::ZERO {
        return market_implied;
    }
    (w_market * market_implied + w_base_rate * base_rate) / sum
}

fn wilson_half_width_cents(fair_probability: Decimal, sample_count: u64) -> Decimal {
    if sample_count == 0 {
        return Decimal::from(100u64);
    }
    let Some(p) = fair_probability.to_string().parse::<f64>().ok() else {
        return Decimal::from(100u64);
    };
    let n = sample_count as f64;
    let standard_error = (p * (1.0 - p) / n).sqrt();
    let half_width = 1.96 * standard_error;
    if half_width.is_finite() {
        Decimal::from_str(&format!("{:.4}", half_width * 100.0)).unwrap_or(Decimal::from(100u64))
    } else {
        Decimal::from(100u64)
    }
}

/// Additive uncertainty in cents (probability × 100), clamped to `[0, 100]`.
#[must_use]
pub fn fair_value_uncertainty_cents(
    config: &HighProbabilityConfig,
    resolution: Option<&FairValueBucketResolution>,
    spread_cents: Decimal,
    stale: bool,
    risk_tags: &[String],
) -> Decimal {
    let mut total = Decimal::ZERO;

    let model_error = match resolution {
        Some(resolved) => match (resolved.bucket.confidence_low, resolved.bucket.confidence_high) {
            (Some(low), Some(high)) => ((high - low).max(Decimal::ZERO)) * Decimal::from(100u64),
            _ => wilson_half_width_cents(resolved.bucket.fair_probability, resolved.bucket.sample_count),
        },
        None => Decimal::from(100u64),
    };
    total += model_error;
    total += resolution
        .map(|resolved| resolved.coarseness_margin_cents)
        .unwrap_or(Decimal::from(10u64));
    total += spread_cents / Decimal::from(2u64);
    if stale {
        total += Decimal::from(5u64);
    }
    let tag_count = u64::from(compute_risk_features(risk_tags).active_count()).min(4);
    total += (Decimal::new(15, 1) * Decimal::from(tag_count)).min(Decimal::from(6u64));
    total += config.default_risk_margin * Decimal::from(100u64);

    total.max(Decimal::ZERO).min(Decimal::from(100u64))
}

/// Confidence in `[0, 1]`: sample saturation minus coarseness / spread /
/// staleness / risk penalties.
#[must_use]
pub fn fair_value_confidence(
    config: &HighProbabilityConfig,
    resolution: Option<&FairValueBucketResolution>,
    spread_cents: Decimal,
    stale: bool,
    risk_tags: &[String],
) -> Decimal {
    let target = Decimal::from(config.fair_value_target_sample_count);
    let sample_count = resolution.map(|resolved| resolved.bucket.sample_count).unwrap_or(0);
    let base = if target > Decimal::ZERO {
        (Decimal::from(sample_count) / target)
            .max(Decimal::ZERO)
            .min(Decimal::ONE)
    } else {
        Decimal::ZERO
    };
    let mut penalty = Decimal::ZERO;
    penalty +=
        Decimal::from(resolution.map(|resolved| resolved.fallback_level).unwrap_or(5)) * Decimal::new(8, 2);
    if config.max_spread_cents > Decimal::ZERO {
        let spread_ratio = (spread_cents / config.max_spread_cents)
            .max(Decimal::ZERO)
            .min(Decimal::ONE);
        penalty += spread_ratio * Decimal::new(15, 2);
    }
    if stale {
        penalty += Decimal::new(15, 2);
    }
    let risk_count = u64::from(compute_risk_features(risk_tags).active_count()).min(3);
    penalty += Decimal::from(risk_count) * Decimal::new(5, 2);

    (base - penalty).max(Decimal::ZERO).min(Decimal::ONE)
}

fn fair_value_side_from_outcome(outcome: &str) -> FairValueSide {
    if outcome.trim().eq_ignore_ascii_case("no") {
        FairValueSide::NoComplement
    } else {
        FairValueSide::Yes
    }
}

fn fair_value_midpoint(input: &FairValuePricingInput) -> Decimal {
    match (input.best_bid, input.best_ask) {
        (Some(bid), Some(ask)) if bid > Decimal::ZERO && ask > Decimal::ZERO => {
            clamp_decimal((bid + ask) / Decimal::from(2u64), Decimal::ZERO, Decimal::ONE)
        }
        _ => input.reference_price,
    }
}

fn fair_value_spread_cents(input: &FairValuePricingInput) -> Decimal {
    match (input.best_bid, input.best_ask) {
        (Some(bid), Some(ask)) => ((ask - bid).max(Decimal::ZERO)) * Decimal::from(100u64),
        _ => input.reference_spread_cents.max(Decimal::ZERO),
    }
}

fn fair_value_is_stale(input: &FairValuePricingInput, now: OffsetDateTime, stale_book_ms: i64) -> bool {
    let Some(confirmed) = input.confirmed_at_ms.filter(|value| *value > 0) else {
        return true;
    };
    let now_ms = now.unix_timestamp().saturating_mul(1000);
    (now_ms - confirmed) > stale_book_ms
}

/// Pick the input used to derive `fair_yes` for a condition: prefer a YES-side
/// token whose price is in the research range, else a NO-side token in range
/// (which is complemented to YES scale).
fn choose_fair_value_input(
    inputs: &[FairValuePricingInput],
) -> Option<&FairValuePricingInput> {
    let yes_in_range = inputs.iter().find(|input| {
        fair_value_side_from_outcome(&input.outcome) == FairValueSide::Yes
            && high_probability_price_bucket(input.executable_price()).is_some()
    });
    if yes_in_range.is_some() {
        return yes_in_range;
    }
    inputs.iter().find(|input| {
        fair_value_side_from_outcome(&input.outcome) == FairValueSide::NoComplement
            && high_probability_price_bucket(input.executable_price()).is_some()
    })
}

/// Build a single fair value estimate from one pricing input.
#[must_use]
pub fn build_fair_value_estimate(
    config: &HighProbabilityConfig,
    input: &FairValuePricingInput,
    index: &FairValueBucketIndex,
    now: OffsetDateTime,
) -> Option<FairValueEstimate> {
    let config = config.clone().normalized();
    let input = input.clone().normalized();
    let price_used = input.executable_price();
    // `resolve_fair_value_bucket` returns None when the price is outside the
    // research range or there is no historical data at all.
    let resolution = resolve_fair_value_bucket(&input, index, config.min_bucket_samples)?;
    let side = fair_value_side_from_outcome(&input.outcome);

    let midpoint = fair_value_midpoint(&input);
    let market_implied_yes = match side {
        FairValueSide::Yes => midpoint,
        FairValueSide::NoComplement => Decimal::ONE - midpoint,
    };
    let base_rate_yes = match side {
        FairValueSide::Yes => resolution.bucket.fair_probability,
        FairValueSide::NoComplement => Decimal::ONE - resolution.bucket.fair_probability,
    };
    let fair_mid = blend_fair_value_mid(
        market_implied_yes,
        base_rate_yes,
        config.fair_value_market_weight,
        config.fair_value_base_rate_weight,
    );

    let spread_cents = fair_value_spread_cents(&input);
    let stale = fair_value_is_stale(&input, now, config.fair_value_stale_book_ms);
    let uncertainty_cents = fair_value_uncertainty_cents(
        &config,
        Some(&resolution),
        spread_cents,
        stale,
        &input.risk_tags,
    );
    let confidence = fair_value_confidence(
        &config,
        Some(&resolution),
        spread_cents,
        stale,
        &input.risk_tags,
    );

    let uncertainty_probability = uncertainty_cents / Decimal::from(100u64);
    let fair_yes_low = clamp_decimal(fair_mid - uncertainty_probability, Decimal::ZERO, Decimal::ONE);
    let fair_yes_high = clamp_decimal(fair_mid + uncertainty_probability, Decimal::ZERO, Decimal::ONE);
    let fair_yes_mid = clamp_decimal(fair_mid, Decimal::ZERO, Decimal::ONE);

    let (reason_codes, live_eligible) = fair_value_eligibility(
        &config,
        &input,
        Some(&resolution),
        confidence,
        uncertainty_cents,
        spread_cents,
        stale,
    );
    let input_hash = fair_value_input_hash(
        &config.model_version,
        &input.condition_id,
        &resolution.bucket.bucket_key,
        side,
        price_used,
        resolution.bucket.sample_count,
        &input.risk_tags,
    );
    let expires_at = now
        .checked_add(time::Duration::seconds(config.fair_value_ttl_sec))
        .unwrap_or(now);

    Some(FairValueEstimate {
        id: 0,
        condition_id: input.condition_id.clone(),
        token_id: input.token_id.clone(),
        side_used: side,
        price_used,
        fair_yes_low,
        fair_yes_mid,
        fair_yes_high,
        market_implied: clamp_decimal(market_implied_yes, Decimal::ZERO, Decimal::ONE),
        base_rate: clamp_decimal(base_rate_yes, Decimal::ZERO, Decimal::ONE),
        confidence,
        uncertainty_cents,
        sample_count: resolution.bucket.sample_count,
        bucket_key: resolution.bucket.bucket_key.clone(),
        fallback_level: resolution.fallback_level,
        model_version: config.model_version.clone(),
        input_hash,
        reason_codes,
        live_eligible,
        computed_at: now,
        expires_at,
        created_at: now,
    })
}

fn fair_value_eligibility(
    config: &HighProbabilityConfig,
    input: &FairValuePricingInput,
    resolution: Option<&FairValueBucketResolution>,
    confidence: Decimal,
    uncertainty_cents: Decimal,
    spread_cents: Decimal,
    stale: bool,
) -> (Vec<String>, bool) {
    let mut reasons: Vec<String> = Vec::new();
    if confidence < config.min_confidence {
        reasons.push("confidence_below_min".to_string());
    }
    if uncertainty_cents > config.fair_value_max_uncertainty_cents {
        reasons.push("uncertainty_above_max".to_string());
    }
    let (fallback_level, sample_count) = match resolution {
        Some(resolved) => (resolved.fallback_level, resolved.bucket.sample_count),
        None => {
            reasons.push("bucket_missing".to_string());
            (5, 0)
        }
    };
    if fallback_level > 3 {
        reasons.push("bucket_fallback_too_coarse".to_string());
    }
    if sample_count < config.min_bucket_samples {
        reasons.push("sample_count_below_min".to_string());
    }
    let excluded = config
        .excluded_risk_tags
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    for tag in &input.risk_tags {
        if excluded.contains(tag) {
            reasons.push(format!("excluded_risk_tag:{tag}"));
        }
    }
    if spread_cents > config.max_spread_cents {
        reasons.push("spread_too_wide".to_string());
    }
    let book_complete = matches!((input.best_bid, input.best_ask),
        (Some(bid), Some(ask)) if bid > Decimal::ZERO && ask > Decimal::ZERO
    );
    if !book_complete {
        reasons.push("book_incomplete".to_string());
    }
    match input.ask_depth_usd {
        Some(depth) if depth >= config.min_depth_usd => {}
        Some(_) => reasons.push("ask_depth_below_min".to_string()),
        None => reasons.push("ask_depth_missing".to_string()),
    }
    if stale {
        reasons.push("orderbook_stale".to_string());
    }

    let live_eligible = reasons.is_empty();
    if live_eligible {
        reasons.push("eligible".to_string());
    }
    (reasons, live_eligible)
}

/// Build fair value estimates for many conditions, one row per condition.
#[must_use]
pub fn build_fair_value_estimates(
    config: &HighProbabilityConfig,
    inputs_by_condition: &BTreeMap<String, Vec<FairValuePricingInput>>,
    bucket_stats: &[HighProbabilityBucketStats],
    now: OffsetDateTime,
) -> Vec<FairValueEstimate> {
    let config = config.clone().normalized();
    let index = FairValueBucketIndex::new(bucket_stats.to_vec());
    let mut estimates = Vec::new();
    for inputs in inputs_by_condition.values() {
        let Some(chosen) = choose_fair_value_input(inputs) else {
            continue;
        };
        let input = chosen.clone().normalized();
        if high_probability_price_bucket(input.executable_price()).is_none() {
            continue;
        }
        if let Some(estimate) = build_fair_value_estimate(&config, &input, &index, now) {
            estimates.push(estimate);
        }
    }
    estimates.sort_by(|left, right| right.confidence.cmp(&left.confidence));
    estimates
}

/// Stable SHA-256 fingerprint of the inputs that define an estimate. Used for
/// cache keying and audit; deliberately not `DefaultHasher` (which is not stable
/// across processes or Rust versions).
#[must_use]
pub fn fair_value_input_hash(
    model_version: &str,
    condition_id: &str,
    bucket_key: &str,
    side: FairValueSide,
    price_used: Decimal,
    sample_count: u64,
    risk_tags: &[String],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(HIGH_PROBABILITY_FAIR_VALUE_HASH_VERSION.as_bytes());
    hasher.update([0]);
    hasher.update(model_version.trim().as_bytes());
    hasher.update([0]);
    hasher.update(condition_id.trim().as_bytes());
    hasher.update([0]);
    hasher.update(bucket_key.as_bytes());
    hasher.update([0]);
    hasher.update(side.as_str().as_bytes());
    hasher.update([0]);
    hasher.update(format!("{price_used}").as_bytes());
    hasher.update([0]);
    hasher.update(sample_count.to_le_bytes());
    hasher.update([0]);
    let mut normalized_tags = risk_tags
        .iter()
        .filter_map(|tag| {
            let tag = tag.trim();
            (!tag.is_empty()).then(|| tag.to_ascii_lowercase())
        })
        .collect::<Vec<_>>();
    normalized_tags.sort();
    normalized_tags.dedup();
    for tag in &normalized_tags {
        hasher.update(tag.as_bytes());
        hasher.update([0]);
    }

    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}
