use polyedge_domain::{
    AmbiguityLevel, Edge, EventStatus, EvidenceDirection, EvidenceStatus, ExecutionRequestStatus,
    ExposureRatio, MarketStatus, OrderDraftStatus, OrderStatus, Probability, Quantity,
    SignalAction, SignalLifecycleState, SignalSide, SignedUsdAmount, SystemMode, TimeHorizon,
    TradabilityStatus, UsdAmount,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiMeta {
    pub request_id: String,
    pub trace_id: String,
    #[serde(with = "time::serde::rfc3339")]
    pub generated_at: OffsetDateTime,
}

impl ApiMeta {
    #[must_use]
    pub fn new(request_id: impl Into<String>, trace_id: impl Into<String>) -> Self {
        Self {
            request_id: request_id.into(),
            trace_id: trace_id.into(),
            generated_at: OffsetDateTime::now_utc(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub data: T,
    pub meta: ApiMeta,
}

impl<T> ApiResponse<T> {
    #[must_use]
    pub fn new(data: T, request_id: impl Into<String>, trace_id: impl Into<String>) -> Self {
        Self {
            data,
            meta: ApiMeta::new(request_id, trace_id),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<BTreeMap<String, String>>,
    pub retryable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorMeta {
    pub request_id: String,
    pub trace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    pub error: ApiError,
    pub meta: ApiErrorMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthData {
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyStatus {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessData {
    pub status: String,
    pub postgres: DependencyStatus,
    pub redis: DependencyStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemModeData {
    pub mode: SystemMode,
    pub environment: String,
    pub version: i64,
    pub replayed: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionSystemModeRequest {
    pub to_mode: SystemMode,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketData {
    pub id: String,
    pub question: String,
    pub category: String,
    pub status: MarketStatus,
    pub best_bid: Probability,
    pub best_ask: Probability,
    pub mid_price: Probability,
    pub volume_24h: UsdAmount,
    pub ambiguity_level: AmbiguityLevel,
    pub tradability_status: TradabilityStatus,
    pub resolution_source: String,
    pub edge_case_notes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polymarket_condition_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polymarket_yes_asset_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polymarket_no_asset_id: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventData {
    pub id: String,
    pub source: String,
    pub summary: String,
    pub relevance_score: Probability,
    pub confidence: Probability,
    pub status: EventStatus,
    pub related_market_ids: Vec<String>,
    pub reason_trace: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceData {
    pub id: String,
    pub market_id: String,
    pub event_id: String,
    pub direction: EvidenceDirection,
    pub strength: Probability,
    pub source_reliability: Probability,
    pub novelty: Probability,
    pub resolution_relevance: Probability,
    pub status: EvidenceStatus,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalData {
    pub id: String,
    pub market_id: String,
    pub event_id: String,
    pub action: SignalAction,
    pub side: SignalSide,
    pub market_price: Probability,
    pub fair_price: Probability,
    pub edge: Edge,
    pub confidence: Probability,
    pub lifecycle_state: SignalLifecycleState,
    pub reason: String,
    pub risk_decision: String,
    pub evidence_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_by_user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub approved_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejected_by_user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub rejected_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskStateData {
    pub mode: SystemMode,
    pub kill_switch: bool,
    pub daily_pnl: SignedUsdAmount,
    pub gross_exposure: ExposureRatio,
    pub net_exposure: ExposureRatio,
    pub open_alerts: u32,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

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
pub struct ProbabilityEstimateData {
    pub id: String,
    pub market_id: String,
    pub event_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal_id: Option<String>,
    pub prior_price: Probability,
    pub posterior_price: Probability,
    pub fair_price: Probability,
    pub market_price: Probability,
    pub edge: Edge,
    pub confidence: Probability,
    pub time_horizon: TimeHorizon,
    pub model_version: String,
    pub reason_codes: Vec<String>,
    pub evidence_count: u32,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalTransitionData {
    pub id: String,
    pub signal_id: String,
    pub from_state: SignalLifecycleState,
    pub to_state: SignalLifecycleState,
    pub trigger_type: String,
    pub trigger_payload: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecomputeSignalRequest {
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApproveSignalRequest {
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_version: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectSignalRequest {
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_version: Option<i64>,
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
pub struct TriggerKillSwitchRequest {
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_version: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseKillSwitchRequest {
    pub reason: String,
    pub to_mode: SystemMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_version: Option<i64>,
}

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
pub struct RecomputeSignalData {
    pub signal: SignalData,
    pub estimate: ProbabilityEstimateData,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition: Option<SignalTransitionData>,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApproveSignalData {
    pub signal: SignalData,
    pub risk_state: RiskStateData,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectSignalData {
    pub signal: SignalData,
    pub risk_state: RiskStateData,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitExecutionData {
    pub order_draft: OrderDraftData,
    pub execution_request: ExecutionRequestData,
    pub risk_state: RiskStateData,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KillSwitchData {
    pub risk_state: RiskStateData,
    pub replayed: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MarketListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<MarketStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tradability_status: Option<TradabilityStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<EventStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EvidenceListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<EvidenceStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SignalListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "status")]
    pub lifecycle_state: Option<SignalLifecycleState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProbabilityEstimateListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SignalTransitionListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrderDraftListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connector_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<OrderDraftStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecutionRequestListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connector_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<ExecutionRequestStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrderListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connector_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<OrderStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TradeListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connector_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PositionListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connector_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side: Option<SignalSide>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u16>,
}
