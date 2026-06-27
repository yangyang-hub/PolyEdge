#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmartMoneyMode {
    Observe,
    Paper,
    Approval,
    LiveGuarded,
}

impl SmartMoneyMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Observe => "observe",
            Self::Paper => "paper",
            Self::Approval => "approval",
            Self::LiveGuarded => "live_guarded",
        }
    }
}

impl FromStr for SmartMoneyMode {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "observe" => Ok(Self::Observe),
            "paper" => Ok(Self::Paper),
            "approval" => Ok(Self::Approval),
            "live_guarded" => Ok(Self::LiveGuarded),
            other => Err(AppError::invalid_input(
                "SMART_MONEY_MODE_INVALID",
                format!("unknown smart money mode: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmartWalletCandidateStatus {
    Candidate,
    Watch,
    Tracked,
    Blocked,
    Rejected,
}

impl SmartWalletCandidateStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Candidate => "candidate",
            Self::Watch => "watch",
            Self::Tracked => "tracked",
            Self::Blocked => "blocked",
            Self::Rejected => "rejected",
        }
    }
}

impl FromStr for SmartWalletCandidateStatus {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "candidate" => Ok(Self::Candidate),
            "watch" => Ok(Self::Watch),
            "tracked" => Ok(Self::Tracked),
            "blocked" => Ok(Self::Blocked),
            "rejected" => Ok(Self::Rejected),
            other => Err(AppError::invalid_input(
                "SMART_WALLET_CANDIDATE_STATUS_INVALID",
                format!("unknown smart wallet candidate status: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmartWalletTier {
    Blocked,
    Candidate,
    Watch,
    Approved,
}

impl SmartWalletTier {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Blocked => "blocked",
            Self::Candidate => "candidate",
            Self::Watch => "watch",
            Self::Approved => "approved",
        }
    }
}

impl FromStr for SmartWalletTier {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "blocked" => Ok(Self::Blocked),
            "candidate" => Ok(Self::Candidate),
            "watch" => Ok(Self::Watch),
            "approved" => Ok(Self::Approved),
            other => Err(AppError::invalid_input(
                "SMART_WALLET_TIER_INVALID",
                format!("unknown smart wallet tier: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmartMoneySide {
    Buy,
    Sell,
}

impl SmartMoneySide {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
        }
    }
}

impl FromStr for SmartMoneySide {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "buy" => Ok(Self::Buy),
            "sell" => Ok(Self::Sell),
            other => Err(AppError::invalid_input(
                "SMART_MONEY_SIDE_INVALID",
                format!("unknown smart money side: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmartSignalStatus {
    New,
    Rejected,
    Observe,
    Paper,
    ApprovalRequired,
    LiveReady,
    Executed,
    Expired,
}

impl SmartSignalStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Rejected => "rejected",
            Self::Observe => "observe",
            Self::Paper => "paper",
            Self::ApprovalRequired => "approval_required",
            Self::LiveReady => "live_ready",
            Self::Executed => "executed",
            Self::Expired => "expired",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmartSignalDecisionValue {
    Allow,
    Observe,
    Reject,
}

impl SmartSignalDecisionValue {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Observe => "observe",
            Self::Reject => "reject",
        }
    }
}

impl FromStr for SmartSignalDecisionValue {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "allow" => Ok(Self::Allow),
            "observe" => Ok(Self::Observe),
            "reject" => Ok(Self::Reject),
            other => Err(AppError::invalid_input(
                "SMART_SIGNAL_DECISION_INVALID",
                format!("unknown smart signal decision: {other}"),
            )),
        }
    }
}

impl FromStr for SmartSignalStatus {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "new" => Ok(Self::New),
            "rejected" => Ok(Self::Rejected),
            "observe" => Ok(Self::Observe),
            "paper" => Ok(Self::Paper),
            "approval_required" => Ok(Self::ApprovalRequired),
            "live_ready" => Ok(Self::LiveReady),
            "executed" => Ok(Self::Executed),
            "expired" => Ok(Self::Expired),
            other => Err(AppError::invalid_input(
                "SMART_SIGNAL_STATUS_INVALID",
                format!("unknown smart signal status: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SmartMoneyConfig {
    pub enabled: bool,
    pub mode: SmartMoneyMode,
    pub discovery_enabled: bool,
    pub wallet_advisory_enabled: bool,
    pub signal_advisory_enabled: bool,
    pub signal_advisory_provider: RewardAiProvider,
    pub signal_advisory_request_format: RewardAiRequestFormat,
    pub signal_advisory_model: String,
    pub min_trade_count: i64,
    pub min_settled_trade_count: i64,
    pub min_total_volume_usd: Decimal,
    pub min_copyability_score: Decimal,
    pub max_signal_age_ms: i64,
    pub max_price_slippage_cents: Decimal,
    pub min_orderbook_depth_usd: Decimal,
    pub max_wallet_exposure_usd: Decimal,
    pub max_market_exposure_usd: Decimal,
    pub max_daily_notional_usd: Decimal,
}

impl Default for SmartMoneyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: SmartMoneyMode::Observe,
            discovery_enabled: true,
            wallet_advisory_enabled: false,
            signal_advisory_enabled: false,
            signal_advisory_provider: RewardAiProvider::OpenAi,
            signal_advisory_request_format: RewardAiRequestFormat::OpenAiResponses,
            signal_advisory_model: "gpt-4.1-mini".to_string(),
            min_trade_count: 50,
            min_settled_trade_count: 20,
            min_total_volume_usd: Decimal::from(10_000),
            min_copyability_score: Decimal::new(70, 2),
            max_signal_age_ms: 60_000,
            max_price_slippage_cents: Decimal::from(2),
            min_orderbook_depth_usd: Decimal::from(50),
            max_wallet_exposure_usd: Decimal::from(20),
            max_market_exposure_usd: Decimal::from(50),
            max_daily_notional_usd: Decimal::from(100),
        }
    }
}

impl SmartMoneyConfig {
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.min_trade_count = self.min_trade_count.max(0);
        self.min_settled_trade_count = self.min_settled_trade_count.max(0);
        self.min_total_volume_usd = self.min_total_volume_usd.max(Decimal::ZERO);
        self.min_copyability_score = clamp_unit_decimal(self.min_copyability_score);
        self.max_signal_age_ms = self.max_signal_age_ms.max(1_000);
        self.max_price_slippage_cents = self.max_price_slippage_cents.max(Decimal::ZERO);
        self.min_orderbook_depth_usd = self.min_orderbook_depth_usd.max(Decimal::ZERO);
        self.max_wallet_exposure_usd = self.max_wallet_exposure_usd.max(Decimal::ZERO);
        self.max_market_exposure_usd = self.max_market_exposure_usd.max(Decimal::ZERO);
        self.max_daily_notional_usd = self.max_daily_notional_usd.max(Decimal::ZERO);
        self.signal_advisory_model = self.signal_advisory_model.trim().to_string();
        if self.signal_advisory_model.is_empty() {
            self.signal_advisory_model = SmartMoneyConfig::default().signal_advisory_model;
        }
        self.signal_advisory_request_format = reward_ai_effective_request_format(
            self.signal_advisory_provider,
            self.signal_advisory_request_format,
            &self.signal_advisory_model,
        );
        self
    }

