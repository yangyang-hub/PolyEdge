#[async_trait]
pub trait SmartMoneyStore: Send + Sync {
    async fn load_config(&self) -> Result<SmartMoneyConfig>;
    async fn save_config(&self, config: &SmartMoneyConfig) -> Result<()>;
    async fn upsert_candidate(&self, candidate: &SmartWalletCandidate) -> Result<()>;
    async fn update_candidate_status(
        &self,
        wallet_address: &str,
        source: Option<&str>,
        status: SmartWalletCandidateStatus,
        reason: Option<&str>,
        now: OffsetDateTime,
    ) -> Result<u64>;
    async fn list_candidates(
        &self,
        status: Option<SmartWalletCandidateStatus>,
        limit: u16,
    ) -> Result<Vec<SmartWalletCandidate>>;
    async fn upsert_profile(&self, profile: &SmartWalletProfile) -> Result<()>;
    async fn list_profiles(&self, limit: u16) -> Result<Vec<SmartWalletProfile>>;
    async fn upsert_score(&self, score: &SmartWalletScore) -> Result<()>;
    async fn list_scores(
        &self,
        tier: Option<SmartWalletTier>,
        limit: u16,
    ) -> Result<Vec<SmartWalletScore>>;
    async fn record_trades(&self, trades: &[SmartWalletTrade]) -> Result<usize>;
    async fn list_trades(&self, limit: u16) -> Result<Vec<SmartWalletTrade>>;
    async fn list_unprocessed_signal_trades(&self, limit: u16) -> Result<Vec<SmartWalletTrade>>;
    async fn record_signals(&self, signals: &[SmartSignal]) -> Result<usize>;
    async fn list_signals_by_source_trade_ids(
        &self,
        source_trade_ids: &[String],
    ) -> Result<Vec<SmartSignal>>;
    async fn list_signals(&self, limit: u16) -> Result<Vec<SmartSignal>>;
    async fn record_signal_decisions(&self, decisions: &[SmartSignalDecision]) -> Result<usize>;
    async fn list_signal_decisions(&self, limit: u16) -> Result<Vec<SmartSignalDecision>>;
    async fn latest_signal_advisory(
        &self,
        lookup: &SmartSignalAdvisoryLookup,
        now: OffsetDateTime,
    ) -> Result<Option<SmartSignalAdvisory>>;
    async fn save_signal_advisory(&self, advisory: &SmartSignalAdvisory) -> Result<()>;
    async fn list_signal_advisories(&self, limit: u16) -> Result<Vec<SmartSignalAdvisory>>;
}

#[derive(Clone)]
pub struct SmartMoneyService {
    store: Arc<dyn SmartMoneyStore>,
}

impl SmartMoneyService {
    #[must_use]
    pub fn new(store: Arc<dyn SmartMoneyStore>) -> Self {
        Self { store }
    }

    pub async fn read_config(&self) -> Result<SmartMoneyConfig> {
        self.store.load_config().await.map(SmartMoneyConfig::normalized)
    }

    pub async fn update_config(&self, patch: SmartMoneyConfigPatch) -> Result<SmartMoneyConfig> {
        let next = self.read_config().await?.apply_patch(patch);
        self.store.save_config(&next).await?;
        Ok(next)
    }

    pub async fn snapshot(&self) -> Result<SmartMoneySnapshot> {
        let config = self.read_config().await?;
        let candidates = self
            .store
            .list_candidates(None, DEFAULT_SMART_MONEY_LIST_LIMIT)
            .await?;
        let profiles = self
            .store
            .list_profiles(DEFAULT_SMART_MONEY_LIST_LIMIT)
            .await?;
        let scores = self
            .store
            .list_scores(None, DEFAULT_SMART_MONEY_LIST_LIMIT)
            .await?;
        let recent_trades = self
            .store
            .list_trades(DEFAULT_SMART_MONEY_LIST_LIMIT)
            .await?;
        let recent_signals = self
            .store
            .list_signals(DEFAULT_SMART_MONEY_LIST_LIMIT)
            .await?;
        let recent_decisions = self
            .store
            .list_signal_decisions(DEFAULT_SMART_MONEY_LIST_LIMIT)
            .await?;
        let recent_signal_advisories = self
            .store
            .list_signal_advisories(DEFAULT_SMART_MONEY_LIST_LIMIT)
            .await?;

        let watch_wallets = candidates
            .iter()
            .filter(|candidate| candidate.status == SmartWalletCandidateStatus::Watch)
            .count();
        let tracked_wallets = candidates
            .iter()
            .filter(|candidate| candidate.status == SmartWalletCandidateStatus::Tracked)
            .count();
        let blocked_wallets = candidates
            .iter()
            .filter(|candidate| candidate.status == SmartWalletCandidateStatus::Blocked)
            .count();
        let last_trade_at = recent_trades
            .iter()
            .map(|trade| trade.source_timestamp)
            .max();

        Ok(SmartMoneySnapshot {
            status: SmartMoneyStatus {
                enabled: config.enabled,
                mode: config.mode,
                candidates: candidates.len(),
                watch_wallets,
                tracked_wallets,
                blocked_wallets,
                profiles: profiles.len(),
                scored_wallets: scores.len(),
                recent_trades: recent_trades.len(),
                recent_signals: recent_signals.len(),
                recent_decisions: recent_decisions.len(),
                recent_signal_advisories: recent_signal_advisories.len(),
                last_trade_at,
            },
            config,
            candidates,
            profiles,
            scores,
            recent_trades,
            recent_signals,
            recent_decisions,
            recent_signal_advisories,
        })
    }

