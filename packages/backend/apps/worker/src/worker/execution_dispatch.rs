async fn dispatch_candidate(
    state: &AppState,
    executor: &PaperExecutor,
    candidate: ExecutionDispatchCandidate,
) -> Result<bool> {
    let request_id = new_trace_id();
    let trace_id = new_trace_id();
    let actor = worker_actor(&request_id);
    let execution_request_id = candidate.execution_request.id.clone();

    match executor.submit(&build_paper_order_request(candidate)) {
        Ok(PaperExecutionOutcome::Submitted(acceptance)) => {
            state
                .execution_service
                .mark_execution_submitted(MarkExecutionSubmittedCommand {
                    execution_request_id: execution_request_id.clone(),
                    account_id: PAPER_ACCOUNT_ID.to_string(),
                    external_order_id: acceptance.external_order_id.clone(),
                    request_id,
                    trace_id: trace_id.clone(),
                    actor,
                })
                .await?;
            info!(
                trace_id = %trace_id,
                execution_request_id = %execution_request_id,
                external_order_id = %acceptance.external_order_id,
                submitted_at = %acceptance.submitted_at,
                "paper executor accepted queued execution request",
            );
            Ok(true)
        }
        Ok(PaperExecutionOutcome::Rejected(rejection)) => {
            state
                .execution_service
                .mark_execution_failed(MarkExecutionFailedCommand {
                    execution_request_id: execution_request_id.clone(),
                    failure_code: rejection.code.clone(),
                    failure_message: rejection.message.clone(),
                    request_id,
                    trace_id: trace_id.clone(),
                    actor,
                })
                .await?;
            info!(
                trace_id = %trace_id,
                execution_request_id = %execution_request_id,
                failure_code = %rejection.code,
                "paper executor rejected queued execution request",
            );
            Ok(false)
        }
        Err(error) => {
            state
                .execution_service
                .mark_execution_failed(MarkExecutionFailedCommand {
                    execution_request_id: execution_request_id.clone(),
                    failure_code: error.code().to_string(),
                    failure_message: error.message().to_string(),
                    request_id,
                    trace_id: trace_id.clone(),
                    actor,
                })
                .await?;
            info!(
                trace_id = %trace_id,
                execution_request_id = %execution_request_id,
                failure_code = %error.code(),
                "paper executor dispatch failed before submission",
            );
            Ok(false)
        }
    }
}

async fn dispatch_live_polymarket_candidate(
    state: &AppState,
    connector: &LivePolymarketConnector,
    candidate: ExecutionDispatchCandidate,
) -> Result<bool> {
    let request_id = new_trace_id();
    let trace_id = new_trace_id();
    let actor = worker_actor(&request_id);
    let execution_request_id = candidate.execution_request.id.clone();
    let market = state
        .market_event_service
        .get_market(&candidate.order_draft.market_id)
        .await?;

    match connector
        .submit(&build_live_polymarket_order_request(candidate, &market)?)
        .await
    {
        Ok(LivePolymarketExecutionOutcome::Accepted(acceptance)) => {
            state
                .execution_service
                .mark_execution_submitted(MarkExecutionSubmittedCommand {
                    execution_request_id: execution_request_id.clone(),
                    account_id: connector.account_id().to_string(),
                    external_order_id: acceptance.order_id.clone(),
                    request_id,
                    trace_id: trace_id.clone(),
                    actor,
                })
                .await?;
            info!(
                trace_id = %trace_id,
                execution_request_id = %execution_request_id,
                external_order_id = %acceptance.order_id,
                accepted_at = %acceptance.accepted_at,
                "live polymarket connector accepted queued execution request",
            );
            Ok(true)
        }
        Ok(LivePolymarketExecutionOutcome::Rejected(rejection)) => {
            state
                .execution_service
                .mark_execution_failed(MarkExecutionFailedCommand {
                    execution_request_id: execution_request_id.clone(),
                    failure_code: rejection.code.clone(),
                    failure_message: rejection.message.clone(),
                    request_id,
                    trace_id: trace_id.clone(),
                    actor,
                })
                .await?;
            info!(
                trace_id = %trace_id,
                execution_request_id = %execution_request_id,
                failure_code = %rejection.code,
                "live polymarket connector rejected queued execution request",
            );
            Ok(false)
        }
        Err(error) => {
            state
                .execution_service
                .mark_execution_failed(MarkExecutionFailedCommand {
                    execution_request_id: execution_request_id.clone(),
                    failure_code: error.code().to_string(),
                    failure_message: error.message().to_string(),
                    request_id,
                    trace_id: trace_id.clone(),
                    actor,
                })
                .await?;
            info!(
                trace_id = %trace_id,
                execution_request_id = %execution_request_id,
                failure_code = %error.code(),
                "live polymarket connector dispatch failed before submission",
            );
            Ok(false)
        }
    }
}

