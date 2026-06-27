pub struct InMemorySmartMoneyStore {
    config: RwLock<SmartMoneyConfig>,
    candidates: RwLock<Vec<SmartWalletCandidate>>,
    profiles: RwLock<Vec<SmartWalletProfile>>,
    scores: RwLock<Vec<SmartWalletScore>>,
    trades: RwLock<Vec<SmartWalletTrade>>,
    signals: RwLock<Vec<SmartSignal>>,
    decisions: RwLock<Vec<SmartSignalDecision>>,
    signal_advisories: RwLock<Vec<SmartSignalAdvisory>>,
}

impl InMemorySmartMoneyStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: RwLock::new(SmartMoneyConfig::default()),
            candidates: RwLock::new(Vec::new()),
            profiles: RwLock::new(Vec::new()),
            scores: RwLock::new(Vec::new()),
            trades: RwLock::new(Vec::new()),
            signals: RwLock::new(Vec::new()),
            decisions: RwLock::new(Vec::new()),
            signal_advisories: RwLock::new(Vec::new()),
        }
    }
}

#[async_trait]
impl SmartMoneyStore for InMemorySmartMoneyStore {
    async fn load_config(&self) -> Result<SmartMoneyConfig> {
        Ok(self.config.read().await.clone().normalized())
    }

    async fn save_config(&self, config: &SmartMoneyConfig) -> Result<()> {
        *self.config.write().await = config.clone().normalized();
        Ok(())
    }

    async fn upsert_candidate(&self, candidate: &SmartWalletCandidate) -> Result<()> {
        let mut candidates = self.candidates.write().await;
        if let Some(existing) = candidates.iter_mut().find(|existing| {
            existing.wallet_address == candidate.wallet_address && existing.source == candidate.source
        }) {
            existing.last_seen_at = candidate.last_seen_at;
            existing.last_analyzed_at = candidate.last_analyzed_at;
            existing.reason = candidate.reason.clone();
            existing.raw = candidate.raw.clone();
        } else {
            let mut candidate = candidate.clone();
            candidate.id = i64::try_from(candidates.len() + 1).unwrap_or(i64::MAX);
            candidates.push(candidate);
        }
        Ok(())
    }

    async fn update_candidate_status(
        &self,
        wallet_address: &str,
        source: Option<&str>,
        status: SmartWalletCandidateStatus,
        reason: Option<&str>,
        now: OffsetDateTime,
    ) -> Result<u64> {
        let mut candidates = self.candidates.write().await;
        let mut updated = 0u64;
        for candidate in candidates.iter_mut().filter(|candidate| {
            candidate.wallet_address == wallet_address
                && source.is_none_or(|source| candidate.source == source)
        }) {
            candidate.status = status;
            candidate.last_seen_at = now;
            if matches!(
                status,
                SmartWalletCandidateStatus::Watch | SmartWalletCandidateStatus::Tracked
            ) {
                candidate.promoted_at = Some(now);
            }
            if matches!(
                status,
                SmartWalletCandidateStatus::Blocked | SmartWalletCandidateStatus::Rejected
            ) {
                candidate.rejected_at = Some(now);
            }
            if let Some(reason) = reason {
                candidate.reason = Some(reason.to_string());
            }
            updated += 1;
        }
        Ok(updated)
    }

    async fn list_candidates(
        &self,
        status: Option<SmartWalletCandidateStatus>,
        limit: u16,
    ) -> Result<Vec<SmartWalletCandidate>> {
        let mut candidates = self.candidates.read().await.clone();
        if let Some(status) = status {
            candidates.retain(|candidate| candidate.status == status);
        }
        candidates.sort_by(|left, right| right.last_seen_at.cmp(&left.last_seen_at));
        candidates.truncate(usize::from(limit));
        Ok(candidates)
    }

    async fn upsert_profile(&self, profile: &SmartWalletProfile) -> Result<()> {
        let mut profiles = self.profiles.write().await;
        if let Some(existing) = profiles
            .iter_mut()
            .find(|existing| existing.wallet_address == profile.wallet_address)
        {
            *existing = profile.clone().normalized();
        } else {
            profiles.push(profile.clone().normalized());
        }
        Ok(())
    }

    async fn list_profiles(&self, limit: u16) -> Result<Vec<SmartWalletProfile>> {
        let mut profiles = self.profiles.read().await.clone();
        profiles.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        profiles.truncate(usize::from(limit));
        Ok(profiles)
    }

    async fn upsert_score(&self, score: &SmartWalletScore) -> Result<()> {
        let mut scores = self.scores.write().await;
        if let Some(existing) = scores
            .iter_mut()
            .find(|existing| existing.wallet_address == score.wallet_address)
        {
            *existing = score.clone();
        } else {
            scores.push(score.clone());
        }
        Ok(())
    }

    async fn list_scores(
        &self,
        tier: Option<SmartWalletTier>,
        limit: u16,
    ) -> Result<Vec<SmartWalletScore>> {
        let mut scores = self.scores.read().await.clone();
        if let Some(tier) = tier {
            scores.retain(|score| score.tier == tier);
        }
        scores.sort_by(|left, right| right.total_score.cmp(&left.total_score));
        scores.truncate(usize::from(limit));
        Ok(scores)
    }

    async fn record_trades(&self, trades: &[SmartWalletTrade]) -> Result<usize> {
        let mut store = self.trades.write().await;
        let existing_ids: HashSet<String> = store.iter().map(|trade| trade.id.clone()).collect();
        let mut inserted = 0usize;
        for trade in trades {
            if existing_ids.contains(&trade.id) {
                continue;
            }
            store.push(trade.clone());
            inserted += 1;
        }
        store.sort_by(|left, right| right.source_timestamp.cmp(&left.source_timestamp));
        store.truncate(10_000);
        Ok(inserted)
    }

