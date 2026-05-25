#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderDraftView {
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
pub struct ExecutionRequestView {
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
pub struct OrderView {
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
pub struct TradeView {
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
pub struct PositionView {
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

#[derive(Debug, Clone)]
pub struct OrderDraftListFilters {
    pub signal_id: Option<String>,
    pub connector_name: Option<String>,
    pub status: Option<OrderDraftStatus>,
    pub limit: u16,
}

impl OrderDraftListFilters {
    pub fn new(
        signal_id: Option<String>,
        connector_name: Option<String>,
        status: Option<OrderDraftStatus>,
        limit: Option<u16>,
    ) -> Result<Self> {
        Ok(Self {
            signal_id: validate_optional_id("signal_id", signal_id)?,
            connector_name: validate_optional_connector_name(connector_name)?,
            status,
            limit: validate_limit(limit)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ExecutionRequestListFilters {
    pub signal_id: Option<String>,
    pub connector_name: Option<String>,
    pub status: Option<ExecutionRequestStatus>,
    pub limit: u16,
}

impl ExecutionRequestListFilters {
    pub fn new(
        signal_id: Option<String>,
        connector_name: Option<String>,
        status: Option<ExecutionRequestStatus>,
        limit: Option<u16>,
    ) -> Result<Self> {
        Ok(Self {
            signal_id: validate_optional_id("signal_id", signal_id)?,
            connector_name: validate_optional_connector_name(connector_name)?,
            status,
            limit: validate_limit(limit)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct DispatchExecutionListFilters {
    pub connector_name: Option<String>,
    pub limit: u16,
}

impl DispatchExecutionListFilters {
    pub fn new(connector_name: Option<String>, limit: Option<u16>) -> Result<Self> {
        Ok(Self {
            connector_name: validate_optional_connector_name(connector_name)?,
            limit: validate_limit(limit)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct OrderListFilters {
    pub signal_id: Option<String>,
    pub market_id: Option<String>,
    pub connector_name: Option<String>,
    pub status: Option<OrderStatus>,
    pub limit: u16,
}

impl OrderListFilters {
    pub fn new(
        signal_id: Option<String>,
        market_id: Option<String>,
        connector_name: Option<String>,
        status: Option<OrderStatus>,
        limit: Option<u16>,
    ) -> Result<Self> {
        Ok(Self {
            signal_id: validate_optional_id("signal_id", signal_id)?,
            market_id: validate_optional_id("market_id", market_id)?,
            connector_name: validate_optional_connector_name(connector_name)?,
            status,
            limit: validate_limit(limit)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct TradeListFilters {
    pub order_id: Option<String>,
    pub signal_id: Option<String>,
    pub market_id: Option<String>,
    pub connector_name: Option<String>,
    pub limit: u16,
}

impl TradeListFilters {
    pub fn new(
        order_id: Option<String>,
        signal_id: Option<String>,
        market_id: Option<String>,
        connector_name: Option<String>,
        limit: Option<u16>,
    ) -> Result<Self> {
        Ok(Self {
            order_id: validate_optional_id("order_id", order_id)?,
            signal_id: validate_optional_id("signal_id", signal_id)?,
            market_id: validate_optional_id("market_id", market_id)?,
            connector_name: validate_optional_connector_name(connector_name)?,
            limit: validate_limit(limit)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct PositionListFilters {
    pub market_id: Option<String>,
    pub connector_name: Option<String>,
    pub side: Option<SignalSide>,
    pub limit: u16,
}

impl PositionListFilters {
    pub fn new(
        market_id: Option<String>,
        connector_name: Option<String>,
        side: Option<SignalSide>,
        limit: Option<u16>,
    ) -> Result<Self> {
        Ok(Self {
            market_id: validate_optional_id("market_id", market_id)?,
            connector_name: validate_optional_connector_name(connector_name)?,
            side,
            limit: validate_limit(limit)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ReconcileExecutionListFilters {
    pub connector_name: Option<String>,
    pub limit: u16,
}

impl ReconcileExecutionListFilters {
    pub fn new(connector_name: Option<String>, limit: Option<u16>) -> Result<Self> {
        Ok(Self {
            connector_name: validate_optional_connector_name(connector_name)?,
            limit: validate_limit(limit)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct SubmitExecutionCommand {
    pub signal_id: String,
    pub expected_signal_version: Option<i64>,
    pub limit_price: Probability,
    pub quantity: Quantity,
    pub connector_name: Option<String>,
    pub reason: String,
    pub request_id: String,
    pub trace_id: String,
    pub actor: AuthenticatedActor,
}

#[derive(Debug, Clone)]
pub struct SubmitExecutionStoreCommand {
    pub signal_id: String,
    pub expected_signal_version: Option<i64>,
    pub limit_price: Probability,
    pub quantity: Quantity,
    pub connector_name: String,
    pub reason: String,
    pub requested_by_user_id: String,
    pub trace_id: String,
    pub mode: SystemMode,
    pub risk_state_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSubmissionResult {
    pub order_draft: OrderDraftView,
    pub execution_request: ExecutionRequestView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionDispatchCandidate {
    pub order_draft: OrderDraftView,
    pub execution_request: ExecutionRequestView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionDispatchResult {
    pub order_draft: OrderDraftView,
    pub execution_request: ExecutionRequestView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionReconciliationCandidate {
    pub order_draft: OrderDraftView,
    pub execution_request: ExecutionRequestView,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<OrderView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionFillResult {
    pub order: OrderView,
    pub trade: TradeView,
    pub position: PositionView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSubmissionReceipt {
    pub order_draft: OrderDraftView,
    pub execution_request: ExecutionRequestView,
    pub risk_state: RiskStateView,
    pub replayed: bool,
}

#[derive(Debug, Clone)]
pub struct MarkExecutionSubmittedCommand {
    pub execution_request_id: String,
    pub account_id: String,
    pub external_order_id: String,
    pub request_id: String,
    pub trace_id: String,
    pub actor: AuthenticatedActor,
}

#[derive(Debug, Clone)]
pub struct MarkOrderOpenCommand {
    pub order_id: String,
    pub request_id: String,
    pub trace_id: String,
    pub actor: AuthenticatedActor,
}

#[derive(Debug, Clone)]
pub struct SyncExternalOrderStatusCommand {
    pub connector_name: String,
    pub external_order_id: String,
    pub status: OrderStatus,
    pub request_id: String,
    pub trace_id: String,
    pub actor: AuthenticatedActor,
}

#[derive(Debug, Clone)]
pub struct MarkExecutionFailedCommand {
    pub execution_request_id: String,
    pub failure_code: String,
    pub failure_message: String,
    pub request_id: String,
    pub trace_id: String,
    pub actor: AuthenticatedActor,
}

#[derive(Debug, Clone)]
pub struct ReconcileExecutionFillCommand {
    pub execution_request_id: String,
    pub account_id: String,
    pub external_trade_id: String,
    pub fill_price: Probability,
    pub filled_quantity: Quantity,
    pub fee: UsdAmount,
    pub request_id: String,
    pub trace_id: String,
    pub actor: AuthenticatedActor,
}

#[derive(Debug, Clone)]
pub struct ReconcileExternalTradeCommand {
    pub connector_name: String,
    pub external_order_id: String,
    pub account_id: String,
    pub external_trade_id: String,
    pub fill_price: Probability,
    pub filled_quantity: Quantity,
    pub fee: UsdAmount,
    pub request_id: String,
    pub trace_id: String,
    pub actor: AuthenticatedActor,
}