fn build_paper_order_request(candidate: ExecutionDispatchCandidate) -> PaperOrderRequest {
    PaperOrderRequest {
        execution_request_id: candidate.execution_request.id,
        connector_name: candidate.order_draft.connector_name,
        market_id: candidate.order_draft.market_id,
        side: candidate.order_draft.side,
        limit_price: candidate.order_draft.limit_price,
        quantity: candidate.order_draft.quantity,
        notional: candidate.order_draft.notional,
    }
}

fn build_live_polymarket_order_request(
    candidate: ExecutionDispatchCandidate,
    market: &MarketView,
) -> Result<LivePolymarketOrderRequest> {
    Ok(LivePolymarketOrderRequest {
        execution_request_id: candidate.execution_request.id,
        connector_name: candidate.order_draft.connector_name,
        market_id: candidate.order_draft.market_id,
        side: candidate.order_draft.side,
        limit_price: candidate.order_draft.limit_price,
        quantity: candidate.order_draft.quantity,
        notional: candidate.order_draft.notional,
        market_refs: polymarket_market_refs(market)?,
    })
}

async fn reconcile_candidate(
    state: &AppState,
    executor: &PaperExecutor,
    candidate: ExecutionReconciliationCandidate,
) -> Result<()> {
    let request_id = new_trace_id();
    let trace_id = new_trace_id();
    let actor = worker_actor(&request_id);
    let external_order_id = candidate
        .execution_request
        .external_order_id
        .clone()
        .or(candidate.order_draft.external_order_id.clone())
        .unwrap_or_default();
    let fill = executor.reconcile_fill(&build_paper_fill_request(candidate))?;

    state
        .execution_service
        .reconcile_external_trade(ReconcileExternalTradeCommand {
            connector_name: PAPER_EXECUTOR_NAME.to_string(),
            external_order_id: external_order_id.clone(),
            account_id: fill.account_id.clone(),
            external_trade_id: fill.external_trade_id.clone(),
            fill_price: fill.fill_price,
            filled_quantity: fill.filled_quantity,
            fee: fill.fee,
            request_id,
            trace_id: trace_id.clone(),
            actor,
        })
        .await?;

    info!(
        trace_id = %trace_id,
        external_order_id = %external_order_id,
        external_trade_id = %fill.external_trade_id,
        executed_at = %fill.executed_at,
        "paper executor reconciled submitted execution fill",
    );

    Ok(())
}

async fn reconcile_live_polymarket_candidate(
    state: &AppState,
    connector: &LivePolymarketConnector,
    candidate: ExecutionReconciliationCandidate,
) -> Result<()> {
    let external_order_id = candidate
        .execution_request
        .external_order_id
        .clone()
        .or(candidate.order_draft.external_order_id.clone())
        .unwrap_or_default();

    let updates = connector
        .collect_trade_updates(&LivePolymarketTradeSyncRequest {
            connector_name: candidate.execution_request.connector_name.clone(),
            account_id: connector.account_id().to_string(),
            external_order_id: external_order_id.clone(),
        })
        .await?;

    for update in updates {
        let request_id = new_trace_id();
        let trace_id = new_trace_id();
        let actor = worker_actor(&request_id);
        state
            .execution_service
            .reconcile_external_trade(ReconcileExternalTradeCommand {
                connector_name: update.connector_name.clone(),
                external_order_id: update.external_order_id.clone(),
                account_id: update.account_id.clone(),
                external_trade_id: update.external_trade_id.clone(),
                fill_price: update.fill_price,
                filled_quantity: update.filled_quantity,
                fee: update.fee,
                request_id,
                trace_id: trace_id.clone(),
                actor,
            })
            .await?;

        info!(
            trace_id = %trace_id,
            external_order_id = %update.external_order_id,
            external_trade_id = %update.external_trade_id,
            connector_name = %update.connector_name,
            "live polymarket connector reconciled external trade update",
        );
    }

    Ok(())
}

