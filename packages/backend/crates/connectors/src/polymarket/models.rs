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
    pub chain_id: u64,
    pub signature_type: PolymarketSignatureScheme,
    pub funder: Option<String>,
    pub private_key: String,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub api_passphrase: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolymarketTokenOrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolymarketOrderLifecycleStatus {
    Open,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
    Expired,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct PolymarketOrderSnapshot {
    pub external_order_id: String,
    pub status: PolymarketOrderLifecycleStatus,
    pub filled_quantity: Decimal,
}

/// An open order returned from the Polymarket CLOB API, converted to a
/// crate-local type so downstream crates do not need to depend on the SDK.
#[derive(Debug, Clone)]
pub struct PolymarketOpenOrder {
    pub id: String,
    pub market: String,
    pub asset_id: String,
    pub side: PolymarketTokenOrderSide,
    pub original_size: Decimal,
    pub size_matched: Decimal,
    pub price: Decimal,
    pub outcome: String,
    pub lifecycle_status: PolymarketOrderLifecycleStatus,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct PolymarketMatchedOrderHint {
    pub external_order_id: String,
    pub token_id: String,
    pub price: Decimal,
    pub size_matched: Decimal,
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
    Unmatched,
    Canceled,
    Unknown,
}

impl PolymarketAcceptedOrderStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Matched => "matched",
            Self::Delayed => "delayed",
            Self::Unmatched => "unmatched",
            Self::Canceled => "canceled",
            Self::Unknown => "unknown",
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
    signature_type: PolymarketSignatureScheme,
}
