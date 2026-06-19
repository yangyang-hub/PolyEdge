include!("service/store.rs");

const MEMORY_EVENT_LIMIT: usize = 200;
const MEMORY_FILL_LIMIT: usize = 200;
const LOW_COMPETITION_SHADOW_REPORT_WINDOW_HOURS: u64 = 24;
const LOW_COMPETITION_OBSERVATION_READ_LIMIT: u16 = 5_000;

#[derive(Clone)]
pub struct RewardBotService {
    store: Arc<dyn RewardBotStore>,
    memory: Arc<RwLock<RewardBotMemoryState>>,
    runtime_revision_tx: watch::Sender<(u64, u64)>,
    command_wake_tx: watch::Sender<bool>,
}

#[derive(Default)]
struct RewardBotMemoryState {
    config: Option<RewardBotConfig>,
    account: Option<RewardAccountState>,
    positions: Option<Vec<RewardPosition>>,
    worker_heartbeats: HashMap<String, OffsetDateTime>,
    events: Vec<RewardRiskEvent>,
    fills: Vec<RewardFill>,
    external_open_order_count: Option<usize>,
}

impl RewardBotService {
    #[must_use]
    pub fn new(store: Arc<dyn RewardBotStore>) -> Self {
        let (runtime_revision_tx, _) = watch::channel((0, 0));
        let (command_wake_tx, _) = watch::channel(false);
        Self {
            store,
            memory: Arc::new(RwLock::new(RewardBotMemoryState::default())),
            runtime_revision_tx,
            command_wake_tx,
        }
    }

    pub async fn read_config(&self) -> Result<RewardBotConfig> {
        if let Some(config) = self.memory.read().await.config.clone() {
            return Ok(config);
        }
        let config = self.store.load_config().await?.normalized();
        self.memory.write().await.config = Some(config.clone());
        Ok(config)
    }

    #[must_use]
    pub fn subscribe_runtime_changes(&self) -> watch::Receiver<(u64, u64)> {
        self.runtime_revision_tx.subscribe()
    }

    #[must_use]
    pub fn subscribe_command_wake(&self) -> watch::Receiver<bool> {
        self.command_wake_tx.subscribe()
    }

    fn wake_command_processor(&self) {
        self.command_wake_tx.send_modify(|flag| *flag = !*flag);
    }

    fn notify_runtime_change(&self, config_changed: bool) {
        self.runtime_revision_tx.send_modify(|(revision, config_revision)| {
            *revision = revision.wrapping_add(1);
            if config_changed {
                *config_revision = config_revision.wrapping_add(1);
            }
        });
    }

    pub async fn update_config(&self, patch: RewardBotConfigPatch) -> Result<RewardBotConfig> {
        let current = self.read_config().await?;
        let next = current.apply_patch(patch);
        if next.account_id != current.account_id
            && (self.store.count_open_orders(&current.account_id).await? > 0
                || self.store.count_account_positions(&current.account_id).await? > 0)
        {
            return Err(AppError::conflict(
                "REWARD_ACCOUNT_CHANGE_BLOCKED",
                "cannot change rewards account_id while the current account has open orders or non-zero positions",
            ));
        }
        self.store.save_config(&next).await?;
        {
            let mut memory = self.memory.write().await;
            memory.config = Some(next.clone());
            if next.account_id != current.account_id {
                memory.account = None;
                memory.positions = None;
                memory.events.clear();
                memory.fills.clear();
                memory.external_open_order_count = None;
            }
        }
        self.notify_runtime_change(true);
        Ok(next)
    }

    pub async fn enqueue_control_command(
        &self,
        action: RewardControlAction,
        reason: &str,
        trace_id: &str,
    ) -> Result<RewardControlCommand> {
        let config = self.read_config().await?;
        let now = OffsetDateTime::now_utc();
        let command = RewardControlCommand {
            id: reward_control_command_id(trace_id),
            action,
            account_id: Some(config.account_id.clone()),
            reason: reason.to_string(),
            status: RewardControlCommandStatus::Pending,
            requested_at: now,
            started_at: None,
            completed_at: None,
            trace_id: Some(trace_id.to_string()),
            error: None,
        };

        self.store.enqueue_control_command(command.clone()).await?;
        self.log_event_to_store_and_memory(new_risk_event(
                Some(config.account_id),
                None,
                None,
                "reward_control_command_queued",
                RewardRiskSeverity::Info,
                format!("Queued rewards control command: {}", action.as_str()),
                json!({
                    "command_id": &command.id,
                    "action": action.as_str(),
                    "reason": reason,
                    "trace_id": trace_id,
                }),
            ))
            .await?;
        self.notify_runtime_change(false);
        self.wake_command_processor();
        Ok(command)
    }

