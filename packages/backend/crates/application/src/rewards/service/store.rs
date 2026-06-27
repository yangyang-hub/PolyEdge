#[async_trait]
pub trait RewardBotStore: Send + Sync {
    async fn load_config(&self) -> Result<RewardBotConfig>;
    async fn save_config(&self, config: &RewardBotConfig) -> Result<()>;
    async fn record_worker_heartbeat(
        &self,
        account_id: &str,
        observed_at: OffsetDateTime,
    ) -> Result<()>;
    async fn latest_worker_heartbeat(&self, account_id: &str) -> Result<Option<OffsetDateTime>>;
    /// Prune unbounded rewards history older than `cutoff`.
    ///
    /// Implementations must preserve open-like orders, fills, positions, and
    /// account state; this is only for terminal order rows and event-like rows.
    async fn prune_history(&self, cutoff: OffsetDateTime) -> Result<RewardHistoryPruneReport>;
    async fn enqueue_control_command(&self, command: RewardControlCommand) -> Result<bool>;
    async fn claim_next_control_command(
        &self,
        trace_id: &str,
        now: OffsetDateTime,
    ) -> Result<Option<RewardControlCommand>>;
    async fn complete_control_command(
        &self,
        command_id: &str,
        trace_id: &str,
        now: OffsetDateTime,
    ) -> Result<()>;
    async fn fail_control_command(
        &self,
        command_id: &str,
        trace_id: &str,
        error: &str,
        now: OffsetDateTime,
    ) -> Result<()>;
    async fn upsert_markets(&self, markets: &[RewardMarket]) -> Result<()>;
    async fn upsert_market_event_windows(&self, windows: &[RewardMarketEventWindow])
    -> Result<()>;
    async fn list_effective_market_event_windows(
        &self,
        condition_ids: &[String],
    ) -> Result<Vec<RewardMarketEventWindow>>;
    /// Replace the current rewards quote plan snapshot.
    async fn save_quote_plans(&self, plans: &[RewardQuotePlan]) -> Result<()>;
    /// Append legacy low-competition observations when reading historical data.
    async fn record_low_competition_observations(
        &self,
        observations: &[RewardLowCompetitionObservation],
    ) -> Result<()>;
    /// Read legacy low-competition observations for one account.
    async fn list_low_competition_observations(
        &self,
        account_id: &str,
        since: OffsetDateTime,
        limit: u16,
    ) -> Result<Vec<RewardLowCompetitionObservation>>;
    async fn record_market_candle_sample(&self, sample: &RewardMarketCandleSample) -> Result<()>;
    async fn list_recent_market_candles(
        &self,
        condition_id: &str,
        interval_sec: i32,
        limit_per_token: u16,
    ) -> Result<Vec<RewardMarketCandle>>;
    async fn latest_market_advisory(
        &self,
        request: &RewardAiAdvisoryRequest,
        now: OffsetDateTime,
    ) -> Result<Option<RewardMarketAdvisory>>;
    async fn save_market_advisory(&self, advisory: &RewardMarketAdvisory) -> Result<()>;
    async fn latest_market_info_risk(
        &self,
        request: &RewardInfoRiskAssessmentRequest,
        now: OffsetDateTime,
    ) -> Result<Option<RewardMarketInfoRisk>>;
    async fn latest_market_info_risks(
        &self,
        condition_ids: &[String],
        now: OffsetDateTime,
    ) -> Result<Vec<RewardMarketInfoRisk>>;
    async fn save_market_info_risk(&self, risk: &RewardMarketInfoRisk) -> Result<()>;
    async fn record_llm_call(&self, call: &RewardLlmCallRecord) -> Result<()>;
    async fn list_llm_call_daily_stats(
        &self,
        since: OffsetDateTime,
        limit: u16,
    ) -> Result<Vec<RewardLlmCallDailyStats>>;
    async fn list_markets(&self, limit: u16) -> Result<Vec<RewardMarket>>;
    /// List candidate markets with SQL-level filtering by config parameters.
    /// The SQL WHERE clause pushes down midpoint, daily-rate, spread, token-count,
    /// and market-quality checks, returning only markets likely to pass the Rust
    /// planner. `safety_limit` is a generous upper bound, not the primary filter.
    async fn list_candidate_markets(
        &self,
        filter: &RewardCandidateFilter,
        safety_limit: u16,
    ) -> Result<Vec<RewardMarket>>;
    /// List all active markets without a row limit for explicit catalog exports.
    async fn list_all_active_markets(&self) -> Result<Vec<RewardMarket>>;
    /// Count active markets and return their latest update timestamp without loading rows.
    async fn active_market_summary(&self) -> Result<(usize, Option<OffsetDateTime>)>;
    /// List all quote plans without a row limit (used by worker live cycle).
    async fn list_all_quote_plans(&self) -> Result<Vec<RewardQuotePlan>>;
    /// Count quote plans by strategy/readiness status.
    async fn count_quote_plans(&self) -> Result<RewardQuotePlanCounts>;
    /// Return the latest `updated_at` across all quote plans.
    async fn latest_quote_plan_updated_at(&self) -> Result<Option<OffsetDateTime>>;
    /// List a single page of quote plans with server-side filter/sort/pagination.
    async fn list_quote_plans_page(
        &self,
        query: &RewardQuotePlanListQuery,
    ) -> Result<RewardQuotePlanPage>;
    async fn list_orders_page(&self, query: &RewardOrderListQuery) -> Result<RewardOrderPage>;
    async fn list_positions(&self, account_id: &str, limit: u16) -> Result<Vec<RewardPosition>>;
    async fn list_events(&self, account_id: &str, limit: u16) -> Result<Vec<RewardRiskEvent>>;
    async fn log_event(&self, event: RewardRiskEvent) -> Result<()>;

