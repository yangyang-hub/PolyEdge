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
pub struct RewardQuotePlan {
    pub condition_id: String,
    pub market_slug: String,
    pub question: String,
    pub score: Decimal,
    pub eligible: bool,
    pub reason: String,
    #[serde(default = "default_reward_plan_quote_mode")]
    pub quote_mode: RewardPlanQuoteMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended_quote_mode: Option<RewardPlanQuoteMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub book_metrics: Option<RewardMarketBookMetrics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_advisory: Option<RewardMarketAdvisory>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info_risk: Option<RewardMarketInfoRisk>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub midpoint: Option<Decimal>,
    pub total_daily_rate: Decimal,
    pub rewards_max_spread: Decimal,
    pub rewards_min_size: Decimal,
    pub legs: Vec<RewardQuoteLeg>,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

const fn default_reward_plan_quote_mode() -> RewardPlanQuoteMode {
    RewardPlanQuoteMode::Double
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
pub struct RewardBotSnapshot {
    pub config: RewardBotConfig,
    pub status: RewardBotStatus,
    pub account: RewardAccountState,
    pub markets: Vec<RewardMarket>,
    pub quote_plans: Vec<RewardQuotePlan>,
    pub plans_page: RewardListPage,
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