    pub async fn record_worker_heartbeat(
        &self,
        account_id: &str,
        observed_at: OffsetDateTime,
    ) -> Result<()> {
        self.store
            .record_worker_heartbeat(account_id, observed_at)
            .await?;
        self.memory
            .write()
            .await
            .worker_heartbeats
            .insert(account_id.to_string(), observed_at);
        Ok(())
    }

    pub async fn claim_next_control_command(
        &self,
        trace_id: &str,
    ) -> Result<Option<RewardControlCommand>> {
        self.store
            .claim_next_control_command(trace_id, OffsetDateTime::now_utc())
            .await
    }

    pub async fn complete_control_command(
        &self,
        command: &RewardControlCommand,
        trace_id: &str,
    ) -> Result<()> {
        self.store
            .complete_control_command(&command.id, trace_id, OffsetDateTime::now_utc())
            .await?;
        self.log_event_to_store_and_memory(new_risk_event(
                command.account_id.clone(),
                None,
                None,
                "reward_control_command_completed",
                RewardRiskSeverity::Info,
                format!("Completed rewards control command: {}", command.action.as_str()),
                json!({
                    "command_id": command.id,
                    "action": command.action.as_str(),
                    "trace_id": trace_id,
                }),
            ))
            .await
    }

    pub async fn fail_control_command(
        &self,
        command: &RewardControlCommand,
        trace_id: &str,
        error: &AppError,
    ) -> Result<()> {
        let error_message = error.to_string();
        self.store
            .fail_control_command(
                &command.id,
                trace_id,
                &error_message,
                OffsetDateTime::now_utc(),
            )
            .await?;
        self.log_event_to_store_and_memory(new_risk_event(
                command.account_id.clone(),
                None,
                None,
                "reward_control_command_failed",
                RewardRiskSeverity::Critical,
                format!(
                    "Failed rewards control command {}: {error_message}",
                    command.action.as_str()
                ),
                json!({
                    "command_id": command.id,
                    "action": command.action.as_str(),
                    "trace_id": trace_id,
                    "error": error_message,
                }),
            ))
            .await
    }

    /// Persist reward markets fetched by the sync worker.
    pub async fn upsert_reward_markets(&self, markets: &[RewardMarket]) -> Result<()> {
        if markets.is_empty() {
            return Err(AppError::conflict(
                "REWARD_MARKET_REPLACEMENT_EMPTY",
                "refusing to replace the reward market catalog with an empty snapshot",
            ));
        }
        self.store.upsert_markets(markets).await
    }

    /// List all active reward markets from the database.
    pub async fn list_active_reward_markets(&self) -> Result<Vec<RewardMarket>> {
        self.store.list_all_active_markets().await
    }

    pub async fn latest_market_advisory(
        &self,
        request: &RewardAiAdvisoryRequest,
    ) -> Result<Option<RewardMarketAdvisory>> {
        self.store
            .latest_market_advisory(request, OffsetDateTime::now_utc())
            .await
    }

    pub async fn save_market_advisory(
        &self,
        advisory: &RewardMarketAdvisory,
    ) -> Result<()> {
        self.store.save_market_advisory(advisory).await
    }

    pub async fn latest_market_info_risk(
        &self,
        request: &RewardInfoRiskAssessmentRequest,
    ) -> Result<Option<RewardMarketInfoRisk>> {
        self.store
            .latest_market_info_risk(request, OffsetDateTime::now_utc())
            .await
    }

    pub async fn latest_market_info_risks(
        &self,
        condition_ids: &[String],
    ) -> Result<Vec<RewardMarketInfoRisk>> {
        self.store
            .latest_market_info_risks(condition_ids, OffsetDateTime::now_utc())
            .await
    }

