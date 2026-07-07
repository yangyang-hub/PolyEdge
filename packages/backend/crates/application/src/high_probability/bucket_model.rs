#[derive(Debug, Default)]
struct BucketAccumulator {
    samples: u64,
    wins: u64,
    executable_price_sum: Decimal,
    settlement_pnl_sum: Decimal,
    settlement_pnl_count: u64,
    drawdown_sum: Decimal,
    drawdown_count: u64,
    hold_seconds_sum: i128,
    hold_seconds_count: u64,
    break_70: u64,
    break_60: u64,
    break_50: u64,
}

pub fn build_high_probability_bucket_stats(
    config: &HighProbabilityConfig,
    samples: &[HighProbabilitySample],
) -> Vec<HighProbabilityBucketStats> {
    let config = config.clone().normalized();
    let mut buckets: BTreeMap<String, (Value, BucketAccumulator)> = BTreeMap::new();
    for sample in samples.iter().filter(|sample| sample.is_settled_for_stats()) {
        let sample = sample.clone().normalized();
        let (bucket_key, dimensions) = high_probability_bucket_key(&sample);
        let (_, accumulator) = buckets
            .entry(bucket_key)
            .or_insert_with(|| (dimensions, BucketAccumulator::default()));
        accumulator.samples += 1;
        if sample.outcome == HighProbabilitySampleOutcome::Win {
            accumulator.wins += 1;
        }
        accumulator.executable_price_sum += sample.executable_price;
        if let Some(settlement_pnl) = sample.settlement_pnl {
            accumulator.settlement_pnl_sum += settlement_pnl;
            accumulator.settlement_pnl_count += 1;
        }
        if let Some(drawdown) = sample.max_drawdown_cents {
            accumulator.drawdown_sum += drawdown;
            accumulator.drawdown_count += 1;
            if drawdown >= Decimal::new(10, 0) {
                accumulator.break_70 += 1;
            }
            if drawdown >= Decimal::new(20, 0) {
                accumulator.break_60 += 1;
            }
            if drawdown >= Decimal::new(30, 0) {
                accumulator.break_50 += 1;
            }
        }
        if let Some(hold_seconds) = sample.hold_seconds {
            accumulator.hold_seconds_sum += i128::from(hold_seconds.max(0));
            accumulator.hold_seconds_count += 1;
        }
    }

    let now = OffsetDateTime::now_utc();
    buckets
        .into_iter()
        .filter_map(|(bucket_key, (dimensions, accumulator))| {
            if accumulator.samples < config.min_bucket_samples {
                return None;
            }
            let sample_count_decimal = Decimal::from(accumulator.samples);
            let win_rate = Decimal::from(accumulator.wins) / sample_count_decimal;
            let fair_probability = conservative_fair_probability(accumulator.wins, accumulator.samples);
            let avg_entry = accumulator.executable_price_sum / sample_count_decimal;
            let required_buffer =
                config.min_required_edge + config.fee_buffer + config.default_risk_margin;
            Some(HighProbabilityBucketStats {
                id: 0,
                model_version: config.model_version.clone(),
                bucket_key,
                bucket_dimensions: dimensions,
                sample_count: accumulator.samples,
                win_count: accumulator.wins,
                win_rate,
                fair_probability,
                confidence_low: Some(fair_probability),
                confidence_high: Some(win_rate.max(fair_probability)),
                expected_pnl: average_decimal(
                    accumulator.settlement_pnl_sum,
                    accumulator.settlement_pnl_count,
                ),
                avg_max_drawdown_cents: average_decimal(
                    accumulator.drawdown_sum,
                    accumulator.drawdown_count,
                ),
                break_70_rate: rate(accumulator.break_70, accumulator.drawdown_count),
                break_60_rate: rate(accumulator.break_60, accumulator.drawdown_count),
                break_50_rate: rate(accumulator.break_50, accumulator.drawdown_count),
                avg_hold_seconds: average_i64(
                    accumulator.hold_seconds_sum,
                    accumulator.hold_seconds_count,
                ),
                recommended_max_entry_price: Some(
                    (fair_probability - required_buffer).max(Decimal::ZERO).min(avg_entry),
                ),
                computed_at: now,
            })
        })
        .collect()
}

