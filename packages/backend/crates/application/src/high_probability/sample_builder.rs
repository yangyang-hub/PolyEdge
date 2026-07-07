pub fn build_high_probability_samples_from_reward_candles(
    inputs: &[HighProbabilityRewardCandleSampleInput],
) -> Vec<HighProbabilitySample> {
    let mut by_token = BTreeMap::<(String, String), Vec<HighProbabilityRewardCandleSampleInput>>::new();
    for input in inputs {
        let input = input.clone().normalized();
        if high_probability_price_bucket(input.close).is_none() {
            continue;
        }
        by_token
            .entry((input.condition_id.clone(), input.token_id.clone()))
            .or_default()
            .push(input);
    }

    let now = OffsetDateTime::now_utc();
    let mut samples = Vec::new();
    for ((_condition_id, _token_id), mut candles) in by_token {
        candles.sort_by(|left, right| left.bucket_start.cmp(&right.bucket_start));
        let mut touched_buckets = BTreeMap::<String, bool>::new();
        for index in 0..candles.len() {
            let candle = &candles[index];
            let Some(price_bucket) = high_probability_price_bucket(candle.close) else {
                continue;
            };
            if touched_buckets.contains_key(price_bucket) {
                continue;
            }
            touched_buckets.insert(price_bucket.to_string(), true);
            let min_future_close = candles[index..]
                .iter()
                .map(|future| future.close)
                .min()
                .unwrap_or(candle.close);
            let max_future_close = candles[index..]
                .iter()
                .map(|future| future.close)
                .max()
                .unwrap_or(candle.close);
            let max_drawdown_cents =
                ((candle.close - min_future_close).max(Decimal::ZERO)) * Decimal::from(100u64);
            // Past-window research features (at-sample-time information only).
            // The forward labels above remain top-level for exit-rule backtesting.
            let feature_vector = compute_high_probability_feature_vector(
                &candles[..=index],
                300,
                candle.spread_cents_close,
                None,
                candle.liquidity_usd,
                candle.bucket_start,
                candle.resolved_at,
                None,
                &candle.risk_tags,
                candle.bucket_start.unix_timestamp().saturating_mul(1000),
            );
            samples.push(HighProbabilitySample {
                id: 0,
                condition_id: candle.condition_id.clone(),
                token_id: candle.token_id.clone(),
                side: high_probability_side_from_outcome(&candle.outcome),
                sampled_at: candle.bucket_start,
                trigger_kind: HighProbabilityTriggerKind::FirstTouch,
                executable_price: candle.close,
                price_bucket: price_bucket.to_string(),
                market_type: candle.market_type.clone(),
                time_to_resolution_bucket: high_probability_time_to_resolution_bucket(
                    candle.bucket_start,
                    candle.resolved_at,
                ),
                liquidity_bucket: high_probability_liquidity_bucket(candle.liquidity_usd),
                spread_bucket: Some(high_probability_spread_bucket(candle.spread_cents_close)),
                path_features: json!({
                    "source": "reward_market_candles",
                    "trigger": "first_touch",
                    "min_future_close": min_future_close,
                    "max_future_close": max_future_close,
                    "future_candle_count": candles.len() - index,
                    "features": feature_vector_to_json(&feature_vector),
                }),
                risk_tags: candle.risk_tags.clone(),
                outcome: high_probability_sample_outcome(candle),
                settlement_pnl: high_probability_settlement_pnl(candle),
                max_drawdown_cents: Some(max_drawdown_cents),
                hold_seconds: high_probability_hold_seconds(candle.bucket_start, candle.resolved_at),
                created_at: now,
            });
        }
    }
    samples.sort_by(|left, right| left.sampled_at.cmp(&right.sampled_at));
    samples
}

