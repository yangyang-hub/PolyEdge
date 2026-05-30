#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopyTradeMode {
    Paper,
    Live,
}

impl CopyTradeMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Paper => "paper",
            Self::Live => "live",
        }
    }
}

impl FromStr for CopyTradeMode {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "paper" => Ok(Self::Paper),
            "live" => Ok(Self::Live),
            other => Err(AppError::invalid_input(
                "COPYTRADE_MODE_INVALID",
                format!("unknown copytrade mode: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopySizingMode {
    FixedUsd,
    ProportionalToSource,
    CapitalRatio,
    MirrorPortfolioWeight,
}

impl CopySizingMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FixedUsd => "fixed_usd",
            Self::ProportionalToSource => "proportional_to_source",
            Self::CapitalRatio => "capital_ratio",
            Self::MirrorPortfolioWeight => "mirror_portfolio_weight",
        }
    }
}

impl FromStr for CopySizingMode {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "fixed_usd" => Ok(Self::FixedUsd),
            "proportional_to_source" => Ok(Self::ProportionalToSource),
            "capital_ratio" => Ok(Self::CapitalRatio),
            "mirror_portfolio_weight" => Ok(Self::MirrorPortfolioWeight),
            other => Err(AppError::invalid_input(
                "COPYTRADE_SIZING_MODE_INVALID",
                format!("unknown copytrade sizing mode: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopyOrderSide {
    Buy,
    Sell,
}

impl CopyOrderSide {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
        }
    }
}

impl FromStr for CopyOrderSide {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "buy" => Ok(Self::Buy),
            "sell" => Ok(Self::Sell),
            other => Err(AppError::invalid_input(
                "COPYTRADE_ORDER_SIDE_INVALID",
                format!("unknown copytrade order side: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopyOrderStatus {
    Planned,
    Open,
    Filled,
    Cancelled,
    Skipped,
    Error,
}

impl CopyOrderStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Open => "open",
            Self::Filled => "filled",
            Self::Cancelled => "cancelled",
            Self::Skipped => "skipped",
            Self::Error => "error",
        }
    }

    #[must_use]
    pub const fn is_open_like(self) -> bool {
        matches!(self, Self::Planned | Self::Open)
    }
}

impl FromStr for CopyOrderStatus {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "planned" => Ok(Self::Planned),
            "open" => Ok(Self::Open),
            "filled" => Ok(Self::Filled),
            "cancelled" => Ok(Self::Cancelled),
            "skipped" => Ok(Self::Skipped),
            "error" => Ok(Self::Error),
            other => Err(AppError::invalid_input(
                "COPYTRADE_ORDER_STATUS_INVALID",
                format!("unknown copytrade order status: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackedWalletStatus {
    Active,
    Paused,
}

impl TrackedWalletStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
        }
    }
}

impl FromStr for TrackedWalletStatus {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "active" => Ok(Self::Active),
            "paused" => Ok(Self::Paused),
            other => Err(AppError::invalid_input(
                "COPYTRADE_WALLET_STATUS_INVALID",
                format!("unknown tracked wallet status: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopyEventSeverity {
    Info,
    Warning,
    Critical,
}

impl CopyEventSeverity {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }
}

impl FromStr for CopyEventSeverity {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "info" => Ok(Self::Info),
            "warning" => Ok(Self::Warning),
            "critical" => Ok(Self::Critical),
            other => Err(AppError::invalid_input(
                "COPYTRADE_EVENT_SEVERITY_INVALID",
                format!("unknown copytrade event severity: {other}"),
            )),
        }
    }
}

// ── Config ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CopyTradeConfig {
    pub enabled: bool,
    pub mode: CopyTradeMode,
    pub account_id: String,
    pub account_capital_usd: Decimal,
    pub sizing_mode: CopySizingMode,
    /// When `sizing_mode` is `FixedUsd`: notional USD per copied trade.
    pub fixed_usd_per_trade: Decimal,
    /// When `sizing_mode` is `ProportionalToSource`: our_size = source_size * factor.
    pub proportional_factor: Decimal,
    /// When `sizing_mode` is `CapitalRatio`: share of our capital allocated pro-rata
    /// to the source wallet's portfolio, applied per trade.
    pub capital_ratio: Decimal,
    pub min_source_trade_usd: Decimal,
    pub max_price: Decimal,
    pub min_price: Decimal,
    pub copy_sells: bool,
    pub max_position_per_market_usd: Decimal,
    pub per_wallet_max_exposure_usd: Decimal,
    pub max_total_exposure_usd: Decimal,
    pub max_open_copy_orders: u16,
    pub daily_loss_limit_usd: Decimal,
    pub cooldown_secs: u64,
    pub max_slippage_cents: Decimal,
    pub fill_rate_per_tick: Decimal,
    pub max_fill_ratio: Decimal,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CopyTradeConfigPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<CopyTradeMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_capital_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sizing_mode: Option<CopySizingMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_usd_per_trade: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proportional_factor: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capital_ratio: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_source_trade_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_price: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_price: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy_sells: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_position_per_market_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_wallet_max_exposure_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_total_exposure_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_open_copy_orders: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daily_loss_limit_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cooldown_secs: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_slippage_cents: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_rate_per_tick: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_fill_ratio: Option<Decimal>,
}

