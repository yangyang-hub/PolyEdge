use crate::{
    market_event::MarketEventService,
    risk::{RiskPolicy, RiskService, RiskStateView},
    system_mode::{AuditLogEntry, AuditLogSink, AuthenticatedActor},
};
use polyedge_domain::{
    AppError, ExecutionRequestStatus, ExposureRatio, OrderDraftStatus, OrderStatus, Probability,
    Quantity, Result, SignalSide, SignedUsdAmount, SystemMode, UsdAmount,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use time::OffsetDateTime;

const DEFAULT_LIST_LIMIT: u16 = 100;
const MAX_LIST_LIMIT: u16 = 200;
pub const DEFAULT_EXECUTION_CONNECTOR: &str = "paper_executor";

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

pub struct ExecutionService {
    market_event_service: Arc<MarketEventService>,
    risk_service: Arc<RiskService>,
    audit_log_sink: Arc<dyn AuditLogSink>,
}

impl ExecutionService {
    pub fn new(
        market_event_service: Arc<MarketEventService>,
        risk_service: Arc<RiskService>,
        audit_log_sink: Arc<dyn AuditLogSink>,
    ) -> Self {
        Self {
            market_event_service,
            risk_service,
            audit_log_sink,
        }
    }

    pub async fn list_order_drafts(
        &self,
        filters: OrderDraftListFilters,
    ) -> Result<Vec<OrderDraftView>> {
        self.market_event_service.list_order_drafts(filters).await
    }

    pub async fn list_execution_requests(
        &self,
        filters: ExecutionRequestListFilters,
    ) -> Result<Vec<ExecutionRequestView>> {
        self.market_event_service
            .list_execution_requests(filters)
            .await
    }

    pub async fn list_orders(&self, filters: OrderListFilters) -> Result<Vec<OrderView>> {
        self.market_event_service.list_orders(filters).await
    }

    pub async fn list_trades(&self, filters: TradeListFilters) -> Result<Vec<TradeView>> {
        self.market_event_service.list_trades(filters).await
    }

    pub async fn list_positions(&self, filters: PositionListFilters) -> Result<Vec<PositionView>> {
        self.market_event_service.list_positions(filters).await
    }

    pub async fn list_dispatch_candidates(
        &self,
        filters: DispatchExecutionListFilters,
    ) -> Result<Vec<ExecutionDispatchCandidate>> {
        self.market_event_service
            .list_dispatch_candidates(filters)
            .await
    }

    pub async fn list_reconciliation_candidates(
        &self,
        filters: ReconcileExecutionListFilters,
    ) -> Result<Vec<ExecutionReconciliationCandidate>> {
        self.market_event_service
            .list_reconciliation_candidates(filters)
            .await
    }

    pub async fn submit_execution_request(
        &self,
        command: SubmitExecutionCommand,
    ) -> Result<ExecutionSubmissionReceipt> {
        if command.signal_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "SIGNAL_ID_REQUIRED",
                "signal id must not be empty",
            ));
        }

        if command.reason.trim().is_empty() {
            return Err(AppError::invalid_input(
                "EXECUTION_REQUEST_REASON_REQUIRED",
                "execution request reason must not be empty",
            ));
        }

        if command.quantity.value() <= Decimal::ZERO {
            return Err(AppError::invalid_input(
                "EXECUTION_REQUEST_QUANTITY_REQUIRED",
                "execution quantity must be greater than zero",
            ));
        }

        let connector_name = normalize_connector_name(command.connector_name.clone())?;
        let risk_state = self.risk_service.read_state().await?;
        validate_execution_mode(risk_state.mode, risk_state.kill_switch)?;

        let result = match self
            .market_event_service
            .submit_execution_request(SubmitExecutionStoreCommand {
                signal_id: command.signal_id.clone(),
                expected_signal_version: command.expected_signal_version,
                limit_price: command.limit_price,
                quantity: command.quantity,
                connector_name,
                reason: command.reason.clone(),
                requested_by_user_id: command.actor.user_id.clone(),
                trace_id: command.trace_id.clone(),
                mode: risk_state.mode,
                risk_state_version: risk_state.version,
            })
            .await
        {
            Ok(result) => result,
            Err(error) => {
                self.append_audit(
                    &command,
                    AuditResultMarker::Rejected,
                    Some(error.code().to_string()),
                )
                .await?;
                return Err(error);
            }
        };

        self.append_audit(&command, AuditResultMarker::Succeeded, None)
            .await?;

        Ok(ExecutionSubmissionReceipt {
            order_draft: result.order_draft,
            execution_request: result.execution_request,
            risk_state,
            replayed: false,
        })
    }

    pub async fn mark_execution_submitted(
        &self,
        command: MarkExecutionSubmittedCommand,
    ) -> Result<ExecutionDispatchResult> {
        if command.execution_request_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "EXECUTION_REQUEST_ID_REQUIRED",
                "execution_request_id must not be empty",
            ));
        }

        if command.account_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "EXECUTION_ACCOUNT_ID_REQUIRED",
                "account_id must not be empty",
            ));
        }

        if command.external_order_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "EXTERNAL_ORDER_ID_REQUIRED",
                "external_order_id must not be empty",
            ));
        }

        let result = self
            .market_event_service
            .mark_execution_submitted(
                command.execution_request_id.clone(),
                command.account_id.clone(),
                command.external_order_id.clone(),
                command.trace_id.clone(),
            )
            .await?;

        self.append_worker_audit(
            "execution.request.dispatch",
            &command.request_id,
            &command.trace_id,
            &command.actor,
            "execution_request",
            &result.execution_request.id,
            "paper connector accepted queued execution request",
            AuditResultMarker::Succeeded,
            None,
        )
        .await?;

        Ok(result)
    }

    pub async fn mark_order_open(&self, command: MarkOrderOpenCommand) -> Result<OrderView> {
        if command.order_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "ORDER_ID_REQUIRED",
                "order_id must not be empty",
            ));
        }

        let order = self
            .market_event_service
            .mark_order_open(command.order_id.clone(), command.trace_id.clone())
            .await?;

        self.audit_log_sink
            .append(AuditLogEntry {
                occurred_at: OffsetDateTime::now_utc(),
                request_id: command.request_id.clone(),
                trace_id: command.trace_id.clone(),
                actor: command.actor,
                action: "execution.order.poll".to_string(),
                resource_type: "order".to_string(),
                resource_id: order.id.clone(),
                reason: "paper connector observed submitted order as open".to_string(),
                result: AuditResultMarker::Succeeded.into(),
                error_code: None,
            })
            .await?;

        Ok(order)
    }

    pub async fn sync_external_order_status(
        &self,
        command: SyncExternalOrderStatusCommand,
    ) -> Result<OrderView> {
        let connector_name = normalize_connector_name(Some(command.connector_name.clone()))?;

        if command.external_order_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "EXTERNAL_ORDER_ID_REQUIRED",
                "external_order_id must not be empty",
            ));
        }

        let current_order = self
            .market_event_service
            .get_order_by_external_ref(connector_name, command.external_order_id.clone())
            .await?;

        let next_order = match command.status {
            OrderStatus::Open => match current_order.status {
                OrderStatus::Submitted => {
                    self.market_event_service
                        .mark_order_open(current_order.id.clone(), command.trace_id.clone())
                        .await?
                }
                OrderStatus::Open | OrderStatus::PartiallyFilled => current_order,
                _ => {
                    return Err(AppError::conflict(
                        "STATE_EXTERNAL_ORDER_STATUS_INVALID",
                        "open status cannot be applied to the current order state",
                    ));
                }
            },
            OrderStatus::Canceled => match current_order.status {
                OrderStatus::Submitted | OrderStatus::Open | OrderStatus::PartiallyFilled => {
                    self.market_event_service
                        .mark_order_canceled(current_order.id.clone(), command.trace_id.clone())
                        .await?
                }
                OrderStatus::Canceled => current_order,
                _ => {
                    return Err(AppError::conflict(
                        "STATE_EXTERNAL_ORDER_STATUS_INVALID",
                        "canceled status cannot be applied to the current order state",
                    ));
                }
            },
            _ => {
                return Err(AppError::invalid_input(
                    "EXTERNAL_ORDER_STATUS_UNSUPPORTED",
                    "only open and canceled external order statuses are currently supported",
                ));
            }
        };

        self.audit_log_sink
            .append(AuditLogEntry {
                occurred_at: OffsetDateTime::now_utc(),
                request_id: command.request_id,
                trace_id: command.trace_id,
                actor: command.actor,
                action: "execution.order.sync_status".to_string(),
                resource_type: "order".to_string(),
                resource_id: next_order.id.clone(),
                reason: format!(
                    "connector status update mapped external_order_id={} to {}",
                    command.external_order_id,
                    next_order.status.as_str()
                ),
                result: AuditResultMarker::Succeeded.into(),
                error_code: None,
            })
            .await?;

        Ok(next_order)
    }

    pub async fn mark_execution_failed(
        &self,
        command: MarkExecutionFailedCommand,
    ) -> Result<ExecutionDispatchResult> {
        if command.execution_request_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "EXECUTION_REQUEST_ID_REQUIRED",
                "execution_request_id must not be empty",
            ));
        }

        if command.failure_code.trim().is_empty() {
            return Err(AppError::invalid_input(
                "EXECUTION_FAILURE_CODE_REQUIRED",
                "failure_code must not be empty",
            ));
        }

        if command.failure_message.trim().is_empty() {
            return Err(AppError::invalid_input(
                "EXECUTION_FAILURE_MESSAGE_REQUIRED",
                "failure_message must not be empty",
            ));
        }

        let result = self
            .market_event_service
            .mark_execution_failed(
                command.execution_request_id.clone(),
                command.failure_code.clone(),
                command.failure_message.clone(),
                command.trace_id.clone(),
            )
            .await?;

        self.append_worker_audit(
            "execution.request.dispatch",
            &command.request_id,
            &command.trace_id,
            &command.actor,
            "execution_request",
            &result.execution_request.id,
            &command.failure_message,
            AuditResultMarker::Failed,
            Some(command.failure_code.clone()),
        )
        .await?;

        Ok(result)
    }

    pub async fn reconcile_execution_fill(
        &self,
        command: ReconcileExecutionFillCommand,
    ) -> Result<ExecutionFillResult> {
        if command.execution_request_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "EXECUTION_REQUEST_ID_REQUIRED",
                "execution_request_id must not be empty",
            ));
        }

        if command.account_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "EXECUTION_ACCOUNT_ID_REQUIRED",
                "account_id must not be empty",
            ));
        }

        if command.external_trade_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "EXTERNAL_TRADE_ID_REQUIRED",
                "external_trade_id must not be empty",
            ));
        }

        if command.filled_quantity.value() <= Decimal::ZERO {
            return Err(AppError::invalid_input(
                "EXECUTION_FILL_QUANTITY_REQUIRED",
                "filled_quantity must be greater than zero",
            ));
        }

        self.apply_reconciled_fill(
            &command.execution_request_id,
            command.account_id.trim(),
            command.external_trade_id.trim(),
            command.fill_price,
            command.filled_quantity,
            command.fee,
            &command.request_id,
            &command.trace_id,
            &command.actor,
            "execution.request.reconcile_fill",
            "execution_request",
            &command.execution_request_id,
            "paper connector reconciled a submitted execution fill",
        )
        .await
    }

    pub async fn reconcile_external_trade(
        &self,
        command: ReconcileExternalTradeCommand,
    ) -> Result<ExecutionFillResult> {
        let connector_name = normalize_connector_name(Some(command.connector_name.clone()))?;

        if command.external_order_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "EXTERNAL_ORDER_ID_REQUIRED",
                "external_order_id must not be empty",
            ));
        }

        if command.account_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "EXECUTION_ACCOUNT_ID_REQUIRED",
                "account_id must not be empty",
            ));
        }

        if command.external_trade_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "EXTERNAL_TRADE_ID_REQUIRED",
                "external_trade_id must not be empty",
            ));
        }

        if command.filled_quantity.value() <= Decimal::ZERO {
            return Err(AppError::invalid_input(
                "EXECUTION_FILL_QUANTITY_REQUIRED",
                "filled_quantity must be greater than zero",
            ));
        }

        let order = self
            .market_event_service
            .get_order_by_external_ref(connector_name, command.external_order_id.clone())
            .await?;

        self.apply_reconciled_fill(
            &order.execution_request_id,
            command.account_id.trim(),
            command.external_trade_id.trim(),
            command.fill_price,
            command.filled_quantity,
            command.fee,
            &command.request_id,
            &command.trace_id,
            &command.actor,
            "execution.trade.sync",
            "order",
            &order.id,
            "connector trade update reconciled against internal order",
        )
        .await
    }

    async fn apply_reconciled_fill(
        &self,
        execution_request_id: &str,
        account_id: &str,
        external_trade_id: &str,
        fill_price: Probability,
        filled_quantity: Quantity,
        fee: UsdAmount,
        request_id: &str,
        trace_id: &str,
        actor: &AuthenticatedActor,
        audit_action: &str,
        audit_resource_type: &str,
        audit_resource_id: &str,
        audit_reason: &str,
    ) -> Result<ExecutionFillResult> {
        let result = self
            .market_event_service
            .reconcile_execution_fill(
                execution_request_id.to_string(),
                account_id,
                external_trade_id,
                fill_price,
                filled_quantity,
                fee,
                trace_id,
            )
            .await?;

        self.sync_risk_state_after_fill(&result, trace_id).await?;

        self.append_worker_audit(
            audit_action,
            request_id,
            trace_id,
            actor,
            audit_resource_type,
            audit_resource_id,
            audit_reason,
            AuditResultMarker::Succeeded,
            None,
        )
        .await?;

        Ok(result)
    }

    async fn sync_risk_state_after_fill(
        &self,
        fill_result: &ExecutionFillResult,
        trace_id: &str,
    ) -> Result<RiskStateView> {
        let positions = self
            .market_event_service
            .list_positions(PositionListFilters {
                market_id: None,
                connector_name: Some(fill_result.position.connector_name.clone()),
                side: None,
                limit: u16::MAX,
            })
            .await?;
        let (daily_pnl, gross_exposure, net_exposure) =
            aggregate_execution_risk_metrics(&positions, self.risk_service.policy())?;

        self.risk_service
            .sync_execution_metrics(daily_pnl, gross_exposure, net_exposure, trace_id)
            .await
    }

    async fn append_audit(
        &self,
        command: &SubmitExecutionCommand,
        result: AuditResultMarker,
        error_code: Option<String>,
    ) -> Result<()> {
        self.audit_log_sink
            .append(AuditLogEntry {
                occurred_at: OffsetDateTime::now_utc(),
                request_id: command.request_id.clone(),
                trace_id: command.trace_id.clone(),
                actor: command.actor.clone(),
                action: "execution.request.submit".to_string(),
                resource_type: "signal".to_string(),
                resource_id: command.signal_id.clone(),
                reason: command.reason.clone(),
                result: result.into(),
                error_code,
            })
            .await
    }

    async fn append_worker_audit(
        &self,
        action: &str,
        request_id: &str,
        trace_id: &str,
        actor: &AuthenticatedActor,
        resource_type: &str,
        resource_id: &str,
        reason: &str,
        result: AuditResultMarker,
        error_code: Option<String>,
    ) -> Result<()> {
        self.audit_log_sink
            .append(AuditLogEntry {
                occurred_at: OffsetDateTime::now_utc(),
                request_id: request_id.to_string(),
                trace_id: trace_id.to_string(),
                actor: actor.clone(),
                action: action.to_string(),
                resource_type: resource_type.to_string(),
                resource_id: resource_id.to_string(),
                reason: reason.to_string(),
                result: result.into(),
                error_code,
            })
            .await
    }
}