    pub async fn save_market_info_risk(&self, risk: &RewardMarketInfoRisk) -> Result<()> {
        self.store.save_market_info_risk(risk).await
    }

    pub async fn save_quote_plans(&self, plans: &[RewardQuotePlan]) -> Result<()> {
        self.store.save_quote_plans(plans).await
    }

    pub async fn record_low_competition_observations(
        &self,
        observations: &[RewardLowCompetitionObservation],
    ) -> Result<()> {
        if observations.is_empty() {
            return Ok(());
        }
        self.store
            .record_low_competition_observations(observations)
            .await
    }

    /// List a bounded candidate pool for one rewards strategy tick.
    pub async fn list_reward_run_candidate_markets(&self) -> Result<Vec<RewardMarket>> {
        Ok(self
            .list_reward_run_candidate_market_profiles()
            .await?
            .into_iter()
            .map(|candidate| candidate.market)
            .collect())
    }

    pub async fn list_reward_run_candidate_market_profiles(&self) -> Result<Vec<RewardCandidateMarket>> {
        let config = self.read_config().await?;
        self.list_candidate_market_profiles_for_config(&config).await
    }

    async fn list_candidate_market_profiles_for_config(
        &self,
        config: &RewardBotConfig,
    ) -> Result<Vec<RewardCandidateMarket>> {
        let filter = config.candidate_filter();
        let safety_limit = reward_candidate_safety_limit(&config);
        let markets = self
            .store
            .list_candidate_markets(&filter, safety_limit)
            .await?;
        let mut candidates = select_reward_quote_candidate_market_profiles(
            &markets,
            config,
            RewardStrategyBucket::Standard,
        );
        let mut seen = candidates
            .iter()
            .map(|candidate| candidate.market.condition_id.clone())
            .collect::<HashSet<_>>();

        if let Some(low_filter) = config.low_competition_candidate_filter() {
            let low_config = config.config_for_strategy_bucket(RewardStrategyBucket::LowCompetition);
            let low_safety_limit = reward_candidate_safety_limit(&low_config);
            let low_markets = self
                .store
                .list_candidate_markets(&low_filter, low_safety_limit)
                .await?;
            for candidate in select_reward_quote_candidate_market_profiles(
                &low_markets,
                config,
                RewardStrategyBucket::LowCompetition,
            ) {
                if seen.insert(candidate.market.condition_id.clone()) {
                    candidates.push(candidate);
                }
            }
        }

        Ok(candidates)
    }

    /// Return distinct token IDs from markets where the reward bot currently has
    /// open-like orders or non-zero positions. This is a much smaller set than
    /// `list_reward_run_candidate_markets` and is used by the orderbook stream to
    /// subscribe only to relevant orderbook channels.
    pub async fn list_active_reward_book_token_ids(&self) -> Result<Vec<String>> {
        let config = self.read_config().await?;
        let account_id = &config.account_id;

        let open_orders = self.store.list_open_orders(account_id).await?;
        let positions = self.store.list_account_positions(account_id).await?;

        let mut seen = HashSet::new();
        let mut token_ids = Vec::new();

        for order in &open_orders {
            if order.token_id.trim().is_empty() || !seen.insert(order.token_id.clone()) {
                continue;
            }
            token_ids.push(order.token_id.clone());
        }
        for position in &positions {
            if position.token_id.trim().is_empty() || !seen.insert(position.token_id.clone()) {
                continue;
            }
            token_ids.push(position.token_id.clone());
        }

        Ok(token_ids)
    }

    /// Return distinct token IDs from **all** reward candidate markets, regardless
    /// of whether the bot currently has orders or positions. This breaks the cold
    /// start loop: the orderbook stream can subscribe to reward market tokens even
    /// before the bot has placed its first order.
    pub async fn list_all_reward_candidate_token_ids(&self) -> Result<Vec<String>> {
        let candidates = self.list_reward_run_candidate_market_profiles().await?;
        let markets = candidates
            .into_iter()
            .map(|candidate| candidate.market)
            .collect::<Vec<_>>();
        Ok(select_reward_book_token_ids(&markets))
    }

