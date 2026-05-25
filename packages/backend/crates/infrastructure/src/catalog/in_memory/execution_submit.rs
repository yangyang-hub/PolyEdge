impl InMemoryMarketEventStore {
async fn market_event_submit_execution_request(
        &self,
        command: &SubmitExecutionStoreCommand,
    ) -> Result<ExecutionSubmissionResult> {
        let signal = {
            let signals = self.signals.read().await;
            signals.get(&command.signal_id).cloned().ok_or_else(|| {
                AppError::not_found(
                    "SIGNAL_NOT_FOUND",
                    format!("signal was not found: {}", command.signal_id),
                )
            })?
        };

        if let Some(expected_signal_version) = command.expected_signal_version {
            if signal.version != expected_signal_version {
                return Err(AppError::conflict(
                    "STATE_VERSION_MISMATCH",
                    "signal version does not match the expected_signal_version",
                ));
            }
        }

        validate_signal_for_execution(&signal)?;

        {
            let execution_requests = self.execution_requests.read().await;
            if execution_requests.values().any(|request| {
                request.signal_id == signal.id && request.signal_version == signal.version
            }) {
                return Err(AppError::conflict(
                    "STATE_EXECUTION_REQUEST_ALREADY_EXISTS",
                    "an execution request already exists for the current signal version",
                ));
            }
        }

        let now = OffsetDateTime::now_utc();
        let order_draft = OrderDraftView {
            id: format!("odr_{}", Uuid::now_v7()),
            signal_id: signal.id.clone(),
            signal_version: signal.version,
            market_id: signal.market_id.clone(),
            connector_name: command.connector_name.clone(),
            side: signal.side,
            limit_price: command.limit_price,
            quantity: command.quantity,
            notional: compute_order_notional(command.limit_price, command.quantity)?,
            status: OrderDraftStatus::Queued,
            created_by_user_id: command.requested_by_user_id.clone(),
            external_order_id: None,
            submitted_at: None,
            failure_code: None,
            failure_message: None,
            created_at: now,
            updated_at: now,
            version: 1,
        };
        let execution_request = ExecutionRequestView {
            id: format!("exr_{}", Uuid::now_v7()),
            signal_id: signal.id,
            signal_version: signal.version,
            order_draft_id: order_draft.id.clone(),
            connector_name: command.connector_name.clone(),
            mode: command.mode,
            requested_by_user_id: command.requested_by_user_id.clone(),
            status: ExecutionRequestStatus::Queued,
            reason: command.reason.clone(),
            external_order_id: None,
            submitted_at: None,
            failure_code: None,
            failure_message: None,
            created_at: now,
            updated_at: now,
            version: 1,
        };

        self.order_drafts
            .write()
            .await
            .insert(order_draft.id.clone(), order_draft.clone());
        self.execution_requests
            .write()
            .await
            .insert(execution_request.id.clone(), execution_request.clone());

        Ok(ExecutionSubmissionResult {
            order_draft,
            execution_request,
        })
    }

async fn market_event_list_dispatch_candidates(
        &self,
        filters: &DispatchExecutionListFilters,
    ) -> Result<Vec<ExecutionDispatchCandidate>> {
        let execution_requests = self.execution_requests.read().await;
        let order_drafts = self.order_drafts.read().await;
        let mut items: Vec<_> = execution_requests
            .values()
            .filter(|request| {
                request.status == ExecutionRequestStatus::Queued
                    && filters
                        .connector_name
                        .as_ref()
                        .is_none_or(|connector_name| &request.connector_name == connector_name)
            })
            .filter_map(|request| {
                let order_draft = order_drafts.get(&request.order_draft_id)?;
                (order_draft.status == OrderDraftStatus::Queued).then(|| {
                    ExecutionDispatchCandidate {
                        order_draft: order_draft.clone(),
                        execution_request: request.clone(),
                    }
                })
            })
            .collect();
        items.sort_by(|left, right| {
            left.execution_request
                .created_at
                .cmp(&right.execution_request.created_at)
                .then_with(|| left.execution_request.id.cmp(&right.execution_request.id))
        });
        items.truncate(usize::from(filters.limit));
        Ok(items)
    }

async fn market_event_list_reconciliation_candidates(
        &self,
        filters: &ReconcileExecutionListFilters,
    ) -> Result<Vec<ExecutionReconciliationCandidate>> {
        let execution_requests = self.execution_requests.read().await;
        let order_drafts = self.order_drafts.read().await;
        let orders = self.orders.read().await;
        let mut items: Vec<_> = execution_requests
            .values()
            .filter(|request| {
                request.status == ExecutionRequestStatus::Submitted
                    && filters
                        .connector_name
                        .as_ref()
                        .is_none_or(|connector_name| &request.connector_name == connector_name)
            })
            .filter_map(|request| {
                let order_draft = order_drafts.get(&request.order_draft_id)?;
                let order = orders
                    .values()
                    .find(|order| order.execution_request_id == request.id)
                    .cloned();
                let is_reconcilable = order.as_ref().is_none_or(|order| {
                    matches!(
                        order.status,
                        OrderStatus::Submitted | OrderStatus::Open | OrderStatus::PartiallyFilled
                    ) && order.filled_quantity.value() < order.quantity.value()
                });
                (order_draft.status == OrderDraftStatus::Submitted && is_reconcilable).then(|| {
                    ExecutionReconciliationCandidate {
                        order_draft: order_draft.clone(),
                        execution_request: request.clone(),
                        order,
                    }
                })
            })
            .collect();
        items.sort_by(|left, right| {
            left.execution_request
                .updated_at
                .cmp(&right.execution_request.updated_at)
                .then_with(|| left.execution_request.id.cmp(&right.execution_request.id))
        });
        items.truncate(usize::from(filters.limit));
        Ok(items)
    }
}
