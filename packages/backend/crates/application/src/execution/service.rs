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

        self.sync_risk_state_after_fill(trace_id).await?;

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

    async fn sync_risk_state_after_fill(&self, trace_id: &str) -> Result<RiskStateView> {
        let positions = self
            .market_event_service
            .list_positions(PositionListFilters {
                market_id: None,
                connector_name: None,
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
