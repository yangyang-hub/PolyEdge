use async_trait::async_trait;
use polyedge_domain::{AppError, Result};
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::HashMap, str::FromStr, sync::Arc};
use time::OffsetDateTime;

const DEFAULT_LIST_LIMIT: u16 = 100;
const MAX_LIST_LIMIT: u16 = 500;
const DEFAULT_TICK: Decimal = Decimal::from_parts(1, 0, 0, false, 2);

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

#[async_trait]
pub trait RewardBotStore: Send + Sync {
    async fn load_config(&self) -> Result<RewardBotConfig>;
    async fn save_config(&self, config: &RewardBotConfig) -> Result<()>;
    async fn upsert_markets(&self, markets: &[RewardMarket]) -> Result<()>;
    async fn save_quote_plans(&self, plans: &[RewardQuotePlan]) -> Result<()>;
    async fn replace_simulated_orders(
        &self,
        account_id: &str,
        orders: &[ManagedRewardOrder],
        trace_id: &str,
    ) -> Result<usize>;
    async fn cancel_open_orders(
        &self,
        account_id: Option<&str>,
        reason: &str,
        trace_id: &str,
    ) -> Result<usize>;
    async fn list_markets(&self, limit: u16) -> Result<Vec<RewardMarket>>;
    async fn list_quote_plans(&self, limit: u16) -> Result<Vec<RewardQuotePlan>>;
    async fn list_orders(&self, limit: u16) -> Result<Vec<ManagedRewardOrder>>;
    async fn list_positions(&self, limit: u16) -> Result<Vec<RewardPosition>>;
    async fn list_events(&self, limit: u16) -> Result<Vec<RewardRiskEvent>>;
    async fn log_event(&self, event: RewardRiskEvent) -> Result<()>;
}

#[derive(Clone)]
pub struct RewardBotService {
    store: Arc<dyn RewardBotStore>,
}

impl RewardBotService {
    #[must_use]
    pub fn new(store: Arc<dyn RewardBotStore>) -> Self {
        Self { store }
    }

    pub async fn read_config(&self) -> Result<RewardBotConfig> {
        self.store
            .load_config()
            .await
            .map(RewardBotConfig::normalized)
    }

    pub async fn update_config(&self, patch: RewardBotConfigPatch) -> Result<RewardBotConfig> {
        let current = self.read_config().await?;
        let next = current.apply_patch(patch);
        self.store.save_config(&next).await?;
        Ok(next)
    }

    pub async fn snapshot(&self) -> Result<RewardBotSnapshot> {
        let config = self.read_config().await?;
        let markets = self.store.list_markets(DEFAULT_LIST_LIMIT).await?;
        let quote_plans = self.store.list_quote_plans(DEFAULT_LIST_LIMIT).await?;
        let orders = self.store.list_orders(200).await?;
        let positions = self.store.list_positions(200).await?;
        let events = self.store.list_events(100).await?;
        let last_scan_at = markets.iter().map(|market| market.updated_at).max();
        let last_run_at = quote_plans.iter().map(|plan| plan.updated_at).max();
        let open_orders = orders
            .iter()
            .filter(|order| order.status.is_open_like())
            .count();
        let error = events
            .iter()
            .find(|event| event.severity == RewardRiskSeverity::Critical)
            .map(|event| event.message.clone());

        Ok(RewardBotSnapshot {
            status: RewardBotStatus {
                enabled: config.enabled,
                running: config.enabled,
                mode: config.mode,
                account_id: config.account_id.clone(),
                markets_tracked: markets.len(),
                eligible_markets: quote_plans.iter().filter(|plan| plan.eligible).count(),
                open_orders,
                positions: positions.len(),
                last_scan_at,
                last_run_at,
                error,
            },
            config,
            markets,
            quote_plans,
            orders,
            positions,
            events,
        })
    }

