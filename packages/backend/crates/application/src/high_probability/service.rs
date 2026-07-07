#[async_trait]
pub trait HighProbabilityStore: Send + Sync {
    async fn load_config(&self) -> Result<HighProbabilityConfig>;
    async fn save_config(&self, config: &HighProbabilityConfig) -> Result<()>;
    async fn record_samples(&self, samples: &[HighProbabilitySample]) -> Result<usize>;
    async fn upsert_market_outcome(&self, outcome: &HighProbabilityMarketOutcome) -> Result<()>;
    async fn list_reward_candle_sample_inputs(
        &self,
        limit: u32,
    ) -> Result<Vec<HighProbabilityRewardCandleSampleInput>>;
    async fn list_observe_candidates(
        &self,
        limit: u16,
    ) -> Result<Vec<HighProbabilityObserveCandidate>>;
    async fn list_samples(
        &self,
        query: HighProbabilitySampleQuery,
    ) -> Result<Vec<HighProbabilitySample>>;
    async fn replace_bucket_stats(
        &self,
        model_version: &str,
        stats: &[HighProbabilityBucketStats],
    ) -> Result<usize>;
    async fn list_bucket_stats(
        &self,
        model_version: Option<&str>,
        limit: u16,
    ) -> Result<Vec<HighProbabilityBucketStats>>;
    async fn record_backtest_result(
        &self,
        result: &HighProbabilityBacktestResult,
    ) -> Result<HighProbabilityBacktestPersistReport>;
    async fn list_backtest_runs(&self, limit: u16) -> Result<Vec<HighProbabilityBacktestRun>>;
    async fn list_backtest_trades(
        &self,
        run_id: i64,
        limit: u16,
    ) -> Result<Vec<HighProbabilityBacktestTrade>>;
    async fn record_observation(&self, observation: &HighProbabilityObservation) -> Result<()>;
    async fn list_observations(&self, limit: u16) -> Result<Vec<HighProbabilityObservation>>;
    async fn record_fair_value(&self, estimate: &FairValueEstimate) -> Result<()>;
    async fn list_fair_values(
        &self,
        model_version: Option<&str>,
        include_expired: bool,
        limit: u16,
    ) -> Result<Vec<FairValueEstimate>>;
}

#[derive(Clone)]
pub struct HighProbabilityService {
    store: Arc<dyn HighProbabilityStore>,
}

impl HighProbabilityService {
    #[must_use]
    pub fn new(store: Arc<dyn HighProbabilityStore>) -> Self {
        Self { store }
    }

    pub async fn read_config(&self) -> Result<HighProbabilityConfig> {
        self.store
            .load_config()
            .await
            .map(HighProbabilityConfig::normalized)
    }

    pub async fn save_config(
        &self,
        config: HighProbabilityConfig,
    ) -> Result<HighProbabilityConfig> {
        let config = config.normalized();
        self.store.save_config(&config).await?;
        Ok(config)
    }

    pub async fn snapshot(&self) -> Result<HighProbabilitySnapshot> {
        let config = self.read_config().await?;
        let bucket_stats = self
            .store
            .list_bucket_stats(
                Some(&config.model_version),
                DEFAULT_HIGH_PROBABILITY_LIST_LIMIT,
            )
            .await?;
        let observations = self
            .store
            .list_observations(DEFAULT_HIGH_PROBABILITY_LIST_LIMIT)
            .await?;
        Ok(HighProbabilitySnapshot {
            config,
            bucket_stats,
            observations,
        })
    }

    pub async fn research_report(&self) -> Result<HighProbabilityResearchReport> {
        let config = self.read_config().await?;
        let sample_limit = MAX_HIGH_PROBABILITY_LIST_LIMIT;
        let samples = self
            .store
            .list_samples(HighProbabilitySampleQuery {
                outcome: None,
                market_type: None,
                limit: sample_limit,
            })
            .await?;
        let bucket_stats = self
            .store
            .list_bucket_stats(Some(&config.model_version), MAX_HIGH_PROBABILITY_LIST_LIMIT)
            .await?;

        Ok(build_high_probability_research_report(
            &config,
            &samples,
            &bucket_stats,
            sample_limit,
        ))
    }

    pub async fn backtest_report(&self) -> Result<HighProbabilityBacktestReport> {
        Ok(self.build_backtest_result().await?.run.report)
    }

    pub async fn run_and_record_backtest(&self) -> Result<HighProbabilityBacktestPersistReport> {
        let result = self.build_backtest_result().await?;
        self.store.record_backtest_result(&result).await
    }