    /// Return distinct token IDs from the latest eligible quote plans. Candidate
    /// registration still provides cold-start coverage before these plans exist.
    pub async fn list_eligible_reward_book_token_ids(&self) -> Result<Vec<String>> {
        let plans = self.store.list_all_quote_plans().await?;
        let mut seen = HashSet::new();
        let mut token_ids = Vec::new();
        for plan in plans.iter().filter(|plan| plan.eligible) {
            for leg in &plan.legs {
                if leg.token_id.trim().is_empty() || !seen.insert(leg.token_id.clone()) {
                    continue;
                }
                token_ids.push(leg.token_id.clone());
            }
        }
        Ok(token_ids)
    }

    /// Return condition ids that should receive prioritized Gamma metadata
    /// refreshes. Active managed markets are first, followed by eligible quote
    /// plans, then rewards candidates selected with a relaxed freshness window
    /// so a stale catalog can recover without waiting for a full Gamma sync.
    pub async fn list_priority_reward_condition_ids(
        &self,
        max_stale_minutes: u64,
        limit: usize,
    ) -> Result<Vec<String>> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let config = self.read_config().await?;
        let account_id = &config.account_id;
        let mut seen = HashSet::new();
        let mut condition_ids = Vec::new();

        let open_orders = self.store.list_open_orders(account_id).await?;
        for order in open_orders {
            push_unique_condition_id(
                &mut condition_ids,
                &mut seen,
                order.condition_id,
                limit,
            );
        }

        if condition_ids.len() < limit {
            let positions = self.store.list_account_positions(account_id).await?;
            for position in positions {
                push_unique_condition_id(
                    &mut condition_ids,
                    &mut seen,
                    position.condition_id,
                    limit,
                );
            }
        }

        if condition_ids.len() < limit {
            let plans = self.store.list_all_quote_plans().await?;
            for plan in plans.into_iter().filter(|plan| plan.eligible) {
                push_unique_condition_id(
                    &mut condition_ids,
                    &mut seen,
                    plan.condition_id,
                    limit,
                );
            }
        }

        if condition_ids.len() < limit {
            let mut relaxed_config = config.clone();
            relaxed_config.max_market_data_age_minutes = max_stale_minutes
                .max(config.max_market_data_age_minutes)
                .clamp(1, 1440);
            let remaining = limit.saturating_sub(condition_ids.len());
            let candidate_limit = u16::try_from(remaining.saturating_mul(10).max(100))
                .unwrap_or(u16::MAX)
                .min(reward_candidate_safety_limit(&relaxed_config));
            let candidates = self
                .list_candidate_market_profiles_for_config(&relaxed_config)
                .await?;
            for candidate in candidates.into_iter().take(usize::from(candidate_limit)) {
                push_unique_condition_id(
                    &mut condition_ids,
                    &mut seen,
                    candidate.market.condition_id,
                    limit,
                );
            }
        }