    pub async fn run_simulation(
        &self,
        markets: Vec<RewardMarket>,
        books: HashMap<String, RewardOrderBook>,
        trace_id: &str,
    ) -> Result<RewardBotRunReport> {
        let config = self.read_config().await?;
        let plans = build_reward_quote_plans(&markets, &books, &config);
        let eligible_plans = plans.iter().filter(|plan| plan.eligible).count();

        self.store.upsert_markets(&markets).await?;
        self.store.save_quote_plans(&plans).await?;

        let mut cancelled_orders = 0;
        let mut simulated_orders = 0;

        if config.enabled {
            if config.mode == RewardBotMode::Live {
                self.store
                    .log_event(new_risk_event(
                        Some(config.account_id.clone()),
                        None,
                        None,
                        "reward_bot_live_unsupported",
                        RewardRiskSeverity::Warning,
                        "Rewards bot live mode is not wired in PolyEdge yet; generated a simulation instead.",
                        json!({ "trace_id": trace_id }),
                    ))
                    .await?;
            }

            let orders = build_simulated_orders(&config, &plans, trace_id);
            cancelled_orders = self
                .store
                .replace_simulated_orders(&config.account_id, &orders, trace_id)
                .await?;
            simulated_orders = orders.len();
        }

        self.store
            .log_event(new_risk_event(
                Some(config.account_id.clone()),
                None,
                None,
                "reward_bot_simulation_run",
                RewardRiskSeverity::Info,
                "Completed rewards quote-plan simulation.",
                json!({
                    "trace_id": trace_id,
                    "markets_scanned": markets.len(),
                    "books_fetched": books.len(),
                    "plans_built": plans.len(),
                    "eligible_plans": eligible_plans,
                    "simulated_orders": simulated_orders,
                    "cancelled_orders": cancelled_orders,
                }),
            ))
            .await?;

        Ok(RewardBotRunReport {
            markets_scanned: markets.len(),
            books_fetched: books.len(),
            plans_built: plans.len(),
            eligible_plans,
            simulated_orders,
            cancelled_orders,
        })
    }

    pub async fn cancel_all_orders(
        &self,
        account_id: Option<&str>,
        reason: &str,
        trace_id: &str,
    ) -> Result<usize> {
        let cancelled = self
            .store
            .cancel_open_orders(account_id, reason, trace_id)
            .await?;
        self.store
            .log_event(new_risk_event(
                account_id.map(str::to_string),
                None,
                None,
                "reward_bot_cancel_all",
                RewardRiskSeverity::Info,
                reason,
                json!({ "trace_id": trace_id, "cancelled_orders": cancelled }),
            ))
            .await?;
        Ok(cancelled)
    }
}

#[must_use]
pub fn select_reward_book_token_ids(
    markets: &[RewardMarket],
    config: &RewardBotConfig,
) -> Vec<String> {
    let market_limit = usize::min(
        markets.len(),
        usize::max(usize::from(config.max_markets) * 4, 12),
    );
    let mut selected = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut candidates = markets.to_vec();
    candidates.sort_by(|left, right| right.total_daily_rate.cmp(&left.total_daily_rate));

    for market in candidates.into_iter().take(market_limit) {
        for token in market.tokens.into_iter().take(2) {
            if token.token_id.trim().is_empty() || !seen.insert(token.token_id.clone()) {
                continue;
            }
            selected.push(token.token_id);
        }
    }

    selected
}

#[must_use]
pub fn build_reward_quote_plans(
    markets: &[RewardMarket],
    books: &HashMap<String, RewardOrderBook>,
    config: &RewardBotConfig,
) -> Vec<RewardQuotePlan> {
    let mut plans = markets
        .iter()
        .map(|market| build_reward_quote_plan(market, books, config))
        .collect::<Vec<_>>();
    plans.sort_by(|left, right| {
        right
            .eligible
            .cmp(&left.eligible)
            .then_with(|| right.score.cmp(&left.score))
            .then_with(|| right.total_daily_rate.cmp(&left.total_daily_rate))
    });
    plans
}

