async fn list_order_drafts(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<OrderDraftListQuery>,
) -> std::result::Result<Json<ApiResponse<Paginated<OrderDraftData>>>, HttpError> {
    let trace_id = new_trace_id();
    let page = PageQuery {
        page: query.page.unwrap_or(1),
        page_size: query.page_size.unwrap_or(20),
        sort_order: None,
    };
    let filters = OrderDraftListFilters::new(query.signal_id, query.connector_name, query.status, None)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let result = state
        .execution_service
        .list_order_drafts_paginated(&filters, &page)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        result.map(order_draft_to_contract),
        auth.request_id,
        trace_id,
    )))
}

async fn list_execution_requests(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<ExecutionRequestListQuery>,
) -> std::result::Result<Json<ApiResponse<Paginated<ExecutionRequestData>>>, HttpError> {
    let trace_id = new_trace_id();
    let page = PageQuery {
        page: query.page.unwrap_or(1),
        page_size: query.page_size.unwrap_or(20),
        sort_order: None,
    };
    let filters = ExecutionRequestListFilters::new(
        query.signal_id,
        query.connector_name,
        query.status,
        None,
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let result = state
        .execution_service
        .list_execution_requests_paginated(&filters, &page)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        result.map(execution_request_to_contract),
        auth.request_id,
        trace_id,
    )))
}

async fn list_orders(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<OrderListQuery>,
) -> std::result::Result<Json<ApiResponse<Paginated<OrderData>>>, HttpError> {
    let trace_id = new_trace_id();
    let page = PageQuery {
        page: query.page.unwrap_or(1),
        page_size: query.page_size.unwrap_or(20),
        sort_order: None,
    };
    let filters = OrderListFilters::new(
        query.signal_id,
        query.market_id,
        query.connector_name,
        query.status,
        None,
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let result = state
        .execution_service
        .list_orders_paginated(&filters, &page)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        result.map(order_to_contract),
        auth.request_id,
        trace_id,
    )))
}

async fn list_trades(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<TradeListQuery>,
) -> std::result::Result<Json<ApiResponse<Paginated<TradeData>>>, HttpError> {
    let trace_id = new_trace_id();
    let page = PageQuery {
        page: query.page.unwrap_or(1),
        page_size: query.page_size.unwrap_or(20),
        sort_order: None,
    };
    let filters = TradeListFilters::new(
        query.order_id,
        query.signal_id,
        query.market_id,
        query.connector_name,
        None,
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let result = state
        .execution_service
        .list_trades_paginated(&filters, &page)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        result.map(trade_to_contract),
        auth.request_id,
        trace_id,
    )))
}