    async fn list_trades(&self, limit: u16) -> Result<Vec<SmartWalletTrade>> {
        let mut trades = self.trades.read().await.clone();
        trades.sort_by(|left, right| right.source_timestamp.cmp(&left.source_timestamp));
        trades.truncate(usize::from(limit));
        Ok(trades)
    }

    async fn list_unprocessed_signal_trades(&self, limit: u16) -> Result<Vec<SmartWalletTrade>> {
        let signal_trade_ids = self
            .signals
            .read()
            .await
            .iter()
            .map(|signal| signal.source_trade_id.clone())
            .collect::<HashSet<_>>();
        let mut trades = self
            .trades
            .read()
            .await
            .iter()
            .filter(|trade| !signal_trade_ids.contains(&trade.id))
            .cloned()
            .collect::<Vec<_>>();
        trades.sort_by(|left, right| right.source_timestamp.cmp(&left.source_timestamp));
        trades.truncate(usize::from(limit));
        Ok(trades)
    }

    async fn record_signals(&self, signals: &[SmartSignal]) -> Result<usize> {
        let mut store = self.signals.write().await;
        let mut existing_source_trade_ids = store
            .iter()
            .map(|signal| signal.source_trade_id.clone())
            .collect::<HashSet<_>>();
        let mut inserted = 0usize;
        for signal in signals {
            if !existing_source_trade_ids.insert(signal.source_trade_id.clone()) {
                continue;
            }
            let mut signal = signal.clone();
            signal.id = i64::try_from(store.len() + 1).unwrap_or(i64::MAX);
            store.push(signal);
            inserted += 1;
        }
        store.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        store.truncate(10_000);
        Ok(inserted)
    }

    async fn list_signals_by_source_trade_ids(
        &self,
        source_trade_ids: &[String],
    ) -> Result<Vec<SmartSignal>> {
        if source_trade_ids.is_empty() {
            return Ok(Vec::new());
        }

        let source_trade_ids = source_trade_ids.iter().cloned().collect::<HashSet<_>>();
        let mut signals = self
            .signals
            .read()
            .await
            .iter()
            .filter(|signal| source_trade_ids.contains(&signal.source_trade_id))
            .cloned()
            .collect::<Vec<_>>();
        signals.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(signals)
    }

    async fn list_signals(&self, limit: u16) -> Result<Vec<SmartSignal>> {
        let mut signals = self.signals.read().await.clone();
        signals.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        signals.truncate(usize::from(limit));
        Ok(signals)
    }

    async fn record_signal_decisions(&self, decisions: &[SmartSignalDecision]) -> Result<usize> {
        let mut store = self.decisions.write().await;
        let mut existing = store
            .iter()
            .map(|decision| (decision.signal_id, decision.stage.clone()))
            .collect::<HashSet<_>>();
        let mut inserted = 0usize;
        for decision in decisions {
            if !existing.insert((decision.signal_id, decision.stage.clone())) {
                continue;
            }
            let mut decision = decision.clone();
            decision.id = i64::try_from(store.len() + 1).unwrap_or(i64::MAX);
            store.push(decision);
            inserted += 1;
        }
        store.sort_by(|left, right| right.decided_at.cmp(&left.decided_at));
        store.truncate(10_000);
        Ok(inserted)
    }

    async fn list_signal_decisions(&self, limit: u16) -> Result<Vec<SmartSignalDecision>> {
        let mut decisions = self.decisions.read().await.clone();
        decisions.sort_by(|left, right| right.decided_at.cmp(&left.decided_at));
        decisions.truncate(usize::from(limit));
        Ok(decisions)
    }

    async fn latest_signal_advisory(
        &self,
        lookup: &SmartSignalAdvisoryLookup,
        now: OffsetDateTime,
    ) -> Result<Option<SmartSignalAdvisory>> {
        Ok(self
            .signal_advisories
            .read()
            .await
            .iter()
            .filter(|advisory| {
                advisory.signal_id == lookup.signal_id
                    && advisory.provider == lookup.provider
                    && advisory.request_format == lookup.request_format
                    && advisory.model == lookup.model
                    && advisory.input_hash == lookup.input_hash
                    && advisory.expires_at > now
            })
            .max_by(|left, right| {
                left.expires_at
                    .cmp(&right.expires_at)
                    .then_with(|| left.created_at.cmp(&right.created_at))
            })
            .cloned())
    }

    async fn save_signal_advisory(&self, advisory: &SmartSignalAdvisory) -> Result<()> {
        let mut store = self.signal_advisories.write().await;
        if let Some(existing) = store.iter_mut().find(|existing| {
            existing.signal_id == advisory.signal_id
                && existing.provider == advisory.provider
                && existing.request_format == advisory.request_format
                && existing.model == advisory.model
                && existing.input_hash == advisory.input_hash
        }) {
            let id = existing.id;
            *existing = SmartSignalAdvisory {
                id,
                ..advisory.clone()
            };
        } else {
            let mut advisory = advisory.clone();
            advisory.id = i64::try_from(store.len() + 1).unwrap_or(i64::MAX);
            store.push(advisory);
        }
        store.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        store.truncate(10_000);
        Ok(())
    }

    async fn list_signal_advisories(&self, limit: u16) -> Result<Vec<SmartSignalAdvisory>> {
        let mut advisories = self.signal_advisories.read().await.clone();
        advisories.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        advisories.truncate(usize::from(limit));
        Ok(advisories)
    }
}
