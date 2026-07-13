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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardCandidateMarket {
    pub market: RewardMarket,
    pub strategy_bucket: RewardStrategyBucket,
    pub strategy_profile: RewardStrategyProfile,
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
pub struct RewardMarketEventWindow {
    pub condition_id: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub event_key: String,
    pub event_type: String,
    #[serde(default)]
    pub event_time_role: RewardEventTimeRole,
    #[serde(default)]
    pub schedule_status: RewardEventScheduleStatus,
    #[serde(default)]
    pub time_precision: RewardEventTimePrecision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_source_field: Option<String>,
    #[serde(default)]
    pub end_policy: RewardEventEndPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub event_start_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub event_end_at: Option<OffsetDateTime>,
    pub confidence: RewardEventTimeConfidence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(default)]
    pub source_payload: Value,
    #[serde(default)]
    pub notes: String,
    #[serde(default = "default_true")]
    pub active: bool,
    #[serde(default, skip_serializing_if = "reward_event_bool_is_false")]
    pub hard_gate_eligible: bool,
    #[serde(
        default = "default_reward_event_producer_version",
        skip_serializing_if = "reward_event_producer_version_is_one"
    )]
    pub producer_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub source_updated_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub observed_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub expires_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub reviewed_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardEventWindowAssessment {
    pub status: RewardEventWindowStatus,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_time_role: Option<RewardEventTimeRole>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule_status: Option<RewardEventScheduleStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_precision: Option<RewardEventTimePrecision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_source_field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_policy: Option<RewardEventEndPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hard_gate_eligible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub producer_version: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub source_updated_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub observed_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub expires_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub event_start_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub event_end_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<RewardEventTimeConfidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_type: Option<String>,
}

