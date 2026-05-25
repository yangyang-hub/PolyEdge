impl InMemoryMarketEventStore {
async fn market_event_mark_execution_submitted(
        &self,
        execution_request_id: &str,
        account_id: &str,
        external_order_id: &str,
        _trace_id: &str,
    ) -> Result<ExecutionDispatchResult> {
        let mut execution_requests = self.execution_requests.write().await;
        let mut order_drafts = self.order_drafts.write().await;
        let mut orders = self.orders.write().await;
        let request = execution_requests
            .get(execution_request_id)
            .cloned()
            .ok_or_else(|| {
                AppError::not_found(
                    "EXECUTION_REQUEST_NOT_FOUND",
                    format!("execution request was not found: {execution_request_id}"),
                )
            })?;

        if request.status != ExecutionRequestStatus::Queued {
            return Err(AppError::conflict(
                "STATE_EXECUTION_REQUEST_NOT_DISPATCHABLE",
                "execution request is no longer queued",
            ));
        }

        let order_draft = order_drafts
            .get(&request.order_draft_id)
            .cloned()
            .ok_or_else(|| {
                AppError::not_found(
                    "ORDER_DRAFT_NOT_FOUND",
                    format!("order draft was not found: {}", request.order_draft_id),
                )
            })?;

        if order_draft.status != OrderDraftStatus::Queued {
            return Err(AppError::conflict(
                "STATE_ORDER_DRAFT_NOT_DISPATCHABLE",
                "order draft is no longer queued",
            ));
        }

        let submitted_at = OffsetDateTime::now_utc();
        let next_order_draft = OrderDraftView {
            status: OrderDraftStatus::Submitted,
            external_order_id: Some(external_order_id.to_string()),
            submitted_at: Some(submitted_at),
            failure_code: None,
            failure_message: None,
            updated_at: submitted_at,
            version: order_draft.version + 1,
            ..order_draft
        };
        let next_request = ExecutionRequestView {
            status: ExecutionRequestStatus::Submitted,
            external_order_id: Some(external_order_id.to_string()),
            submitted_at: Some(submitted_at),
            failure_code: None,
            failure_message: None,
            updated_at: submitted_at,
            version: request.version + 1,
            ..request
        };
        let submitted_order = OrderView {
            id: format!("ord_{}", Uuid::now_v7()),
            signal_id: next_request.signal_id.clone(),
            execution_request_id: next_request.id.clone(),
            order_draft_id: next_order_draft.id.clone(),
            market_id: next_order_draft.market_id.clone(),
            connector_name: next_request.connector_name.clone(),
            account_id: account_id.to_string(),
            external_order_id: external_order_id.to_string(),
            side: next_order_draft.side,
            limit_price: next_order_draft.limit_price,
            quantity: next_order_draft.quantity,
            filled_quantity: Quantity::new(Decimal::ZERO)?,
            avg_fill_price: Probability::new(Decimal::ZERO)?,
            status: OrderStatus::Submitted,
            submitted_at,
            updated_at: submitted_at,
            version: 1,
        };

        order_drafts.insert(next_order_draft.id.clone(), next_order_draft.clone());
        execution_requests.insert(next_request.id.clone(), next_request.clone());
        orders.insert(submitted_order.id.clone(), submitted_order);

        Ok(ExecutionDispatchResult {
            order_draft: next_order_draft,
            execution_request: next_request,
        })
    }

async fn market_event_mark_order_open(&self, order_id: &str, _trace_id: &str) -> Result<OrderView> {
        let mut orders = self.orders.write().await;
        let order = orders.get(order_id).cloned().ok_or_else(|| {
            AppError::not_found(
                "ORDER_NOT_FOUND",
                format!("order was not found: {order_id}"),
            )
        })?;

        let next_order = match order.status {
            OrderStatus::Submitted => OrderView {
                status: OrderStatus::Open,
                updated_at: OffsetDateTime::now_utc(),
                version: order.version + 1,
                ..order
            },
            OrderStatus::Open => order,
            _ => {
                return Err(AppError::conflict(
                    "STATE_ORDER_NOT_POLLABLE",
                    "only submitted/open orders can be polled as open",
                ));
            }
        };

        orders.insert(next_order.id.clone(), next_order.clone());
        Ok(next_order)
    }

async fn market_event_mark_order_canceled(&self, order_id: &str, _trace_id: &str) -> Result<OrderView> {
        let mut orders = self.orders.write().await;
        let mut execution_requests = self.execution_requests.write().await;
        let order = orders.get(order_id).cloned().ok_or_else(|| {
            AppError::not_found(
                "ORDER_NOT_FOUND",
                format!("order was not found: {order_id}"),
            )
        })?;

        let next_order = match order.status {
            OrderStatus::Submitted | OrderStatus::Open | OrderStatus::PartiallyFilled => {
                OrderView {
                    status: OrderStatus::Canceled,
                    updated_at: OffsetDateTime::now_utc(),
                    version: order.version + 1,
                    ..order
                }
            }
            OrderStatus::Canceled => order,
            _ => {
                return Err(AppError::conflict(
                    "STATE_ORDER_NOT_CANCELABLE",
                    "only submitted/open/partially_filled orders can be canceled",
                ));
            }
        };

        if let Some(request) = execution_requests
            .get(&next_order.execution_request_id)
            .cloned()
            .filter(|request| request.status == ExecutionRequestStatus::Submitted)
        {
            execution_requests.insert(
                request.id.clone(),
                ExecutionRequestView {
                    status: ExecutionRequestStatus::Canceled,
                    updated_at: next_order.updated_at,
                    version: request.version + 1,
                    ..request
                },
            );
        }

        orders.insert(next_order.id.clone(), next_order.clone());
        Ok(next_order)
    }

async fn market_event_mark_execution_failed(
        &self,
        execution_request_id: &str,
        failure_code: &str,
        failure_message: &str,
        _trace_id: &str,
    ) -> Result<ExecutionDispatchResult> {
        let mut execution_requests = self.execution_requests.write().await;
        let mut order_drafts = self.order_drafts.write().await;
        let request = execution_requests
            .get(execution_request_id)
            .cloned()
            .ok_or_else(|| {
                AppError::not_found(
                    "EXECUTION_REQUEST_NOT_FOUND",
                    format!("execution request was not found: {execution_request_id}"),
                )
            })?;

        if request.status != ExecutionRequestStatus::Queued {
            return Err(AppError::conflict(
                "STATE_EXECUTION_REQUEST_NOT_DISPATCHABLE",
                "execution request is no longer queued",
            ));
        }

        let order_draft = order_drafts
            .get(&request.order_draft_id)
            .cloned()
            .ok_or_else(|| {
                AppError::not_found(
                    "ORDER_DRAFT_NOT_FOUND",
                    format!("order draft was not found: {}", request.order_draft_id),
                )
            })?;

        if order_draft.status != OrderDraftStatus::Queued {
            return Err(AppError::conflict(
                "STATE_ORDER_DRAFT_NOT_DISPATCHABLE",
                "order draft is no longer queued",
            ));
        }

        let failed_at = OffsetDateTime::now_utc();
        let next_order_draft = OrderDraftView {
            status: OrderDraftStatus::Rejected,
            external_order_id: None,
            submitted_at: None,
            failure_code: Some(failure_code.to_string()),
            failure_message: Some(failure_message.to_string()),
            updated_at: failed_at,
            version: order_draft.version + 1,
            ..order_draft
        };
        let next_request = ExecutionRequestView {
            status: ExecutionRequestStatus::Failed,
            external_order_id: None,
            submitted_at: None,
            failure_code: Some(failure_code.to_string()),
            failure_message: Some(failure_message.to_string()),
            updated_at: failed_at,
            version: request.version + 1,
            ..request
        };

        order_drafts.insert(next_order_draft.id.clone(), next_order_draft.clone());
        execution_requests.insert(next_request.id.clone(), next_request.clone());

        Ok(ExecutionDispatchResult {
            order_draft: next_order_draft,
            execution_request: next_request,
        })
    }

async fn market_event_reconcile_execution_fill(
        &self,
        input: MarketEventExecutionFill<'_>,
    ) -> Result<ExecutionFillResult> {
        let MarketEventExecutionFill {
            execution_request_id,
            account_id,
            external_trade_id,
            fill_price,
            filled_quantity,
            fee,
            trace_id,
        } = input;

        let execution_requests = self.execution_requests.read().await;
        let order_drafts = self.order_drafts.read().await;
        let mut orders = self.orders.write().await;
        let mut trades = self.trades.write().await;
        let mut positions = self.positions.write().await;
        let mut signals = self.signals.write().await;
        let mut transitions = self.signal_transitions.write().await;

        let request = execution_requests
            .get(execution_request_id)
            .cloned()
            .ok_or_else(|| {
                AppError::not_found(
                    "EXECUTION_REQUEST_NOT_FOUND",
                    format!("execution request was not found: {execution_request_id}"),
                )
            })?;

        if request.status != ExecutionRequestStatus::Submitted {
            return Err(AppError::conflict(
                "STATE_EXECUTION_REQUEST_NOT_RECONCILABLE",
                "execution request is not in submitted state",
            ));
        }

        if trades.values().any(|trade| {
            trade.connector_name == request.connector_name
                && trade.external_trade_id == external_trade_id
        }) {
            return Err(AppError::conflict(
                "STATE_TRADE_ALREADY_RECORDED",
                "external trade id has already been recorded",
            ));
        }

        let order_draft = order_drafts
            .get(&request.order_draft_id)
            .cloned()
            .ok_or_else(|| {
                AppError::not_found(
                    "ORDER_DRAFT_NOT_FOUND",
                    format!("order draft was not found: {}", request.order_draft_id),
                )
            })?;

        if order_draft.status != OrderDraftStatus::Submitted {
            return Err(AppError::conflict(
                "STATE_ORDER_DRAFT_NOT_RECONCILABLE",
                "order draft is not in submitted state",
            ));
        }

        let external_order_id = request
            .external_order_id
            .clone()
            .or_else(|| order_draft.external_order_id.clone())
            .ok_or_else(|| {
                AppError::conflict(
                    "STATE_EXTERNAL_ORDER_ID_MISSING",
                    "submitted execution request is missing external_order_id",
                )
            })?;

        let now = OffsetDateTime::now_utc();
        let submitted_at = request
            .submitted_at
            .or(order_draft.submitted_at)
            .unwrap_or(now);
        let order = if let Some(current) = orders
            .values()
            .find(|order| order.execution_request_id == request.id)
            .cloned()
        {
            if !matches!(
                current.status,
                OrderStatus::Submitted | OrderStatus::Open | OrderStatus::PartiallyFilled
            ) {
                return Err(AppError::conflict(
                    "STATE_ORDER_NOT_RECONCILABLE",
                    "existing order is not in a reconcilable state",
                ));
            }

            let next_filled_quantity_value =
                current.filled_quantity.value() + filled_quantity.value();
            if next_filled_quantity_value > current.quantity.value() {
                return Err(AppError::conflict(
                    "STATE_FILL_QUANTITY_EXCEEDS_ORDER",
                    "filled quantity exceeds order quantity",
                ));
            }

            let next_filled_quantity = Quantity::new(next_filled_quantity_value)?;
            let next_avg_fill_price = weighted_fill_price(
                current.avg_fill_price,
                current.filled_quantity,
                fill_price,
                filled_quantity,
            )?;
            let next_status = if next_filled_quantity.value() == current.quantity.value() {
                OrderStatus::Filled
            } else {
                OrderStatus::PartiallyFilled
            };

            OrderView {
                filled_quantity: next_filled_quantity,
                avg_fill_price: next_avg_fill_price,
                status: next_status,
                updated_at: now,
                version: current.version + 1,
                ..current
            }
        } else {
            let next_status = if filled_quantity.value() == order_draft.quantity.value() {
                OrderStatus::Filled
            } else {
                OrderStatus::PartiallyFilled
            };
            OrderView {
                id: format!("ord_{}", Uuid::now_v7()),
                signal_id: request.signal_id.clone(),
                execution_request_id: request.id.clone(),
                order_draft_id: order_draft.id.clone(),
                market_id: order_draft.market_id.clone(),
                connector_name: request.connector_name.clone(),
                account_id: account_id.to_string(),
                external_order_id,
                side: order_draft.side,
                limit_price: order_draft.limit_price,
                quantity: order_draft.quantity,
                filled_quantity,
                avg_fill_price: fill_price,
                status: next_status,
                submitted_at,
                updated_at: now,
                version: 1,
            }
        };
        let trade = TradeView {
            id: format!("trd_{}", Uuid::now_v7()),
            order_id: order.id.clone(),
            signal_id: order.signal_id.clone(),
            market_id: order.market_id.clone(),
            connector_name: order.connector_name.clone(),
            external_trade_id: external_trade_id.to_string(),
            side: order.side,
            price: fill_price,
            quantity: filled_quantity,
            fee,
            executed_at: now,
        };

        let position_key = in_memory_position_key(
            &order.connector_name,
            account_id,
            &order.market_id,
            order.side,
        );
        let position = if let Some(current) = positions.get(&position_key).cloned() {
            build_next_position(current, filled_quantity, fill_price, trace_id)?
        } else {
            PositionView {
                id: position_key.clone(),
                market_id: order.market_id.clone(),
                connector_name: order.connector_name.clone(),
                account_id: account_id.to_string(),
                side: order.side,
                net_quantity: filled_quantity,
                avg_cost: fill_price,
                mark_price: fill_price,
                unrealized_pnl: SignedUsdAmount::new(Decimal::ZERO)?,
                realized_pnl: SignedUsdAmount::new(Decimal::ZERO)?,
                updated_at: now,
                version: 1,
            }
        };

        let current_signal = signals.get(&order.signal_id).cloned().ok_or_else(|| {
            AppError::not_found(
                "SIGNAL_NOT_FOUND",
                format!("signal was not found: {}", order.signal_id),
            )
        })?;

        orders.insert(order.id.clone(), order.clone());
        trades.insert(trade.id.clone(), trade.clone());
        positions.insert(position.id.clone(), position.clone());
        if current_signal.lifecycle_state != SignalLifecycleState::Executed {
            let next_signal = SignalView {
                lifecycle_state: SignalLifecycleState::Executed,
                updated_at: now,
                version: current_signal.version + 1,
                ..current_signal.clone()
            };
            signals.insert(next_signal.id.clone(), next_signal.clone());
            transitions.push(SignalTransitionView {
                id: format!("sgt_{}", Uuid::now_v7()),
                signal_id: next_signal.id.clone(),
                from_state: current_signal.lifecycle_state,
                to_state: SignalLifecycleState::Executed,
                trigger_type: "execution_fill_reconciled".to_string(),
                trigger_payload: json!({
                    "execution_request_id": execution_request_id,
                    "order_id": order.id,
                    "trade_id": trade.id,
                    "trace_id": trace_id,
                }),
                created_at: now,
            });
        }

        Ok(ExecutionFillResult {
            order,
            trade,
            position,
        })
    }
}