    pub async fn list_backtest_runs(
        &self,
        limit: Option<u16>,
    ) -> Result<Vec<HighProbabilityBacktestRun>> {
        self.store
            .list_backtest_runs(validate_high_probability_list_limit(limit))
            .await
    }

    pub async fn list_backtest_trades(
        &self,
        run_id: i64,
        limit: Option<u16>,
    ) -> Result<Vec<HighProbabilityBacktestTrade>> {
        if run_id <= 0 {
            return Err(AppError::invalid_input(
                "HIGH_PROBABILITY_BACKTEST_RUN_ID_INVALID",
                "high probability backtest run id must be positive",
            ));
        }
        self.store
            .list_backtest_trades(run_id, validate_high_probability_list_limit(limit))
            .await
    }

    async fn build_backtest_result(&self) -> Result<HighProbabilityBacktestResult> {
        let config = self.read_config().await?;
        let sample_limit = MAX_HIGH_PROBABILITY_LIST_LIMIT;
        let samples = self
            .store
            .list_samples(HighProbabilitySampleQuery {
                outcome: None,
                market_type: None,
                limit: sample_limit,
            })
            .await?;

        Ok(build_high_probability_backtest_result(
            &config,
            &samples,
            sample_limit,
        ))
    }

    pub async fn record_samples(&self, samples: &[HighProbabilitySample]) -> Result<usize> {
        let normalized = samples
            .iter()
            .cloned()
            .map(HighProbabilitySample::normalized)
            .collect::<Vec<_>>();
        self.store.record_samples(&normalized).await
    }

    pub async fn upsert_market_outcome(
        &self,
        outcome: HighProbabilityMarketOutcome,
    ) -> Result<HighProbabilityMarketOutcome> {
        let outcome = outcome.normalized();
        self.store.upsert_market_outcome(&outcome).await?;
        Ok(outcome)
    }

    pub async fn build_reward_candle_samples(
        &self,
        limit: Option<u32>,
    ) -> Result<HighProbabilitySampleBuildReport> {
        let limit = validate_high_probability_sample_input_limit(limit);
        let inputs = self.store.list_reward_candle_sample_inputs(limit).await?;
        let samples = build_high_probability_samples_from_reward_candles(&inputs);
        let inserted = self.store.record_samples(&samples).await?;
        Ok(HighProbabilitySampleBuildReport {
            candle_inputs_scanned: inputs.len(),
            samples_built: samples.len(),
            samples_inserted: inserted,
        })
    }

    pub async fn refresh_bucket_stats(&self) -> Result<HighProbabilityBucketRefreshReport> {
        let config = self.read_config().await?;
        let samples = self
            .store
            .list_samples(HighProbabilitySampleQuery {
                outcome: None,
                market_type: None,
                limit: MAX_HIGH_PROBABILITY_LIST_LIMIT,
            })
            .await?;
        let settled_samples = samples
            .iter()
            .filter(|sample| sample.is_settled_for_stats())
            .count();
        let stats = build_high_probability_bucket_stats(&config, &samples);
        let saved = self
            .store
            .replace_bucket_stats(&config.model_version, &stats)
            .await?;
        Ok(HighProbabilityBucketRefreshReport {
            samples_scanned: samples.len(),
            settled_samples,
            buckets_computed: stats.len(),
            buckets_saved: saved,
        })
    }

    pub async fn list_observe_candidates(
        &self,
        limit: Option<u16>,
    ) -> Result<Vec<HighProbabilityObserveCandidate>> {
        self.store
            .list_observe_candidates(validate_high_probability_list_limit(limit))
            .await
    }

    pub async fn observe_candidates(
        &self,
        candidates: &[HighProbabilityObserveCandidate],
        quotes: &[HighProbabilityOrderbookQuote],
    ) -> Result<HighProbabilityObserveReport> {
        let config = self.read_config().await?;
        let bucket_stats = self
            .store
            .list_bucket_stats(Some(&config.model_version), MAX_HIGH_PROBABILITY_LIST_LIMIT)
            .await?;
        let observations = build_high_probability_observations(
            &config,
            candidates,
            quotes,
            &bucket_stats,
        );
        let mut report = HighProbabilityObserveReport {
            candidates_scanned: candidates.len(),
            ..HighProbabilityObserveReport::default()
        };
        for observation in observations {
            match observation.decision {
                HighProbabilityDecision::Allow => report.allow_count += 1,
                HighProbabilityDecision::Reject => report.reject_count += 1,
                HighProbabilityDecision::Skip => report.skip_count += 1,
            }
            if observation
                .reasons
                .iter()
                .any(|reason| reason == "orderbook_missing" || reason == "best_ask_missing")
            {
                report.missing_quote_count += 1;
            }
            if observation
                .reasons
                .iter()
                .any(|reason| reason == "trained_bucket_missing")
            {
                report.missing_bucket_count += 1;
            }
            self.store.record_observation(&observation).await?;
            report.observations_recorded += 1;
        }
        Ok(report)
    }

