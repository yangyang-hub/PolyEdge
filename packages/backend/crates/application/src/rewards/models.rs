/// Execution mode is always live. The enum and legacy string aliases are kept
/// only for backward-compatible deserialization of existing DB rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardExecutionMode {
    Live,
}

impl RewardExecutionMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        "live"
    }
}

impl FromStr for RewardExecutionMode {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "validation" | "validate" | "dry_run" | "paper" | "simulation" | "live" => {
                Ok(Self::Live)
            }
            other => Err(AppError::invalid_input(
                "REWARD_EXECUTION_MODE_INVALID",
                format!("unknown reward execution mode: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardOrderSide {
    Buy,
    Sell,
}

impl RewardOrderSide {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
        }
    }
}

impl FromStr for RewardOrderSide {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "buy" => Ok(Self::Buy),
            "sell" => Ok(Self::Sell),
            other => Err(AppError::invalid_input(
                "REWARD_ORDER_SIDE_INVALID",
                format!("unknown reward order side: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PostFillStrategy {
    /// Rest a reverse sell order at the filled buy order price plus `exit_markup_cents`.
    ExitAtMarkup,
    /// Rest a post-only sell at the filled buy order price, then keep quoting normally.
    HoldAndRequote,
    /// Submit a non-post-only sell against the best bid when it can meet the non-loss floor.
    FlattenImmediately,
}

impl PostFillStrategy {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExitAtMarkup => "exit_at_markup",
            Self::HoldAndRequote => "hold_and_requote",
            Self::FlattenImmediately => "flatten_immediately",
        }
    }
}

impl FromStr for PostFillStrategy {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "exit_at_markup" => Ok(Self::ExitAtMarkup),
            "hold_and_requote" => Ok(Self::HoldAndRequote),
            "flatten_immediately" => Ok(Self::FlattenImmediately),
            other => Err(AppError::invalid_input(
                "REWARD_POST_FILL_STRATEGY_INVALID",
                format!("unknown reward post-fill strategy: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardFillRole {
    Maker,
    Taker,
}

impl RewardFillRole {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Maker => "maker",
            Self::Taker => "taker",
        }
    }
}

impl FromStr for RewardFillRole {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "maker" => Ok(Self::Maker),
            "taker" => Ok(Self::Taker),
            other => Err(AppError::invalid_input(
                "REWARD_FILL_ROLE_INVALID",
                format!("unknown reward fill role: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedRewardOrderStatus {
    Planned,
    Open,
    Cancelled,
    Filled,
    ExitPending,
    Error,
}

impl ManagedRewardOrderStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Open => "open",
            Self::Cancelled => "cancelled",
            Self::Filled => "filled",
            Self::ExitPending => "exit_pending",
            Self::Error => "error",
        }
    }