#[derive(Debug, Clone, Copy)]
enum AuditResultMarker {
    Rejected,
    Succeeded,
    Failed,
}

impl From<AuditResultMarker> for polyedge_domain::AuditResult {
    fn from(value: AuditResultMarker) -> Self {
        match value {
            AuditResultMarker::Rejected => Self::Rejected,
            AuditResultMarker::Succeeded => Self::Succeeded,
            AuditResultMarker::Failed => Self::Failed,
        }
    }
}

fn validate_execution_mode(mode: SystemMode, kill_switch: bool) -> Result<()> {
    if kill_switch || mode == SystemMode::KillSwitchLocked {
        return Err(AppError::forbidden(
            "RISK_KILL_SWITCH_ACTIVE",
            "execution submission is blocked while the kill switch is active",
        ));
    }

    match mode {
        SystemMode::ManualConfirm | SystemMode::PaperTrade => Ok(()),
        SystemMode::Research => Err(AppError::conflict(
            "STATE_EXECUTION_MODE_INVALID",
            "execution submission is not available in research mode",
        )),
        SystemMode::LiveAuto => Err(AppError::conflict(
            "STATE_EXECUTION_MODE_NOT_SUPPORTED",
            "live_auto execution submission is not available until the connector is enabled",
        )),
        SystemMode::KillSwitchLocked => Err(AppError::forbidden(
            "RISK_KILL_SWITCH_ACTIVE",
            "execution submission is blocked while the kill switch is active",
        )),
    }
}