    /// Refresh fair value snapshots for the supplied candidates. Reads the
    /// persisted bucket stats once, blends them with the current orderbook, and
    /// upserts one estimate per condition. Returns a zero report when the
    /// provider is disabled. Never places orders or calls the live connector.
    pub async fn refresh_fair_values(
        &self,
        candidates: &[HighProbabilityObserveCandidate],
        quotes: &[HighProbabilityOrderbookQuote],
    ) -> Result<FairValueRefreshReport> {
        let config = self.read_config().await?;
        if !config.fair_value_enabled {
            return Ok(FairValueRefreshReport::default());
        }
        let bucket_stats = self
            .store
            .list_bucket_stats(Some(&config.model_version), MAX_HIGH_PROBABILITY_LIST_LIMIT)
            .await?;
        let quote_by_token = quotes
            .iter()
            .cloned()
            .map(HighProbabilityOrderbookQuote::normalized)
            .map(|quote| (quote.token_id.clone(), quote))
            .collect::<HashMap<_, _>>();
        let mut inputs_by_condition: BTreeMap<String, Vec<FairValuePricingInput>> = BTreeMap::new();
        let mut missing_quote_count = 0usize;
        let mut out_of_range_count = 0usize;
        for candidate in candidates
            .iter()
            .cloned()
            .map(HighProbabilityObserveCandidate::normalized)
        {
            let Some(quote) = quote_by_token.get(&candidate.token_id) else {
                missing_quote_count += 1;
                continue;
            };
            let input = fair_value_pricing_input_from_candidate(&candidate, quote);
            inputs_by_condition
                .entry(candidate.condition_id.clone())
                .or_default()
                .push(input);
        }
        for inputs in inputs_by_condition.values() {
            let has_in_range = inputs
                .iter()
                .any(|input| high_probability_price_bucket(input.executable_price()).is_some());
            if !has_in_range {
                out_of_range_count += 1;
            }
        }

        let conditions_scanned = inputs_by_condition.len();
        let now = OffsetDateTime::now_utc();
        let estimates = build_fair_value_estimates(&config, &inputs_by_condition, &bucket_stats, now);
        let mut live_eligible_count = 0usize;
        for estimate in &estimates {
            if estimate.live_eligible {
                live_eligible_count += 1;
            }
            self.store
                .record_fair_value(&estimate.clone().normalized())
                .await?;
        }
        let unavailable_count = conditions_scanned.saturating_sub(estimates.len());
        let missing_bucket_count = unavailable_count.saturating_sub(out_of_range_count);

        Ok(FairValueRefreshReport {
            conditions_scanned,
            estimates_computed: estimates.len(),
            live_eligible_count,
            unavailable_count,
            missing_bucket_count,
            missing_quote_count,
        })
    }

    pub async fn list_fair_values(
        &self,
        limit: Option<u16>,
    ) -> Result<Vec<FairValueEstimate>> {
        let config = self.read_config().await?;
        self.store
            .list_fair_values(
                Some(&config.model_version),
                false,
                validate_high_probability_list_limit(limit),
            )
            .await
    }
}

fn fair_value_pricing_input_from_candidate(
    candidate: &HighProbabilityObserveCandidate,
    quote: &HighProbabilityOrderbookQuote,
) -> FairValuePricingInput {
    FairValuePricingInput {
        condition_id: candidate.condition_id.clone(),
        token_id: candidate.token_id.clone(),
        outcome: candidate.outcome.clone(),
        reference_price: candidate.reference_price,
        reference_spread_cents: candidate.reference_spread_cents,
        best_bid: quote.best_bid,
        best_ask: quote.best_ask,
        ask_depth_usd: quote.ask_depth_usd,
        market_type: candidate.market_type.clone(),
        liquidity_usd: candidate.liquidity_usd,
        end_at: candidate.end_at,
        observed_at: candidate.observed_at,
        risk_tags: candidate.risk_tags.clone(),
        confirmed_at_ms: quote.confirmed_at_ms,
    }
    .normalized()
}

