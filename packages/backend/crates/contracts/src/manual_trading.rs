#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WalletRiskPolicyInput {
    pub max_open_orders: i64,
    pub max_open_buy_notional: Decimal,
    pub max_total_position_notional: Decimal,
    pub max_market_position_notional: Decimal,
    pub max_order_notional: Decimal,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateWalletAccountRequest {
    pub name: String,
    pub signer_address: String,
    pub funder_address: String,
    pub signature_type: i32,
    pub encrypted_secret: EncryptedWalletSecretInput,
    #[serde(default)]
    pub trading_enabled: bool,
    pub risk_policy: WalletRiskPolicyInput,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_note: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct UpdateWalletAccountRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted_secret: Option<EncryptedWalletSecretInput>,
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
    pub secret: polyedge_domain::WalletSecretMetadata,
    pub risk_policy: polyedge_domain::WalletRiskPolicy,
    pub state: polyedge_domain::WalletAccountState,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncryptedWalletSecretInput {
    pub context_id: String,
    pub key_id: String,
    pub algorithm: String,
    pub wrapped_key: String,
    pub nonce: String,
    pub ciphertext: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletImportPublicJwkData {
    pub kty: String,
    #[serde(rename = "use")]
    pub use_: String,
    pub alg: String,
    pub kid: String,
    pub n: String,
    pub e: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletImportContextData {
    pub context_id: String,
    pub key_id: String,
    pub algorithm: String,
    pub aad_version: String,
    pub public_key: WalletImportPublicJwkData,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
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
    pub reward_minimum_size: Decimal,
    pub reward_maximum_spread: Decimal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reward_daily_rate: Option<Decimal>,
    pub book_freshness_ms: i64,
    pub downward_reprice_confirm_ms: i64,
    pub upward_reprice_confirm_ms: i64,
    pub reprice_cooldown_ms: i64,
    pub max_replaces_per_cycle: i64,
    pub quote_slots: Vec<QuoteSlotInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateMarketStrategyRequest {
    pub name: String,
    pub visibility: polyedge_domain::StrategyVisibility,
    #[serde(with = "time::serde::rfc3339")]
    pub active_from: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub active_until: OffsetDateTime,
    pub market: ManagedMarketInput,
    pub version: StrategyVersionInput,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wallet_ids: Vec<i64>,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct UpdateMarketStrategyRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<polyedge_domain::StrategyStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visibility: Option<polyedge_domain::StrategyVisibility>,
    #[serde(default, with = "time::serde::rfc3339::option", skip_serializing_if = "Option::is_none")]
    pub active_from: Option<OffsetDateTime>,
    #[serde(default, with = "time::serde::rfc3339::option", skip_serializing_if = "Option::is_none")]
    pub active_until: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub market: Option<UpdateManagedMarketRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<StrategyVersionInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_ids: Option<Vec<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketStrategyData {
    pub market: polyedge_domain::ManagedMarket,
    pub outcomes: Vec<polyedge_domain::ManagedMarketOutcome>,
    pub strategy: polyedge_domain::MarketStrategy,
    pub version: polyedge_domain::StrategyVersion,
    pub reward_terms: polyedge_domain::StrategyRewardTerms,
    pub quote_slots: Vec<polyedge_domain::StrategyQuoteSlot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_user_subscription: Option<StrategySubscriptionData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategySubscriptionData {
    pub subscription: polyedge_domain::StrategySubscription,
    pub wallets: Vec<polyedge_domain::StrategySubscriptionWallet>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateStrategySubscriptionRequest {
    pub source_strategy_id: i64,
    pub wallet_ids: Vec<i64>,
    #[serde(default, with = "time::serde::rfc3339::option", skip_serializing_if = "Option::is_none")]
    pub active_until: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct UpdateStrategySubscriptionRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<polyedge_domain::StrategySubscriptionStatus>,
    #[serde(default, with = "time::serde::rfc3339::option", skip_serializing_if = "Option::is_none")]
    pub active_until: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_ids: Option<Vec<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_note: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordCashFlowRequest {
    pub wallet_id: i64,
    pub flow_type: String,
    pub amount: Decimal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_reference: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub occurred_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashFlowData {
    pub id: i64,
    pub owner_user_id: i64,
    pub wallet_id: i64,
    pub flow_type: String,
    pub amount: Decimal,
    pub external_reference: Option<String>,
    pub note: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub occurred_at: OffsetDateTime,
    pub recorded_by_user_id: Option<i64>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

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