impl Default for CopyTradeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: CopyTradeMode::Paper,
            account_id: "copytrade_simulator".to_string(),
            account_capital_usd: decimal("1000"),
            sizing_mode: CopySizingMode::FixedUsd,
            fixed_usd_per_trade: decimal("20"),
            proportional_factor: decimal("0.05"),
            capital_ratio: decimal("0.02"),
            min_source_trade_usd: decimal("5"),
            max_price: decimal("0.95"),
            min_price: decimal("0.05"),
            copy_sells: true,
            max_position_per_market_usd: decimal("100"),
            per_wallet_max_exposure_usd: decimal("200"),
            max_total_exposure_usd: decimal("500"),
            max_open_copy_orders: 20,
            daily_loss_limit_usd: decimal("100"),
            cooldown_secs: 30,
            max_slippage_cents: decimal("3"),
            fill_rate_per_tick: decimal("0.3"),
            max_fill_ratio: decimal("1"),
        }
    }
}

impl CopyTradeConfig {
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.account_id = normalize_account_id(&self.account_id);
        self.account_capital_usd =
            clamp_decimal(self.account_capital_usd, decimal("1"), decimal("100000000"));
        self.fixed_usd_per_trade =
            clamp_decimal(self.fixed_usd_per_trade, decimal("1"), decimal("100000"));
        self.proportional_factor =
            clamp_decimal(self.proportional_factor, decimal("0.001"), decimal("1"));
        self.capital_ratio =
            clamp_decimal(self.capital_ratio, decimal("0.001"), decimal("1"));
        self.min_source_trade_usd =
            clamp_decimal(self.min_source_trade_usd, Decimal::ZERO, decimal("100000"));
        self.max_price = clamp_decimal(self.max_price, decimal("0.01"), Decimal::ONE);
        self.min_price = clamp_decimal(self.min_price, Decimal::ZERO, decimal("0.99"));
        if self.min_price >= self.max_price {
            self.min_price = self.max_price - decimal("0.01").max(Decimal::ZERO);
        }
        self.max_position_per_market_usd =
            clamp_decimal(self.max_position_per_market_usd, decimal("1"), decimal("1000000"));
        self.per_wallet_max_exposure_usd =
            clamp_decimal(self.per_wallet_max_exposure_usd, decimal("1"), decimal("1000000"));
        self.max_total_exposure_usd =
            clamp_decimal(self.max_total_exposure_usd, decimal("1"), decimal("10000000"));
        self.max_open_copy_orders = self.max_open_copy_orders.clamp(1, 200);
        self.daily_loss_limit_usd =
            clamp_decimal(self.daily_loss_limit_usd, Decimal::ZERO, decimal("10000000"));
        self.cooldown_secs = self.cooldown_secs.clamp(0, 3600);
        self.max_slippage_cents =
            clamp_decimal(self.max_slippage_cents, Decimal::ZERO, decimal("50"));
        self.fill_rate_per_tick =
            clamp_decimal(self.fill_rate_per_tick, Decimal::ZERO, Decimal::ONE);
        self.max_fill_ratio = clamp_decimal(self.max_fill_ratio, decimal("0.01"), Decimal::ONE);
        self
    }

    #[must_use]
    pub fn apply_patch(&self, patch: CopyTradeConfigPatch) -> Self {
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
        if let Some(account_capital_usd) = patch.account_capital_usd {
            next.account_capital_usd = account_capital_usd;
        }
        if let Some(sizing_mode) = patch.sizing_mode {
            next.sizing_mode = sizing_mode;
        }
        if let Some(fixed_usd_per_trade) = patch.fixed_usd_per_trade {
            next.fixed_usd_per_trade = fixed_usd_per_trade;
        }
        if let Some(proportional_factor) = patch.proportional_factor {
            next.proportional_factor = proportional_factor;
        }
        if let Some(capital_ratio) = patch.capital_ratio {
            next.capital_ratio = capital_ratio;
        }
        if let Some(min_source_trade_usd) = patch.min_source_trade_usd {
            next.min_source_trade_usd = min_source_trade_usd;
        }
        if let Some(max_price) = patch.max_price {
            next.max_price = max_price;
        }
        if let Some(min_price) = patch.min_price {
            next.min_price = min_price;
        }
        if let Some(copy_sells) = patch.copy_sells {
            next.copy_sells = copy_sells;
        }
        if let Some(max_position_per_market_usd) = patch.max_position_per_market_usd {
            next.max_position_per_market_usd = max_position_per_market_usd;
        }
        if let Some(per_wallet_max_exposure_usd) = patch.per_wallet_max_exposure_usd {
            next.per_wallet_max_exposure_usd = per_wallet_max_exposure_usd;
        }
        if let Some(max_total_exposure_usd) = patch.max_total_exposure_usd {
            next.max_total_exposure_usd = max_total_exposure_usd;
        }
        if let Some(max_open_copy_orders) = patch.max_open_copy_orders {
            next.max_open_copy_orders = max_open_copy_orders;
        }
        if let Some(daily_loss_limit_usd) = patch.daily_loss_limit_usd {
            next.daily_loss_limit_usd = daily_loss_limit_usd;
        }
        if let Some(cooldown_secs) = patch.cooldown_secs {
            next.cooldown_secs = cooldown_secs;
        }
        if let Some(max_slippage_cents) = patch.max_slippage_cents {
            next.max_slippage_cents = max_slippage_cents;
        }
        if let Some(fill_rate_per_tick) = patch.fill_rate_per_tick {
            next.fill_rate_per_tick = fill_rate_per_tick;
        }
        if let Some(max_fill_ratio) = patch.max_fill_ratio {
            next.max_fill_ratio = max_fill_ratio;
        }
        next.normalized()
    }
}