fn build_high_probability_observations(
    config: &HighProbabilityConfig,
    candidates: &[HighProbabilityObserveCandidate],
    quotes: &[HighProbabilityOrderbookQuote],
    bucket_stats: &[HighProbabilityBucketStats],
) -> Vec<HighProbabilityObservation> {
    let config = config.clone().normalized();
    let now = OffsetDateTime::now_utc();
    let quote_by_token = quotes
        .iter()
        .cloned()
        .map(HighProbabilityOrderbookQuote::normalized)
        .map(|quote| (quote.token_id.clone(), quote))
        .collect::<HashMap<_, _>>();
    let bucket_by_key = bucket_stats
        .iter()
        .map(|bucket| (bucket.bucket_key.clone(), bucket))
        .collect::<BTreeMap<_, _>>();
    let excluded_tags = config
        .excluded_risk_tags
        .iter()
        .cloned()
        .collect::<HashSet<_>>();

    candidates
        .iter()
        .cloned()
        .map(HighProbabilityObserveCandidate::normalized)
        .map(|candidate| {
            build_high_probability_observation(
                &config,
                &candidate,
                quote_by_token.get(&candidate.token_id),
                &bucket_by_key,
                &excluded_tags,
                now,
            )
        })
        .collect()
}

fn build_high_probability_observation(
    config: &HighProbabilityConfig,
    candidate: &HighProbabilityObserveCandidate,
    quote: Option<&HighProbabilityOrderbookQuote>,
    bucket_by_key: &BTreeMap<String, &HighProbabilityBucketStats>,
    excluded_tags: &HashSet<String>,
    now: OffsetDateTime,
) -> HighProbabilityObservation {
    let mut reasons = Vec::new();
    let mut fair_probability = None;
    let mut net_edge = None;
    let mut recommended_size_usd = None;
    let mut decision = HighProbabilityDecision::Reject;
    let executable_price = quote
        .and_then(|quote| quote.best_ask)
        .unwrap_or(candidate.reference_price);

    let Some(quote) = quote else {
        reasons.push("orderbook_missing".to_string());
        return high_probability_observation_from_parts(
            config,
            candidate,
            executable_price,
            fair_probability,
            net_edge,
            recommended_size_usd,
            HighProbabilityDecision::Skip,
            reasons,
            now,
        );
    };
    let (sample, spread_cents) = match high_probability_sample_from_observe_candidate(candidate, quote, now)
    {
        Some(value) => value,
        None => {
            // The helper only fails on a missing book or an out-of-range price;
            // keep the distinct diagnostic reasons the page relies on.
            if quote.best_ask.is_none() {
                reasons.push("best_ask_missing".to_string());
            } else if quote.best_bid.is_none() {
                reasons.push("best_bid_missing".to_string());
            } else {
                reasons.push("price_out_of_research_range".to_string());
            }
            return high_probability_observation_from_parts(
                config,
                candidate,
                executable_price,
                fair_probability,
                net_edge,
                recommended_size_usd,
                HighProbabilityDecision::Skip,
                reasons,
                now,
            );
        }
    };
    let best_ask = quote.best_ask.unwrap_or(executable_price);
    let (bucket_key, _) = high_probability_bucket_key(&sample);
    let Some(bucket) = bucket_by_key.get(&bucket_key).copied() else {
        reasons.push("trained_bucket_missing".to_string());
        return high_probability_observation_from_parts(
            config,
            candidate,
            best_ask,
            fair_probability,
            net_edge,
            recommended_size_usd,
            HighProbabilityDecision::Skip,
            reasons,
            now,
        );
    };

    fair_probability = Some(bucket.fair_probability);
    let computed_net_edge =
        bucket.fair_probability - best_ask - config.fee_buffer - config.default_risk_margin;
    net_edge = Some(computed_net_edge);

    for tag in &candidate.risk_tags {
        if excluded_tags.contains(tag) {
            reasons.push(format!("excluded_risk_tag:{tag}"));
        }
    }
    if bucket.fair_probability < config.min_confidence {
        reasons.push("fair_probability_below_min_confidence".to_string());
    }
    if computed_net_edge < config.min_required_edge {
        reasons.push("net_edge_below_required".to_string());
    }
    if bucket
        .recommended_max_entry_price
        .is_some_and(|price| best_ask > price)
    {
        reasons.push("price_above_recommended_entry".to_string());
    }
    if spread_cents > config.max_spread_cents {
        reasons.push("spread_too_wide".to_string());
    }
    if quote
        .ask_depth_usd
        .is_some_and(|depth| depth < config.min_depth_usd)
    {
        reasons.push("ask_depth_below_minimum".to_string());
    }
    if quote.ask_depth_usd.is_none() {
        reasons.push("ask_depth_missing".to_string());
    }

    if reasons.is_empty() {
        decision = HighProbabilityDecision::Allow;
        recommended_size_usd = high_probability_recommended_size(config, best_ask, bucket.fair_probability)
            .filter(|size| *size > Decimal::ZERO);
        reasons.push("edge_gate_passed".to_string());
    }

    high_probability_observation_from_parts(
        config,
        candidate,
        best_ask,
        fair_probability,
        net_edge,
        recommended_size_usd,
        decision,
        reasons,
        now,
    )
}