fn aggregate_execution_risk_metrics(
    positions: &[PositionView],
    policy: &RiskPolicy,
) -> Result<(SignedUsdAmount, ExposureRatio, ExposureRatio)> {
    let reference_nav = policy.exposure_reference_nav.value();
    if reference_nav <= Decimal::ZERO {
        return Err(AppError::internal(
            "RISK_REFERENCE_NAV_INVALID",
            "risk exposure_reference_nav must be greater than zero",
        ));
    }

    let mut daily_pnl = Decimal::ZERO;
    let mut gross_notional = Decimal::ZERO;
    let mut signed_notional = Decimal::ZERO;

    for position in positions {
        let mark_notional = position.net_quantity.value() * position.mark_price.value();
        gross_notional += mark_notional;
        signed_notional += match position.side {
            SignalSide::Yes => mark_notional,
            SignalSide::No => -mark_notional,
        };
        daily_pnl += position.realized_pnl.value() + position.unrealized_pnl.value();
    }

    let gross_exposure = ExposureRatio::new(gross_notional / reference_nav).map_err(|error| {
        AppError::internal(
            "RISK_GROSS_EXPOSURE_COMPUTE_FAILED",
            format!("failed to compute gross exposure: {error}"),
        )
    })?;
    let net_exposure =
        ExposureRatio::new(signed_notional.abs() / reference_nav).map_err(|error| {
            AppError::internal(
                "RISK_NET_EXPOSURE_COMPUTE_FAILED",
                format!("failed to compute net exposure: {error}"),
            )
        })?;
    let daily_pnl = SignedUsdAmount::new(daily_pnl).map_err(|error| {
        AppError::internal(
            "RISK_DAILY_PNL_COMPUTE_FAILED",
            format!("failed to compute daily pnl: {error}"),
        )
    })?;

    Ok((daily_pnl, gross_exposure, net_exposure))
}