// ── Wallet + Analysis ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WalletAnalysisStats {
    pub trades_window: i32,
    pub volume_window_usd: Decimal,
    pub realized_pnl_window: Decimal,
    pub win_rate: Decimal,
    pub roi: Decimal,
    pub avg_trade_usd: Decimal,
    pub markets_traded: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_active_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_analyzed_at: Option<OffsetDateTime>,
}

impl Default for WalletAnalysisStats {
    fn default() -> Self {
        Self {
            trades_window: 0,
            volume_window_usd: Decimal::ZERO,
            realized_pnl_window: Decimal::ZERO,
            win_rate: Decimal::ZERO,
            roi: Decimal::ZERO,
            avg_trade_usd: Decimal::ZERO,
            markets_traded: 0,
            last_active_at: None,
            last_analyzed_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrackedWallet {
    pub address: String,
    pub label: String,
    pub status: TrackedWalletStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sizing_override: Option<CopySizingMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_exposure_override: Option<Decimal>,
    #[serde(with = "time::serde::rfc3339")]
    pub added_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub analysis: WalletAnalysisStats,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceTrade {
    pub id: String,
    pub wallet_address: String,
    pub condition_id: String,
    pub token_id: String,
    pub outcome: String,
    pub side: CopyOrderSide,
    pub price: Decimal,
    pub size: Decimal,
    pub usd_size: Decimal,
    pub title: String,
    pub source_tx_hash: String,
    #[serde(with = "time::serde::rfc3339")]
    pub source_timestamp: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
    pub copied: bool,
    pub decision_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CopyOrder {
    pub id: String,
    pub account_id: String,
    pub wallet_address: String,
    pub source_trade_id: String,
    pub condition_id: String,
    pub token_id: String,
    pub outcome: String,
    pub side: CopyOrderSide,
    pub price: Decimal,
    pub size: Decimal,
    pub notional_usd: Decimal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_order_id: Option<String>,
    pub status: CopyOrderStatus,
    pub reason: String,
    #[serde(default)]
    pub filled_size: Decimal,
    #[serde(default)]
    pub realized_pnl: Decimal,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

impl CopyOrder {
    /// Is this order status one that still has open or fillable inventory?
    #[must_use]
    pub fn remaining_size(&self) -> Decimal {
        (self.size - self.filled_size).max(Decimal::ZERO)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CopyPosition {
    pub account_id: String,
    pub wallet_address: String,
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
pub struct CopyAccountState {
    pub account_id: String,
    pub capital_usd: Decimal,
    pub available_usd: Decimal,
    pub reserved_usd: Decimal,
    pub realized_pnl: Decimal,
    /// PnL realized today (UTC date). Reset to zero when the date rolls over.
    /// Used by the daily loss limit risk check.
    #[serde(default)]
    pub daily_realized_pnl: Decimal,
    pub fees_paid: Decimal,
    pub tick_index: i64,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

impl CopyAccountState {
    #[must_use]
    pub fn fresh(account_id: &str, capital_usd: Decimal, now: OffsetDateTime) -> Self {
        Self {
            account_id: account_id.to_string(),
            capital_usd,
            available_usd: capital_usd,
            reserved_usd: Decimal::ZERO,
            realized_pnl: Decimal::ZERO,
            daily_realized_pnl: Decimal::ZERO,
            fees_paid: Decimal::ZERO,
            tick_index: 0,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CopyEvent {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition_id: Option<String>,
    pub event_type: String,
    pub severity: CopyEventSeverity,
    pub message: String,
    pub metadata: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CopyTradeStatus {
    pub enabled: bool,
    pub running: bool,
    pub mode: CopyTradeMode,
    pub account_id: String,
    pub wallets_tracked: usize,
    pub active_wallets: usize,
    pub open_orders: usize,
    pub positions: usize,
    pub source_trades_detected: usize,
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
pub struct CopyTradeSnapshot {
    pub config: CopyTradeConfig,
    pub status: CopyTradeStatus,
    pub account: CopyAccountState,
    pub wallets: Vec<TrackedWallet>,
    pub source_trades: Vec<SourceTrade>,
    pub orders: Vec<CopyOrder>,
    pub positions: Vec<CopyPosition>,
    pub events: Vec<CopyEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CopyTradeRunReport {
    pub wallets_scanned: usize,
    pub trades_detected: usize,
    pub orders_placed: usize,
    pub orders_filled: usize,
    pub orders_skipped: usize,
}

// ── API input structs (deserialized from POST bodies) ───────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct AddTrackedWalletInput {
    pub address: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub sizing_override: Option<CopySizingMode>,
    #[serde(default)]
    pub max_exposure_override: Option<Decimal>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WalletActionInput {
    pub address: String,
}

// ── Strategy / Engine internals ─────────────────────────────────────────────

/// Inputs from the Data API for a single tracked wallet (deserialized by the
/// worker, passed into the service so it stays connector-agnostic).
#[derive(Debug, Clone)]
pub struct WalletFeedInput {
    pub address: String,
    pub activities: Vec<WalletActivityInput>,
    pub positions: Vec<WalletPositionInput>,
}

#[derive(Debug, Clone)]
pub struct WalletActivityInput {
    pub kind: String,
    pub side: String,
    pub asset: String,
    pub condition_id: String,
    pub outcome: String,
    pub title: String,
    pub slug: String,
    pub price: Decimal,
    pub size: Decimal,
    pub usdc_size: Decimal,
    pub transaction_hash: String,
    pub timestamp: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct WalletPositionInput {
    pub asset: String,
    pub condition_id: String,
    pub outcome: String,
    pub title: String,
    pub slug: String,
    pub size: Decimal,
    pub avg_price: Decimal,
    pub cur_price: Decimal,
    pub realized_pnl: Decimal,
    pub percent_pnl: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CopyBookLevel {
    pub price: Decimal,
    pub size: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CopyOrderBook {
    pub token_id: String,
    pub bids: Vec<CopyBookLevel>,
    pub asks: Vec<CopyBookLevel>,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
}

/// Internal decision returned by the strategy layer (not persisted).
#[derive(Debug, Clone)]
pub struct CopyDecision {
    pub copy: bool,
    pub reason: String,
    pub size: Decimal,
    pub price: Decimal,
}

/// The full set of state changes produced by a single copy-trading tick.
/// Persisted atomically by `CopyTradeStore::apply_copy_tick`.
#[derive(Debug, Clone, PartialEq)]
pub struct CopySimulationOutcome {
    pub account: CopyAccountState,
    /// Orders to upsert, keyed by `id`.
    pub orders: Vec<CopyOrder>,
    /// Positions to upsert, keyed by `(account_id, token_id)`.
    pub positions: Vec<CopyPosition>,
    pub fills: Vec<CopyFill>,
    pub events: Vec<CopyEvent>,
    /// Source trades to mark as processed (copied=true).
    pub processed_source_trade_ids: Vec<String>,
    pub report: CopyTradeRunReport,
}

/// A fill against a simulated copy order.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CopyFill {
    pub id: String,
    pub order_id: String,
    pub account_id: String,
    pub wallet_address: String,
    pub condition_id: String,
    pub token_id: String,
    pub outcome: String,
    pub side: CopyOrderSide,
    pub price: Decimal,
    pub size: Decimal,
    pub notional_usd: Decimal,
    pub realized_pnl: Decimal,
    pub reason: String,
    pub trace_id: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

/// How the strategy engine resolved its sizing for a given source trade.
/// Logged as the `reason` field on the copy order for audit / debugging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopySkipReason {
    BelowMinSize,
    PriceOutOfRange,
    CopySellsDisabled,
    MaxOrdersReached,
    PositionCapExceeded,
    WalletExposureCapExceeded,
    TotalExposureCapExceeded,
    DailyLossLimit,
    CooldownActive,
    SlippageExceeded,
    NoOrderBook,
    NoSufficientLiquidity,
    WalletPaused,
}

impl CopySkipReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BelowMinSize => "below_min_size",
            Self::PriceOutOfRange => "price_out_of_range",
            Self::CopySellsDisabled => "copy_sells_disabled",
            Self::MaxOrdersReached => "max_orders_reached",
            Self::PositionCapExceeded => "position_cap_exceeded",
            Self::WalletExposureCapExceeded => "wallet_exposure_cap_exceeded",
            Self::TotalExposureCapExceeded => "total_exposure_cap_exceeded",
            Self::DailyLossLimit => "daily_loss_limit",
            Self::CooldownActive => "cooldown_active",
            Self::SlippageExceeded => "slippage_exceeded",
            Self::NoOrderBook => "no_order_book",
            Self::NoSufficientLiquidity => "no_sufficient_liquidity",
            Self::WalletPaused => "wallet_paused",
        }
    }
}

impl FromStr for CopySkipReason {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "below_min_size" => Ok(Self::BelowMinSize),
            "price_out_of_range" => Ok(Self::PriceOutOfRange),
            "copy_sells_disabled" => Ok(Self::CopySellsDisabled),
            "max_orders_reached" => Ok(Self::MaxOrdersReached),
            "position_cap_exceeded" => Ok(Self::PositionCapExceeded),
            "wallet_exposure_cap_exceeded" => Ok(Self::WalletExposureCapExceeded),
            "total_exposure_cap_exceeded" => Ok(Self::TotalExposureCapExceeded),
            "daily_loss_limit" => Ok(Self::DailyLossLimit),
            "cooldown_active" => Ok(Self::CooldownActive),
            "slippage_exceeded" => Ok(Self::SlippageExceeded),
            "no_order_book" => Ok(Self::NoOrderBook),
            "no_sufficient_liquidity" => Ok(Self::NoSufficientLiquidity),
            "wallet_paused" => Ok(Self::WalletPaused),
            other => Err(AppError::invalid_input(
                "COPYTRADE_SKIP_REASON_INVALID",
                format!("unknown copytrade skip reason: {other}"),
            )),
        }
    }
}

/// Internal tick context for the simulation engine.
pub(crate) struct CopyTickContext {
    pub now: OffsetDateTime,
    pub config: CopyTradeConfig,
    pub account: CopyAccountState,
    pub orders: Vec<CopyOrder>,
    pub positions: HashMap<String, CopyPosition>,
    pub fills: Vec<CopyFill>,
    pub events: Vec<CopyEvent>,
    pub processed_source_trade_ids: Vec<String>,
    pub trace_id: String,
    pub seq: usize,
    pub filled_orders: usize,
    pub placed_orders: usize,
}