fn high_probability_recommended_size(
    config: &HighProbabilityConfig,
    executable_price: Decimal,
    fair_probability: Decimal,
) -> Option<Decimal> {
    if executable_price >= Decimal::ONE || fair_probability <= executable_price {
        return None;
    }
    let kelly_fraction = (fair_probability - executable_price) / (Decimal::ONE - executable_price);
    let discounted = kelly_fraction * config.conservative_kelly_multiplier;
    Some((config.max_single_trade_usd * discounted).min(config.max_single_trade_usd))
}

fn high_probability_observation_from_parts(
    config: &HighProbabilityConfig,
    candidate: &HighProbabilityObserveCandidate,
    executable_price: Decimal,
    fair_probability: Option<Decimal>,
    net_edge: Option<Decimal>,
    recommended_size_usd: Option<Decimal>,
    decision: HighProbabilityDecision,
    reasons: Vec<String>,
    now: OffsetDateTime,
) -> HighProbabilityObservation {
    HighProbabilityObservation {
        id: 0,
        observed_at: now,
        condition_id: candidate.condition_id.clone(),
        token_id: candidate.token_id.clone(),
        mode: config.mode,
        executable_price,
        fair_probability,
        net_edge,
        recommended_size_usd,
        decision,
        reasons,
        model_version: Some(config.model_version.clone()),
        created_at: now,
    }
}

fn build_high_probability_research_report(
    config: &HighProbabilityConfig,
    samples: &[HighProbabilitySample],
    bucket_stats: &[HighProbabilityBucketStats],
    sample_limit: u16,
) -> HighProbabilityResearchReport {
    let mut win_samples = 0usize;
    let mut loss_samples = 0usize;
    let mut voided_samples = 0usize;
    let mut unknown_samples = 0usize;
    for sample in samples {
        match sample.outcome {
            HighProbabilitySampleOutcome::Win => win_samples += 1,
            HighProbabilitySampleOutcome::Loss => loss_samples += 1,
            HighProbabilitySampleOutcome::Voided => voided_samples += 1,
            HighProbabilitySampleOutcome::Unknown => unknown_samples += 1,
        }
    }

    let settled_samples = win_samples + loss_samples;
    let qualified_bucket_count = bucket_stats
        .iter()
        .filter(|bucket| {
            bucket.sample_count >= config.min_bucket_samples
                && bucket.fair_probability >= config.min_confidence
                && bucket
                    .recommended_max_entry_price
                    .is_some_and(|price| price > Decimal::ZERO)
        })
        .count();
    let positive_expected_pnl_bucket_count = bucket_stats
        .iter()
        .filter(|bucket| {
            bucket
                .expected_pnl
                .is_some_and(|expected| expected > Decimal::ZERO)
        })
        .count();
    let best_bucket = bucket_stats
        .iter()
        .filter(|bucket| bucket.expected_pnl.is_some())
        .max_by(|left, right| compare_optional_decimal(left.expected_pnl, right.expected_pnl))
        .cloned();
    let worst_bucket = bucket_stats
        .iter()
        .filter(|bucket| bucket.expected_pnl.is_some())
        .min_by(|left, right| compare_optional_decimal(left.expected_pnl, right.expected_pnl))
        .cloned();

    HighProbabilityResearchReport {
        generated_at: OffsetDateTime::now_utc(),
        model_version: config.model_version.clone(),
        market_scope: config.market_scope.clone(),
        sample_limit,
        samples_scanned: samples.len(),
        settled_samples,
        win_samples,
        loss_samples,
        voided_samples,
        unknown_samples,
        bucket_count: bucket_stats.len(),
        qualified_bucket_count,
        positive_expected_pnl_bucket_count,
        weighted_win_rate: weighted_bucket_decimal(bucket_stats, |bucket| Some(bucket.win_rate)),
        weighted_expected_pnl: weighted_bucket_decimal(bucket_stats, |bucket| bucket.expected_pnl),
        weighted_break_70_rate: weighted_bucket_decimal(bucket_stats, |bucket| {
            bucket.break_70_rate
        }),
        best_bucket,
        worst_bucket,
        notes: high_probability_report_notes(samples, bucket_stats, sample_limit),
    }
}

