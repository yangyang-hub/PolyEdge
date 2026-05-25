#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardBotMode {
    DryRun,
    Live,
}

impl RewardBotMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DryRun => "dry_run",
            Self::Live => "live",
        }
    }
}

impl FromStr for RewardBotMode {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "dry_run" => Ok(Self::DryRun),
            "live" => Ok(Self::Live),
            other => Err(AppError::invalid_input(
                "REWARD_BOT_MODE_INVALID",
                format!("unknown reward bot mode: {other}"),
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
    pub mode: RewardBotMode,
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
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct RewardBotConfigPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<RewardBotMode>,
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
}

impl Default for RewardBotConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: RewardBotMode::DryRun,
            account_id: "reward_simulator".to_string(),
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
        }
    }
}

impl RewardBotConfig {
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.account_id = normalize_account_id(&self.account_id);
        self.max_markets = clamp_u16(self.max_markets, 1, 50);
        self.max_open_orders = clamp_u16(self.max_open_orders, 1, 200);
        self.per_market_usd = clamp_decimal(self.per_market_usd, decimal("1"), decimal("100000"));
        self.quote_size_usd = clamp_decimal(self.quote_size_usd, decimal("1"), self.per_market_usd);
        self.min_daily_reward =
            clamp_decimal(self.min_daily_reward, Decimal::ZERO, decimal("100000"));
        self.min_market_score = clamp_decimal(self.min_market_score, Decimal::ZERO, decimal("100"));
        self.max_spread_cents = clamp_decimal(self.max_spread_cents, decimal("0.1"), decimal("99"));
        self.quote_edge_cents = clamp_decimal(self.quote_edge_cents, Decimal::ZERO, decimal("50"));
        self.safety_margin_cents =
            clamp_decimal(self.safety_margin_cents, Decimal::ZERO, decimal("20"));
        self.min_midpoint = clamp_decimal(self.min_midpoint, decimal("0.01"), decimal("0.49"));
        self.max_midpoint = clamp_decimal(self.max_midpoint, decimal("0.51"), decimal("0.99"));
        if self.max_midpoint <= self.min_midpoint {
            self.max_midpoint = Decimal::min(decimal("0.99"), self.min_midpoint + decimal("0.1"));
        }
        self.stale_book_ms = self.stale_book_ms.clamp(1_000, 120_000);
        self.min_scoring_check_sec = self.min_scoring_check_sec.clamp(15, 600);
        self.max_position_usd =
            clamp_decimal(self.max_position_usd, decimal("1"), decimal("100000"));
        self.max_global_position_usd = clamp_decimal(
            self.max_global_position_usd,
            decimal("1"),
            decimal("1000000"),
        );
        self.exit_markup_cents =
            clamp_decimal(self.exit_markup_cents, Decimal::ZERO, decimal("50"));
        self
    }

    #[must_use]
    pub fn apply_patch(&self, patch: RewardBotConfigPatch) -> Self {
        let mut next = self.clone();
        if let Some(enabled) = patch.enabled {
            next.enabled = enabled;
        }
        if let Some(mode) = patch.mode {
            next.mode = mode;
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
    pub mode: RewardBotMode,
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
    pub markets: Vec<RewardMarket>,
    pub quote_plans: Vec<RewardQuotePlan>,
    pub orders: Vec<ManagedRewardOrder>,
    pub positions: Vec<RewardPosition>,
    pub events: Vec<RewardRiskEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RewardBotRunReport {
    pub markets_scanned: usize,
    pub books_fetched: usize,
    pub plans_built: usize,
    pub eligible_plans: usize,
    pub simulated_orders: usize,
    pub cancelled_orders: usize,
}

#[derive(Debug, Clone)]
struct TokenBookState {
    midpoint: Decimal,
    best_ask: Option<Decimal>,
    fresh: bool,
}