const fn default_true() -> bool {
    true
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
pub struct RewardFairValueComponent {
    pub source: String,
    pub value: Decimal,
    pub weight: Decimal,
    pub confidence: Decimal,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardFairValueEstimate {
    pub condition_id: String,
    pub source: String,
    pub fair_yes: Decimal,
    pub fair_no: Decimal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub market_midpoint_yes: Option<Decimal>,
    pub confidence: Decimal,
    pub uncertainty_cents: Decimal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub midpoint_deviation_cents: Option<Decimal>,
    pub sample_count: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<RewardFairValueComponent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub do_not_quote_reason: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardQuoteEdge {
    pub token_id: String,
    pub outcome: String,
    pub side: RewardOrderSide,
    pub quote_price: Decimal,
    pub fair_price: Decimal,
    pub raw_edge_cents: Decimal,
    pub expected_reward_rebate_cents: Decimal,
    pub uncertainty_cents: Decimal,
    pub effective_edge_cents: Decimal,
    /// Trading edge plus the separately estimated LP/rebate contribution. This
    /// is informational and must not be used by the quote admission gate.
    #[serde(default)]
    pub reward_adjusted_edge_cents: Decimal,
    pub min_raw_edge_cents: Decimal,
    pub min_effective_edge_cents: Decimal,
    pub passed: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardFairValueDecision {
    pub estimate: RewardFairValueEstimate,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<RewardQuoteEdge>,
    pub expected_reward_rebate_cents: Decimal,
    #[serde(
        default,
        skip_serializing_if = "reward_fair_value_assessment_was_evaluated"
    )]
    pub assessment_status: RewardFairValueAssessmentStatus,
    pub passed: bool,
    pub reason: String,
}

fn reward_fair_value_assessment_was_evaluated(
    status: &RewardFairValueAssessmentStatus,
) -> bool {
    *status == RewardFairValueAssessmentStatus::Evaluated
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardMarketSelectionMetrics {
    #[serde(default)]
    pub base_quality_score: Decimal,
    #[serde(default)]
    pub opportunity_score: Decimal,
    #[serde(default)]
    pub reward_density_score: Decimal,
    #[serde(default)]
    pub fair_value_edge_score: Decimal,
    #[serde(default)]
    pub exit_score: Decimal,
    #[serde(default)]
    pub stability_score: Decimal,
    #[serde(default)]
    pub competition_penalty: Decimal,
    #[serde(default)]
    pub allocation_penalty: Decimal,
    #[serde(default)]
    pub risk_penalty: Decimal,
    #[serde(default)]
    pub selection_score: Decimal,
    #[serde(default)]
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RewardHistoryPruneReport {
    pub terminal_orders_deleted: u64,
    pub risk_events_deleted: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardQuotePlan {
    pub condition_id: String,
    pub market_slug: String,
    pub question: String,
    pub score: Decimal,
    #[serde(default)]
    pub selection_score: Decimal,
    pub eligible: bool,
    #[serde(default)]
    pub pre_ai_eligible: bool,
    #[serde(default = "default_reward_quote_readiness")]
    pub quote_readiness: RewardQuoteReadiness,
    pub reason: String,
    #[serde(default = "default_reward_strategy_bucket")]
    pub strategy_bucket: RewardStrategyBucket,
    #[serde(default = "default_reward_strategy_profile")]
    pub strategy_profile: RewardStrategyProfile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_run_id: Option<i64>,
    #[serde(default = "default_reward_plan_quote_mode")]
    pub quote_mode: RewardPlanQuoteMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended_quote_mode: Option<RewardPlanQuoteMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub book_metrics: Option<RewardMarketBookMetrics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opportunity_metrics: Option<RewardOpportunityMetrics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_metrics: Option<RewardMarketSelectionMetrics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fair_value: Option<RewardFairValueDecision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_advisory: Option<RewardMarketAdvisory>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info_risk: Option<RewardMarketInfoRisk>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_window: Option<RewardEventWindowAssessment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub midpoint: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub live_skip_until: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live_skip_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub first_quote_observed_at: Option<OffsetDateTime>,
    /// Timestamp when this pre_ai_eligible plan first lacked a cached AI
    /// advisory. Used to implement a grace period before dropping
    /// `eligible`. Cleared when a cached advisory becomes available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub ai_advisory_pending_since: Option<OffsetDateTime>,
    /// Timestamp when this pre_ai_eligible plan first lacked a cached
    /// info-risk assessment (only relevant under enforce mode). Used to
    /// implement a grace period before dropping `eligible`. Cleared when
    /// a cached risk becomes available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub info_risk_pending_since: Option<OffsetDateTime>,
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

const fn default_reward_strategy_profile() -> RewardStrategyProfile {
    RewardStrategyProfile::Standard
}

const fn default_reward_exit_strategy_source() -> RewardExitStrategySource {
    RewardExitStrategySource::Configured
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
    #[serde(default = "default_reward_strategy_profile")]
    pub strategy_profile: RewardStrategyProfile,
    #[serde(default = "default_reward_exit_strategy_source")]
    pub exit_strategy_source: RewardExitStrategySource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_strategy_selected: Option<PostFillStrategy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_floor_price: Option<Decimal>,
    #[serde(default)]
    pub exit_reselect_count: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub exit_last_reselected_at: Option<OffsetDateTime>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardMergeIntent {
    pub id: String,
    pub account_id: String,
    pub condition_id: String,
    pub yes_token_id: String,
    pub no_token_id: String,
    pub merge_size: Decimal,
    pub yes_position_size: Decimal,
    pub no_position_size: Decimal,
    pub yes_avg_price: Decimal,
    pub no_avg_price: Decimal,
    pub status: RewardMergeIntentStatus,
    pub reason: String,
    pub source_fill_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tx_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submitted_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmed_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed_reason: Option<String>,
    #[serde(default)]
    pub retry_count: i32,
    pub trace_id: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
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
    /// Total reward provider (AI advisory + info-risk) HTTP calls for the day.
    pub provider_calls: u64,
    pub total_calls: u64,
    pub failed_calls: u64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RewardQuotePlanBlockerCounts {
    pub waiting_orderbook: usize,
    pub ai_pending: usize,
    pub info_risk_pending: usize,
    pub ai_stop_new: usize,
    pub provider_size: usize,
    pub info_risk: usize,
    #[serde(default)]
    pub event_window: usize,
    #[serde(default)]
    pub fair_value: usize,
    #[serde(default)]
    pub competition: usize,
    pub funding: usize,
    pub maker_budget: usize,
    pub inventory_headroom: usize,
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
            // Only count ineligible plans in blocker breakdowns.
            // Eligible plans that are provider_pending (grace period) are
            // tracked via `provider_pending` readiness count, not as blockers.
            if !plan.eligible {
                counts.blockers.record(plan, readiness);
            }
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
        } else if reason.starts_with("AI advisory stop_new:") {
            self.ai_stop_new += 1;
        } else if reason.starts_with("provider size adjustment below required rewards quote:") {
            self.provider_size += 1;
        } else if reason.starts_with("info risk ") {
            self.info_risk += 1;
        } else if reason.starts_with("event window") {
            self.event_window += 1;
        } else if reason.starts_with("fair value gate:") {
            self.fair_value += 1;
        } else if reason.starts_with("competition multiple ") {
            self.competition += 1;
        } else if reason.starts_with("live funding below rewards minimum:") {
            self.funding += 1;
        } else if reason.starts_with("maker market budget below required rewards quote:") {
            self.maker_budget += 1;
        } else if reason.starts_with("inventory headroom below required rewards quote:") {
            self.inventory_headroom += 1;
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
        // Eligible plans within provider grace period are ProviderPending,
        // not WaitingOrderbook — they have passed deterministic gates but
        // are awaiting AI/info-risk cache population.
        if plan.pre_ai_eligible && reward_quote_plan_provider_pending(plan) {
            return RewardQuoteReadiness::ProviderPending;
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
    // Traditional: plan was already dropped to ineligible due to missing cache.
    if !plan.eligible
        && plan.pre_ai_eligible
        && (plan.reason.starts_with("AI advisory pending:")
            || plan.reason.starts_with("info risk pending:"))
    {
        return true;
    }
    // Grace period: plan is still eligible but within the provider pending
    // window (ai_advisory_pending_since or info_risk_pending_since is set).
    plan.ai_advisory_pending_since.is_some() || plan.info_risk_pending_since.is_some()
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BookSnapshot {
    pub bids: Vec<RewardBookLevel>,
    pub asks: Vec<RewardBookLevel>,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
struct TokenBookState {
    midpoint: Decimal,
    best_bid: Option<Decimal>,
    best_ask: Option<Decimal>,
    bid_prices: Vec<Decimal>,
}