fn high_probability_report_notes(
    samples: &[HighProbabilitySample],
    bucket_stats: &[HighProbabilityBucketStats],
    sample_limit: u16,
) -> Vec<String> {
    let mut notes = Vec::new();
    if samples.is_empty() {
        notes.push("no_samples_built".to_string());
    }
    if !samples.is_empty() && bucket_stats.is_empty() {
        notes.push("no_qualified_buckets".to_string());
    }
    if samples.len() >= usize::from(sample_limit) {
        notes.push("sample_query_limited".to_string());
    }
    if samples
        .iter()
        .any(|sample| !sample.outcome.is_settled_for_stats())
    {
        notes.push("contains_unsettled_or_voided_samples".to_string());
    }
    notes
}

fn weighted_bucket_decimal<F>(
    bucket_stats: &[HighProbabilityBucketStats],
    value: F,
) -> Option<Decimal>
where
    F: Fn(&HighProbabilityBucketStats) -> Option<Decimal>,
{
    let mut weighted_sum = Decimal::ZERO;
    let mut weight_sum = 0u64;
    for bucket in bucket_stats {
        let Some(value) = value(bucket) else {
            continue;
        };
        weighted_sum += value * Decimal::from(bucket.sample_count);
        weight_sum += bucket.sample_count;
    }
    (weight_sum > 0).then(|| weighted_sum / Decimal::from(weight_sum))
}

fn compare_optional_decimal(left: Option<Decimal>, right: Option<Decimal>) -> std::cmp::Ordering {
    left.unwrap_or(Decimal::ZERO)
        .cmp(&right.unwrap_or(Decimal::ZERO))
}

fn decimal_from_usize(value: usize) -> Decimal {
    Decimal::from(u64::try_from(value).unwrap_or(u64::MAX))
}

#[derive(Debug)]
struct ExitRuleAccumulator {
    rule_key: &'static str,
    trade_count: usize,
    win_trades: usize,
    total_pnl: Decimal,
    total_entry_cost: Decimal,
    cumulative_pnl: Decimal,
    peak_pnl: Decimal,
    max_drawdown: Decimal,
    missing_path_features: usize,
}

impl ExitRuleAccumulator {
    fn new(rule_key: &'static str) -> Self {
        Self {
            rule_key,
            trade_count: 0,
            win_trades: 0,
            total_pnl: Decimal::ZERO,
            total_entry_cost: Decimal::ZERO,
            cumulative_pnl: Decimal::ZERO,
            peak_pnl: Decimal::ZERO,
            max_drawdown: Decimal::ZERO,
            missing_path_features: 0,
        }
    }

    fn record(
        &mut self,
        sample: &HighProbabilitySample,
        pnl: Decimal,
        path_feature_missing: bool,
    ) {
        self.trade_count += 1;
        if pnl > Decimal::ZERO {
            self.win_trades += 1;
        }
        self.total_pnl += pnl;
        self.total_entry_cost += sample.executable_price;
        self.cumulative_pnl += pnl;
        self.peak_pnl = self.peak_pnl.max(self.cumulative_pnl);
        self.max_drawdown = self.max_drawdown.max(self.peak_pnl - self.cumulative_pnl);
        if path_feature_missing {
            self.missing_path_features += 1;
        }
    }

    fn finish(self) -> HighProbabilityBacktestExitRuleReport {
        let mut notes = Vec::new();
        if self.missing_path_features > 0 {
            notes.push("missing_path_features_fallback_to_settlement".to_string());
        }
        HighProbabilityBacktestExitRuleReport {
            rule_key: self.rule_key.to_string(),
            trade_count: self.trade_count,
            win_rate: (self.trade_count > 0)
                .then(|| decimal_from_usize(self.win_trades) / decimal_from_usize(self.trade_count)),
            total_pnl: self.total_pnl,
            average_pnl: (self.trade_count > 0)
                .then(|| self.total_pnl / decimal_from_usize(self.trade_count)),
            total_entry_cost: self.total_entry_cost,
            roi: (self.total_entry_cost > Decimal::ZERO).then(|| self.total_pnl / self.total_entry_cost),
            max_drawdown: self.max_drawdown,
            notes,
        }
    }
}