        Ok(condition_ids)
    }

    pub async fn prepare_live_cycle(
        &self,
        candidate_markets: Vec<RewardCandidateMarket>,
        books: HashMap<String, RewardOrderBook>,
        _trace_id: &str,
        force_orders: bool,
        ai_min_confidence: Decimal,
        ai_model: &str,
    ) -> Result<RewardLiveCycle> {
        let config = self.read_config().await?;
        let markets = candidate_markets
            .iter()
            .map(|candidate| candidate.market.clone())
            .collect::<Vec<_>>();
        let mut plans = build_reward_quote_plans_for_candidates(&candidate_markets, &books, &config);
        let previous_plans = self.store.list_all_quote_plans().await?;
        apply_unexpired_live_orderbook_skips(
            &mut plans,
            &previous_plans,
            OffsetDateTime::now_utc(),
        );
        let pre_ai_eligible_condition_ids = plans
            .iter()
            .filter(|plan| plan.eligible)
            .map(|plan| plan.condition_id.clone())
            .collect::<Vec<_>>();
        if config.ai_advisory_enabled {
            let carried_advisories = reward_ai_advisories_from_quote_plans(
                &previous_plans,
                &config,
                ai_model,
                OffsetDateTime::now_utc(),
            );
            apply_existing_reward_ai_advisories(
                &mut plans,
                &carried_advisories,
                &config,
                ai_min_confidence,
            );
        }
        let account = self.load_account_state_cached(&config).await?;
        let open_orders = self.store.list_open_orders(&account.account_id).await?;
        let positions = self.list_account_positions_cached(&account.account_id).await?;
        let should_execute = config.enabled || force_orders;

        Ok(RewardLiveCycle {
            config,
            account,
            markets,
            plans,
            pre_ai_eligible_condition_ids,
            open_orders,
            positions,
            should_execute,
        })
    }

    /// Load mutable live state for sync/cancel/reconcile paths without scanning
    /// the full reward market catalog. Full ticks pass candidate markets through
    /// `prepare_live_cycle`.
    pub async fn current_live_cycle_state(&self) -> Result<RewardLiveCycle> {
        let config = self.read_config().await?;
        let account = self.load_account_state_cached(&config).await?;
        let open_orders = self.store.list_open_orders(&account.account_id).await?;
        let positions = self.list_account_positions_cached(&account.account_id).await?;
        let plans = self.store.list_all_quote_plans().await?;
        let pre_ai_eligible_condition_ids = plans
            .iter()
            .filter(|plan| plan.eligible)
            .map(|plan| plan.condition_id.clone())
            .collect::<Vec<_>>();
        Ok(RewardLiveCycle {
            should_execute: config.enabled,
            config,
            account,
            markets: Vec::new(),
            plans,
            pre_ai_eligible_condition_ids,
            open_orders,
            positions,
        })
    }

    pub async fn get_managed_order_by_external_order_id(
        &self,
        external_order_id: &str,
    ) -> Result<Option<ManagedRewardOrder>> {
        self.store
            .get_order_by_external_order_id(external_order_id)
            .await
    }

    pub async fn reward_fill_exists(&self, fill_id: &str) -> Result<bool> {
        self.store.reward_fill_exists(fill_id).await
    }

    pub async fn latest_reward_fill_at(
        &self,
        account_id: &str,
    ) -> Result<Option<OffsetDateTime>> {
        self.store.latest_fill_at(account_id).await
    }

    pub async fn record_live_reset_cancel_all(&self, trace_id: &str) -> Result<()> {
        let config = self.read_config().await?;
        self.log_event_to_store_and_memory(new_risk_event(
                Some(config.account_id.clone()),
                None,
                None,
                "reward_bot_live_reset_cancel_all",
                RewardRiskSeverity::Info,
                "Reset rewards live bot by cancelling managed live orders without clearing local ledger.",
                json!({ "trace_id": trace_id }),
            ))
            .await
    }

    pub async fn apply_live_tick_outcome(
        &self,
        outcome: &RewardTickOutcome,
        trace_id: &str,
    ) -> Result<()> {
        self.store.apply_tick_outcome(outcome, trace_id).await?;
        let mut memory = self.memory.write().await;
        memory.account = Some(outcome.account.clone());
        if let Some(positions) = memory.positions.as_mut() {
            for position in &outcome.positions {
                if let Some(existing) = positions.iter_mut().find(|stored| {
                    stored.account_id == position.account_id && stored.token_id == position.token_id
                }) {
                    *existing = position.clone();
                } else if position.size != Decimal::ZERO {
                    positions.push(position.clone());
                }
            }
            positions.retain(|position| position.size != Decimal::ZERO);
        }
        if !outcome.events.is_empty() {
            memory.events.extend(outcome.events.iter().cloned());
            memory.events.sort_by(|a, b| b.created_at.cmp(&a.created_at));
            memory.events.truncate(MEMORY_EVENT_LIMIT);
        }
        if !outcome.fills.is_empty() {
            memory.fills.extend(outcome.fills.iter().cloned());
            memory.fills.sort_by(|a, b| b.created_at.cmp(&a.created_at));
            memory.fills.truncate(MEMORY_FILL_LIMIT);
        }
        memory.external_open_order_count = None;
        Ok(())
    }

    /// Persist external-synced account state and optionally replace positions.
    pub async fn apply_account_sync(
        &self,
        account: &RewardAccountState,
        positions: Option<&[RewardPosition]>,
        trace_id: &str,
    ) -> Result<()> {
        self.store
            .apply_account_sync(account, positions, trace_id)
            .await?;
        let mut memory = self.memory.write().await;
        memory.account = Some(account.clone());
        if let Some(positions) = positions {
            memory.positions = Some(positions.to_vec());
        }
        Ok(())
    }

    pub async fn reset_state(&self, trace_id: &str) -> Result<()> {
        let config = self.read_config().await?;
        self.store.reset_state(&config, trace_id).await?;
        {
            let mut memory = self.memory.write().await;
            memory.account = Some(RewardAccountState::fresh(
                &config.account_id,
                config.account_capital_usd,
                OffsetDateTime::now_utc(),
            ));
            memory.positions = Some(Vec::new());
            memory.events.clear();
            memory.fills.clear();
            memory.external_open_order_count = None;
        }
        self.log_event_to_store_and_memory(new_risk_event(
                Some(config.account_id.clone()),
                None,
                None,
                "reward_bot_reset",
                RewardRiskSeverity::Info,
                "Reset rewards account, orders, positions and fills.",
                json!({ "trace_id": trace_id, "capital_usd": config.account_capital_usd }),
            ))
            .await
    }
}

