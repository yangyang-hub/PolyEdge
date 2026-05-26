async fn reconcile_paper_fills(
    state: &AppState,
    connector_name: Option<String>,
    limit: Option<u16>,
) -> Result<FillReconciliationReport> {
    let connector_name = connector_name.unwrap_or_else(|| PAPER_EXECUTOR_NAME.to_string());
    let candidates = state
        .execution_service
        .list_reconciliation_candidates(ReconcileExecutionListFilters::new(
            Some(connector_name),
            limit,
        )?)
        .await?;
    let executor = PaperExecutor::new();
    let mut report = FillReconciliationReport {
        scanned: candidates.len(),
        ..FillReconciliationReport::default()
    };

    for candidate in candidates {
        reconcile_candidate(state, &executor, candidate).await?;
        report.reconciled += 1;
    }

    Ok(report)
}

async fn reconcile_polymarket_fills(
    state: &AppState,
    connector_name: Option<String>,
    limit: Option<u16>,
) -> Result<FillReconciliationReport> {
    let connector_name = connector_name.unwrap_or_else(|| POLYMARKET_CONNECTOR_NAME.to_string());
    let candidates = state
        .execution_service
        .list_reconciliation_candidates(ReconcileExecutionListFilters::new(
            Some(connector_name),
            limit,
        )?)
        .await?;
    let mut report = FillReconciliationReport {
        scanned: candidates.len(),
        ..FillReconciliationReport::default()
    };

    let connector = build_live_polymarket_connector(state).await?;
    for candidate in candidates {
        reconcile_live_polymarket_candidate(state, &connector, candidate).await?;
        report.reconciled += 1;
    }

    Ok(report)
}

async fn poll_paper_order_statuses(
    state: &AppState,
    connector_name: Option<String>,
    limit: Option<u16>,
) -> Result<OrderStatusPollReport> {
    let connector_name = connector_name.unwrap_or_else(|| PAPER_EXECUTOR_NAME.to_string());
    let orders = state
        .execution_service
        .list_orders(OrderListFilters::new(
            None,
            None,
            Some(connector_name.clone()),
            Some(OrderStatus::Submitted),
            limit,
        )?)
        .await?;
    let executor = PaperExecutor::new();
    let mut report = OrderStatusPollReport {
        scanned: orders.len(),
        ..OrderStatusPollReport::default()
    };

    for order in orders {
        if poll_order_status_candidate(state, &executor, order).await? {
            report.opened += 1;
        }
    }

    Ok(report)
}

async fn poll_polymarket_order_statuses(
    state: &AppState,
    connector_name: Option<String>,
    limit: Option<u16>,
) -> Result<OrderStatusPollReport> {
    let connector_name = connector_name.unwrap_or_else(|| POLYMARKET_CONNECTOR_NAME.to_string());
    let orders = state
        .execution_service
        .list_orders(OrderListFilters::new(
            None,
            None,
            Some(connector_name.clone()),
            Some(OrderStatus::Submitted),
            limit,
        )?)
        .await?;
    let mut report = OrderStatusPollReport {
        scanned: orders.len(),
        ..OrderStatusPollReport::default()
    };

    let connector = build_live_polymarket_connector(state).await?;
    for order in orders {
        if poll_live_polymarket_order_status_candidate(state, &connector, order).await? {
            report.opened += 1;
        }
    }

    Ok(report)
}