#[cfg(test)]
fn build_high_probability_backtest_report(
    config: &HighProbabilityConfig,
    samples: &[HighProbabilitySample],
    sample_limit: u16,
) -> HighProbabilityBacktestReport {
    build_high_probability_backtest_result(config, samples, sample_limit)
        .run
        .report
}

fn build_high_probability_backtest_result(
    config: &HighProbabilityConfig,
    samples: &[HighProbabilitySample],
    sample_limit: u16,
) -> HighProbabilityBacktestResult {
    let config = config.clone().normalized();
    let generated_at = OffsetDateTime::now_utc();
    let mut settled_samples = samples
        .iter()
        .filter(|sample| sample.is_settled_for_stats())
        .cloned()
        .map(HighProbabilitySample::normalized)
        .collect::<Vec<_>>();
    settled_samples.sort_by(|left, right| left.sampled_at.cmp(&right.sampled_at));

    let split_index = walk_forward_split_index(settled_samples.len());
    let (train_samples, test_samples) = settled_samples.split_at(split_index);
    let train_bucket_stats = build_high_probability_bucket_stats(&config, train_samples);
    let bucket_by_key = train_bucket_stats
        .iter()
        .map(|bucket| (bucket.bucket_key.clone(), bucket))
        .collect::<BTreeMap<_, _>>();

    let mut skipped_no_bucket_count = 0usize;
    let mut skipped_no_edge_count = 0usize;
    let mut trade_count = 0usize;
    let mut win_trades = 0usize;
    let mut loss_trades = 0usize;
    let mut total_pnl = Decimal::ZERO;
    let mut total_entry_cost = Decimal::ZERO;
    let mut cumulative_pnl = Decimal::ZERO;
    let mut peak_pnl = Decimal::ZERO;
    let mut max_drawdown = Decimal::ZERO;
    let mut trades = Vec::new();
    let mut exit_rules = [
        ExitRuleAccumulator::new("settlement"),
        ExitRuleAccumulator::new("take_profit_90"),
        ExitRuleAccumulator::new("take_profit_95"),
        ExitRuleAccumulator::new("stop_loss_70"),
        ExitRuleAccumulator::new("stop_loss_60"),
    ];

    for sample in test_samples {
        let (bucket_key, _) = high_probability_bucket_key(sample);
        let Some(bucket) = bucket_by_key.get(&bucket_key) else {
            skipped_no_bucket_count += 1;
            continue;
        };
        let net_edge = bucket.fair_probability
            - sample.executable_price
            - config.fee_buffer
            - config.default_risk_margin;
        let entry_allowed = net_edge >= config.min_required_edge
            && bucket.fair_probability >= config.min_confidence
            && bucket
                .recommended_max_entry_price
                .is_some_and(|price| sample.executable_price <= price);
        if !entry_allowed {
            skipped_no_edge_count += 1;
            continue;
        }

        let Some(pnl) = sample.settlement_pnl else {
            skipped_no_edge_count += 1;
            continue;
        };
        trade_count += 1;
        if sample.outcome == HighProbabilitySampleOutcome::Win {
            win_trades += 1;
        } else {
            loss_trades += 1;
        }
        total_pnl += pnl;
        total_entry_cost += sample.executable_price;
        cumulative_pnl += pnl;
        peak_pnl = peak_pnl.max(cumulative_pnl);
        let drawdown = peak_pnl - cumulative_pnl;
        max_drawdown = max_drawdown.max(drawdown);
        for rule in &mut exit_rules {
            let (rule_pnl, path_feature_missing) = exit_rule_pnl(rule.rule_key, sample, pnl);
            rule.record(sample, rule_pnl, path_feature_missing);
        }
        trades.push(HighProbabilityBacktestTrade {
            id: 0,
            run_id: 0,
            sample_id: sample.id,
            condition_id: sample.condition_id.clone(),
            token_id: sample.token_id.clone(),
            sampled_at: sample.sampled_at,
            bucket_key,
            executable_price: sample.executable_price,
            fair_probability: bucket.fair_probability,
            net_edge,
            recommended_max_entry_price: bucket.recommended_max_entry_price,
            outcome: sample.outcome,
            settlement_pnl: pnl,
            cumulative_pnl,
            drawdown,
            reasons: Vec::new(),
            created_at: generated_at,
        });
    }

    let report = HighProbabilityBacktestReport {
        generated_at,
        model_version: config.model_version.clone(),
        market_scope: config.market_scope.clone(),
        sample_limit,
        train_sample_count: train_samples.len(),
        test_sample_count: test_samples.len(),
        candidate_count: test_samples.len(),
        trade_count,
        skipped_no_bucket_count,
        skipped_no_edge_count,
        win_trades,
        loss_trades,
        win_rate: (trade_count > 0)
            .then(|| decimal_from_usize(win_trades) / decimal_from_usize(trade_count)),
        total_pnl,
        average_pnl: (trade_count > 0).then(|| total_pnl / decimal_from_usize(trade_count)),
        total_entry_cost,
        roi: (total_entry_cost > Decimal::ZERO).then(|| total_pnl / total_entry_cost),
        max_drawdown,
        average_entry_price: (trade_count > 0)
            .then(|| total_entry_cost / decimal_from_usize(trade_count)),
        train_start_at: train_samples.first().map(|sample| sample.sampled_at),
        train_end_at: train_samples.last().map(|sample| sample.sampled_at),
        test_start_at: test_samples.first().map(|sample| sample.sampled_at),
        test_end_at: test_samples.last().map(|sample| sample.sampled_at),
        exit_rule_reports: exit_rules
            .into_iter()
            .map(ExitRuleAccumulator::finish)
            .collect(),
        notes: high_probability_backtest_notes(
            samples,
            &settled_samples,
            test_samples,
            trade_count,
            sample_limit,
        ),
    };
    HighProbabilityBacktestResult {
        run: HighProbabilityBacktestRun {
            id: 0,
            run_at: generated_at,
            report,
        },
        trades,
        config,
    }
}

