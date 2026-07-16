#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WalletRiskPolicyInput {
    pub max_open_orders: i64,
    pub max_open_buy_notional: Decimal,
    pub max_total_position_notional: Decimal,
    pub max_market_position_notional: Decimal,
    pub max_order_notional: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateWalletAccountRequest {
    pub name: String,
    pub signer_address: String,
    pub funder_address: String,
    pub signature_type: i32,
    pub credential_provider: polyedge_domain::CredentialProvider,
    pub credential_locator: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_key_version: Option<String>,
    #[serde(default)]
    pub trading_enabled: bool,
    pub risk_policy: WalletRiskPolicyInput,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct UpdateWalletAccountRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_provider: Option<polyedge_domain::CredentialProvider>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_locator: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_key_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<polyedge_domain::WalletAccountStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trading_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_policy: Option<WalletRiskPolicyInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletAccountData {
    pub account: polyedge_domain::WalletAccount,
    pub credential: polyedge_domain::WalletCredentialRef,
    pub risk_policy: polyedge_domain::WalletRiskPolicy,
    pub state: polyedge_domain::WalletAccountState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedMarketInput {
    pub condition_id: String,
    pub slug: String,
    pub question: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polymarket_url: Option<String>,
    pub yes_token_id: String,
    pub no_token_id: String,
    pub reward_minimum_size: Decimal,
    pub reward_maximum_spread: Decimal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reward_daily_rate: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuoteSlotInput {
    pub slot_key: String,
    pub outcome: polyedge_domain::QuoteOutcome,
    pub quantity: Decimal,
    pub pricing_mode: polyedge_domain::QuotePricingMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_price: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub book_rank: Option<i64>,
    #[serde(default)]
    pub price_offset: Decimal,
    pub minimum_price: Decimal,
    pub maximum_price: Decimal,
    #[serde(default = "default_true")]
    pub post_only: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StrategyVersionInput {
    pub book_freshness_ms: i64,
    pub downward_reprice_confirm_ms: i64,
    pub upward_reprice_confirm_ms: i64,
    pub reprice_cooldown_ms: i64,
    pub max_replaces_per_cycle: i64,
    pub quote_slots: Vec<QuoteSlotInput>,
    pub wallet_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateMarketStrategyRequest {
    pub name: String,
    pub market: ManagedMarketInput,
    pub version: StrategyVersionInput,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct UpdateManagedMarketRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polymarket_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<MarketStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reward_minimum_size: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reward_maximum_spread: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reward_daily_rate: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct UpdateMarketStrategyRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<polyedge_domain::StrategyStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub market: Option<UpdateManagedMarketRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<StrategyVersionInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketStrategyData {
    pub market: polyedge_domain::ManagedMarket,
    pub outcomes: Vec<polyedge_domain::ManagedMarketOutcome>,
    pub reward_terms: polyedge_domain::MarketRewardTerms,
    pub strategy: polyedge_domain::MarketStrategy,
    pub version: polyedge_domain::StrategyVersion,
    pub quote_slots: Vec<polyedge_domain::StrategyQuoteSlot>,
    pub wallet_targets: Vec<polyedge_domain::StrategyWalletTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateExecutionBatchRequest {
    pub strategy_id: i64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wallet_ids: Vec<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CancelExecutionBatchRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionBatchData {
    pub batch: polyedge_domain::ExecutionBatch,
    pub jobs: Vec<polyedge_domain::WalletExecutionJob>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletExecutionJobData {
    pub job: polyedge_domain::WalletExecutionJob,
    pub actions: Vec<polyedge_domain::ExecutionAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteOperationData {
    pub accepted: bool,
    pub operation_id: String,
    pub resource_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CreateCancellationBatchRequest {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wallet_ids: Vec<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub condition_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemRuntimeStateData {
    pub kill_switch_locked: bool,
    pub trading_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub version: i64,
    pub updated_by: String,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateSystemRuntimeStateRequest {
    pub kill_switch_locked: bool,
    pub trading_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_note: Option<String>,
}

pub type ManagedOrderData = polyedge_domain::ManagedOrder;
pub type ManagedPositionData = polyedge_domain::ManagedPosition;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ManualTradingListQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub market_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u64>,
}