    pub async fn upsert_candidate(
        &self,
        wallet_address: &str,
        source: &str,
        reason: Option<String>,
        raw: Value,
    ) -> Result<SmartWalletCandidate> {
        let wallet_address = normalize_smart_wallet_address(wallet_address)?;
        let now = OffsetDateTime::now_utc();
        let candidate = SmartWalletCandidate {
            id: 0,
            wallet_address,
            source: source.trim().to_string(),
            status: SmartWalletCandidateStatus::Candidate,
            first_seen_at: now,
            last_seen_at: now,
            last_analyzed_at: None,
            promoted_at: None,
            rejected_at: None,
            reason,
            raw,
        };
        self.store.upsert_candidate(&candidate).await?;
        Ok(candidate)
    }

    pub async fn update_candidate_status(
        &self,
        update: SmartWalletCandidateStatusUpdate,
    ) -> Result<()> {
        let wallet_address = normalize_smart_wallet_address(&update.wallet_address)?;
        let source = update
            .source
            .as_deref()
            .map(str::trim)
            .filter(|source| !source.is_empty());
        let reason = update
            .reason
            .as_deref()
            .map(str::trim)
            .filter(|reason| !reason.is_empty());
        let now = OffsetDateTime::now_utc();
        let updated = self
            .store
            .update_candidate_status(&wallet_address, source, update.status, reason, now)
            .await?;
        if updated > 0 {
            return Ok(());
        }

        let candidate = SmartWalletCandidate {
            id: 0,
            wallet_address,
            source: source.unwrap_or("manual").to_string(),
            status: update.status,
            first_seen_at: now,
            last_seen_at: now,
            last_analyzed_at: None,
            promoted_at: promoted_at_for_status(update.status, now),
            rejected_at: rejected_at_for_status(update.status, now),
            reason: reason.map(ToString::to_string),
            raw: json!({
                "source": "manual",
                "status": update.status.as_str(),
                "reason": reason
            }),
        };
        self.store.upsert_candidate(&candidate).await
    }

    pub async fn save_profile_and_score(
        &self,
        profile: SmartWalletProfile,
    ) -> Result<SmartWalletScore> {
        let profile = SmartWalletProfile {
            wallet_address: normalize_smart_wallet_address(&profile.wallet_address)?,
            ..profile
        }
        .normalized();
        let config = self.read_config().await?;
        let score = build_smart_wallet_score(&config, &profile);
        self.store.upsert_profile(&profile).await?;
        self.store.upsert_score(&score).await?;
        Ok(score)
    }

    pub async fn record_trades(&self, trades: &[SmartWalletTrade]) -> Result<usize> {
        let normalized = trades
            .iter()
            .map(|trade| {
                Ok(SmartWalletTrade {
                    wallet_address: normalize_smart_wallet_address(&trade.wallet_address)?,
                    ..trade.clone()
                })
            })
            .collect::<Result<Vec<_>>>()?;
        self.store.record_trades(&normalized).await
    }

    pub async fn list_signal_candidate_trades(
        &self,
        limit: Option<u16>,
    ) -> Result<Vec<SmartWalletTrade>> {
        self.store
            .list_unprocessed_signal_trades(validate_smart_money_list_limit(limit))
            .await
    }