fn exit_rule_pnl(
    rule_key: &str,
    sample: &HighProbabilitySample,
    settlement_pnl: Decimal,
) -> (Decimal, bool) {
    match rule_key {
        "take_profit_90" => take_profit_rule_pnl(sample, settlement_pnl, Decimal::new(90, 2)),
        "take_profit_95" => take_profit_rule_pnl(sample, settlement_pnl, Decimal::new(95, 2)),
        "stop_loss_70" => stop_loss_rule_pnl(sample, settlement_pnl, Decimal::new(70, 2)),
        "stop_loss_60" => stop_loss_rule_pnl(sample, settlement_pnl, Decimal::new(60, 2)),
        _ => (settlement_pnl, false),
    }
}

fn take_profit_rule_pnl(
    sample: &HighProbabilitySample,
    settlement_pnl: Decimal,
    target_price: Decimal,
) -> (Decimal, bool) {
    let Some(max_future_close) = decimal_path_feature(sample, "max_future_close") else {
        return (settlement_pnl, true);
    };
    if max_future_close >= target_price && target_price > sample.executable_price {
        (target_price - sample.executable_price, false)
    } else {
        (settlement_pnl, false)
    }
}

fn stop_loss_rule_pnl(
    sample: &HighProbabilitySample,
    settlement_pnl: Decimal,
    stop_price: Decimal,
) -> (Decimal, bool) {
    let Some(min_future_close) = decimal_path_feature(sample, "min_future_close") else {
        return (settlement_pnl, true);
    };
    if sample.executable_price > stop_price && min_future_close <= stop_price {
        (stop_price - sample.executable_price, false)
    } else {
        (settlement_pnl, false)
    }
}

fn decimal_path_feature(sample: &HighProbabilitySample, key: &str) -> Option<Decimal> {
    let value = sample.path_features.get(key)?;
    if let Some(value) = value.as_str() {
        return Decimal::from_str(value).ok();
    }
    if let Some(value) = value.as_i64() {
        return Some(Decimal::from(value));
    }
    if let Some(value) = value.as_u64() {
        return Some(Decimal::from(value));
    }
    value
        .as_f64()
        .and_then(|value| Decimal::from_str(&value.to_string()).ok())
}

fn walk_forward_split_index(sample_count: usize) -> usize {
    if sample_count < 2 {
        return sample_count;
    }
    ((sample_count * 7) / 10).clamp(1, sample_count - 1)
}

fn high_probability_backtest_notes(
    samples: &[HighProbabilitySample],
    settled_samples: &[HighProbabilitySample],
    test_samples: &[HighProbabilitySample],
    trade_count: usize,
    sample_limit: u16,
) -> Vec<String> {
    let mut notes = Vec::new();
    if settled_samples.len() < 2 {
        notes.push("insufficient_settled_samples".to_string());
    }
    if !test_samples.is_empty() && trade_count == 0 {
        notes.push("no_backtest_trades".to_string());
    }
    if samples.len() >= usize::from(sample_limit) {
        notes.push("sample_query_limited".to_string());
    }
    notes
}
