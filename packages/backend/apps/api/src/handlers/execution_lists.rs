async fn list_signal_transitions(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Path(signal_id): Path<String>,
    Query(query): Query<SignalTransitionListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<SignalTransitionData>>>, HttpError> {
    let trace_id = new_trace_id();
    let filters = SignalTransitionListFilters::new(signal_id, query.limit)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let transitions = state
        .market_event_service
        .list_signal_transitions(filters)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        transitions
            .into_iter()
            .map(signal_transition_to_contract)
            .collect(),
        auth.request_id,
        trace_id,
    )))
}

async fn list_order_drafts(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<OrderDraftListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<OrderDraftData>>>, HttpError> {
    let trace_id = new_trace_id();
    let filters = OrderDraftListFilters::new(
        query.signal_id,
        query.connector_name,
        query.status,
        query.limit,
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let order_drafts = state
        .execution_service
        .list_order_drafts(filters)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        order_drafts
            .into_iter()
            .map(order_draft_to_contract)
            .collect(),
        auth.request_id,
        trace_id,
    )))
}

async fn list_execution_requests(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<ExecutionRequestListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<ExecutionRequestData>>>, HttpError> {
    let trace_id = new_trace_id();
    let filters = ExecutionRequestListFilters::new(
        query.signal_id,
        query.connector_name,
        query.status,
        query.limit,
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let execution_requests = state
        .execution_service
        .list_execution_requests(filters)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        execution_requests
            .into_iter()
            .map(execution_request_to_contract)
            .collect(),
        auth.request_id,
        trace_id,
    )))
}

async fn list_orders(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<OrderListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<OrderData>>>, HttpError> {
    let trace_id = new_trace_id();
    let filters = OrderListFilters::new(
        query.signal_id,
        query.market_id,
        query.connector_name,
        query.status,
        query.limit,
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let orders = state
        .execution_service
        .list_orders(filters)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        orders.into_iter().map(order_to_contract).collect(),
        auth.request_id,
        trace_id,
    )))
}

async fn list_trades(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<TradeListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<TradeData>>>, HttpError> {
    let trace_id = new_trace_id();
    let filters = TradeListFilters::new(
        query.order_id,
        query.signal_id,
        query.market_id,
        query.connector_name,
        query.limit,
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let trades = state
        .execution_service
        .list_trades(filters)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        trades.into_iter().map(trade_to_contract).collect(),
        auth.request_id,
        trace_id,
    )))
}

async fn list_positions(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<PositionListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<PositionData>>>, HttpError> {
    let trace_id = new_trace_id();
    let filters = PositionListFilters::new(
        query.market_id,
        query.connector_name,
        query.side,
        query.limit,
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let positions = state
        .execution_service
        .list_positions(filters)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        positions.into_iter().map(position_to_contract).collect(),
        auth.request_id,
        trace_id,
    )))
}
