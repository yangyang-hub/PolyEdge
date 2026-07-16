async fn list_cash_flows(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ManualTradingListQuery>,
) -> Result<Json<ApiResponse<Vec<polyedge_contracts::CashFlowData>>>> {
    let context = authorize(&state, &headers, None).await?;
    Ok(response(
        state.store.list_cash_flows(context.actor, &query).await?,
        &context,
    ))
}

async fn record_cash_flow(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<polyedge_contracts::RecordCashFlowRequest>,
) -> Result<Json<ApiResponse<WriteOperationData>>> {
    let context = authorize_mutation(&state, &headers, Some(CASH_FLOW_RECORD_SCOPE)).await?;
    if !context.actor.is_admin() {
        return Err(ServerError::Forbidden);
    }
    let lease = begin_write(&state, &headers, "cash_flow.record", &request).await?;
    if let WriteLease::Replay(replay) = lease {
        return Ok(Json(replay));
    }
    let flow = state
        .store
        .record_cash_flow(context.actor, &request, &context.request_id)
        .await?;
    finish_write(
        &state,
        "cash_flow.record",
        lease,
        completed_operation("cash_flow", flow.id),
        &context,
    )
    .await
}
