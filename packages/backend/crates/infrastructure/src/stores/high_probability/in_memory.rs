pub struct InMemoryHighProbabilityStore {
    config: RwLock<HighProbabilityConfig>,
    market_outcomes: RwLock<Vec<HighProbabilityMarketOutcome>>,
    reward_candle_inputs: RwLock<Vec<HighProbabilityRewardCandleSampleInput>>,
    samples: RwLock<Vec<HighProbabilitySample>>,
    bucket_stats: RwLock<Vec<HighProbabilityBucketStats>>,
    backtest_runs: RwLock<Vec<HighProbabilityBacktestRun>>,
    backtest_trades: RwLock<Vec<HighProbabilityBacktestTrade>>,
    observations: RwLock<Vec<HighProbabilityObservation>>,
    fair_values: RwLock<Vec<FairValueEstimate>>,
}

impl InMemoryHighProbabilityStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: RwLock::new(HighProbabilityConfig::default()),
            market_outcomes: RwLock::new(Vec::new()),
            reward_candle_inputs: RwLock::new(Vec::new()),
            samples: RwLock::new(Vec::new()),
            bucket_stats: RwLock::new(Vec::new()),
            backtest_runs: RwLock::new(Vec::new()),
            backtest_trades: RwLock::new(Vec::new()),
            observations: RwLock::new(Vec::new()),
            fair_values: RwLock::new(Vec::new()),
        }
    }
}

#[async_trait]
impl HighProbabilityStore for InMemoryHighProbabilityStore {
    async fn load_config(&self) -> Result<HighProbabilityConfig> {
        Ok(self.config.read().await.clone().normalized())
    }

    async fn save_config(&self, config: &HighProbabilityConfig) -> Result<()> {
        *self.config.write().await = config.clone().normalized();
        Ok(())
    }

    async fn record_samples(&self, samples: &[HighProbabilitySample]) -> Result<usize> {
        let mut store = self.samples.write().await;
        let existing_keys: HashSet<(String, String, OffsetDateTime, String, String)> = store
            .iter()
            .map(|sample| {
                (
                    sample.condition_id.clone(),
                    sample.token_id.clone(),
                    sample.sampled_at,
                    sample.trigger_kind.as_str().to_string(),
                    sample.price_bucket.clone(),
                )
            })
            .collect();
        let mut inserted = 0usize;
        for sample in samples {
            let key = (
                sample.condition_id.clone(),
                sample.token_id.clone(),
                sample.sampled_at,
                sample.trigger_kind.as_str().to_string(),
                sample.price_bucket.clone(),
            );
            if existing_keys.contains(&key) {
                continue;
            }
            let mut sample = sample.clone().normalized();
            sample.id = i64::try_from(store.len() + 1).unwrap_or(i64::MAX);
            store.push(sample);
            inserted += 1;
        }
        store.sort_by(|left, right| right.sampled_at.cmp(&left.sampled_at));
        Ok(inserted)
    }

    async fn upsert_market_outcome(&self, outcome: &HighProbabilityMarketOutcome) -> Result<()> {
        let mut outcomes = self.market_outcomes.write().await;
        let outcome = outcome.clone().normalized();
        if let Some(existing) = outcomes
            .iter_mut()
            .find(|existing| existing.condition_id == outcome.condition_id)
        {
            *existing = outcome;
        } else {
            outcomes.push(outcome);
        }
        Ok(())
    }

    async fn list_reward_candle_sample_inputs(
        &self,
        limit: u32,
    ) -> Result<Vec<HighProbabilityRewardCandleSampleInput>> {
        let mut inputs = self.reward_candle_inputs.read().await.clone();
        inputs.sort_by(|left, right| left.bucket_start.cmp(&right.bucket_start));
        inputs.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
        Ok(inputs)
    }

