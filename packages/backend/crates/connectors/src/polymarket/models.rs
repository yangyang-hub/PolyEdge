#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolymarketSignatureScheme {
    Eoa,
    Proxy,
    GnosisSafe,
}

impl From<PolymarketSignatureScheme> for SignatureType {
    fn from(value: PolymarketSignatureScheme) -> Self {
        match value {
            PolymarketSignatureScheme::Eoa => SignatureType::Eoa,
            PolymarketSignatureScheme::Proxy => SignatureType::Proxy,
            PolymarketSignatureScheme::GnosisSafe => SignatureType::GnosisSafe,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LivePolymarketConfig {
    pub account_id: String,
    pub clob_host: String,
    pub ws_host: String,
    pub chain_id: u64,
    pub signature_type: PolymarketSignatureScheme,
    pub funder: Option<String>,
    pub private_key: String,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub api_passphrase: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PolymarketMarketRefs {
    pub condition_id: String,
    pub yes_asset_id: String,
    pub no_asset_id: String,
}

impl PolymarketMarketRefs {
    pub fn asset_id_for_side(&self, side: SignalSide) -> Result<U256> {
        let raw = match side {
            SignalSide::Yes => &self.yes_asset_id,
            SignalSide::No => &self.no_asset_id,
        };

        parse_u256("polymarket_asset_id", raw, "POLYMARKET_ASSET_ID_INVALID")
    }

    pub fn condition_id(&self) -> Result<B256> {
        parse_b256(
            "polymarket_condition_id",
            &self.condition_id,
            "POLYMARKET_CONDITION_ID_INVALID",
        )
    }
}

#[derive(Debug, Clone)]
pub struct LivePolymarketOrderRequest {
    pub execution_request_id: String,
    pub connector_name: String,
    pub market_id: String,
    pub side: SignalSide,
    pub limit_price: Probability,
    pub quantity: Quantity,
    pub notional: UsdAmount,
    pub market_refs: PolymarketMarketRefs,
}

#[derive(Debug, Clone)]
pub struct LivePolymarketOrderStatusRequest {
    pub connector_name: String,
    pub external_order_id: String,
}

#[derive(Debug, Clone)]
pub struct LivePolymarketTradeSyncRequest {
    pub connector_name: String,
    pub account_id: String,
    pub external_order_id: String,
}

#[derive(Debug, Clone)]
pub struct LivePolymarketOrderAcceptance {
    pub order_id: String,
    pub accepted_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub enum LivePolymarketExecutionOutcome {
    Accepted(LivePolymarketOrderAcceptance),
    Rejected(MockPolymarketOrderRejection),
}

#[derive(Debug, Clone)]
pub struct LivePolymarketConnector {
    client: ClobClient<Authenticated<Normal>>,
    private_key: String,
    chain_id: u64,
    account_id: String,
    ws_host: String,
}

#[derive(Debug, Clone)]
pub struct PolymarketBookLevel {
    pub price: Probability,
    pub size: Quantity,
}

#[derive(Debug, Clone)]
pub struct PolymarketSingleTokenBook {
    pub asset_id: String,
    pub best_bid: Option<PolymarketBookLevel>,
    pub best_ask: Option<PolymarketBookLevel>,
    pub raw_payload: serde_json::Value,
    pub observed_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct PolymarketBinaryBookSnapshot {
    pub condition_id: String,
    pub yes_asset_id: String,
    pub no_asset_id: String,
    pub yes: PolymarketSingleTokenBook,
    pub no: PolymarketSingleTokenBook,
    pub observed_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct PolymarketBookConnector {
    client: ClobClient<Unauthenticated>,
}

#[derive(Debug, Clone)]
pub struct MockPolymarketOrderRequest {
    pub execution_request_id: String,
    pub connector_name: String,
    pub market_id: String,
    pub side: SignalSide,
    pub limit_price: Probability,
    pub quantity: Quantity,
    pub notional: UsdAmount,
}

#[derive(Debug, Clone)]
pub struct MockPolymarketOrderAcceptance {
    pub order_id: String,
    pub accepted_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct MockPolymarketOrderRejection {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum MockPolymarketExecutionOutcome {
    Accepted(MockPolymarketOrderAcceptance),
    Rejected(MockPolymarketOrderRejection),
}

#[derive(Debug, Clone)]
pub struct MockPolymarketOrderStatusRequest {
    pub connector_name: String,
    pub external_order_id: String,
    pub current_status: OrderStatus,
}

#[derive(Debug, Clone)]
pub struct MockPolymarketOrderStatusPayload {
    pub event_id: String,
    pub order_id: String,
    pub status: String,
    pub observed_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct MockPolymarketFillRequest {
    pub execution_request_id: String,
    pub connector_name: String,
    pub account_id: String,
    pub external_order_id: String,
    pub market_id: String,
    pub side: SignalSide,
    pub fill_price: Probability,
    pub total_quantity: Quantity,
    pub already_filled_quantity: Quantity,
}

#[derive(Debug, Clone)]
pub struct MockPolymarketTradePayload {
    pub event_id: String,
    pub order_id: String,
    pub account_id: String,
    pub trade_id: String,
    pub price: Probability,
    pub size: Quantity,
    pub fee: UsdAmount,
    pub executed_at: OffsetDateTime,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MockPolymarketConnector;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorOrderStatusUpdate {
    pub event_id: String,
    pub connector_name: String,
    pub external_order_id: String,
    pub status: OrderStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorTradeFillUpdate {
    pub event_id: String,
    pub connector_name: String,
    pub external_order_id: String,
    pub account_id: String,
    pub external_trade_id: String,
    pub fill_price: Probability,
    pub filled_quantity: Quantity,
    pub fee: UsdAmount,
}
