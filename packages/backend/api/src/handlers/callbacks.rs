#[derive(Debug, Clone)]
struct ConnectorOrderStatusInput {
    event_id: String,
    connector_name: String,
    external_order_id: String,
    status: OrderStatus,
}

#[derive(Debug, Clone)]
struct ConnectorTradeFillInput {
    event_id: String,
    connector_name: String,
    external_order_id: String,
    account_id: String,
    external_trade_id: String,
    fill_price: Probability,
    filled_quantity: Quantity,
    fee: UsdAmount,
}

impl From<ConnectorOrderStatusUpdate> for ConnectorOrderStatusInput {
    fn from(value: ConnectorOrderStatusUpdate) -> Self {
        Self {
            event_id: value.event_id,
            connector_name: value.connector_name,
            external_order_id: value.external_order_id,
            status: value.status,
        }
    }
}

impl From<ConnectorTradeFillUpdate> for ConnectorTradeFillInput {
    fn from(value: ConnectorTradeFillUpdate) -> Self {
        Self {
            event_id: value.event_id,
            connector_name: value.connector_name,
            external_order_id: value.external_order_id,
            account_id: value.account_id,
            external_trade_id: value.external_trade_id,
            fill_price: value.fill_price,
            filled_quantity: value.filled_quantity,
            fee: value.fee,
        }
    }
}

async fn connector_order_status_callback(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Json(payload): Json<ConnectorOrderStatusCallbackRequest>,
) -> std::result::Result<Json<ApiResponse<ConnectorOrderStatusCallbackData>>, HttpError> {
    let trace_id = new_trace_id();
    let connector_name = normalize_callback_connector_name(&payload.connector_name)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let callback = ConnectorOrderStatusInput {
        event_id: validate_callback_event_id(&payload.event_id).map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
        })?,
        connector_name,
        external_order_id: payload.external_order_id.clone(),
        status: payload.status,
    };
    let response_data =
        process_connector_order_status_callback(&state, &auth, callback, &payload, &trace_id)
            .await?;

    Ok(Json(ApiResponse::new(
        response_data,
        auth.request_id,
        trace_id,
    )))
}

async fn connector_trade_fill_callback(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Json(payload): Json<ConnectorTradeFillCallbackRequest>,
) -> std::result::Result<Json<ApiResponse<ConnectorTradeFillCallbackData>>, HttpError> {
    let trace_id = new_trace_id();
    let connector_name = normalize_callback_connector_name(&payload.connector_name)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let callback = ConnectorTradeFillInput {
        event_id: validate_callback_event_id(&payload.event_id).map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
        })?,
        connector_name,
        external_order_id: payload.external_order_id.clone(),
        account_id: payload.account_id.clone(),
        external_trade_id: payload.external_trade_id.clone(),
        fill_price: payload.fill_price,
        filled_quantity: payload.filled_quantity,
        fee: payload.fee,
    };
    let response_data =
        process_connector_trade_fill_callback(&state, &auth, callback, &payload, &trace_id).await?;

    Ok(Json(ApiResponse::new(
        response_data,
        auth.request_id,
        trace_id,
    )))
}