fn build_reward_quote_plan(
    market: &RewardMarket,
    books: &HashMap<String, RewardOrderBook>,
    config: &RewardBotConfig,
) -> RewardQuotePlan {
    let now = OffsetDateTime::now_utc();
    let yes_token = market
        .tokens
        .iter()
        .find(|token| token.outcome.to_lowercase().contains("yes"))
        .or_else(|| market.tokens.first());
    let no_token = market
        .tokens
        .iter()
        .find(|token| token.outcome.to_lowercase().contains("no"))
        .or_else(|| market.tokens.get(1));
    let (Some(yes_token), Some(no_token)) = (yes_token, no_token) else {
        return empty_plan(market, "missing YES/NO token", now, None);
    };

    let yes_state = get_token_book_state(yes_token, books, config, now);
    let no_state = get_token_book_state(no_token, books, config, now);
    let midpoint = yes_state
        .as_ref()
        .map(|state| state.midpoint)
        .or_else(|| no_state.as_ref().map(|state| Decimal::ONE - state.midpoint));
    let Some(midpoint) = midpoint else {
        return empty_plan(market, "missing book or fallback token price", now, None);
    };

    if config.mode == RewardBotMode::Live
        && (!yes_state.as_ref().is_some_and(|state| state.fresh)
            || !no_state.as_ref().is_some_and(|state| state.fresh))
    {
        return empty_plan(
            market,
            "live mode requires fresh YES and NO books",
            now,
            Some(midpoint),
        );
    }

    if midpoint < config.min_midpoint || midpoint > config.max_midpoint {
        return empty_plan(
            market,
            "midpoint is too close to 0/1 for the first rewards strategy",
            now,
            Some(midpoint),
        );
    }

    if market.total_daily_rate < config.min_daily_reward {
        return empty_plan(
            market,
            "daily reward is below threshold",
            now,
            Some(midpoint),
        );
    }

    let max_spread_cents = Decimal::min(
        normalize_reward_spread_cents(market.rewards_max_spread),
        config.max_spread_cents,
    );
    if max_spread_cents <= Decimal::ZERO {
        return empty_plan(
            market,
            "invalid rewards spread setting",
            now,
            Some(midpoint),
        );
    }

    let quote_edge = Decimal::min(config.quote_edge_cents, max_spread_cents) / decimal("100");
    let safety = config.safety_margin_cents / decimal("100");
    let yes_bid = floor_to_tick(
        Decimal::max(decimal("0.01"), midpoint - quote_edge),
        DEFAULT_TICK,
    );
    let no_mid = Decimal::ONE - midpoint;
    let no_bid = floor_to_tick(
        Decimal::max(decimal("0.01"), no_mid - quote_edge),
        DEFAULT_TICK,
    );

    if yes_state
        .as_ref()
        .and_then(|state| state.best_ask)
        .is_some_and(|best_ask| yes_bid >= best_ask)
    {
        return empty_plan(market, "YES bid would touch best ask", now, Some(midpoint));
    }

    if no_state
        .as_ref()
        .and_then(|state| state.best_ask)
        .is_some_and(|best_ask| no_bid >= best_ask)
    {
        return empty_plan(market, "NO bid would touch best ask", now, Some(midpoint));
    }

    if yes_bid + no_bid > Decimal::ONE - safety {
        return empty_plan(
            market,
            "YES/NO bids do not leave enough safety margin",
            now,
            Some(midpoint),
        );
    }

    let legs = vec![
        make_leg(
            &yes_token.token_id,
            &yes_token.outcome,
            yes_bid,
            market.rewards_min_size,
            config,
        ),
        make_leg(
            &no_token.token_id,
            &no_token.outcome,
            no_bid,
            market.rewards_min_size,
            config,
        ),
    ];

    if market.rewards_min_size > Decimal::ZERO
        && legs.iter().any(|leg| leg.size < market.rewards_min_size)
    {
        return empty_plan(
            market,
            "per-market budget cannot satisfy rewards minimum size",
            now,
            Some(midpoint),
        );
    }

    let score = score_market(market, max_spread_cents, midpoint, &legs);
    let eligible = score >= config.min_market_score;

    RewardQuotePlan {
        condition_id: market.condition_id.clone(),
        market_slug: market.market_slug.clone(),
        question: market.question.clone(),
        score,
        eligible,
        reason: if eligible {
            "eligible for simulated post-only quotes".to_string()
        } else {
            "score is below threshold".to_string()
        },
        midpoint: Some(midpoint),
        total_daily_rate: market.total_daily_rate,
        rewards_max_spread: market.rewards_max_spread,
        rewards_min_size: market.rewards_min_size,
        legs,
        updated_at: now,
    }
}

fn empty_plan(
    market: &RewardMarket,
    reason: impl Into<String>,
    now: OffsetDateTime,
    midpoint: Option<Decimal>,
) -> RewardQuotePlan {
    RewardQuotePlan {
        condition_id: market.condition_id.clone(),
        market_slug: market.market_slug.clone(),
        question: market.question.clone(),
        score: Decimal::ZERO,
        eligible: false,
        reason: reason.into(),
        midpoint,
        total_daily_rate: market.total_daily_rate,
        rewards_max_spread: market.rewards_max_spread,
        rewards_min_size: market.rewards_min_size,
        legs: Vec::new(),
        updated_at: now,
    }
}