    #[must_use]
    pub fn apply_patch(mut self, patch: SmartMoneyConfigPatch) -> Self {
        if let Some(value) = patch.enabled {
            self.enabled = value;
        }
        if let Some(value) = patch.mode {
            self.mode = value;
        }
        if let Some(value) = patch.discovery_enabled {
            self.discovery_enabled = value;
        }
        if let Some(value) = patch.wallet_advisory_enabled {
            self.wallet_advisory_enabled = value;
        }
        if let Some(value) = patch.signal_advisory_enabled {
            self.signal_advisory_enabled = value;
        }
        if let Some(value) = patch.signal_advisory_provider {
            self.signal_advisory_provider = value;
        }
        if let Some(value) = patch.signal_advisory_request_format {
            self.signal_advisory_request_format = value;
        }
        if let Some(value) = patch.signal_advisory_model {
            self.signal_advisory_model = value;
        }
        if let Some(value) = patch.min_trade_count {
            self.min_trade_count = value;
        }
        if let Some(value) = patch.min_settled_trade_count {
            self.min_settled_trade_count = value;
        }
        if let Some(value) = patch.min_total_volume_usd {
            self.min_total_volume_usd = value;
        }
        if let Some(value) = patch.min_copyability_score {
            self.min_copyability_score = value;
        }
        if let Some(value) = patch.max_signal_age_ms {
            self.max_signal_age_ms = value;
        }
        if let Some(value) = patch.max_price_slippage_cents {
            self.max_price_slippage_cents = value;
        }
        if let Some(value) = patch.min_orderbook_depth_usd {
            self.min_orderbook_depth_usd = value;
        }
        if let Some(value) = patch.max_wallet_exposure_usd {
            self.max_wallet_exposure_usd = value;
        }
        if let Some(value) = patch.max_market_exposure_usd {
            self.max_market_exposure_usd = value;
        }
        if let Some(value) = patch.max_daily_notional_usd {
            self.max_daily_notional_usd = value;
        }
        self.normalized()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SmartMoneyConfigPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<SmartMoneyMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovery_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_advisory_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal_advisory_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal_advisory_provider: Option<RewardAiProvider>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal_advisory_request_format: Option<RewardAiRequestFormat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal_advisory_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_trade_count: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_settled_trade_count: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_total_volume_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_copyability_score: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_signal_age_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_price_slippage_cents: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_orderbook_depth_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_wallet_exposure_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_market_exposure_usd: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_daily_notional_usd: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SmartWalletCandidate {
    pub id: i64,
    pub wallet_address: String,
    pub source: String,
    pub status: SmartWalletCandidateStatus,
    pub first_seen_at: OffsetDateTime,
    pub last_seen_at: OffsetDateTime,
    pub last_analyzed_at: Option<OffsetDateTime>,
    pub promoted_at: Option<OffsetDateTime>,
    pub rejected_at: Option<OffsetDateTime>,
    pub reason: Option<String>,
    pub raw: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SmartWalletCandidateStatusUpdate {
    pub wallet_address: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub status: SmartWalletCandidateStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SmartWalletProfile {
    pub wallet_address: String,
    pub trade_count: i64,
    pub settled_trade_count: i64,
    pub total_volume_usd: Decimal,
    pub realized_pnl_usd: Decimal,
    pub roi: Decimal,
    pub win_rate: Decimal,
    pub max_drawdown_usd: Decimal,
    pub avg_trade_usd: Decimal,
    pub median_trade_usd: Decimal,
    pub avg_hold_secs: Option<i64>,
    pub active_days: i64,
    pub markets_traded: i64,
    pub category_concentration_score: Decimal,
    pub market_concentration_score: Decimal,
    pub low_liquidity_trade_ratio: Decimal,
    pub stale_copy_window_ratio: Decimal,
    pub last_trade_at: Option<OffsetDateTime>,
    pub updated_at: OffsetDateTime,
}

impl SmartWalletProfile {
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.trade_count = self.trade_count.max(0);
        self.settled_trade_count = self.settled_trade_count.max(0);
        self.total_volume_usd = self.total_volume_usd.max(Decimal::ZERO);
        self.win_rate = clamp_unit_decimal(self.win_rate);
        self.category_concentration_score = clamp_unit_decimal(self.category_concentration_score);
        self.market_concentration_score = clamp_unit_decimal(self.market_concentration_score);
        self.low_liquidity_trade_ratio = clamp_unit_decimal(self.low_liquidity_trade_ratio);
        self.stale_copy_window_ratio = clamp_unit_decimal(self.stale_copy_window_ratio);
        self.active_days = self.active_days.max(0);
        self.markets_traded = self.markets_traded.max(0);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SmartWalletScore {
    pub wallet_address: String,
    pub total_score: Decimal,
    pub profit_score: Decimal,
    pub consistency_score: Decimal,
    pub risk_score: Decimal,
    pub liquidity_score: Decimal,
    pub recency_score: Decimal,
    pub copyability_score: Decimal,
    pub tier: SmartWalletTier,
    pub explanation: Value,
    pub scoring_version: String,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SmartWalletTrade {
    pub id: String,
    pub wallet_address: String,
    pub source: String,
    pub condition_id: String,
    pub token_id: Option<String>,
    pub side: SmartMoneySide,
    pub outcome: Option<String>,
    pub price: Decimal,
    pub size: Decimal,
    pub notional_usd: Decimal,
    pub tx_hash: Option<String>,
    pub source_timestamp: OffsetDateTime,
    pub discovered_at: OffsetDateTime,
    pub raw: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SmartSignal {
    pub id: i64,
    pub source_trade_id: String,
    pub wallet_address: String,
    pub condition_id: String,
    pub token_id: Option<String>,
    pub side: SmartMoneySide,
    pub source_price: Decimal,
    pub current_price: Option<Decimal>,
    pub price_slippage_cents: Option<Decimal>,
    pub latency_ms: Option<i64>,
    pub source_notional_usd: Decimal,
    pub consensus_wallet_count: i64,
    pub score: Decimal,
    pub status: SmartSignalStatus,
    pub reason: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SmartSignalDecision {
    pub id: i64,
    pub signal_id: i64,
    pub decision: SmartSignalDecisionValue,
    pub stage: String,
    pub mode: SmartMoneyMode,
    pub rejection_reason: Option<String>,
    pub risk_checks: Value,
    pub decided_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SmartSignalAdvisoryLookup {
    pub signal_id: i64,
    pub provider: String,
    pub request_format: String,
    pub model: String,
    pub input_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SmartSignalAdvisory {
    pub id: i64,
    pub signal_id: i64,
    pub provider: String,
    pub request_format: String,
    pub model: String,
    pub input_hash: String,
    pub recommendation: SmartSignalDecisionValue,
    pub confidence: Decimal,
    pub risk_tags: Vec<String>,
    pub summary: String,
    pub reasons: Vec<String>,
    pub raw_output: Value,
    pub expires_at: OffsetDateTime,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SmartSignalAdvisoryRequest {
    pub signal_id: i64,
    pub provider: String,
    pub request_format: String,
    pub model: String,
    pub input_hash: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SmartSignalAdvisoryDecision {
    pub recommendation: SmartSignalDecisionValue,
    pub confidence: Decimal,
    pub risk_tags: Vec<String>,
    pub summary: String,
    pub reasons: Vec<String>,
    pub raw_output: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SmartSignalBookQuote {
    pub token_id: String,
    pub best_bid: Option<Decimal>,
    pub best_ask: Option<Decimal>,
    pub bid_depth_usd: Decimal,
    pub ask_depth_usd: Decimal,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SmartSignalGenerationReport {
    pub trades_scanned: usize,
    pub signals_generated: usize,
    pub decisions_recorded: usize,
    pub observe_signals: usize,
    pub rejected_signals: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SmartMoneyStatus {
    pub enabled: bool,
    pub mode: SmartMoneyMode,
    pub candidates: usize,
    pub watch_wallets: usize,
    pub tracked_wallets: usize,
    pub blocked_wallets: usize,
    pub profiles: usize,
    pub scored_wallets: usize,
    pub recent_trades: usize,
    pub recent_signals: usize,
    pub recent_decisions: usize,
    pub recent_signal_advisories: usize,
    pub last_trade_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SmartMoneySnapshot {
    pub status: SmartMoneyStatus,
    pub config: SmartMoneyConfig,
    pub candidates: Vec<SmartWalletCandidate>,
    pub profiles: Vec<SmartWalletProfile>,
    pub scores: Vec<SmartWalletScore>,
    pub recent_trades: Vec<SmartWalletTrade>,
    pub recent_signals: Vec<SmartSignal>,
    pub recent_decisions: Vec<SmartSignalDecision>,
    pub recent_signal_advisories: Vec<SmartSignalAdvisory>,
}