async fn poll_order_status_candidate(
    state: &AppState,
    executor: &PaperExecutor,
    order: polyedge_application::OrderView,
) -> Result<bool> {
    let request_id = new_trace_id();
    let trace_id = new_trace_id();
    let actor = worker_actor(&request_id);
    let snapshot = executor.poll_order_status(&build_paper_order_status_request(order.clone()))?;

    if snapshot.status == OrderStatus::Open && order.status == OrderStatus::Submitted {
        state
            .execution_service
            .sync_external_order_status(SyncExternalOrderStatusCommand {
                connector_name: order.connector_name.clone(),
                external_order_id: order.external_order_id.clone(),
                status: snapshot.status,
                request_id,
                trace_id: trace_id.clone(),
                actor,
            })
            .await?;
        info!(
            trace_id = %trace_id,
            order_id = %order.id,
            external_order_id = %snapshot.external_order_id,
            observed_at = %snapshot.observed_at,
            "paper executor observed submitted order as open",
        );
        return Ok(true);
    }

    Ok(false)
}

async fn poll_live_polymarket_order_status_candidate(
    state: &AppState,
    connector: &LivePolymarketConnector,
    order: polyedge_application::OrderView,
) -> Result<bool> {
    let request_id = new_trace_id();
    let trace_id = new_trace_id();
    let actor = worker_actor(&request_id);
    let update = connector
        .poll_order_status(&LivePolymarketOrderStatusRequest {
            connector_name: order.connector_name.clone(),
            external_order_id: order.external_order_id.clone(),
        })
        .await?;

    let Some(update) = update else {
        return Ok(false);
    };

    state
        .execution_service
        .sync_external_order_status(SyncExternalOrderStatusCommand {
            connector_name: update.connector_name.clone(),
            external_order_id: update.external_order_id.clone(),
            status: update.status,
            request_id,
            trace_id: trace_id.clone(),
            actor,
        })
        .await?;

    info!(
        trace_id = %trace_id,
        order_id = %order.id,
        external_order_id = %update.external_order_id,
        connector_name = %update.connector_name,
        status = %update.status.as_str(),
        "live polymarket connector observed external order status change",
    );

    Ok(update.status == OrderStatus::Open && order.status == OrderStatus::Submitted)
}

fn build_paper_fill_request(candidate: ExecutionReconciliationCandidate) -> PaperFillRequest {
    let already_filled_quantity = candidate.order.as_ref().map_or_else(
        || Quantity::new(0.into()).expect("zero quantity"),
        |order| order.filled_quantity,
    );
    PaperFillRequest {
        execution_request_id: candidate.execution_request.id,
        connector_name: candidate.execution_request.connector_name,
        account_id: PAPER_ACCOUNT_ID.to_string(),
        external_order_id: candidate
            .execution_request
            .external_order_id
            .or(candidate.order_draft.external_order_id)
            .unwrap_or_default(),
        market_id: candidate.order_draft.market_id,
        side: candidate.order_draft.side,
        fill_price: candidate.order_draft.limit_price,
        total_quantity: candidate.order_draft.quantity,
        already_filled_quantity,
    }
}

fn build_paper_order_status_request(
    order: polyedge_application::OrderView,
) -> PaperOrderStatusRequest {
    PaperOrderStatusRequest {
        connector_name: order.connector_name,
        external_order_id: order.external_order_id,
        current_status: order.status,
    }
}
