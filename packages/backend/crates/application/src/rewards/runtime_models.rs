#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardToken {
    pub token_id: String,
    pub outcome: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardMarket {
    pub condition_id: String,
    pub question: String,
    pub market_slug: String,
    pub event_slug: String,
    #[serde(default)]
    pub category: String,
    pub image: String,
    pub rewards_max_spread: Decimal,
    pub rewards_min_size: Decimal,
    pub total_daily_rate: Decimal,
    #[serde(default)]
    pub liquidity_usd: Decimal,
    #[serde(default)]
    pub volume_24h_usd: Decimal,
    #[serde(default)]
    pub market_spread_cents: Decimal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub end_at: Option<OffsetDateTime>,
    #[serde(default)]
    pub ambiguity_level: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub market_synced_at: Option<OffsetDateTime>,
    pub tokens: Vec<RewardToken>,
    pub active: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RewardCandidateMarket {
    pub market: RewardMarket,
    pub strategy_bucket: RewardStrategyBucket,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardBookLevel {
    pub price: Decimal,
    pub size: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardOrderBook {
    pub token_id: String,
    pub bids: Vec<RewardBookLevel>,
    pub asks: Vec<RewardBookLevel>,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub confirmed_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardMarketCandle {
    pub token_id: String,
    pub condition_id: String,
    pub outcome: String,
    pub interval_sec: i32,
    #[serde(with = "time::serde::rfc3339")]
    pub bucket_start: OffsetDateTime,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub best_bid_close: Decimal,
    pub best_ask_close: Decimal,
    pub spread_cents_close: Decimal,
    pub sample_count: i32,
    #[serde(with = "time::serde::rfc3339")]
    pub close_observed_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RewardMarketCandleSample {
    pub token_id: String,
    pub interval_sec: i32,
    pub bucket_start: OffsetDateTime,
    pub midpoint: Decimal,
    pub best_bid: Decimal,
    pub best_ask: Decimal,
    pub spread_cents: Decimal,
    pub observed_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardQuoteLeg {
    pub token_id: String,
    pub outcome: String,
    pub side: RewardOrderSide,
    pub price: Decimal,
    pub size: Decimal,
    pub notional_usd: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardBookSideMetrics {
    pub top1_depth_share: Decimal,
    pub top3_depth_share: Decimal,
    pub book_hhi: Decimal,
    pub exit_depth_usd: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardMarketBookMetrics {
    pub yes_probability: Decimal,
    pub recommended_quote_mode: RewardPlanQuoteMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yes: Option<RewardBookSideMetrics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no: Option<RewardBookSideMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardLowCompetitionMetrics {
    #[serde(default)]
    pub planned_notional_usd: Decimal,
    #[serde(default)]
    pub competition_probe_notional_usd: Decimal,
    pub qualified_competition_usd: Decimal,
    #[serde(default)]
    pub competition_share_bps: Decimal,
    #[serde(default)]
    pub competition_multiple: Decimal,
    pub estimated_reward_per_100_usd_day: Decimal,
    pub competition_density: Decimal,
    #[serde(default)]
    pub account_effective_available_usd: Decimal,
    #[serde(default)]
    pub low_competition_open_buy_notional_usd: Decimal,
    #[serde(default)]
    pub low_competition_open_buy_notional_usd_after_plan: Decimal,
    #[serde(default)]
    pub condition_buy_notional_usd_after_plan: Decimal,
    #[serde(default)]
    pub account_allocation_bps: Decimal,
    #[serde(default)]
    pub market_allocation_bps: Decimal,
    pub exit_depth_usd: Decimal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_slippage_cents: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bad_fill_recovery_days: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub midpoint_range_cents: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_of_book_flip_count: Option<u64>,
    pub sample_count: u64,
    pub eligible_for_low_competition: bool,
    #[serde(default)]
    pub rejection_reasons: Vec<String>,
    #[serde(default)]
    pub not_low_competition: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_low_competition_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardOpportunityMetrics {
    #[serde(default)]
    pub planned_notional_usd: Decimal,
    #[serde(default)]
    pub probe_notional_usd: Decimal,
    pub qualified_competition_usd: Decimal,
    #[serde(default)]
    pub competition_share_bps: Decimal,
    #[serde(default)]
    pub competition_multiple: Decimal,
    pub estimated_reward_per_100_usd_day: Decimal,
    pub competition_density: Decimal,
    #[serde(default)]
    pub account_effective_available_usd: Decimal,
    #[serde(default)]
    pub open_buy_notional_usd: Decimal,
    #[serde(default)]
    pub open_buy_notional_usd_after_plan: Decimal,
    #[serde(default)]
    pub condition_buy_notional_usd_after_plan: Decimal,
    #[serde(default)]
    pub account_allocation_bps: Decimal,
    #[serde(default)]
    pub market_allocation_bps: Decimal,
    pub exit_depth_usd: Decimal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_slippage_cents: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bad_fill_recovery_days: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub midpoint_range_cents: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_of_book_flip_count: Option<u64>,
    pub sample_count: u64,
    #[serde(default)]
    pub reward_score: Decimal,
    #[serde(default)]
    pub competition_score: Decimal,
    #[serde(default)]
    pub exit_score: Decimal,
    #[serde(default)]
    pub stability_score: Decimal,
    #[serde(default)]
    pub opportunity_score: Decimal,
    #[serde(default)]
    pub score_adjustment: Decimal,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardLowCompetitionObservation {
    pub id: String,
    pub account_id: String,
    pub condition_id: String,
    pub market_slug: String,
    pub question: String,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
    pub mode: RewardLowCompetitionMode,
    pub planned_notional_usd: Decimal,
    pub competition_probe_notional_usd: Decimal,
    pub qualified_competition_usd: Decimal,
    pub competition_share_bps: Decimal,
    pub competition_multiple: Decimal,
    pub estimated_reward_per_100_usd_day: Decimal,
    pub competition_density: Decimal,
    pub account_effective_available_usd: Decimal,
    pub low_competition_open_buy_notional_usd: Decimal,
    pub low_competition_open_buy_notional_usd_after_plan: Decimal,
    pub condition_buy_notional_usd_after_plan: Decimal,
    pub account_allocation_bps: Decimal,
    pub market_allocation_bps: Decimal,
    pub exit_depth_usd: Decimal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_slippage_cents: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub midpoint_range_cents: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_of_book_flip_count: Option<u64>,
    pub sample_count: u64,
    pub sample_insufficient: bool,
    pub eligible_for_low_competition: bool,
    pub final_eligible: bool,
    pub ai_blocked: bool,
    pub info_risk_blocked: bool,
    pub standard_plan_overlap: bool,
    #[serde(default)]
    pub not_low_competition: bool,
    #[serde(default)]
    pub rejection_reasons: Vec<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardLowCompetitionShadowReport {
    pub window_hours: u64,
    #[serde(with = "time::serde::rfc3339")]
    pub generated_at: OffsetDateTime,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub latest_observed_at: Option<OffsetDateTime>,
    pub observations: usize,
    pub unique_markets: usize,
    pub gate_pass_count: usize,
    pub final_pass_count: usize,
    pub sample_insufficient_count: usize,
    pub ai_blocked_count: usize,
    pub info_risk_blocked_count: usize,
    pub standard_overlap_count: usize,
    pub not_low_competition_count: usize,
    pub gate_pass_ratio: Decimal,
    pub final_pass_ratio: Decimal,
    pub sample_insufficient_ratio: Decimal,
    pub ai_blocked_ratio: Decimal,
    pub info_risk_blocked_ratio: Decimal,
    pub standard_overlap_ratio: Decimal,
    pub not_low_competition_ratio: Decimal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub competition_share_bps_median: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_allocation_bps_p90: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub market_allocation_bps_p90: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_reward_per_100_usd_day_median: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_reward_per_100_usd_day_p90: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_depth_multiple_median: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub midpoint_range_cents_p95: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_slippage_cents_p95: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bad_fill_recovery_days_p95: Option<Decimal>,
    pub should_consider_enforce: bool,
    #[serde(default)]
    pub recommendation_reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RewardHistoryPruneReport {
    pub terminal_orders_deleted: u64,
    pub risk_events_deleted: u64,
    pub low_competition_observations_deleted: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardQuotePlan {
    pub condition_id: String,
    pub market_slug: String,
    pub question: String,
    pub score: Decimal,
    pub eligible: bool,
    #[serde(default)]
    pub pre_ai_eligible: bool,
    #[serde(default = "default_reward_quote_readiness")]
    pub quote_readiness: RewardQuoteReadiness,
    pub reason: String,
    #[serde(default = "default_reward_strategy_bucket")]
    pub strategy_bucket: RewardStrategyBucket,
    #[serde(default = "default_reward_plan_quote_mode")]
    pub quote_mode: RewardPlanQuoteMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended_quote_mode: Option<RewardPlanQuoteMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub book_metrics: Option<RewardMarketBookMetrics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opportunity_metrics: Option<RewardOpportunityMetrics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_metrics: Option<RewardLowCompetitionMetrics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_advisory: Option<RewardMarketAdvisory>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info_risk: Option<RewardMarketInfoRisk>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub midpoint: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub live_skip_until: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live_skip_reason: Option<String>,
    pub total_daily_rate: Decimal,
    pub rewards_max_spread: Decimal,
    pub rewards_min_size: Decimal,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub orderbook_token_ids: Vec<String>,
    pub legs: Vec<RewardQuoteLeg>,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

const fn default_reward_plan_quote_mode() -> RewardPlanQuoteMode {
    RewardPlanQuoteMode::Double
}

const fn default_reward_quote_readiness() -> RewardQuoteReadiness {
    RewardQuoteReadiness::Blocked
}

const fn default_reward_strategy_bucket() -> RewardStrategyBucket {
    RewardStrategyBucket::None
}

const fn default_reward_order_strategy_bucket() -> RewardStrategyBucket {
    RewardStrategyBucket::Standard
}

#[derive(Debug, Clone, PartialEq)]
pub struct RewardLiveQuoteMaterialization {
    pub quote_mode: RewardPlanQuoteMode,
    pub recommended_quote_mode: Option<RewardPlanQuoteMode>,
    pub book_metrics: Option<RewardMarketBookMetrics>,
    pub midpoint: Decimal,
    pub legs: Vec<RewardQuoteLeg>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManagedRewardOrder {
    pub id: String,
    pub account_id: String,
    pub condition_id: String,
    pub token_id: String,
    pub outcome: String,
    pub side: RewardOrderSide,
    pub price: Decimal,
    pub size: Decimal,
    #[serde(default = "default_reward_order_strategy_bucket")]
    pub strategy_bucket: RewardStrategyBucket,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_order_id: Option<String>,
    pub status: ManagedRewardOrderStatus,
    pub scoring: bool,
    pub reason: String,
    #[serde(default)]
    pub filled_size: Decimal,
    #[serde(default)]
    pub reward_earned: Decimal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_scored_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardPosition {
    pub account_id: String,
    pub condition_id: String,
    pub token_id: String,
    pub outcome: String,
    pub size: Decimal,
    pub avg_price: Decimal,
    pub realized_pnl: Decimal,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

/// Fund-pool ledger shared across every market the bot quotes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardAccountState {
    pub account_id: String,
    /// Polymarket funding wallet address (0x…), preferring configured `FUNDER`.
    pub wallet_address: Option<String>,
    /// Total deposited capital (the configured fund pool).
    pub capital_usd: Decimal,
    /// Cash not consumed by fills.
    pub available_usd: Decimal,
    /// Total notional of all active buy orders on Polymarket (bot-managed +
    /// external), synced from `list_open_orders()` for account observability.
    /// Resting maker orders do not reserve this amount in placement checks.
    pub external_buy_notional: Decimal,
    /// Snapshot-frozen notional of active Polymarket buy orders that are NOT
    /// tracked as bot-managed (true external/unknown occupancy). Computed once
    /// per CLOB open-order snapshot as `external_buy_notional - managed`, using
    /// both sides from the same snapshot so it stays stable between snapshots.
    /// Funding precheck reads this directly instead of recomputing
    /// `external_buy_notional(stale) - managed(now)`, which used to spike
    /// whenever managed buys were cancelled between snapshots and made
    /// `eligible_markets` oscillate to 0.
    pub unmanaged_external_buy_notional: Decimal,
    /// Legacy hard-reserve field. New rewards ticks release it and keep resting
    /// buy reservations soft across markets.
    pub reserved_usd: Decimal,
    pub realized_pnl: Decimal,
    pub reward_earned_usd: Decimal,
    pub fees_paid: Decimal,
    /// Monotonic per-account tick counter; also seeds the deterministic fill RNG.
    pub tick_index: i64,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

impl RewardAccountState {
    #[must_use]
    pub fn fresh(account_id: &str, capital_usd: Decimal, now: OffsetDateTime) -> Self {
        Self {
            account_id: account_id.to_string(),
            wallet_address: None,
            capital_usd,
            available_usd: capital_usd,
            external_buy_notional: Decimal::ZERO,
            unmanaged_external_buy_notional: Decimal::ZERO,
            reserved_usd: Decimal::ZERO,
            realized_pnl: Decimal::ZERO,
            reward_earned_usd: Decimal::ZERO,
            fees_paid: Decimal::ZERO,
            tick_index: 0,
            updated_at: now,
        }
    }
}

/// One execution event against a managed order (maker fill) or a taker flatten.
/// Drives the "吃单" (order-taken) detail view on the frontend.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardFill {
    pub id: String,
    pub order_id: String,
    pub account_id: String,
    pub condition_id: String,
    pub token_id: String,
    pub outcome: String,
    pub side: RewardOrderSide,
    pub price: Decimal,
    pub size: Decimal,
    pub notional_usd: Decimal,
    pub role: RewardFillRole,
    pub realized_pnl: Decimal,
    pub reason: String,
    pub trace_id: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardRiskEvent {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_order_id: Option<String>,
    pub event_type: String,
    pub severity: RewardRiskSeverity,
    pub message: String,
    pub metadata: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardBotStatus {
    pub enabled: bool,
    pub running: bool,
    pub account_id: String,
    pub markets_tracked: usize,
    pub eligible_markets: usize,
    #[serde(default)]
    pub ready_quote_markets: usize,
    #[serde(default)]
    pub waiting_orderbook_markets: usize,
    #[serde(default)]
    pub provider_pending_markets: usize,
    #[serde(default)]
    pub blocker_counts: RewardQuotePlanBlockerCounts,
    pub plans_total: usize,
    pub open_orders: usize,
    pub positions: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_scan_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_run_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardLlmCallRecord {
    pub id: String,
    pub task_type: String,
    pub model_version: String,
    pub prompt_version: String,
    pub input_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_output: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parsed_output: Option<Value>,
    pub validation_result: Value,
    #[serde(default)]
    pub fallback_used: bool,
    pub latency_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_estimate: Option<Decimal>,
    pub trace_id: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RewardLlmCallDailyStats {
    /// UTC calendar day in `YYYY-MM-DD` format.
    pub day: String,
    pub ai_advisory_calls: u64,
    pub info_risk_calls: u64,
    pub total_calls: u64,
    pub failed_calls: u64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RewardQuotePlanBlockerCounts {
    pub waiting_orderbook: usize,
    pub ai_pending: usize,
    pub info_risk_pending: usize,
    pub ai_confidence_low: usize,
    pub ai_watch: usize,
    pub ai_avoid: usize,
    pub info_risk: usize,
    pub low_competition: usize,
    pub funding: usize,
    pub live_validation: usize,
    pub other: usize,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RewardQuotePlanCounts {
    pub total: usize,
    pub eligible: usize,
    pub ready_to_quote: usize,
    pub waiting_orderbook: usize,
    pub provider_pending: usize,
    pub blockers: RewardQuotePlanBlockerCounts,
}

impl RewardQuotePlanCounts {
    #[must_use]
    pub fn from_plans<'a>(plans: impl IntoIterator<Item = &'a RewardQuotePlan>) -> Self {
        let mut counts = Self::default();
        for plan in plans {
            counts.total += 1;
            if plan.eligible {
                counts.eligible += 1;
            }
            let readiness = reward_quote_plan_readiness(plan);
            counts.blockers.record(plan, readiness);
            match readiness {
                RewardQuoteReadiness::ReadyToQuote => counts.ready_to_quote += 1,
                RewardQuoteReadiness::WaitingOrderbook => counts.waiting_orderbook += 1,
                RewardQuoteReadiness::ProviderPending => counts.provider_pending += 1,
                RewardQuoteReadiness::Blocked => {}
            }
        }
        counts
    }
}

impl RewardQuotePlanBlockerCounts {
    fn record(&mut self, plan: &RewardQuotePlan, readiness: RewardQuoteReadiness) {
        let reason = plan.reason.as_str();
        if readiness == RewardQuoteReadiness::WaitingOrderbook {
            self.waiting_orderbook += 1;
            return;
        }
        if reason.starts_with("AI advisory pending:") {
            self.ai_pending += 1;
        } else if reason.starts_with("info risk pending:") {
            self.info_risk_pending += 1;
        } else if reason.starts_with("AI advisory confidence") {
            self.ai_confidence_low += 1;
        } else if reason.starts_with("AI advisory watch:") {
            self.ai_watch += 1;
        } else if reason.starts_with("AI advisory avoid:") {
            self.ai_avoid += 1;
        } else if reason.starts_with("info risk ") {
            self.info_risk += 1;
        } else if reason.starts_with("live funding below rewards minimum:") {
            self.funding += 1;
        } else if reason.starts_with("live orderbook validation skipped until ") {
            self.live_validation += 1;
        } else if readiness == RewardQuoteReadiness::Blocked {
            self.other += 1;
        }
    }
}

#[must_use]
pub fn reward_quote_plan_readiness(plan: &RewardQuotePlan) -> RewardQuoteReadiness {
    if reward_quote_plan_waiting_orderbook(plan) {
        return RewardQuoteReadiness::WaitingOrderbook;
    }

    if plan.eligible {
        if plan.quote_mode != RewardPlanQuoteMode::None && reward_quote_plan_has_live_legs(plan) {
            return RewardQuoteReadiness::ReadyToQuote;
        }
        return RewardQuoteReadiness::WaitingOrderbook;
    }

    if plan.pre_ai_eligible && reward_quote_plan_provider_pending(plan) {
        return RewardQuoteReadiness::ProviderPending;
    }

    RewardQuoteReadiness::Blocked
}

pub fn refresh_reward_quote_plan_readiness(plan: &mut RewardQuotePlan) {
    plan.quote_readiness = reward_quote_plan_readiness(plan);
}

fn reward_quote_plan_has_live_legs(plan: &RewardQuotePlan) -> bool {
    !plan.legs.is_empty()
        && plan.legs.iter().all(|leg| {
            leg.price > Decimal::ZERO
                && leg.size > Decimal::ZERO
                && leg.notional_usd > Decimal::ZERO
        })
}

fn reward_quote_plan_waiting_orderbook(plan: &RewardQuotePlan) -> bool {
    plan.reason.starts_with("waiting for fresh orderbook data")
}

fn reward_quote_plan_provider_pending(plan: &RewardQuotePlan) -> bool {
    plan.reason.starts_with("AI advisory pending:") || plan.reason.starts_with("info risk pending:")
}

/// Best-effort live quote for a token, injected into the API snapshot so the
/// frontend can show 买一/卖一 and derive position PnL. Populated at read time
/// from the orderbook cache; absent (or the whole map `None`) when the
/// orderbook service is unavailable or the token has no book yet.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardTokenQuote {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_bid: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_ask: Option<Decimal>,
    /// Mid price `(best_bid + best_ask) / 2`, degrading to the available side
    /// when only one is present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mark_price: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardBotSnapshot {
    pub config: RewardBotConfig,
    pub status: RewardBotStatus,
    pub account: RewardAccountState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_report: Option<RewardLowCompetitionShadowReport>,
    #[serde(default)]
    pub llm_usage: Vec<RewardLlmCallDailyStats>,
    pub markets: Vec<RewardMarket>,
    pub quote_plans: Vec<RewardQuotePlan>,
    pub plans_page: RewardListPage,
    pub orders: Vec<ManagedRewardOrder>,
    pub orders_page: RewardListPage,
    pub positions: Vec<RewardPosition>,
    pub fills: Vec<RewardFill>,
    pub events: Vec<RewardRiskEvent>,
    /// Token-id keyed live quotes for the snapshot's positions and orders,
    /// populated best-effort by the API layer. `None` when not enriched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_quotes: Option<HashMap<String, RewardTokenQuote>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RewardLiveCycle {
    pub config: RewardBotConfig,
    pub account: RewardAccountState,
    pub markets: Vec<RewardMarket>,
    pub plans: Vec<RewardQuotePlan>,
    pub previous_plans: Vec<RewardQuotePlan>,
    pub pre_ai_eligible_condition_ids: Vec<String>,
    pub open_orders: Vec<ManagedRewardOrder>,
    pub positions: Vec<RewardPosition>,
    pub should_execute: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RewardBotRunReport {
    pub markets_scanned: usize,
    pub books_fetched: usize,
    pub plans_built: usize,
    pub eligible_plans: usize,
    pub placed_orders: usize,
    pub cancelled_orders: usize,
    pub filled_orders: usize,
    pub risk_cancelled_orders: usize,
    pub reward_accrued: Decimal,
}

/// Point-in-time snapshot of a token's order book, stored for historical
/// comparison in risk-control checks (depth drop, fill velocity, mass cancel).
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct BookSnapshot {
    pub bids: Vec<RewardBookLevel>,
    pub asks: Vec<RewardBookLevel>,
    pub observed_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
struct TokenBookState {
    midpoint: Decimal,
    best_ask: Option<Decimal>,
    bid_prices: Vec<Decimal>,
}
