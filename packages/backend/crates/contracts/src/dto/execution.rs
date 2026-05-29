// Execution DTOs: order drafts, execution requests, orders, trades, positions, and submit request/result.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderDraftData {
    pub id: String,
    pub signal_id: String,
    pub signal_version: i64,
    pub market_id: String,
    pub connector_name: String,
    pub side: SignalSide,
    pub limit_price: Probability,
    pub quantity: Quantity,
    pub notional: UsdAmount,
    pub status: OrderDraftStatus,
    pub created_by_user_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_order_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub submitted_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_message: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRequestData {
    pub id: String,
    pub signal_id: String,
    pub signal_version: i64,
    pub order_draft_id: String,
    pub connector_name: String,
    pub mode: SystemMode,
    pub requested_by_user_id: String,
    pub status: ExecutionRequestStatus,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_order_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub submitted_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_message: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderData {
    pub id: String,
    pub signal_id: String,
    pub execution_request_id: String,
    pub order_draft_id: String,
    pub market_id: String,
    pub connector_name: String,
    pub account_id: String,
    pub external_order_id: String,
    pub side: SignalSide,
    pub limit_price: Probability,
    pub quantity: Quantity,
    pub filled_quantity: Quantity,
    pub avg_fill_price: Probability,
    pub status: OrderStatus,
    #[serde(with = "time::serde::rfc3339")]
    pub submitted_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeData {
    pub id: String,
    pub order_id: String,
    pub signal_id: String,
    pub market_id: String,
    pub connector_name: String,
    pub external_trade_id: String,
    pub side: SignalSide,
    pub price: Probability,
    pub quantity: Quantity,
    pub fee: UsdAmount,
    #[serde(with = "time::serde::rfc3339")]
    pub executed_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionData {
    pub id: String,
    pub market_id: String,
    pub connector_name: String,
    pub account_id: String,
    pub side: SignalSide,
    pub net_quantity: Quantity,
    pub avg_cost: Probability,
    pub mark_price: Probability,
    pub unrealized_pnl: SignedUsdAmount,
    pub realized_pnl: SignedUsdAmount,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitExecutionRequest {
    pub limit_price: Probability,
    pub quantity: Quantity,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_signal_version: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connector_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitExecutionData {
    pub order_draft: OrderDraftData,
    pub execution_request: ExecutionRequestData,
    pub risk_state: RiskStateData,
    pub replayed: bool,
}
