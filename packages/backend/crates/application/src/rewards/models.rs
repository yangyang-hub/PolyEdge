#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardExecutionMode {
    /// Deprecated: legacy event-validation mode, kept for serde backward compatibility.
    Validation,
    /// Live mode: worker submits/cancels Polymarket orders through the rewards
    /// live executor.
    Live,
}

impl RewardExecutionMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Validation => "live",
            Self::Live => "live",
        }
    }

    /// Always returns false — validation mode is deprecated.
    #[must_use]
    pub const fn is_validation(self) -> bool {
        false
    }

    #[must_use]
    pub const fn is_live(self) -> bool {
        true
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
    /// Rest a reverse sell order at `avg_price + exit_markup_cents` to take profit.
    ExitAtMarkup,
    /// Keep the filled inventory and keep quoting the market for more rewards.
    HoldAndRequote,
    /// Immediately cross the opposite book at market to flatten the position.
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardBotConfig {
    pub enabled: bool,
    pub execution_mode: RewardExecutionMode,
    pub account_id: String,
    pub max_markets: u16,
    pub max_open_orders: u16,
    pub per_market_usd: Decimal,
    pub quote_size_usd: Decimal,
    pub min_daily_reward: Decimal,
    pub min_market_score: Decimal,
    pub max_spread_cents: Decimal,
    pub quote_edge_cents: Decimal,
    pub safety_margin_cents: Decimal,
    pub min_midpoint: Decimal,
    pub max_midpoint: Decimal,
    pub stale_book_ms: u64,
    pub min_scoring_check_sec: u64,
    pub max_position_usd: Decimal,
    pub max_global_position_usd: Decimal,
    pub exit_markup_cents: Decimal,
    pub cancel_on_fill: bool,
    /// Total validation fund pool shared across every market. Resting validation
    /// buy orders reuse this pool; cash is consumed only when validation fills occur.
    pub account_capital_usd: Decimal,
    /// Legacy dry-run competition multiplier. Reward accrual now requires a
    /// fresh cached book and measures competing depth directly from that book.
    pub reward_competition_factor: Decimal,
    /// Polymarket single-sided divisor `c` in the `Qmin` formula.
    pub single_sided_divisor_c: Decimal,
    /// Per-tick fill probability for a resting order whose price merely touches the
    /// opposite top of book (orders crossed through always fill).
    pub fill_rate_per_tick: Decimal,
    /// Fraction of an order's remaining size consumed by a single fill event.
    pub max_fill_ratio: Decimal,
    /// Cancel and re-quote when the midpoint drifts more than this many cents.
    pub requote_drift_cents: Decimal,
    /// What to do with inventory once a quote leg is filled.
    pub post_fill_strategy: PostFillStrategy,
    // -- Risk control fields (0 = disabled) --
    /// Minimum total bid depth (USD) above our order price to keep resting.
    /// Cancels when the book above us is thinner than this threshold.
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
    pub execution_mode: Option<RewardExecutionMode>,
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
    pub min_market_score: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_spread_cents: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote_edge_cents: Option<Decimal>,
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
    pub reward_competition_factor: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub single_sided_divisor_c: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_rate_per_tick: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_fill_ratio: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requote_drift_cents: Option<Decimal>,
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

impl Default for RewardBotConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            execution_mode: RewardExecutionMode::Live,
            account_id: "reward_validator".to_string(),
            max_markets: 3,
            max_open_orders: 12,
            per_market_usd: decimal("20"),
            quote_size_usd: decimal("10"),
            min_daily_reward: decimal("1"),
            min_market_score: decimal("15"),
            max_spread_cents: decimal("8"),
            quote_edge_cents: decimal("1"),
            safety_margin_cents: decimal("1"),
            min_midpoint: decimal("0.1"),
            max_midpoint: decimal("0.9"),
            stale_book_ms: 10_000,
            min_scoring_check_sec: 45,
            max_position_usd: decimal("20"),
            max_global_position_usd: decimal("50"),
            exit_markup_cents: decimal("1"),
            cancel_on_fill: true,
            account_capital_usd: decimal("1000"),
            reward_competition_factor: decimal("4"),
            single_sided_divisor_c: decimal("3"),
            fill_rate_per_tick: decimal("0.2"),
            max_fill_ratio: decimal("1"),
            requote_drift_cents: decimal("2"),
            post_fill_strategy: PostFillStrategy::ExitAtMarkup,
            // Risk control defaults: all disabled (0 = off)
            min_depth_usd: Decimal::ZERO,
            cancel_bid_rank: 0,
            depth_drop_pct: Decimal::ZERO,
            depth_drop_window_sec: 10,
            fill_velocity_usd: Decimal::ZERO,
            fill_velocity_window_sec: 10,
            mass_cancel_pct: Decimal::ZERO,
            mass_cancel_window_sec: 10,
            requote_interval_sec: 0,
            requote_jitter_sec: 0,
            reconcile_interval_sec: 5,
        }
    }
}