fn apply_unexpired_live_orderbook_skips(
    plans: &mut [RewardQuotePlan],
    previous_plans: &[RewardQuotePlan],
    now: OffsetDateTime,
) {
    let previous_by_condition = previous_plans
        .iter()
        .filter_map(|plan| {
            let skip_until = plan.live_skip_until?;
            if skip_until <= now {
                return None;
            }
            if live_orderbook_skip_reason_is_transient(plan.live_skip_reason.as_deref()) {
                return None;
            }
            Some((plan.condition_id.as_str(), (skip_until, plan.live_skip_reason.clone())))
        })
        .collect::<HashMap<_, _>>();

    for plan in plans {
        let Some((skip_until, skip_reason)) = previous_by_condition.get(plan.condition_id.as_str())
        else {
            continue;
        };
        plan.eligible = false;
        plan.quote_mode = RewardPlanQuoteMode::None;
        plan.reason = format!(
            "live orderbook validation skipped until {}: {}",
            skip_until,
            skip_reason
                .as_deref()
                .unwrap_or("recent live orderbook validation failed")
        );
        plan.live_skip_until = Some(*skip_until);
        plan.live_skip_reason = skip_reason.clone();
    }
}

fn live_orderbook_skip_reason_is_transient(skip_reason: Option<&str>) -> bool {
    let Some(reason) = skip_reason else {
        return false;
    };
    let reason = reason.to_ascii_lowercase();
    reason.contains("missing fresh orderbook midpoint")
        || reason.contains("waiting for fresh orderbook")
        || reason.contains("orderbook unavailable")
        || reason.contains("orderbook is empty")
        || reason.contains("orderbook stale")
}

include!("service_cache.rs");

/// Safety cap for candidate market queries. This is NOT the primary filter —
/// the SQL WHERE clause does the real filtering. This LIMIT exists only to
/// prevent unbounded result sets in pathological cases.
fn reward_candidate_safety_limit(config: &RewardBotConfig) -> u16 {
    let cap = if config.max_markets == 0 {
        1000u16
    } else {
        config.max_markets.saturating_mul(50).max(1000)
    };
    cap.min(5000)
}

fn push_unique_condition_id(
    condition_ids: &mut Vec<String>,
    seen: &mut HashSet<String>,
    condition_id: String,
    limit: usize,
) {
    if condition_ids.len() >= limit {
        return;
    }
    let condition_id = condition_id.trim();
    if condition_id.is_empty() || !seen.insert(condition_id.to_string()) {
        return;
    }
    condition_ids.push(condition_id.to_string());
}

fn reward_worker_is_running(
    config: &RewardBotConfig,
    heartbeat: Option<OffsetDateTime>,
    now: OffsetDateTime,
) -> bool {
    const HEARTBEAT_TTL: TimeDuration = TimeDuration::minutes(2);

    config.enabled && heartbeat.is_some_and(|heartbeat| heartbeat >= now - HEARTBEAT_TTL)
}

#[cfg(test)]
mod reward_service_tests {
    include!("service/tests.rs");
}

fn reward_control_command_id(trace_id: &str) -> String {
    format!("rewcmd_{}", trace_id.trim_start_matches("trc_"))
}