fn high_probability_bucket_key(sample: &HighProbabilitySample) -> (String, Value) {
    let time_bucket = sample
        .time_to_resolution_bucket
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let liquidity_bucket = sample
        .liquidity_bucket
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let spread_bucket = sample
        .spread_bucket
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let key = format!(
        "type={}|price={}|time={}|liquidity={}|spread={}",
        sample.market_type, sample.price_bucket, time_bucket, liquidity_bucket, spread_bucket
    );
    let dimensions = json!({
        "market_type": sample.market_type,
        "price_bucket": sample.price_bucket,
        "time_to_resolution_bucket": time_bucket,
        "liquidity_bucket": liquidity_bucket,
        "spread_bucket": spread_bucket,
    });
    (key, dimensions)
}

fn conservative_fair_probability(wins: u64, samples: u64) -> Decimal {
    if samples == 0 {
        return Decimal::ZERO;
    }
    // Beta(1,1) posterior mean is intentionally conservative for small buckets.
    Decimal::from(wins + 1) / Decimal::from(samples + 2)
}

/// Build a synthetic high-probability sample (used only to derive bucket
/// dimensions) from an observe candidate and its orderbook quote. Shared by the
/// observe path and the fair value provider so both resolve the same bucket for
/// the same candidate.
///
/// Returns `None` when the book is incomplete (no best bid/ask) or the
/// executable price is outside the research range. Callers map the `None` case
/// to their own diagnostic reason codes.
#[must_use]
pub fn high_probability_sample_from_observe_candidate(
    candidate: &HighProbabilityObserveCandidate,
    quote: &HighProbabilityOrderbookQuote,
    now: OffsetDateTime,
) -> Option<(HighProbabilitySample, Decimal)> {
    let candidate = candidate.clone().normalized();
    let quote = quote.clone().normalized();
    let best_bid = quote.best_bid?;
    let best_ask = quote.best_ask?;
    let price_bucket = high_probability_price_bucket(best_ask)?.to_string();
    let spread_cents = ((best_ask - best_bid).max(Decimal::ZERO)) * Decimal::from(100u64);
    let sample = HighProbabilitySample {
        id: 0,
        condition_id: candidate.condition_id,
        token_id: candidate.token_id,
        side: high_probability_side_from_outcome(&candidate.outcome),
        sampled_at: candidate.observed_at,
        trigger_kind: HighProbabilityTriggerKind::FirstTouch,
        executable_price: best_ask,
        price_bucket,
        market_type: candidate.market_type,
        time_to_resolution_bucket: high_probability_time_to_resolution_bucket(
            candidate.observed_at,
            candidate.end_at,
        ),
        liquidity_bucket: high_probability_liquidity_bucket(candidate.liquidity_usd),
        spread_bucket: Some(high_probability_spread_bucket(spread_cents)),
        path_features: json!({
            "source": "observe_reward_market_candles",
            "reference_price": candidate.reference_price,
            "reference_spread_cents": candidate.reference_spread_cents,
            "best_bid": best_bid,
            "best_ask": best_ask,
            "ask_depth_usd": quote.ask_depth_usd,
            "confirmed_at_ms": quote.confirmed_at_ms,
        }),
        risk_tags: candidate.risk_tags,
        outcome: HighProbabilitySampleOutcome::Unknown,
        settlement_pnl: None,
        max_drawdown_cents: None,
        hold_seconds: None,
        created_at: now,
    }
    .normalized();
    Some((sample, spread_cents))
}

fn average_decimal(sum: Decimal, count: u64) -> Option<Decimal> {
    (count > 0).then(|| sum / Decimal::from(count))
}

fn average_i64(sum: i128, count: u64) -> Option<i64> {
    if count == 0 {
        return None;
    }
    let avg = sum / i128::from(count);
    Some(i64::try_from(avg).unwrap_or(i64::MAX))
}

fn rate(count: u64, total: u64) -> Option<Decimal> {
    (total > 0).then(|| Decimal::from(count) / Decimal::from(total))
}