async fn polymarket_order_status_callback(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Json(payload): Json<PolymarketOrderStatusCallbackRequest>,
) -> std::result::Result<Json<ApiResponse<ConnectorOrderStatusCallbackData>>, HttpError> {
    let trace_id = new_trace_id();
    let normalized = normalize_polymarket_order_status_update(
        &payload.event_id,
        &payload.order_id,
        payload.status.as_str(),
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let response_data =
        process_connector_order_status_callback(&state, &auth, normalized, &payload, &trace_id)
            .await?;

    Ok(Json(ApiResponse::new(
        response_data,
        auth.request_id,
        trace_id,
    )))
}

async fn polymarket_trade_fill_callback(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Json(payload): Json<PolymarketTradeFillCallbackRequest>,
) -> std::result::Result<Json<ApiResponse<ConnectorTradeFillCallbackData>>, HttpError> {
    let trace_id = new_trace_id();
    let normalized = normalize_polymarket_trade_fill_update(
        &payload.event_id,
        &payload.order_id,
        &payload.account_id,
        &payload.trade_id,
        payload.price,
        payload.size,
        payload.fee,
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let response_data =
        process_connector_trade_fill_callback(&state, &auth, normalized, &payload, &trace_id)
            .await?;

    Ok(Json(ApiResponse::new(
        response_data,
        auth.request_id,
        trace_id,
    )))
}

async fn process_connector_order_status_callback<T: serde::Serialize>(
    state: &AppState,
    auth: &AuthContext,
    callback: impl Into<ConnectorOrderStatusInput>,
    payload: &T,
    trace_id: &str,
) -> std::result::Result<ConnectorOrderStatusCallbackData, HttpError> {
    let callback = callback.into();
    let source_system = callback_source(CONNECTOR_ORDER_STATUS_SOURCE, &callback.connector_name);
    let request_hash = hash_json(payload).map_err(|error| {
        HttpError::with_meta(error, auth.request_id.clone(), trace_id.to_string())
    })?;

    match state
        .external_event_store
        .begin(&source_system, &callback.event_id, &request_hash, trace_id)
        .await
        .map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.to_string())
        })? {
        ExternalEventBegin::Replay => {
            let order = state
                .market_event_service
                .get_order_by_external_ref(
                    callback.connector_name.clone(),
                    callback.external_order_id.clone(),
                )
                .await
                .map_err(|error| {
                    HttpError::with_meta(error, auth.request_id.clone(), trace_id.to_string())
                })?;

            return Ok(connector_order_status_to_contract(order, true));
        }
        ExternalEventBegin::New => {}
    }

    let actor = authenticated_actor(auth);
    let order = match state
        .execution_service
        .sync_external_order_status(SyncExternalOrderStatusCommand {
            connector_name: callback.connector_name.clone(),
            external_order_id: callback.external_order_id.clone(),
            status: callback.status,
            request_id: auth.request_id.clone(),
            trace_id: trace_id.to_string(),
            actor,
        })
        .await
    {
        Ok(order) => order,
        Err(error) => {
            state
                .external_event_store
                .abandon(&source_system, &callback.event_id, trace_id)
                .await
                .map_err(|abandon_error| {
                    HttpError::with_meta(
                        abandon_error,
                        auth.request_id.clone(),
                        trace_id.to_string(),
                    )
                })?;
            return Err(HttpError::with_meta(
                error,
                auth.request_id.clone(),
                trace_id.to_string(),
            ));
        }
    };

    state
        .external_event_store
        .mark_processed(&source_system, &callback.event_id, trace_id)
        .await
        .map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.to_string())
        })?;

    Ok(connector_order_status_to_contract(order, false))
}

async fn process_connector_trade_fill_callback<T: serde::Serialize>(
    state: &AppState,
    auth: &AuthContext,
    callback: impl Into<ConnectorTradeFillInput>,
    payload: &T,
    trace_id: &str,
) -> std::result::Result<ConnectorTradeFillCallbackData, HttpError> {
    let callback = callback.into();
    let source_system = callback_source(CONNECTOR_TRADE_FILL_SOURCE, &callback.connector_name);
    let request_hash = hash_json(payload).map_err(|error| {
        HttpError::with_meta(error, auth.request_id.clone(), trace_id.to_string())
    })?;

    match state
        .external_event_store
        .begin(&source_system, &callback.event_id, &request_hash, trace_id)
        .await
        .map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.to_string())
        })? {
        ExternalEventBegin::Replay => {
            return build_trade_fill_callback_response(
                state,
                &callback.connector_name,
                &callback.external_order_id,
                &callback.account_id,
                &callback.external_trade_id,
                true,
            )
            .await
            .map_err(|error| {
                HttpError::with_meta(error, auth.request_id.clone(), trace_id.to_string())
            });
        }
        ExternalEventBegin::New => {}
    }

    let actor = authenticated_actor(auth);
    let fill_result = match state
        .execution_service
        .reconcile_external_trade(ReconcileExternalTradeCommand {
            connector_name: callback.connector_name.clone(),
            external_order_id: callback.external_order_id.clone(),
            account_id: callback.account_id.clone(),
            external_trade_id: callback.external_trade_id.clone(),
            fill_price: callback.fill_price,
            filled_quantity: callback.filled_quantity,
            fee: callback.fee,
            request_id: auth.request_id.clone(),
            trace_id: trace_id.to_string(),
            actor,
        })
        .await
    {
        Ok(fill_result) => fill_result,
        Err(error) => {
            state
                .external_event_store
                .abandon(&source_system, &callback.event_id, trace_id)
                .await
                .map_err(|abandon_error| {
                    HttpError::with_meta(
                        abandon_error,
                        auth.request_id.clone(),
                        trace_id.to_string(),
                    )
                })?;
            return Err(HttpError::with_meta(
                error,
                auth.request_id.clone(),
                trace_id.to_string(),
            ));
        }
    };

    state
        .external_event_store
        .mark_processed(&source_system, &callback.event_id, trace_id)
        .await
        .map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.to_string())
        })?;

    let risk_state = state.risk_service.read_state().await.map_err(|error| {
        HttpError::with_meta(error, auth.request_id.clone(), trace_id.to_string())
    })?;

    connector_trade_fill_to_contract(fill_result, risk_state, false, state)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.to_string()))
}
