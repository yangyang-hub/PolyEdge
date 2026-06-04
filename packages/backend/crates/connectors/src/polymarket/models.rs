#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolymarketSignatureScheme {
    Eoa,
    Proxy,
    GnosisSafe,
    Poly1271,
}

impl From<PolymarketSignatureScheme> for SignatureType {
    fn from(value: PolymarketSignatureScheme) -> Self {
        match value {
            PolymarketSignatureScheme::Eoa => SignatureType::Eoa,
            PolymarketSignatureScheme::Proxy => SignatureType::Proxy,
            PolymarketSignatureScheme::GnosisSafe => SignatureType::GnosisSafe,
            PolymarketSignatureScheme::Poly1271 => SignatureType::Poly1271,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolymarketTokenOrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone)]
pub struct LivePolymarketTokenOrderRequest {
    pub client_order_id: String,
    pub connector_name: String,
    pub token_id: String,
    pub side: PolymarketTokenOrderSide,
    pub limit_price: Probability,
    pub quantity: Quantity,
    pub post_only: bool,
}

#[derive(Debug, Clone)]
pub struct LivePolymarketCancelOrderRequest {
    pub connector_name: String,
    pub external_order_id: String,
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
pub struct LivePolymarketTradeSyncOutcome {
    pub updates: Vec<ConnectorTradeFillUpdate>,
    pub order_status: Option<ConnectorOrderStatusUpdate>,
}

#[derive(Debug, Clone)]
pub struct LivePolymarketOrderAcceptance {
    pub order_id: String,
    pub status: PolymarketAcceptedOrderStatus,
    pub submitted_quantity: Quantity,
    pub accepted_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolymarketAcceptedOrderStatus {
    Live,
    Matched,
    Delayed,
}

impl PolymarketAcceptedOrderStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Matched => "matched",
            Self::Delayed => "delayed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LivePolymarketCancelAcceptance {
    pub external_order_id: String,
    pub cancelled_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub enum LivePolymarketExecutionOutcome {
    Accepted(LivePolymarketOrderAcceptance),
    Rejected(PolymarketOrderRejection),
}

#[derive(Debug, Clone)]
pub enum LivePolymarketCancelOutcome {
    Accepted(LivePolymarketCancelAcceptance),
    Rejected(PolymarketOrderRejection),
}

#[derive(Debug, Clone)]
pub struct PolymarketOrderRejection {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct LivePolymarketConnector {
    client: ClobClient<Authenticated<Normal>>,
    private_key: String,
    chain_id: u64,
    account_id: String,
    ws_host: String,
    signature_type: PolymarketSignatureScheme,
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