    /// Load the fund-pool ledger, seeding a fresh one from `config` if absent.
    async fn load_account_state(&self, config: &RewardBotConfig) -> Result<RewardAccountState>;
    /// Currently open-like orders for an account (planned/open/exit_pending).
    async fn list_open_orders(&self, account_id: &str) -> Result<Vec<ManagedRewardOrder>>;
    /// Count open-like orders without loading full rows.
    async fn count_open_orders(&self, account_id: &str) -> Result<usize>;
    /// Count open-like orders that have been submitted to Polymarket.
    async fn count_external_open_orders(&self, account_id: &str) -> Result<usize>;
    /// Lookup a managed rewards order by its external Polymarket order id.
    async fn get_order_by_external_order_id(
        &self,
        external_order_id: &str,
    ) -> Result<Option<ManagedRewardOrder>>;
    /// Non-zero inventory for an account.
    async fn list_account_positions(&self, account_id: &str) -> Result<Vec<RewardPosition>>;
    /// Count non-zero positions without loading full rows.
    async fn count_account_positions(&self, account_id: &str) -> Result<usize>;
    async fn list_fills(&self, account_id: &str, limit: u16) -> Result<Vec<RewardFill>>;
    async fn reward_fill_exists(&self, fill_id: &str) -> Result<bool>;
    /// Timestamp of the latest confirmed managed fill for an account.
    async fn latest_fill_at(&self, account_id: &str) -> Result<Option<OffsetDateTime>>;
    /// Persist account, order, fill, position, and event changes atomically.
    ///
    /// Reward market catalogs and quote-plan snapshots have separate full-replacement
    /// methods and must not be changed by incremental live-state persistence.
    async fn apply_tick_outcome(&self, outcome: &RewardTickOutcome, trace_id: &str) -> Result<()>;
    /// Persist account state from external sync and optionally replace all positions
    /// for the account. `None` preserves the stored positions when the external
    /// position request failed; `Some` is a complete authoritative snapshot.
    async fn apply_account_sync(
        &self,
        account: &RewardAccountState,
        positions: Option<&[RewardPosition]>,
        trace_id: &str,
    ) -> Result<()>;
    /// Reset state: cancel orders, clear fills/positions, reset the ledger to capital.
    async fn reset_state(&self, config: &RewardBotConfig, trace_id: &str) -> Result<()>;
}