fn normalize_connector_name(connector_name: Option<String>) -> Result<String> {
    let connector_name = connector_name
        .unwrap_or_else(|| DEFAULT_EXECUTION_CONNECTOR.to_string())
        .trim()
        .to_ascii_lowercase();

    if connector_name.is_empty() {
        return Err(AppError::invalid_input(
            "EXECUTION_CONNECTOR_NAME_REQUIRED",
            "connector name must not be empty",
        ));
    }

    Ok(connector_name)
}

fn validate_limit(limit: Option<u16>) -> Result<u16> {
    let limit = limit.unwrap_or(DEFAULT_LIST_LIMIT);
    if limit == 0 || limit > MAX_LIST_LIMIT {
        return Err(AppError::invalid_input(
            "LIST_LIMIT_INVALID",
            format!("limit must be within 1..={MAX_LIST_LIMIT}"),
        ));
    }

    Ok(limit)
}

fn validate_optional_id(field_name: &str, value: Option<String>) -> Result<Option<String>> {
    value
        .map(|raw| {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                Err(AppError::invalid_input(
                    "FILTER_ID_INVALID",
                    format!("{field_name} must not be empty when provided"),
                ))
            } else {
                Ok(trimmed.to_string())
            }
        })
        .transpose()
}

fn validate_optional_connector_name(value: Option<String>) -> Result<Option<String>> {
    value
        .map(|raw| normalize_connector_name(Some(raw)))
        .transpose()
}
