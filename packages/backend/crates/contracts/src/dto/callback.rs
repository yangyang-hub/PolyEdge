// Connector and Polymarket callback request/result DTOs.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorOrderStatusCallbackRequest {
    pub event_id: String,
    pub connector_name: String,
    pub external_order_id: String,
    pub status: OrderStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorTradeFillCallbackRequest {
    pub event_id: String,
    pub connector_name: String,
    pub external_order_id: String,
    pub account_id: String,
    pub external_trade_id: String,
    pub fill_price: Probability,
    pub filled_quantity: Quantity,
    pub fee: UsdAmount,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolymarketOrderStatus {
    Live,
    Matched,
    Delayed,
    Canceled,
}

impl PolymarketOrderStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Matched => "matched",
            Self::Delayed => "delayed",
            Self::Canceled => "canceled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolymarketOrderStatusCallbackRequest {
    pub event_id: String,
    pub order_id: String,
    pub status: PolymarketOrderStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolymarketTradeFillCallbackRequest {
    pub event_id: String,
    pub order_id: String,
    pub account_id: String,
    pub trade_id: String,
    pub price: Probability,
    pub size: Quantity,
    pub fee: UsdAmount,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorOrderStatusCallbackData {
    pub order: OrderData,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorTradeFillCallbackData {
    pub order: OrderData,
    pub trade: TradeData,
    pub position: PositionData,
    pub risk_state: RiskStateData,
    pub replayed: bool,
}