fn get_token_book_state(
    token: &RewardToken,
    books: &HashMap<String, RewardOrderBook>,
    config: &RewardBotConfig,
    now: OffsetDateTime,
) -> Option<TokenBookState> {
    let book = books.get(&token.token_id);
    let fresh = book
        .and_then(|book| {
            (now - book.observed_at)
                .whole_milliseconds()
                .try_into()
                .ok()
        })
        .is_some_and(|age_ms: u64| age_ms <= config.stale_book_ms);
    let (best_bid, best_ask) = if fresh {
        (
            book.and_then(|book| book.bids.first().map(|level| level.price)),
            book.and_then(|book| book.asks.first().map(|level| level.price)),
        )
    } else {
        (None, None)
    };

    if let (Some(best_bid), Some(best_ask)) = (best_bid, best_ask) {
        if best_bid > Decimal::ZERO && best_ask > Decimal::ZERO {
            return Some(TokenBookState {
                midpoint: (best_bid + best_ask) / decimal("2"),
                best_ask: Some(best_ask),
                fresh: true,
            });
        }
    }

    if config.mode != RewardBotMode::Live {
        if let Some(price) = token
            .price
            .filter(|price| *price > Decimal::ZERO && *price < Decimal::ONE)
        {
            return Some(TokenBookState {
                midpoint: price,
                best_ask: None,
                fresh: false,
            });
        }
    }

    None
}

fn make_leg(
    token_id: &str,
    outcome: &str,
    price: Decimal,
    rewards_min_size: Decimal,
    config: &RewardBotConfig,
) -> RewardQuoteLeg {
    let target_size = config.quote_size_usd / price;
    let max_leg_size = config.per_market_usd / decimal("2") / price;
    let size = Decimal::min(Decimal::max(rewards_min_size, target_size), max_leg_size)
        .round_dp_with_strategy(2, RoundingStrategy::ToZero);

    RewardQuoteLeg {
        token_id: token_id.to_string(),
        outcome: if outcome.trim().is_empty() {
            token_id.to_string()
        } else {
            outcome.to_string()
        },
        side: RewardOrderSide::Buy,
        price,
        size,
        notional_usd: (price * size).round_dp(2),
    }
}

fn score_market(
    market: &RewardMarket,
    max_spread_cents: Decimal,
    midpoint: Decimal,
    legs: &[RewardQuoteLeg],
) -> Decimal {
    let reward_rate = decimal_to_f64(market.total_daily_rate).sqrt();
    let reward_score = f64::min(50.0, reward_rate * 12.0);
    let spread_score = f64::min(25.0, decimal_to_f64(max_spread_cents) * 4.0);
    let midpoint_score = f64::max(0.0, 15.0 - f64::abs(decimal_to_f64(midpoint) - 0.5) * 30.0);
    let notional = legs
        .iter()
        .fold(Decimal::ZERO, |sum, leg| sum + leg.notional_usd);
    let size_score = if notional > Decimal::ZERO { 10.0 } else { 0.0 };

    decimal_from_f64(reward_score + spread_score + midpoint_score + size_score).round_dp(2)
}

fn build_simulated_orders(
    config: &RewardBotConfig,
    plans: &[RewardQuotePlan],
    trace_id: &str,
) -> Vec<ManagedRewardOrder> {
    let now = OffsetDateTime::now_utc();
    let mut orders = Vec::new();

    for plan in plans
        .iter()
        .filter(|plan| plan.eligible)
        .take(usize::from(config.max_markets))
    {
        if orders.len() >= usize::from(config.max_open_orders) {
            break;
        }

        for leg in &plan.legs {
            if orders.len() >= usize::from(config.max_open_orders) {
                break;
            }

            let id = format!(
                "rew_order_{}_{}",
                trace_id.trim_start_matches("trc_"),
                orders.len() + 1
            );
            orders.push(ManagedRewardOrder {
                id,
                account_id: config.account_id.clone(),
                condition_id: plan.condition_id.clone(),
                token_id: leg.token_id.clone(),
                outcome: leg.outcome.clone(),
                side: RewardOrderSide::Buy,
                price: leg.price,
                size: leg.size,
                external_order_id: Some(format!(
                    "sim:{}:{}:{}",
                    plan.condition_id, leg.token_id, trace_id
                )),
                status: ManagedRewardOrderStatus::Open,
                scoring: true,
                reason: "simulated post-only rewards quote".to_string(),
                created_at: now,
                updated_at: now,
            });
        }
    }

    orders
}

#[must_use]
pub fn validate_reward_list_limit(limit: Option<u16>) -> u16 {
    limit.unwrap_or(DEFAULT_LIST_LIMIT).clamp(1, MAX_LIST_LIMIT)
}

#[must_use]
pub fn new_risk_event(
    account_id: Option<String>,
    condition_id: Option<String>,
    external_order_id: Option<String>,
    event_type: impl Into<String>,
    severity: RewardRiskSeverity,
    message: impl Into<String>,
    metadata: Value,
) -> RewardRiskEvent {
    let now = OffsetDateTime::now_utc();
    RewardRiskEvent {
        id: format!("rew_evt_{}", now.unix_timestamp_nanos()),
        account_id,
        condition_id,
        external_order_id,
        event_type: event_type.into(),
        severity,
        message: message.into(),
        metadata,
        created_at: now,
    }
}