    async fn list_observe_candidates(
        &self,
        limit: u16,
    ) -> Result<Vec<HighProbabilityObserveCandidate>> {
        let mut latest_by_token = BTreeMap::<String, HighProbabilityRewardCandleSampleInput>::new();
        for input in self.reward_candle_inputs.read().await.iter().cloned() {
            let input = input.normalized();
            if !matches!(input.outcome_status, HighProbabilityMarketOutcomeStatus::Unresolved) {
                continue;
            }
            latest_by_token
                .entry(input.token_id.clone())
                .and_modify(|existing| {
                    if input.bucket_start > existing.bucket_start {
                        *existing = input.clone();
                    }
                })
                .or_insert(input);
        }
        let mut candidates = latest_by_token
            .into_values()
            .map(|input| HighProbabilityObserveCandidate {
                condition_id: input.condition_id,
                token_id: input.token_id,
                outcome: input.outcome,
                observed_at: input.bucket_start,
                reference_price: input.close,
                reference_spread_cents: input.spread_cents_close,
                market_type: input.market_type,
                liquidity_usd: input.liquidity_usd,
                end_at: input.resolved_at,
                risk_tags: input.risk_tags,
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| right.observed_at.cmp(&left.observed_at));
        candidates.truncate(usize::from(limit));
        Ok(candidates)
    }

    async fn list_samples(
        &self,
        query: HighProbabilitySampleQuery,
    ) -> Result<Vec<HighProbabilitySample>> {
        let mut samples = self.samples.read().await.clone();
        if let Some(outcome) = query.outcome {
            samples.retain(|sample| sample.outcome == outcome);
        }
        if let Some(market_type) = query.market_type {
            samples.retain(|sample| sample.market_type == market_type);
        }
        samples.sort_by(|left, right| right.sampled_at.cmp(&left.sampled_at));
        samples.truncate(usize::from(query.limit));
        Ok(samples)
    }

    async fn replace_bucket_stats(
        &self,
        model_version: &str,
        stats: &[HighProbabilityBucketStats],
    ) -> Result<usize> {
        let mut store = self.bucket_stats.write().await;
        store.retain(|stats| stats.model_version != model_version);
        for stats in stats {
            let mut stats = stats.clone();
            stats.id = i64::try_from(store.len() + 1).unwrap_or(i64::MAX);
            store.push(stats);
        }
        store.sort_by(|left, right| right.computed_at.cmp(&left.computed_at));
        Ok(stats.len())
    }

    async fn list_bucket_stats(
        &self,
        model_version: Option<&str>,
        limit: u16,
    ) -> Result<Vec<HighProbabilityBucketStats>> {
        let mut stats = self.bucket_stats.read().await.clone();
        if let Some(model_version) = model_version {
            stats.retain(|stats| stats.model_version == model_version);
        }
        stats.sort_by(|left, right| right.sample_count.cmp(&left.sample_count));
        stats.truncate(usize::from(limit));
        Ok(stats)
    }

    async fn record_backtest_result(
        &self,
        result: &HighProbabilityBacktestResult,
    ) -> Result<HighProbabilityBacktestPersistReport> {
        let mut runs = self.backtest_runs.write().await;
        let mut trades = self.backtest_trades.write().await;
        let run_id = i64::try_from(runs.len() + 1).unwrap_or(i64::MAX);
        let mut run = result.run.clone();
        run.id = run_id;
        runs.push(run);
        runs.sort_by(|left, right| right.run_at.cmp(&left.run_at));

        let mut trades_saved = 0usize;
        for trade in &result.trades {
            let mut trade = trade.clone();
            trade.id = i64::try_from(trades.len() + 1).unwrap_or(i64::MAX);
            trade.run_id = run_id;
            trades.push(trade);
            trades_saved += 1;
        }
        trades.sort_by(|left, right| {
            left.run_id
                .cmp(&right.run_id)
                .then(left.sampled_at.cmp(&right.sampled_at))
                .then(left.id.cmp(&right.id))
        });

        Ok(HighProbabilityBacktestPersistReport {
            run_id,
            trades_saved,
        })
    }

    async fn list_backtest_runs(&self, limit: u16) -> Result<Vec<HighProbabilityBacktestRun>> {
        let mut runs = self.backtest_runs.read().await.clone();
        runs.sort_by(|left, right| right.run_at.cmp(&left.run_at));
        runs.truncate(usize::from(limit));
        Ok(runs)
    }

    async fn list_backtest_trades(
        &self,
        run_id: i64,
        limit: u16,
    ) -> Result<Vec<HighProbabilityBacktestTrade>> {
        let mut trades = self.backtest_trades.read().await.clone();
        trades.retain(|trade| trade.run_id == run_id);
        trades.sort_by(|left, right| left.sampled_at.cmp(&right.sampled_at));
        trades.truncate(usize::from(limit));
        Ok(trades)
    }

    async fn record_observation(&self, observation: &HighProbabilityObservation) -> Result<()> {
        let mut observations = self.observations.write().await;
        let mut observation = observation.clone();
        observation.id = i64::try_from(observations.len() + 1).unwrap_or(i64::MAX);
        observations.push(observation);
        observations.sort_by(|left, right| right.observed_at.cmp(&left.observed_at));
        observations.truncate(10_000);
        Ok(())
    }

    async fn list_observations(&self, limit: u16) -> Result<Vec<HighProbabilityObservation>> {
        let mut observations = self.observations.read().await.clone();
        observations.sort_by(|left, right| right.observed_at.cmp(&left.observed_at));
        observations.truncate(usize::from(limit));
        Ok(observations)
    }

    async fn record_fair_value(&self, estimate: &FairValueEstimate) -> Result<()> {
        let mut store = self.fair_values.write().await;
        let estimate = estimate.clone().normalized();
        if let Some(existing) = store
            .iter_mut()
            .find(|existing| {
                existing.condition_id == estimate.condition_id
                    && existing.model_version == estimate.model_version
            })
        {
            let id = existing.id;
            *existing = estimate;
            existing.id = id;
        } else {
            let mut estimate = estimate;
            estimate.id = i64::try_from(store.len() + 1).unwrap_or(i64::MAX);
            store.push(estimate);
        }
        store.sort_by(|left, right| {
            right
                .live_eligible
                .cmp(&left.live_eligible)
                .then(right.confidence.cmp(&left.confidence))
                .then(right.computed_at.cmp(&left.computed_at))
        });
        store.truncate(10_000);
        Ok(())
    }

    async fn list_fair_values(
        &self,
        model_version: Option<&str>,
        include_expired: bool,
        limit: u16,
    ) -> Result<Vec<FairValueEstimate>> {
        let mut estimates = self.fair_values.read().await.clone();
        if let Some(model_version) = model_version {
            estimates.retain(|estimate| estimate.model_version == model_version);
        }
        if !include_expired {
            let now = OffsetDateTime::now_utc();
            estimates.retain(|estimate| estimate.expires_at > now);
        }
        estimates.sort_by(|left, right| {
            right
                .live_eligible
                .cmp(&left.live_eligible)
                .then(right.confidence.cmp(&left.confidence))
                .then(right.computed_at.cmp(&left.computed_at))
        });
        estimates.truncate(usize::from(limit));
        Ok(estimates)
    }
}