fn high_probability_sample_outcome(
    input: &HighProbabilityRewardCandleSampleInput,
) -> HighProbabilitySampleOutcome {
    match input.outcome_status {
        HighProbabilityMarketOutcomeStatus::Resolved => {
            if input
                .winning_token_id
                .as_deref()
                .is_some_and(|winner| winner == input.token_id)
            {
                HighProbabilitySampleOutcome::Win
            } else {
                HighProbabilitySampleOutcome::Loss
            }
        }
        HighProbabilityMarketOutcomeStatus::Voided => HighProbabilitySampleOutcome::Voided,
        HighProbabilityMarketOutcomeStatus::Unresolved | HighProbabilityMarketOutcomeStatus::Ambiguous => {
            HighProbabilitySampleOutcome::Unknown
        }
    }
}

fn high_probability_settlement_pnl(
    input: &HighProbabilityRewardCandleSampleInput,
) -> Option<Decimal> {
    match high_probability_sample_outcome(input) {
        HighProbabilitySampleOutcome::Win => Some(Decimal::ONE - input.close),
        HighProbabilitySampleOutcome::Loss => Some(-input.close),
        HighProbabilitySampleOutcome::Voided | HighProbabilitySampleOutcome::Unknown => None,
    }
}

fn high_probability_price_bucket(price: Decimal) -> Option<&'static str> {
    let buckets = [
        (Decimal::new(55, 2), Decimal::new(60, 2), "0.55-0.60"),
        (Decimal::new(60, 2), Decimal::new(65, 2), "0.60-0.65"),
        (Decimal::new(65, 2), Decimal::new(70, 2), "0.65-0.70"),
        (Decimal::new(70, 2), Decimal::new(75, 2), "0.70-0.75"),
        (Decimal::new(75, 2), Decimal::new(80, 2), "0.75-0.80"),
        (Decimal::new(80, 2), Decimal::new(85, 2), "0.80-0.85"),
        (Decimal::new(85, 2), Decimal::new(90, 2), "0.85-0.90"),
        (Decimal::new(90, 2), Decimal::new(95, 2), "0.90-0.95"),
        (Decimal::new(95, 2), Decimal::ONE, "0.95-1.00"),
    ];
    buckets
        .iter()
        .find(|(min, max, _)| price >= *min && price < *max)
        .map(|(_, _, label)| *label)
        .or_else(|| (price == Decimal::ONE).then_some("0.95-1.00"))
}

fn high_probability_side_from_outcome(outcome: &str) -> String {
    if outcome.trim().eq_ignore_ascii_case("no") {
        "no".to_string()
    } else {
        "yes".to_string()
    }
}

fn high_probability_time_to_resolution_bucket(
    sampled_at: OffsetDateTime,
    resolved_at: Option<OffsetDateTime>,
) -> Option<String> {
    let resolved_at = resolved_at?;
    let seconds = (resolved_at - sampled_at).whole_seconds().max(0);
    let bucket = if seconds <= 3_600 {
        "lte_1h"
    } else if seconds <= 21_600 {
        "lte_6h"
    } else if seconds <= 86_400 {
        "lte_1d"
    } else if seconds <= 604_800 {
        "lte_7d"
    } else {
        "gt_7d"
    };
    Some(bucket.to_string())
}

fn high_probability_hold_seconds(
    sampled_at: OffsetDateTime,
    resolved_at: Option<OffsetDateTime>,
) -> Option<i64> {
    resolved_at.map(|resolved_at| (resolved_at - sampled_at).whole_seconds().max(0))
}

fn high_probability_liquidity_bucket(liquidity_usd: Option<Decimal>) -> Option<String> {
    let liquidity_usd = liquidity_usd?;
    let bucket = if liquidity_usd < Decimal::new(1_000, 0) {
        "thin"
    } else if liquidity_usd < Decimal::new(10_000, 0) {
        "medium"
    } else {
        "high"
    };
    Some(bucket.to_string())
}

fn high_probability_spread_bucket(spread_cents: Decimal) -> String {
    if spread_cents <= Decimal::ONE {
        "tight".to_string()
    } else if spread_cents <= Decimal::new(3, 0) {
        "normal".to_string()
    } else {
        "wide".to_string()
    }
}