impl RewardBotConfig {
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.account_id = normalize_account_id(&self.account_id);
        self.max_markets = clamp_u16(self.max_markets, 0, u16::MAX);
        self.max_open_orders = clamp_u16(self.max_open_orders, 0, u16::MAX);
        self.per_market_usd = clamp_decimal(self.per_market_usd, Decimal::ZERO, decimal("1000000"));
        let quote_size_cap = if self.per_market_usd == Decimal::ZERO {
            decimal("1000000")
        } else {
            self.per_market_usd
        };
        self.quote_size_usd = clamp_decimal(self.quote_size_usd, Decimal::ZERO, quote_size_cap);
        self.min_daily_reward =
            clamp_decimal(self.min_daily_reward, Decimal::ZERO, decimal("100000"));
        self.min_market_score = clamp_decimal(self.min_market_score, Decimal::ZERO, decimal("100"));
        self.max_spread_cents =
            clamp_decimal(self.max_spread_cents, decimal("0.1"), decimal("1000"));
        self.quote_edge_cents = clamp_decimal(self.quote_edge_cents, Decimal::ZERO, decimal("50"));
        self.safety_margin_cents =
            clamp_decimal(self.safety_margin_cents, Decimal::ZERO, decimal("20"));
        self.min_midpoint = clamp_decimal(self.min_midpoint, Decimal::ZERO, decimal("0.49"));
        self.max_midpoint = clamp_decimal(self.max_midpoint, decimal("0.51"), Decimal::ONE);
        if self.max_midpoint <= self.min_midpoint {
            self.max_midpoint = Decimal::min(Decimal::ONE, self.min_midpoint + decimal("0.1"));
        }
        self.stale_book_ms = self.stale_book_ms.clamp(0, 120_000);
        self.min_scoring_check_sec = self.min_scoring_check_sec.clamp(0, 600);
        self.max_position_usd =
            clamp_decimal(self.max_position_usd, Decimal::ZERO, decimal("1000000"));
        self.max_global_position_usd = clamp_decimal(
            self.max_global_position_usd,
            Decimal::ZERO,
            decimal("10000000"),
        );
        self.exit_markup_cents =
            clamp_decimal(self.exit_markup_cents, Decimal::ZERO, decimal("50"));
        self.account_capital_usd =
            clamp_decimal(self.account_capital_usd, decimal("1"), decimal("100000000"));
        self.reward_competition_factor = clamp_decimal(
            self.reward_competition_factor,
            decimal("1"),
            decimal("10000"),
        );
        self.single_sided_divisor_c =
            clamp_decimal(self.single_sided_divisor_c, decimal("1"), decimal("100"));
        self.fill_rate_per_tick =
            clamp_decimal(self.fill_rate_per_tick, Decimal::ZERO, Decimal::ONE);
        self.max_fill_ratio = clamp_decimal(self.max_fill_ratio, decimal("0.01"), Decimal::ONE);
        self.requote_drift_cents =
            clamp_decimal(self.requote_drift_cents, Decimal::ZERO, decimal("99"));
        // Risk control clamps
        self.min_depth_usd =
            clamp_decimal(self.min_depth_usd, Decimal::ZERO, decimal("1000000"));
        self.cancel_bid_rank = self.cancel_bid_rank.clamp(0, 20);
        self.depth_drop_pct =
            clamp_decimal(self.depth_drop_pct, Decimal::ZERO, decimal("100"));
        self.depth_drop_window_sec = self.depth_drop_window_sec.clamp(0, 300);
        self.fill_velocity_usd =
            clamp_decimal(self.fill_velocity_usd, Decimal::ZERO, decimal("1000000"));
        self.fill_velocity_window_sec = self.fill_velocity_window_sec.clamp(0, 300);
        self.mass_cancel_pct =
            clamp_decimal(self.mass_cancel_pct, Decimal::ZERO, decimal("100"));
        self.mass_cancel_window_sec = self.mass_cancel_window_sec.clamp(0, 300);
        self.requote_interval_sec = self.requote_interval_sec.clamp(0, 3600);
        self.requote_jitter_sec = self.requote_jitter_sec.clamp(0, 600);
        self.reconcile_interval_sec = self.reconcile_interval_sec.clamp(1, 60);
        self
    }

    #[must_use]
    pub fn apply_patch(&self, patch: RewardBotConfigPatch) -> Self {
        let mut next = self.clone();
        if let Some(enabled) = patch.enabled {
            next.enabled = enabled;
        }
        if let Some(execution_mode) = patch.execution_mode {
            next.execution_mode = execution_mode;
        }
        if let Some(account_id) = patch.account_id {
            next.account_id = account_id;
        }
        if let Some(max_markets) = patch.max_markets {
            next.max_markets = max_markets;
        }
        if let Some(max_open_orders) = patch.max_open_orders {
            next.max_open_orders = max_open_orders;
        }
        if let Some(per_market_usd) = patch.per_market_usd {
            next.per_market_usd = per_market_usd;
        }
        if let Some(quote_size_usd) = patch.quote_size_usd {
            next.quote_size_usd = quote_size_usd;
        }
        if let Some(min_daily_reward) = patch.min_daily_reward {
            next.min_daily_reward = min_daily_reward;
        }
        if let Some(min_market_score) = patch.min_market_score {
            next.min_market_score = min_market_score;
        }
        if let Some(max_spread_cents) = patch.max_spread_cents {
            next.max_spread_cents = max_spread_cents;
        }
        if let Some(quote_edge_cents) = patch.quote_edge_cents {
            next.quote_edge_cents = quote_edge_cents;
        }
        if let Some(safety_margin_cents) = patch.safety_margin_cents {
            next.safety_margin_cents = safety_margin_cents;
        }
        if let Some(min_midpoint) = patch.min_midpoint {
            next.min_midpoint = min_midpoint;
        }
        if let Some(max_midpoint) = patch.max_midpoint {
            next.max_midpoint = max_midpoint;
        }
        if let Some(stale_book_ms) = patch.stale_book_ms {
            next.stale_book_ms = stale_book_ms;
        }
        if let Some(min_scoring_check_sec) = patch.min_scoring_check_sec {
            next.min_scoring_check_sec = min_scoring_check_sec;
        }
        if let Some(max_position_usd) = patch.max_position_usd {
            next.max_position_usd = max_position_usd;
        }
        if let Some(max_global_position_usd) = patch.max_global_position_usd {
            next.max_global_position_usd = max_global_position_usd;
        }
        if let Some(exit_markup_cents) = patch.exit_markup_cents {
            next.exit_markup_cents = exit_markup_cents;
        }
        if let Some(cancel_on_fill) = patch.cancel_on_fill {
            next.cancel_on_fill = cancel_on_fill;
        }
        if let Some(account_capital_usd) = patch.account_capital_usd {
            next.account_capital_usd = account_capital_usd;
        }
        if let Some(reward_competition_factor) = patch.reward_competition_factor {
            next.reward_competition_factor = reward_competition_factor;
        }
        if let Some(single_sided_divisor_c) = patch.single_sided_divisor_c {
            next.single_sided_divisor_c = single_sided_divisor_c;
        }
        if let Some(fill_rate_per_tick) = patch.fill_rate_per_tick {
            next.fill_rate_per_tick = fill_rate_per_tick;
        }
        if let Some(max_fill_ratio) = patch.max_fill_ratio {
            next.max_fill_ratio = max_fill_ratio;
        }
        if let Some(requote_drift_cents) = patch.requote_drift_cents {
            next.requote_drift_cents = requote_drift_cents;
        }
        if let Some(post_fill_strategy) = patch.post_fill_strategy {
            next.post_fill_strategy = post_fill_strategy;
        }
        // Risk control patches
        if let Some(v) = patch.min_depth_usd { next.min_depth_usd = v; }
        if let Some(v) = patch.cancel_bid_rank { next.cancel_bid_rank = v; }
        if let Some(v) = patch.depth_drop_pct { next.depth_drop_pct = v; }
        if let Some(v) = patch.depth_drop_window_sec { next.depth_drop_window_sec = v; }
        if let Some(v) = patch.fill_velocity_usd { next.fill_velocity_usd = v; }
        if let Some(v) = patch.fill_velocity_window_sec { next.fill_velocity_window_sec = v; }
        if let Some(v) = patch.mass_cancel_pct { next.mass_cancel_pct = v; }
        if let Some(v) = patch.mass_cancel_window_sec { next.mass_cancel_window_sec = v; }
        if let Some(v) = patch.requote_interval_sec { next.requote_interval_sec = v; }
        if let Some(v) = patch.requote_jitter_sec { next.requote_jitter_sec = v; }
        if let Some(v) = patch.reconcile_interval_sec { next.reconcile_interval_sec = v; }
        next.normalized()
    }
}

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
    pub image: String,
    pub rewards_max_spread: Decimal,
    pub rewards_min_size: Decimal,
    pub total_daily_rate: Decimal,
    pub tokens: Vec<RewardToken>,
    pub active: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
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
pub struct RewardQuotePlan {
    pub condition_id: String,
    pub market_slug: String,
    pub question: String,
    pub score: Decimal,
    pub eligible: bool,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub midpoint: Option<Decimal>,
    pub total_daily_rate: Decimal,
    pub rewards_max_spread: Decimal,
    pub rewards_min_size: Decimal,
    pub legs: Vec<RewardQuoteLeg>,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
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

/// Validation fund-pool ledger shared across every market the bot quotes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardAccountState {
    pub account_id: String,
    /// Total deposited capital (the configured fund pool).
    pub capital_usd: Decimal,
    /// Cash not consumed by validation fills.
    pub available_usd: Decimal,
    /// Legacy hard-reserve field. New rewards validation ticks release it and
    /// keep resting buy reservations soft across markets.
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
            capital_usd,
            available_usd: capital_usd,
            reserved_usd: Decimal::ZERO,
            realized_pnl: Decimal::ZERO,
            reward_earned_usd: Decimal::ZERO,
            fees_paid: Decimal::ZERO,
            tick_index: 0,
            updated_at: now,
        }
    }
}

/// One validation/live execution event against a managed order (maker fill) or a
/// taker flatten. Drives the "吃单" (order-taken) detail view on the frontend.
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
pub struct RewardBotSnapshot {
    pub config: RewardBotConfig,
    pub status: RewardBotStatus,
    pub account: RewardAccountState,
    pub markets: Vec<RewardMarket>,
    pub quote_plans: Vec<RewardQuotePlan>,
    pub orders: Vec<ManagedRewardOrder>,
    pub orders_page: RewardListPage,
    pub positions: Vec<RewardPosition>,
    pub fills: Vec<RewardFill>,
    pub events: Vec<RewardRiskEvent>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RewardLiveCycle {
    pub config: RewardBotConfig,
    pub account: RewardAccountState,
    pub markets: Vec<RewardMarket>,
    pub plans: Vec<RewardQuotePlan>,
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
    pub simulated_orders: usize,
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
}