    #[must_use]
    pub const fn is_open_like(self) -> bool {
        matches!(self, Self::Planned | Self::Open | Self::ExitPending)
    }
}

impl FromStr for ManagedRewardOrderStatus {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "planned" => Ok(Self::Planned),
            "open" => Ok(Self::Open),
            "cancelled" => Ok(Self::Cancelled),
            "filled" => Ok(Self::Filled),
            "exit_pending" => Ok(Self::ExitPending),
            "error" => Ok(Self::Error),
            other => Err(AppError::invalid_input(
                "REWARD_ORDER_STATUS_INVALID",
                format!("unknown reward order status: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardRiskSeverity {
    Info,
    Warning,
    Critical,
}

impl RewardRiskSeverity {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }
}

impl FromStr for RewardRiskSeverity {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "info" => Ok(Self::Info),
            "warning" => Ok(Self::Warning),
            "critical" => Ok(Self::Critical),
            other => Err(AppError::invalid_input(
                "REWARD_RISK_SEVERITY_INVALID",
                format!("unknown reward risk severity: {other}"),
            )),
        }
    }
}

/// Parameters extracted from `RewardBotConfig` for SQL-level candidate market
/// filtering. These are the conditions that can be pushed into the database
/// query, leaving only orderbook-dependent checks for the Rust planner.
#[derive(Debug, Clone)]
pub struct RewardCandidateFilter {
    /// Minimum total_daily_rate (from `config.min_daily_reward`).
    pub min_daily_reward: Decimal,
    /// Minimum midpoint value (from `config.min_midpoint`).
    pub min_midpoint: Decimal,
    /// Maximum midpoint value (from `config.max_midpoint`).
    pub max_midpoint: Decimal,
    pub min_market_liquidity_usd: Decimal,
    pub min_market_volume_24h_usd: Decimal,
    pub min_hours_to_end: u64,
    pub max_market_spread_cents: Decimal,
    pub max_market_data_age_minutes: u64,
    pub max_rewards_spread_cents: Decimal,
    pub allow_dominant_single_side: bool,
    pub dominant_min_probability: Decimal,
    pub dominant_max_probability: Decimal,
    /// Low-competition discovery uses the same hard filters but a different
    /// ordering so low-liquidity candidates are not starved by standard ranking.
    pub prefer_low_competition_ordering: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardBotConfig {
    pub enabled: bool,
    pub account_id: String,
    pub max_markets: u16,
    pub max_open_orders: u16,
    pub per_market_usd: Decimal,
    pub quote_size_usd: Decimal,
    pub min_daily_reward: Decimal,
    /// Minimum Gamma-reported CLOB liquidity required for a new quote.
    pub min_market_liquidity_usd: Decimal,
    /// Minimum Gamma-reported 24h volume required for a new quote.
    pub min_market_volume_24h_usd: Decimal,
    /// Minimum known time remaining before market settlement.
    pub min_hours_to_end: u64,
    /// Maximum Gamma top-of-book bid/ask spread accepted for a candidate.
    pub max_market_spread_cents: Decimal,
    /// Maximum age of the synchronized Gamma market metadata.
    pub max_market_data_age_minutes: u64,
    pub min_market_score: Decimal,
    pub max_spread_cents: Decimal,
    pub quote_mode: RewardQuoteMode,
    pub selection_mode: RewardSelectionMode,
    /// Bid price level used for new YES/NO quotes (1=best bid, 3=third bid).
    pub quote_bid_rank: u16,
    /// Allow auto mode to quote only the dominant outcome in one-sided markets.
    pub dominant_single_side_enabled: bool,
    pub dominant_min_probability: Decimal,
    pub dominant_max_probability: Decimal,
    pub dominant_min_exit_depth_usd: Decimal,
    pub max_top1_depth_share: Decimal,
    pub max_top3_depth_share: Decimal,
    pub max_book_hhi: Decimal,
    pub preferred_categories: Vec<String>,
    pub preferred_category_score_bonus: Decimal,
    /// Unified opportunity scoring is applied to every reward market plan. It
    /// uses competition, reward density, exit depth and book stability as score
    /// adjustments, without creating a separate execution sleeve.
    pub opportunity_metrics_enabled: bool,
    pub opportunity_probe_notional_usd: Decimal,
    pub opportunity_min_reward_per_100_usd_day: Decimal,
    pub opportunity_max_competition_multiple: Decimal,
    pub opportunity_max_account_allocation_bps: u16,
    pub opportunity_max_market_allocation_bps: u16,
    pub opportunity_min_exit_depth_usd: Decimal,
    pub opportunity_min_exit_depth_multiple: Decimal,
    pub opportunity_max_entry_exit_slippage_cents: Decimal,
    pub opportunity_max_bad_fill_recovery_days: Decimal,
    pub opportunity_observation_window_sec: u64,
    pub opportunity_min_book_samples: u64,
    pub opportunity_max_midpoint_range_cents: Decimal,
    pub opportunity_max_top_of_book_flip_count: u64,
    pub opportunity_reward_weight: Decimal,
    pub opportunity_competition_weight: Decimal,
    pub opportunity_exit_weight: Decimal,
    pub opportunity_stability_weight: Decimal,
    pub low_competition_mode: RewardLowCompetitionMode,
    pub low_competition_max_markets: u16,
    pub low_competition_max_open_orders: u16,
    pub low_competition_per_market_usd: Decimal,
    pub low_competition_max_position_usd: Decimal,
    pub low_competition_probe_notional_usd: Decimal,
    pub low_competition_min_competition_share_bps: u16,
    pub low_competition_max_competition_multiple: Decimal,
    /// 早期剔除阈值：候选 competition_multiple 超过该值时判定为"伪低竞争"
    /// （高竞争市场混入），仅用于下游 prewarm/observation 降级，不进入正式 gate。
    pub low_competition_candidate_max_competition_multiple: Decimal,
    pub low_competition_max_account_allocation_bps: u16,
    pub low_competition_max_market_allocation_bps: u16,
    pub low_competition_candidate_liquidity_filter_enabled: bool,
    pub low_competition_candidate_volume_filter_enabled: bool,
    pub low_competition_min_market_liquidity_usd: Decimal,
    pub low_competition_min_market_volume_24h_usd: Decimal,
    pub low_competition_max_competition_usd: Decimal,
    pub low_competition_min_reward_per_100_usd_day: Decimal,
    pub low_competition_min_exit_depth_usd: Decimal,
    pub low_competition_min_exit_depth_multiple: Decimal,
    pub low_competition_max_entry_exit_slippage_cents: Decimal,
    pub low_competition_max_bad_fill_recovery_days: Decimal,
    pub low_competition_max_midpoint_range_cents: Decimal,
    pub low_competition_max_top_of_book_flip_count: u64,
    pub low_competition_observation_window_sec: u64,
    pub low_competition_min_book_samples: u64,
    pub low_competition_quote_bid_rank: u16,
    pub low_competition_safety_margin_cents: Decimal,
    pub low_competition_max_spread_cents: Decimal,
    pub low_competition_max_market_spread_cents: Decimal,
    pub low_competition_min_market_score: Decimal,
    pub low_competition_require_ai_allow: bool,
    pub low_competition_info_risk_avoid_level: RewardInfoRiskLevel,
    pub low_competition_cancel_confirm_sec: u64,
    pub low_competition_cancel_share_threshold_ratio_bps: u16,
    pub low_competition_cancel_competition_multiple_factor: Decimal,
    pub low_competition_cancel_max_exit_slippage_cents: Decimal,
    pub low_competition_cancel_min_exit_depth_usd: Decimal,
    pub low_competition_cancel_exit_depth_multiple: Decimal,
    pub low_competition_cancel_midpoint_range_floor_cents: Decimal,
    pub low_competition_global_open_order_share_bps: u16,
    pub ai_advisory_enabled: bool,
    pub ai_provider: RewardAiProvider,
    pub ai_request_format: RewardAiRequestFormat,
    pub ai_advisory_ttl_sec: u64,
    /// Number of advisory markets to send in one provider request. 1 = single-market calls.
    pub ai_advisory_batch_size: u16,
    /// Apply provider strategy hints directly to live quote mode, bid rank and
    /// condition notional caps. Hints never bypass deterministic hard risk.
    pub ai_strategy_hint_enabled: bool,
    /// Minimum provider confidence required before live quote strategy hints
    /// affect quoting. The binary allow/avoid gate still applies independently.
    pub ai_strategy_hint_min_confidence: Decimal,
    pub info_risk_enabled: bool,
    pub info_risk_mode: RewardSelectionMode,
    pub info_risk_avoid_level: RewardInfoRiskLevel,
    pub info_risk_ttl_sec: u64,
    /// Number of info-risk markets to send in one provider request. 1 = single-market calls.
    pub info_risk_batch_size: u16,
    /// When info-risk enforce mode is active, require a cached provider risk
    /// result before the first live BUY quote for a condition.
    pub require_info_risk_before_first_quote: bool,
    /// Minimum observation time before first live BUY quote for a condition.
    /// Existing open orders/positions bypass this so risk updates can still
    /// manage active inventory.
    pub first_quote_quarantine_sec: u64,
    pub safety_margin_cents: Decimal,
    pub min_midpoint: Decimal,
    pub max_midpoint: Decimal,
    pub stale_book_ms: u64,
    pub min_scoring_check_sec: u64,
    pub max_position_usd: Decimal,
    pub max_global_position_usd: Decimal,
    pub exit_markup_cents: Decimal,
    pub cancel_on_fill: bool,
    /// Total fund pool shared across every market. Resting buy orders reuse
    /// this pool; cash is consumed only when fills occur.
    pub account_capital_usd: Decimal,
    /// Cancel and re-quote when the selected bid-level target moves by more
    /// than this many cents after the reprice guard confirms the move.
    pub requote_drift_cents: Decimal,
    /// Require the target drift to persist for this many seconds before
    /// replacing a resting quote. 0 = no history confirmation.
    pub requote_drift_confirm_sec: u64,
    /// Minimum age before a resting order can be cancelled only for drift.
    pub requote_drift_cooldown_sec: u64,
    /// Maximum orders cancelled for drift in one reconcile cycle. Hard risk
    /// cancels are not limited by this setting. 0 = drift reprice disabled.
    pub requote_drift_max_cancels_per_cycle: u16,
    /// What to do with inventory once a quote leg is filled.
    pub post_fill_strategy: PostFillStrategy,
    // -- Risk control fields (0 = disabled) --
    /// Minimum external bid depth (USD) at or above our order price to keep resting.
    /// The managed order's own remaining notional is excluded.
    pub min_depth_usd: Decimal,
    /// Cancel when our order's bid rank rises to this level or better (1=best).
    /// E.g. 2 = cancel when promoted to bid-1 or bid-2. 0 = disabled.
    pub cancel_bid_rank: u16,
    /// Cancel when the top-N bid depth drops by this percentage within the
    /// detection window. E.g. 30 = cancel on 30% drop. 0 = disabled.
    pub depth_drop_pct: Decimal,
    /// Sliding window (seconds) for depth-drop detection.
    pub depth_drop_window_sec: u64,
    /// Cancel when ask-side depth decreases by this USD amount within the
    /// window (inferred as aggressive taker fills). 0 = disabled.
    pub fill_velocity_usd: Decimal,
    /// Sliding window (seconds) for fill-velocity detection.
    pub fill_velocity_window_sec: u64,
    /// Cancel when total bid depth shrinks by this percentage within the
    /// window (inferred as mass cancel by other makers). 0 = disabled.
    pub mass_cancel_pct: Decimal,
    /// Sliding window (seconds) for mass-cancel detection.
    pub mass_cancel_window_sec: u64,
    /// Force-cancel and re-place resting orders after this many seconds to
    /// stay at the back of the queue. 0 = disabled.
    pub requote_interval_sec: u64,
    /// Random jitter added to requote interval (0..jitter seconds).
    pub requote_jitter_sec: u64,
    /// How often the fast reconcile loop runs (seconds). Full cycle remains
    /// at the worker's poll_interval_secs.
    pub reconcile_interval_sec: u64,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct RewardBotConfigPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_markets: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_open_orders: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_market_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote_size_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_daily_reward: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_market_liquidity_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_market_volume_24h_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_hours_to_end: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_market_spread_cents: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_market_data_age_minutes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_market_score: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_spread_cents: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote_mode: Option<RewardQuoteMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_mode: Option<RewardSelectionMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote_bid_rank: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dominant_single_side_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dominant_min_probability: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dominant_max_probability: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dominant_min_exit_depth_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_top1_depth_share: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_top3_depth_share: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_book_hhi: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_categories: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_category_score_bonus: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opportunity_metrics_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opportunity_probe_notional_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opportunity_min_reward_per_100_usd_day: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opportunity_max_competition_multiple: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opportunity_max_account_allocation_bps: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opportunity_max_market_allocation_bps: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opportunity_min_exit_depth_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opportunity_min_exit_depth_multiple: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opportunity_max_entry_exit_slippage_cents: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opportunity_max_bad_fill_recovery_days: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opportunity_observation_window_sec: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opportunity_min_book_samples: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opportunity_max_midpoint_range_cents: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opportunity_max_top_of_book_flip_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opportunity_reward_weight: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opportunity_competition_weight: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opportunity_exit_weight: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opportunity_stability_weight: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_mode: Option<RewardLowCompetitionMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_max_markets: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_max_open_orders: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_per_market_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_max_position_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_probe_notional_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_min_competition_share_bps: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_max_competition_multiple: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_candidate_max_competition_multiple: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_max_account_allocation_bps: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_max_market_allocation_bps: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_candidate_liquidity_filter_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_candidate_volume_filter_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_min_market_liquidity_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_min_market_volume_24h_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_max_competition_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_min_reward_per_100_usd_day: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_min_exit_depth_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_min_exit_depth_multiple: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_max_entry_exit_slippage_cents: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_max_bad_fill_recovery_days: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_max_midpoint_range_cents: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_max_top_of_book_flip_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_observation_window_sec: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_min_book_samples: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_quote_bid_rank: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_safety_margin_cents: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_max_spread_cents: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_max_market_spread_cents: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_min_market_score: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_require_ai_allow: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_info_risk_avoid_level: Option<RewardInfoRiskLevel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_cancel_confirm_sec: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_cancel_share_threshold_ratio_bps: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_cancel_competition_multiple_factor: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_cancel_max_exit_slippage_cents: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_cancel_min_exit_depth_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_cancel_exit_depth_multiple: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_cancel_midpoint_range_floor_cents: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_competition_global_open_order_share_bps: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_advisory_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_provider: Option<RewardAiProvider>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_request_format: Option<RewardAiRequestFormat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_advisory_ttl_sec: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_advisory_batch_size: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_strategy_hint_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_strategy_hint_min_confidence: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info_risk_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info_risk_mode: Option<RewardSelectionMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info_risk_avoid_level: Option<RewardInfoRiskLevel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info_risk_ttl_sec: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info_risk_batch_size: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub require_info_risk_before_first_quote: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_quote_quarantine_sec: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safety_margin_cents: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_midpoint: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_midpoint: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_book_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_scoring_check_sec: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_position_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_global_position_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_markup_cents: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancel_on_fill: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_capital_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requote_drift_cents: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requote_drift_confirm_sec: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requote_drift_cooldown_sec: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requote_drift_max_cancels_per_cycle: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_fill_strategy: Option<PostFillStrategy>,
    // -- Risk control fields --
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_depth_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancel_bid_rank: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth_drop_pct: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth_drop_window_sec: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_velocity_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_velocity_window_sec: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mass_cancel_pct: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mass_cancel_window_sec: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requote_interval_sec: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requote_jitter_sec: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconcile_interval_sec: Option<u64>,
}