fn normalize_reward_spread_cents(raw: Decimal) -> Decimal {
    if raw <= Decimal::ZERO {
        return Decimal::ZERO;
    }

    if raw > decimal("10") {
        raw / decimal("100")
    } else {
        raw
    }
}

fn floor_to_tick(value: Decimal, tick: Decimal) -> Decimal {
    (value / tick).floor() * tick
}

fn normalize_account_id(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "reward_simulator".to_string()
    } else {
        trimmed.to_string()
    }
}

fn clamp_u16(value: u16, min: u16, max: u16) -> u16 {
    value.clamp(min, max)
}

fn clamp_decimal(value: Decimal, min: Decimal, max: Decimal) -> Decimal {
    Decimal::min(max, Decimal::max(min, value))
}

fn decimal(value: &str) -> Decimal {
    Decimal::from_str_exact(value).expect("static reward configuration default must be valid")
}

fn decimal_from_f64(value: f64) -> Decimal {
    if !value.is_finite() {
        return Decimal::ZERO;
    }

    Decimal::from_str(&format!("{value:.6}")).unwrap_or(Decimal::ZERO)
}

fn decimal_to_f64(value: Decimal) -> f64 {
    value.to_string().parse::<f64>().unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::{
        RewardBookLevel, RewardBotConfig, RewardMarket, RewardOrderBook, RewardToken,
        build_reward_quote_plans, decimal,
    };
    use std::collections::HashMap;
    use time::OffsetDateTime;

    #[test]
    fn quote_plan_uses_fallback_prices_in_dry_run() {
        let market = RewardMarket {
            condition_id: "cond_1".to_string(),
            question: "Will the event happen?".to_string(),
            market_slug: "event".to_string(),
            event_slug: "event".to_string(),
            image: String::new(),
            rewards_max_spread: decimal("800"),
            rewards_min_size: decimal("5"),
            total_daily_rate: decimal("25"),
            tokens: vec![
                RewardToken {
                    token_id: "yes".to_string(),
                    outcome: "YES".to_string(),
                    price: Some(decimal("0.52")),
                },
                RewardToken {
                    token_id: "no".to_string(),
                    outcome: "NO".to_string(),
                    price: Some(decimal("0.48")),
                },
            ],
            active: true,
            updated_at: OffsetDateTime::now_utc(),
        };

        let plans =
            build_reward_quote_plans(&[market], &HashMap::new(), &RewardBotConfig::default());

        assert_eq!(plans.len(), 1);
        assert!(plans[0].eligible);
        assert_eq!(plans[0].legs.len(), 2);
        assert_eq!(plans[0].legs[0].price, decimal("0.51"));
        assert_eq!(plans[0].legs[1].price, decimal("0.47"));
    }

    #[test]
    fn quote_plan_avoids_touching_best_ask() {
        let now = OffsetDateTime::now_utc();
        let market = RewardMarket {
            condition_id: "cond_2".to_string(),
            question: "Will the event happen?".to_string(),
            market_slug: "event".to_string(),
            event_slug: "event".to_string(),
            image: String::new(),
            rewards_max_spread: decimal("8"),
            rewards_min_size: decimal("1"),
            total_daily_rate: decimal("25"),
            tokens: vec![
                RewardToken {
                    token_id: "yes".to_string(),
                    outcome: "YES".to_string(),
                    price: Some(decimal("0.52")),
                },
                RewardToken {
                    token_id: "no".to_string(),
                    outcome: "NO".to_string(),
                    price: Some(decimal("0.48")),
                },
            ],
            active: true,
            updated_at: now,
        };
        let mut books = HashMap::new();
        books.insert(
            "yes".to_string(),
            RewardOrderBook {
                token_id: "yes".to_string(),
                bids: vec![RewardBookLevel {
                    price: decimal("0.51"),
                    size: decimal("100"),
                }],
                asks: vec![RewardBookLevel {
                    price: decimal("0.51"),
                    size: decimal("100"),
                }],
                observed_at: now,
            },
        );

        let config = RewardBotConfig {
            quote_edge_cents: decimal("0"),
            ..RewardBotConfig::default()
        };
        let plans = build_reward_quote_plans(&[market], &books, &config);

        assert!(!plans[0].eligible);
        assert_eq!(plans[0].reason, "YES bid would touch best ask");
    }
}