    pub async fn generate_signals_from_trades(
        &self,
        trades: &[SmartWalletTrade],
        quotes_by_token: &HashMap<String, SmartSignalBookQuote>,
    ) -> Result<SmartSignalGenerationReport> {
        let config = self.read_config().await?;
        let scores = self
            .store
            .list_scores(None, MAX_SMART_MONEY_LIST_LIMIT)
            .await?;
        let scores_by_wallet = scores
            .into_iter()
            .map(|score| (score.wallet_address.to_lowercase(), score.total_score))
            .collect::<HashMap<_, _>>();
        let now = OffsetDateTime::now_utc();
        let signals = trades
            .iter()
            .map(|trade| {
                let quote = trade
                    .token_id
                    .as_ref()
                    .and_then(|token_id| quotes_by_token.get(token_id));
                let wallet_score = scores_by_wallet.get(&trade.wallet_address.to_lowercase()).copied();
                build_smart_signal_from_trade(&config, trade, quote, wallet_score, now)
            })
            .collect::<Vec<_>>();
        let inserted = self.store.record_signals(&signals).await?;
        let source_trade_ids = signals
            .iter()
            .map(|signal| signal.source_trade_id.clone())
            .collect::<Vec<_>>();
        let persisted_signals = self
            .store
            .list_signals_by_source_trade_ids(&source_trade_ids)
            .await?;
        let decisions = persisted_signals
            .iter()
            .filter_map(|signal| build_smart_signal_decision_for_gate(signal, now))
            .collect::<Vec<_>>();
        let decisions_recorded = self.store.record_signal_decisions(&decisions).await?;
        let observe_signals = signals
            .iter()
            .filter(|signal| signal.status == SmartSignalStatus::Observe)
            .count();
        let rejected_signals = signals
            .iter()
            .filter(|signal| signal.status == SmartSignalStatus::Rejected)
            .count();

        Ok(SmartSignalGenerationReport {
            trades_scanned: trades.len(),
            signals_generated: inserted,
            decisions_recorded,
            observe_signals,
            rejected_signals,
        })
    }

    pub async fn list_candidates(
        &self,
        status: Option<SmartWalletCandidateStatus>,
        limit: Option<u16>,
    ) -> Result<Vec<SmartWalletCandidate>> {
        self.store
            .list_candidates(status, validate_smart_money_list_limit(limit))
            .await
    }

    pub async fn list_scores(
        &self,
        tier: Option<SmartWalletTier>,
        limit: Option<u16>,
    ) -> Result<Vec<SmartWalletScore>> {
        self.store
            .list_scores(tier, validate_smart_money_list_limit(limit))
            .await
    }

    pub async fn list_profiles(&self, limit: Option<u16>) -> Result<Vec<SmartWalletProfile>> {
        self.store
            .list_profiles(validate_smart_money_list_limit(limit))
            .await
    }

    pub async fn list_trades(&self, limit: Option<u16>) -> Result<Vec<SmartWalletTrade>> {
        self.store
            .list_trades(validate_smart_money_list_limit(limit))
            .await
    }

    pub async fn list_signals(&self, limit: Option<u16>) -> Result<Vec<SmartSignal>> {
        self.store
            .list_signals(validate_smart_money_list_limit(limit))
            .await
    }

    pub async fn latest_signal_advisory(
        &self,
        lookup: &SmartSignalAdvisoryLookup,
        now: OffsetDateTime,
    ) -> Result<Option<SmartSignalAdvisory>> {
        self.store.latest_signal_advisory(lookup, now).await
    }

    pub async fn save_signal_advisory(&self, advisory: &SmartSignalAdvisory) -> Result<()> {
        self.store.save_signal_advisory(advisory).await
    }

    pub async fn list_signal_advisories(
        &self,
        limit: Option<u16>,
    ) -> Result<Vec<SmartSignalAdvisory>> {
        self.store
            .list_signal_advisories(validate_smart_money_list_limit(limit))
            .await
    }

    pub fn build_signal_advisory_request(
        &self,
        provider: &str,
        request_format: &str,
        model: &str,
        config: &SmartMoneyConfig,
        signal: &SmartSignal,
        context: SmartSignalAdvisoryContext<'_>,
    ) -> Result<SmartSignalAdvisoryRequest> {
        build_smart_signal_advisory_request(
            provider,
            request_format,
            model,
            config,
            signal,
            context,
        )
    }
}

fn promoted_at_for_status(
    status: SmartWalletCandidateStatus,
    now: OffsetDateTime,
) -> Option<OffsetDateTime> {
    matches!(
        status,
        SmartWalletCandidateStatus::Watch | SmartWalletCandidateStatus::Tracked
    )
    .then_some(now)
}

fn rejected_at_for_status(
    status: SmartWalletCandidateStatus,
    now: OffsetDateTime,
) -> Option<OffsetDateTime> {
    matches!(
        status,
        SmartWalletCandidateStatus::Blocked | SmartWalletCandidateStatus::Rejected
    )
    .then_some(now)
}
