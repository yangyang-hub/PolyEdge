use axum::{
    Router,
    extract::{Extension, Json, Path, Query, State},
    http::{HeaderMap, StatusCode},
    middleware,
    routing::get,
};
use polyedge_application::{
    ApproveSignalCommand, ApproveSignalReceipt, AuthenticatedActor, EventListFilters, EventView,
    EvidenceListFilters, EvidenceView, ExecutionFillResult, ExecutionRequestListFilters,
    ExecutionRequestView, ExecutionSubmissionReceipt, IdempotencyBegin, IdempotencyRequest,
    KillSwitchReceipt, MarketListFilters, MarketView, ModeTransitionCommand, OrderDraftListFilters,
    OrderDraftView, OrderListFilters, OrderView, PositionListFilters, PositionView,
    ProbabilityEstimateListFilters, ProbabilityEstimateView, ReconcileExternalTradeCommand,
    RejectSignalCommand, RejectSignalReceipt, ReleaseKillSwitchCommand, RiskStateView,
    SignalListFilters, SignalTransitionListFilters, SignalTransitionView, SignalView,
    SubmitExecutionCommand, SyncExternalOrderStatusCommand, TradeListFilters, TradeView,
    TriggerKillSwitchCommand,
};
use polyedge_connectors::{
    ConnectorOrderStatusUpdate, ConnectorTradeFillUpdate, normalize_polymarket_order_status_update,
    normalize_polymarket_trade_fill_update,
};
use polyedge_contracts::{
    ApiResponse, ApproveSignalData, ApproveSignalRequest, ConnectorOrderStatusCallbackData,
    ConnectorOrderStatusCallbackRequest, ConnectorTradeFillCallbackData,
    ConnectorTradeFillCallbackRequest, DependencyStatus, EventData, EventListQuery, EvidenceData,
    EvidenceListQuery, ExecutionRequestData, ExecutionRequestListQuery, HealthData, KillSwitchData,
    MarketData, MarketListQuery, OrderData, OrderDraftData, OrderDraftListQuery, OrderListQuery,
    PolymarketOrderStatusCallbackRequest, PolymarketTradeFillCallbackRequest, PositionData,
    PositionListQuery, ProbabilityEstimateData, ProbabilityEstimateListQuery, ReadinessData,
    RecomputeSignalData, RecomputeSignalRequest, RejectSignalData, RejectSignalRequest,
    ReleaseKillSwitchRequest, RiskStateData, SignalData, SignalListQuery, SignalTransitionData,
    SignalTransitionListQuery, SubmitExecutionData, SubmitExecutionRequest, SystemModeData,
    TradeData, TradeListQuery, TransitionSystemModeRequest, TriggerKillSwitchRequest,
};
use polyedge_domain::{AppError, OrderStatus, Probability, Quantity, StepUpScope, UsdAmount};
use polyedge_infrastructure::stores::ExternalEventBegin;
use polyedge_infrastructure::{
    AppState, AuthContext, HttpError, IdempotencyKey, hash_json, new_trace_id,
    request_id_from_headers, require_connector_write_auth, require_console_read_auth,
    require_console_write_auth, require_mode_write_auth,
};
use tower::ServiceBuilder;
use tower_http::{timeout::TimeoutLayer, trace::TraceLayer};

const CONNECTOR_ORDER_STATUS_SOURCE: &str = "connector.orders.status";
const CONNECTOR_TRADE_FILL_SOURCE: &str = "connector.trades.fill";

pub fn build_app(state: AppState) -> Router {
    let system_routes =
        Router::new()
            .route(
                "/mode",
                get(read_system_mode).route_layer(middleware::from_fn_with_state(
                    state.clone(),
                    require_console_read_auth,
                )),
            )
            .route(
                "/mode",
                axum::routing::post(transition_system_mode).route_layer(
                    middleware::from_fn_with_state(state.clone(), require_mode_write_auth),
                ),
            )
            .route(
                "/kill-switch/trigger",
                axum::routing::post(trigger_kill_switch).route_layer(
                    middleware::from_fn_with_state(state.clone(), require_console_write_auth),
                ),
            )
            .route(
                "/kill-switch/release",
                axum::routing::post(release_kill_switch).route_layer(
                    middleware::from_fn_with_state(state.clone(), require_console_write_auth),
                ),
            )
            .with_state(state.clone());

    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route(
            "/api/v1/markets",
            get(list_markets).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/markets/{market_id}",
            get(get_market).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/events",
            get(list_events).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/evidences",
            get(list_evidences).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/signals",
            get(list_signals).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/signals/{signal_id}/transitions",
            get(list_signal_transitions).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/signals/{signal_id}/recompute",
            axum::routing::post(recompute_signal).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_write_auth,
            )),
        )
        .route(
            "/api/v1/signals/{signal_id}/approve",
            axum::routing::post(approve_signal).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_write_auth,
            )),
        )
        .route(
            "/api/v1/signals/{signal_id}/reject",
            axum::routing::post(reject_signal).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_write_auth,
            )),
        )
        .route(
            "/api/v1/signals/{signal_id}/execution-requests",
            axum::routing::post(submit_execution_request).route_layer(
                middleware::from_fn_with_state(state.clone(), require_console_write_auth),
            ),
        )
        .route(
            "/api/v1/orders/drafts",
            get(list_order_drafts).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/orders",
            get(list_orders).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/trades",
            get(list_trades).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/execution/requests",
            get(list_execution_requests).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/positions",
            get(list_positions).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/connectors/callbacks/orders/status",
            axum::routing::post(connector_order_status_callback).route_layer(
                middleware::from_fn_with_state(state.clone(), require_connector_write_auth),
            ),
        )
        .route(
            "/api/v1/connectors/callbacks/trades/fill",
            axum::routing::post(connector_trade_fill_callback).route_layer(
                middleware::from_fn_with_state(state.clone(), require_connector_write_auth),
            ),
        )
        .route(
            "/api/v1/connectors/polymarket/callbacks/orders/status",
            axum::routing::post(polymarket_order_status_callback).route_layer(
                middleware::from_fn_with_state(state.clone(), require_connector_write_auth),
            ),
        )
        .route(
            "/api/v1/connectors/polymarket/callbacks/trades/fill",
            axum::routing::post(polymarket_trade_fill_callback).route_layer(
                middleware::from_fn_with_state(state.clone(), require_connector_write_auth),
            ),
        )
        .route(
            "/api/v1/pricing/estimates",
            get(list_probability_estimates).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/risk/state",
            get(read_risk_state).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .nest("/api/v1/system", system_routes)
        .with_state(state)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(TimeoutLayer::with_status_code(
                    StatusCode::REQUEST_TIMEOUT,
                    std::time::Duration::from_secs(10),
                )),
        )
}

async fn healthz(headers: HeaderMap) -> Json<ApiResponse<HealthData>> {
    let request_id = request_id_from_headers(&headers);
    Json(ApiResponse::new(
        HealthData {
            status: "ok".to_string(),
        },
        request_id,
        new_trace_id(),
    ))
}

async fn readyz(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> (StatusCode, Json<ApiResponse<ReadinessData>>) {
    let request_id = request_id_from_headers(&headers);
    let trace_id = new_trace_id();
    let postgres_status = match state.dependencies.postgres_ready().await {
        Ok(()) => DependencyStatus {
            status: "ready".to_string(),
            detail: None,
        },
        Err(error) => DependencyStatus {
            status: "not_ready".to_string(),
            detail: Some(error.message().to_string()),
        },
    };
    let redis_status = match state.dependencies.redis_ready().await {
        Ok(()) => DependencyStatus {
            status: "ready".to_string(),
            detail: None,
        },
        Err(error) => DependencyStatus {
            status: "not_ready".to_string(),
            detail: Some(error.message().to_string()),
        },
    };

    let status = if postgres_status.status == "ready" && redis_status.status == "ready" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (
        status,
        Json(ApiResponse::new(
            ReadinessData {
                status: if status == StatusCode::OK {
                    "ready".to_string()
                } else {
                    "degraded".to_string()
                },
                postgres: postgres_status,
                redis: redis_status,
            },
            request_id,
            trace_id,
        )),
    )
}

async fn read_system_mode(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> std::result::Result<Json<ApiResponse<SystemModeData>>, HttpError> {
    let trace_id = new_trace_id();
    let snapshot = state
        .system_mode_service
        .read_mode()
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        SystemModeData {
            mode: snapshot.mode,
            environment: snapshot.environment,
            version: snapshot.version,
            replayed: false,
            updated_at: snapshot.updated_at,
        },
        auth.request_id,
        trace_id,
    )))
}

async fn list_markets(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<MarketListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<MarketData>>>, HttpError> {
    let trace_id = new_trace_id();
    let filters = MarketListFilters::new(query.status, query.tradability_status, query.limit)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let markets = state
        .market_event_service
        .list_markets(filters)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        markets.into_iter().map(market_to_contract).collect(),
        auth.request_id,
        trace_id,
    )))
}

async fn get_market(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Path(market_id): Path<String>,
) -> std::result::Result<Json<ApiResponse<MarketData>>, HttpError> {
    let trace_id = new_trace_id();
    let market = state
        .market_event_service
        .get_market(&market_id)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        market_to_contract(market),
        auth.request_id,
        trace_id,
    )))
}

async fn list_events(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<EventListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<EventData>>>, HttpError> {
    let trace_id = new_trace_id();
    let filters = EventListFilters::new(query.status, query.limit)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let events = state
        .market_event_service
        .list_events(filters)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        events.into_iter().map(event_to_contract).collect(),
        auth.request_id,
        trace_id,
    )))
}

async fn list_evidences(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<EvidenceListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<EvidenceData>>>, HttpError> {
    let trace_id = new_trace_id();
    let filters =
        EvidenceListFilters::new(query.market_id, query.event_id, query.status, query.limit)
            .map_err(|error| {
                HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
            })?;
    let evidences = state
        .market_event_service
        .list_evidences(filters)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        evidences.into_iter().map(evidence_to_contract).collect(),
        auth.request_id,
        trace_id,
    )))
}

async fn list_signals(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<SignalListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<SignalData>>>, HttpError> {
    let trace_id = new_trace_id();
    let filters = SignalListFilters::new(
        query.market_id,
        query.event_id,
        query.lifecycle_state,
        query.limit,
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let signals = state
        .market_event_service
        .list_signals(filters)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        signals.into_iter().map(signal_to_contract).collect(),
        auth.request_id,
        trace_id,
    )))
}

async fn list_probability_estimates(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<ProbabilityEstimateListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<ProbabilityEstimateData>>>, HttpError> {
    let trace_id = new_trace_id();
    let filters = ProbabilityEstimateListFilters::new(
        query.market_id,
        query.event_id,
        query.signal_id,
        query.limit,
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let estimates = state
        .market_event_service
        .list_probability_estimates(filters)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        estimates
            .into_iter()
            .map(probability_estimate_to_contract)
            .collect(),
        auth.request_id,
        trace_id,
    )))
}

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
                .abandon(&source_system, &callback.event_id)
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
                .abandon(&source_system, &callback.event_id)
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

    Ok(connector_trade_fill_to_contract(
        fill_result,
        risk_state,
        false,
    ))
}

async fn recompute_signal(
    Extension(auth): Extension<AuthContext>,
    Extension(idempotency_key): Extension<IdempotencyKey>,
    State(state): State<AppState>,
    Path(signal_id): Path<String>,
    Json(payload): Json<RecomputeSignalRequest>,
) -> std::result::Result<Json<ApiResponse<RecomputeSignalData>>, HttpError> {
    let trace_id = new_trace_id();
    let request_hash = hash_json(&payload)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let idempotency_request = IdempotencyRequest {
        scope: "signals.recompute".to_string(),
        idempotency_key: idempotency_key.0,
        request_hash,
        request_id: auth.request_id.clone(),
        actor_user_id: Some(auth.user_id.clone()),
        actor_session_id: Some(auth.session_id.clone()),
        resource_type: Some("signal".to_string()),
        resource_id: Some(signal_id.clone()),
    };

    match state
        .idempotency_store
        .begin(&idempotency_request)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?
    {
        IdempotencyBegin::Replay(response_json) => {
            let mut replayed: RecomputeSignalData =
                serde_json::from_str(&response_json).map_err(|error| {
                    HttpError::with_meta(
                        AppError::internal(
                            "SIGNAL_RECOMPUTE_REPLAY_DESERIALIZE_FAILED",
                            format!("failed to deserialize replayed recompute response: {error}"),
                        ),
                        auth.request_id.clone(),
                        trace_id.clone(),
                    )
                })?;
            replayed.replayed = true;

            return Ok(Json(ApiResponse::new(replayed, auth.request_id, trace_id)));
        }
        IdempotencyBegin::Started => {}
    }

    let result = match state
        .market_event_service
        .recompute_signal(signal_id, payload.reason, trace_id.clone())
        .await
    {
        Ok(result) => result,
        Err(error) => {
            state
                .idempotency_store
                .fail(&idempotency_request, error.code())
                .await
                .map_err(|fail_error| {
                    HttpError::with_meta(fail_error, auth.request_id.clone(), trace_id.clone())
                })?;
            return Err(HttpError::with_meta(
                error,
                auth.request_id.clone(),
                trace_id,
            ));
        }
    };

    let response_data = recompute_signal_to_contract(result, false);
    let response_json = serde_json::to_string(&response_data).map_err(|error| {
        HttpError::with_meta(
            AppError::internal(
                "SIGNAL_RECOMPUTE_RESPONSE_SERIALIZE_FAILED",
                format!("failed to serialize signal recompute response: {error}"),
            ),
            auth.request_id.clone(),
            trace_id.clone(),
        )
    })?;

    state
        .idempotency_store
        .complete(&idempotency_request, &response_json)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        response_data,
        auth.request_id,
        trace_id,
    )))
}

async fn read_risk_state(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> std::result::Result<Json<ApiResponse<RiskStateData>>, HttpError> {
    let trace_id = new_trace_id();
    let risk_state =
        state.risk_service.read_state().await.map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
        })?;

    Ok(Json(ApiResponse::new(
        risk_state_to_contract(risk_state),
        auth.request_id,
        trace_id,
    )))
}

async fn approve_signal(
    Extension(auth): Extension<AuthContext>,
    Extension(idempotency_key): Extension<IdempotencyKey>,
    State(state): State<AppState>,
    Path(signal_id): Path<String>,
    Json(payload): Json<ApproveSignalRequest>,
) -> std::result::Result<Json<ApiResponse<ApproveSignalData>>, HttpError> {
    auth.ensure_scope(StepUpScope::SignalApprove, time::OffsetDateTime::now_utc())
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), new_trace_id()))?;

    let trace_id = new_trace_id();
    let request_hash = hash_json(&payload)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let idempotency_request = IdempotencyRequest {
        scope: "signals.approve".to_string(),
        idempotency_key: idempotency_key.0,
        request_hash,
        request_id: auth.request_id.clone(),
        actor_user_id: Some(auth.user_id.clone()),
        actor_session_id: Some(auth.session_id.clone()),
        resource_type: Some("signal".to_string()),
        resource_id: Some(signal_id.clone()),
    };

    match state
        .idempotency_store
        .begin(&idempotency_request)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?
    {
        IdempotencyBegin::Replay(response_json) => {
            let mut replayed: ApproveSignalData =
                serde_json::from_str(&response_json).map_err(|error| {
                    HttpError::with_meta(
                        AppError::internal(
                            "SIGNAL_APPROVAL_REPLAY_DESERIALIZE_FAILED",
                            format!("failed to deserialize replayed approval response: {error}"),
                        ),
                        auth.request_id.clone(),
                        trace_id.clone(),
                    )
                })?;
            replayed.replayed = true;

            return Ok(Json(ApiResponse::new(replayed, auth.request_id, trace_id)));
        }
        IdempotencyBegin::Started => {}
    }

    let actor = AuthenticatedActor {
        user_id: auth.user_id.clone(),
        session_id: auth.session_id.clone(),
        roles: auth.roles.clone(),
        request_id: auth.request_id.clone(),
        ip: auth.ip.clone(),
        user_agent: auth.user_agent.clone(),
    };

    let receipt = match state
        .risk_service
        .approve_signal(ApproveSignalCommand {
            signal_id,
            reason: payload.reason,
            expected_version: payload.expected_version,
            request_id: auth.request_id.clone(),
            trace_id: trace_id.clone(),
            actor,
        })
        .await
    {
        Ok(receipt) => receipt,
        Err(error) => {
            state
                .idempotency_store
                .fail(&idempotency_request, error.code())
                .await
                .map_err(|fail_error| {
                    HttpError::with_meta(fail_error, auth.request_id.clone(), trace_id.clone())
                })?;
            return Err(HttpError::with_meta(
                error,
                auth.request_id.clone(),
                trace_id,
            ));
        }
    };

    let response_data = approve_signal_to_contract(receipt, false);
    let response_json = serde_json::to_string(&response_data).map_err(|error| {
        HttpError::with_meta(
            AppError::internal(
                "SIGNAL_APPROVAL_RESPONSE_SERIALIZE_FAILED",
                format!("failed to serialize signal approval response: {error}"),
            ),
            auth.request_id.clone(),
            trace_id.clone(),
        )
    })?;

    state
        .idempotency_store
        .complete(&idempotency_request, &response_json)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        response_data,
        auth.request_id,
        trace_id,
    )))
}

async fn reject_signal(
    Extension(auth): Extension<AuthContext>,
    Extension(idempotency_key): Extension<IdempotencyKey>,
    State(state): State<AppState>,
    Path(signal_id): Path<String>,
    Json(payload): Json<RejectSignalRequest>,
) -> std::result::Result<Json<ApiResponse<RejectSignalData>>, HttpError> {
    auth.ensure_scope(StepUpScope::SignalReject, time::OffsetDateTime::now_utc())
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), new_trace_id()))?;

    let trace_id = new_trace_id();
    let request_hash = hash_json(&payload)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let idempotency_request = IdempotencyRequest {
        scope: "signals.reject".to_string(),
        idempotency_key: idempotency_key.0,
        request_hash,
        request_id: auth.request_id.clone(),
        actor_user_id: Some(auth.user_id.clone()),
        actor_session_id: Some(auth.session_id.clone()),
        resource_type: Some("signal".to_string()),
        resource_id: Some(signal_id.clone()),
    };

    match state
        .idempotency_store
        .begin(&idempotency_request)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?
    {
        IdempotencyBegin::Replay(response_json) => {
            let mut replayed: RejectSignalData =
                serde_json::from_str(&response_json).map_err(|error| {
                    HttpError::with_meta(
                        AppError::internal(
                            "SIGNAL_REJECTION_REPLAY_DESERIALIZE_FAILED",
                            format!("failed to deserialize replayed rejection response: {error}"),
                        ),
                        auth.request_id.clone(),
                        trace_id.clone(),
                    )
                })?;
            replayed.replayed = true;

            return Ok(Json(ApiResponse::new(replayed, auth.request_id, trace_id)));
        }
        IdempotencyBegin::Started => {}
    }

    let actor = AuthenticatedActor {
        user_id: auth.user_id.clone(),
        session_id: auth.session_id.clone(),
        roles: auth.roles.clone(),
        request_id: auth.request_id.clone(),
        ip: auth.ip.clone(),
        user_agent: auth.user_agent.clone(),
    };

    let receipt = match state
        .risk_service
        .reject_signal(RejectSignalCommand {
            signal_id,
            reason: payload.reason,
            expected_version: payload.expected_version,
            request_id: auth.request_id.clone(),
            trace_id: trace_id.clone(),
            actor,
        })
        .await
    {
        Ok(receipt) => receipt,
        Err(error) => {
            state
                .idempotency_store
                .fail(&idempotency_request, error.code())
                .await
                .map_err(|fail_error| {
                    HttpError::with_meta(fail_error, auth.request_id.clone(), trace_id.clone())
                })?;
            return Err(HttpError::with_meta(
                error,
                auth.request_id.clone(),
                trace_id,
            ));
        }
    };

    let response_data = reject_signal_to_contract(receipt, false);
    let response_json = serde_json::to_string(&response_data).map_err(|error| {
        HttpError::with_meta(
            AppError::internal(
                "SIGNAL_REJECTION_RESPONSE_SERIALIZE_FAILED",
                format!("failed to serialize signal rejection response: {error}"),
            ),
            auth.request_id.clone(),
            trace_id.clone(),
        )
    })?;

    state
        .idempotency_store
        .complete(&idempotency_request, &response_json)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        response_data,
        auth.request_id,
        trace_id,
    )))
}

async fn submit_execution_request(
    Extension(auth): Extension<AuthContext>,
    Extension(idempotency_key): Extension<IdempotencyKey>,
    State(state): State<AppState>,
    Path(signal_id): Path<String>,
    Json(payload): Json<SubmitExecutionRequest>,
) -> std::result::Result<Json<ApiResponse<SubmitExecutionData>>, HttpError> {
    auth.ensure_scope(
        StepUpScope::ExecutionSubmit,
        time::OffsetDateTime::now_utc(),
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), new_trace_id()))?;

    let trace_id = new_trace_id();
    let request_hash = hash_json(&payload)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let idempotency_request = IdempotencyRequest {
        scope: "execution.requests.submit".to_string(),
        idempotency_key: idempotency_key.0,
        request_hash,
        request_id: auth.request_id.clone(),
        actor_user_id: Some(auth.user_id.clone()),
        actor_session_id: Some(auth.session_id.clone()),
        resource_type: Some("signal".to_string()),
        resource_id: Some(signal_id.clone()),
    };

    match state
        .idempotency_store
        .begin(&idempotency_request)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?
    {
        IdempotencyBegin::Replay(response_json) => {
            let mut replayed: SubmitExecutionData =
                serde_json::from_str(&response_json).map_err(|error| {
                    HttpError::with_meta(
                        AppError::internal(
                            "EXECUTION_REQUEST_REPLAY_DESERIALIZE_FAILED",
                            format!(
                                "failed to deserialize replayed execution submission response: {error}"
                            ),
                        ),
                        auth.request_id.clone(),
                        trace_id.clone(),
                    )
                })?;
            replayed.replayed = true;

            return Ok(Json(ApiResponse::new(replayed, auth.request_id, trace_id)));
        }
        IdempotencyBegin::Started => {}
    }

    let actor = AuthenticatedActor {
        user_id: auth.user_id.clone(),
        session_id: auth.session_id.clone(),
        roles: auth.roles.clone(),
        request_id: auth.request_id.clone(),
        ip: auth.ip.clone(),
        user_agent: auth.user_agent.clone(),
    };

    let receipt = match state
        .execution_service
        .submit_execution_request(SubmitExecutionCommand {
            signal_id,
            expected_signal_version: payload.expected_signal_version,
            limit_price: payload.limit_price,
            quantity: payload.quantity,
            connector_name: payload.connector_name,
            reason: payload.reason,
            request_id: auth.request_id.clone(),
            trace_id: trace_id.clone(),
            actor,
        })
        .await
    {
        Ok(receipt) => receipt,
        Err(error) => {
            state
                .idempotency_store
                .fail(&idempotency_request, error.code())
                .await
                .map_err(|fail_error| {
                    HttpError::with_meta(fail_error, auth.request_id.clone(), trace_id.clone())
                })?;
            return Err(HttpError::with_meta(
                error,
                auth.request_id.clone(),
                trace_id,
            ));
        }
    };

    let response_data = execution_submission_to_contract(receipt, false);
    let response_json = serde_json::to_string(&response_data).map_err(|error| {
        HttpError::with_meta(
            AppError::internal(
                "EXECUTION_REQUEST_RESPONSE_SERIALIZE_FAILED",
                format!("failed to serialize execution submission response: {error}"),
            ),
            auth.request_id.clone(),
            trace_id.clone(),
        )
    })?;

    state
        .idempotency_store
        .complete(&idempotency_request, &response_json)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        response_data,
        auth.request_id,
        trace_id,
    )))
}

async fn trigger_kill_switch(
    Extension(auth): Extension<AuthContext>,
    Extension(idempotency_key): Extension<IdempotencyKey>,
    State(state): State<AppState>,
    Json(payload): Json<TriggerKillSwitchRequest>,
) -> std::result::Result<Json<ApiResponse<KillSwitchData>>, HttpError> {
    auth.ensure_scope(
        StepUpScope::SystemKillSwitchTrigger,
        time::OffsetDateTime::now_utc(),
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), new_trace_id()))?;

    let trace_id = new_trace_id();
    let request_hash = hash_json(&payload)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let idempotency_request = IdempotencyRequest {
        scope: "system.kill_switch.trigger".to_string(),
        idempotency_key: idempotency_key.0,
        request_hash,
        request_id: auth.request_id.clone(),
        actor_user_id: Some(auth.user_id.clone()),
        actor_session_id: Some(auth.session_id.clone()),
        resource_type: Some("risk_state".to_string()),
        resource_id: Some("global".to_string()),
    };

    match state
        .idempotency_store
        .begin(&idempotency_request)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?
    {
        IdempotencyBegin::Replay(response_json) => {
            let mut replayed: KillSwitchData =
                serde_json::from_str(&response_json).map_err(|error| {
                    HttpError::with_meta(
                        AppError::internal(
                            "KILL_SWITCH_REPLAY_DESERIALIZE_FAILED",
                            format!("failed to deserialize replayed kill switch response: {error}"),
                        ),
                        auth.request_id.clone(),
                        trace_id.clone(),
                    )
                })?;
            replayed.replayed = true;

            return Ok(Json(ApiResponse::new(replayed, auth.request_id, trace_id)));
        }
        IdempotencyBegin::Started => {}
    }

    let actor = AuthenticatedActor {
        user_id: auth.user_id.clone(),
        session_id: auth.session_id.clone(),
        roles: auth.roles.clone(),
        request_id: auth.request_id.clone(),
        ip: auth.ip.clone(),
        user_agent: auth.user_agent.clone(),
    };

    let receipt = match state
        .risk_service
        .trigger_kill_switch(TriggerKillSwitchCommand {
            reason: payload.reason,
            expected_version: payload.expected_version,
            request_id: auth.request_id.clone(),
            trace_id: trace_id.clone(),
            actor,
        })
        .await
    {
        Ok(receipt) => receipt,
        Err(error) => {
            state
                .idempotency_store
                .fail(&idempotency_request, error.code())
                .await
                .map_err(|fail_error| {
                    HttpError::with_meta(fail_error, auth.request_id.clone(), trace_id.clone())
                })?;
            return Err(HttpError::with_meta(
                error,
                auth.request_id.clone(),
                trace_id,
            ));
        }
    };

    let response_data = kill_switch_to_contract(receipt, false);
    let response_json = serde_json::to_string(&response_data).map_err(|error| {
        HttpError::with_meta(
            AppError::internal(
                "KILL_SWITCH_RESPONSE_SERIALIZE_FAILED",
                format!("failed to serialize kill switch response: {error}"),
            ),
            auth.request_id.clone(),
            trace_id.clone(),
        )
    })?;

    state
        .idempotency_store
        .complete(&idempotency_request, &response_json)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        response_data,
        auth.request_id,
        trace_id,
    )))
}

async fn release_kill_switch(
    Extension(auth): Extension<AuthContext>,
    Extension(idempotency_key): Extension<IdempotencyKey>,
    State(state): State<AppState>,
    Json(payload): Json<ReleaseKillSwitchRequest>,
) -> std::result::Result<Json<ApiResponse<KillSwitchData>>, HttpError> {
    auth.ensure_scope(
        StepUpScope::SystemKillSwitchRelease,
        time::OffsetDateTime::now_utc(),
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), new_trace_id()))?;

    let trace_id = new_trace_id();
    let request_hash = hash_json(&payload)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let idempotency_request = IdempotencyRequest {
        scope: "system.kill_switch.release".to_string(),
        idempotency_key: idempotency_key.0,
        request_hash,
        request_id: auth.request_id.clone(),
        actor_user_id: Some(auth.user_id.clone()),
        actor_session_id: Some(auth.session_id.clone()),
        resource_type: Some("risk_state".to_string()),
        resource_id: Some("global".to_string()),
    };

    match state
        .idempotency_store
        .begin(&idempotency_request)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?
    {
        IdempotencyBegin::Replay(response_json) => {
            let mut replayed: KillSwitchData =
                serde_json::from_str(&response_json).map_err(|error| {
                    HttpError::with_meta(
                        AppError::internal(
                            "KILL_SWITCH_REPLAY_DESERIALIZE_FAILED",
                            format!("failed to deserialize replayed kill switch response: {error}"),
                        ),
                        auth.request_id.clone(),
                        trace_id.clone(),
                    )
                })?;
            replayed.replayed = true;

            return Ok(Json(ApiResponse::new(replayed, auth.request_id, trace_id)));
        }
        IdempotencyBegin::Started => {}
    }

    let actor = AuthenticatedActor {
        user_id: auth.user_id.clone(),
        session_id: auth.session_id.clone(),
        roles: auth.roles.clone(),
        request_id: auth.request_id.clone(),
        ip: auth.ip.clone(),
        user_agent: auth.user_agent.clone(),
    };

    let receipt = match state
        .risk_service
        .release_kill_switch(ReleaseKillSwitchCommand {
            reason: payload.reason,
            to_mode: payload.to_mode,
            expected_version: payload.expected_version,
            request_id: auth.request_id.clone(),
            trace_id: trace_id.clone(),
            actor,
        })
        .await
    {
        Ok(receipt) => receipt,
        Err(error) => {
            state
                .idempotency_store
                .fail(&idempotency_request, error.code())
                .await
                .map_err(|fail_error| {
                    HttpError::with_meta(fail_error, auth.request_id.clone(), trace_id.clone())
                })?;
            return Err(HttpError::with_meta(
                error,
                auth.request_id.clone(),
                trace_id,
            ));
        }
    };

    let response_data = kill_switch_to_contract(receipt, false);
    let response_json = serde_json::to_string(&response_data).map_err(|error| {
        HttpError::with_meta(
            AppError::internal(
                "KILL_SWITCH_RESPONSE_SERIALIZE_FAILED",
                format!("failed to serialize kill switch response: {error}"),
            ),
            auth.request_id.clone(),
            trace_id.clone(),
        )
    })?;

    state
        .idempotency_store
        .complete(&idempotency_request, &response_json)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        response_data,
        auth.request_id,
        trace_id,
    )))
}

async fn transition_system_mode(
    Extension(auth): Extension<AuthContext>,
    Extension(idempotency_key): Extension<IdempotencyKey>,
    State(state): State<AppState>,
    Json(payload): Json<TransitionSystemModeRequest>,
) -> std::result::Result<Json<ApiResponse<SystemModeData>>, HttpError> {
    if payload.reason.trim().is_empty() {
        return Err(HttpError::with_meta(
            AppError::invalid_input("SYSTEM_MODE_REASON_REQUIRED", "reason must not be empty"),
            auth.request_id.clone(),
            new_trace_id(),
        ));
    }

    let trace_id = new_trace_id();
    let request_hash = hash_json(&payload)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let actor = AuthenticatedActor {
        user_id: auth.user_id.clone(),
        session_id: auth.session_id.clone(),
        roles: auth.roles.clone(),
        request_id: auth.request_id.clone(),
        ip: auth.ip.clone(),
        user_agent: auth.user_agent.clone(),
    };

    let receipt = state
        .system_mode_service
        .transition_mode(ModeTransitionCommand {
            to_mode: payload.to_mode,
            reason: payload.reason,
            request_id: auth.request_id.clone(),
            trace_id: trace_id.clone(),
            idempotency_key: idempotency_key.0,
            request_hash,
            actor,
            required_scope: StepUpScope::SystemModeSwitch,
        })
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        SystemModeData {
            mode: receipt.snapshot.mode,
            environment: receipt.snapshot.environment,
            version: receipt.snapshot.version,
            replayed: receipt.replayed,
            updated_at: receipt.snapshot.updated_at,
        },
        auth.request_id,
        trace_id,
    )))
}

fn market_to_contract(market: MarketView) -> MarketData {
    MarketData {
        id: market.id,
        question: market.question,
        category: market.category,
        status: market.status,
        best_bid: market.best_bid,
        best_ask: market.best_ask,
        mid_price: market.mid_price,
        volume_24h: market.volume_24h,
        ambiguity_level: market.ambiguity_level,
        tradability_status: market.tradability_status,
        resolution_source: market.resolution_source,
        edge_case_notes: market.edge_case_notes,
        polymarket_condition_id: market.polymarket_condition_id,
        polymarket_yes_asset_id: market.polymarket_yes_asset_id,
        polymarket_no_asset_id: market.polymarket_no_asset_id,
        updated_at: market.updated_at,
        version: market.version,
    }
}

fn event_to_contract(event: EventView) -> EventData {
    EventData {
        id: event.id,
        source: event.source,
        summary: event.summary,
        relevance_score: event.relevance_score,
        confidence: event.confidence,
        status: event.status,
        related_market_ids: event.related_market_ids,
        reason_trace: event.reason_trace,
        created_at: event.created_at,
        updated_at: event.updated_at,
        version: event.version,
    }
}

fn evidence_to_contract(evidence: EvidenceView) -> EvidenceData {
    EvidenceData {
        id: evidence.id,
        market_id: evidence.market_id,
        event_id: evidence.event_id,
        direction: evidence.direction,
        strength: evidence.strength,
        source_reliability: evidence.source_reliability,
        novelty: evidence.novelty,
        resolution_relevance: evidence.resolution_relevance,
        status: evidence.status,
        expires_at: evidence.expires_at,
        created_at: evidence.created_at,
        updated_at: evidence.updated_at,
        version: evidence.version,
    }
}

fn signal_to_contract(signal: SignalView) -> SignalData {
    SignalData {
        id: signal.id,
        market_id: signal.market_id,
        event_id: signal.event_id,
        action: signal.action,
        side: signal.side,
        market_price: signal.market_price,
        fair_price: signal.fair_price,
        edge: signal.edge,
        confidence: signal.confidence,
        lifecycle_state: signal.lifecycle_state,
        reason: signal.reason,
        risk_decision: signal.risk_decision,
        evidence_ids: signal.evidence_ids,
        approved_by_user_id: signal.approved_by_user_id,
        approved_at: signal.approved_at,
        rejected_by_user_id: signal.rejected_by_user_id,
        rejected_at: signal.rejected_at,
        updated_at: signal.updated_at,
        version: signal.version,
    }
}

fn order_draft_to_contract(order_draft: OrderDraftView) -> OrderDraftData {
    OrderDraftData {
        id: order_draft.id,
        signal_id: order_draft.signal_id,
        signal_version: order_draft.signal_version,
        market_id: order_draft.market_id,
        connector_name: order_draft.connector_name,
        side: order_draft.side,
        limit_price: order_draft.limit_price,
        quantity: order_draft.quantity,
        notional: order_draft.notional,
        status: order_draft.status,
        created_by_user_id: order_draft.created_by_user_id,
        created_at: order_draft.created_at,
        external_order_id: order_draft.external_order_id,
        submitted_at: order_draft.submitted_at,
        failure_code: order_draft.failure_code,
        failure_message: order_draft.failure_message,
        updated_at: order_draft.updated_at,
        version: order_draft.version,
    }
}

fn execution_request_to_contract(execution_request: ExecutionRequestView) -> ExecutionRequestData {
    ExecutionRequestData {
        id: execution_request.id,
        signal_id: execution_request.signal_id,
        signal_version: execution_request.signal_version,
        order_draft_id: execution_request.order_draft_id,
        connector_name: execution_request.connector_name,
        mode: execution_request.mode,
        requested_by_user_id: execution_request.requested_by_user_id,
        status: execution_request.status,
        reason: execution_request.reason,
        created_at: execution_request.created_at,
        external_order_id: execution_request.external_order_id,
        submitted_at: execution_request.submitted_at,
        failure_code: execution_request.failure_code,
        failure_message: execution_request.failure_message,
        updated_at: execution_request.updated_at,
        version: execution_request.version,
    }
}

fn order_to_contract(order: OrderView) -> OrderData {
    OrderData {
        id: order.id,
        signal_id: order.signal_id,
        execution_request_id: order.execution_request_id,
        order_draft_id: order.order_draft_id,
        market_id: order.market_id,
        connector_name: order.connector_name,
        account_id: order.account_id,
        external_order_id: order.external_order_id,
        side: order.side,
        limit_price: order.limit_price,
        quantity: order.quantity,
        filled_quantity: order.filled_quantity,
        avg_fill_price: order.avg_fill_price,
        status: order.status,
        submitted_at: order.submitted_at,
        updated_at: order.updated_at,
        version: order.version,
    }
}

fn trade_to_contract(trade: TradeView) -> TradeData {
    TradeData {
        id: trade.id,
        order_id: trade.order_id,
        signal_id: trade.signal_id,
        market_id: trade.market_id,
        connector_name: trade.connector_name,
        external_trade_id: trade.external_trade_id,
        side: trade.side,
        price: trade.price,
        quantity: trade.quantity,
        fee: trade.fee,
        executed_at: trade.executed_at,
    }
}

fn position_to_contract(position: PositionView) -> PositionData {
    PositionData {
        id: position.id,
        market_id: position.market_id,
        connector_name: position.connector_name,
        account_id: position.account_id,
        side: position.side,
        net_quantity: position.net_quantity,
        avg_cost: position.avg_cost,
        mark_price: position.mark_price,
        unrealized_pnl: position.unrealized_pnl,
        realized_pnl: position.realized_pnl,
        updated_at: position.updated_at,
        version: position.version,
    }
}

fn risk_state_to_contract(risk_state: RiskStateView) -> RiskStateData {
    RiskStateData {
        mode: risk_state.mode,
        kill_switch: risk_state.kill_switch,
        daily_pnl: risk_state.daily_pnl,
        gross_exposure: risk_state.gross_exposure,
        net_exposure: risk_state.net_exposure,
        open_alerts: risk_state.open_alerts,
        updated_at: risk_state.updated_at,
        version: risk_state.version,
    }
}

fn probability_estimate_to_contract(estimate: ProbabilityEstimateView) -> ProbabilityEstimateData {
    ProbabilityEstimateData {
        id: estimate.id,
        market_id: estimate.market_id,
        event_id: estimate.event_id,
        signal_id: estimate.signal_id,
        prior_price: estimate.prior_price,
        posterior_price: estimate.posterior_price,
        fair_price: estimate.fair_price,
        market_price: estimate.market_price,
        edge: estimate.edge,
        confidence: estimate.confidence,
        time_horizon: estimate.time_horizon,
        model_version: estimate.model_version,
        reason_codes: estimate.reason_codes,
        evidence_count: estimate.evidence_count,
        created_at: estimate.created_at,
    }
}

fn signal_transition_to_contract(transition: SignalTransitionView) -> SignalTransitionData {
    SignalTransitionData {
        id: transition.id,
        signal_id: transition.signal_id,
        from_state: transition.from_state,
        to_state: transition.to_state,
        trigger_type: transition.trigger_type,
        trigger_payload: transition.trigger_payload,
        created_at: transition.created_at,
    }
}

fn recompute_signal_to_contract(
    result: polyedge_application::RecomputeSignalResult,
    replayed: bool,
) -> RecomputeSignalData {
    RecomputeSignalData {
        signal: signal_to_contract(result.signal),
        estimate: probability_estimate_to_contract(result.estimate),
        transition: result.transition.map(signal_transition_to_contract),
        replayed,
    }
}

fn approve_signal_to_contract(receipt: ApproveSignalReceipt, replayed: bool) -> ApproveSignalData {
    ApproveSignalData {
        signal: signal_to_contract(receipt.signal),
        risk_state: risk_state_to_contract(receipt.risk_state),
        replayed,
    }
}

fn reject_signal_to_contract(receipt: RejectSignalReceipt, replayed: bool) -> RejectSignalData {
    RejectSignalData {
        signal: signal_to_contract(receipt.signal),
        risk_state: risk_state_to_contract(receipt.risk_state),
        replayed,
    }
}

fn execution_submission_to_contract(
    receipt: ExecutionSubmissionReceipt,
    replayed: bool,
) -> SubmitExecutionData {
    SubmitExecutionData {
        order_draft: order_draft_to_contract(receipt.order_draft),
        execution_request: execution_request_to_contract(receipt.execution_request),
        risk_state: risk_state_to_contract(receipt.risk_state),
        replayed,
    }
}

fn kill_switch_to_contract(receipt: KillSwitchReceipt, replayed: bool) -> KillSwitchData {
    KillSwitchData {
        risk_state: risk_state_to_contract(receipt.risk_state),
        replayed,
    }
}

fn connector_order_status_to_contract(
    order: OrderView,
    replayed: bool,
) -> ConnectorOrderStatusCallbackData {
    ConnectorOrderStatusCallbackData {
        order: order_to_contract(order),
        replayed,
    }
}

fn connector_trade_fill_to_contract(
    result: ExecutionFillResult,
    risk_state: RiskStateView,
    replayed: bool,
) -> ConnectorTradeFillCallbackData {
    ConnectorTradeFillCallbackData {
        order: order_to_contract(result.order),
        trade: trade_to_contract(result.trade),
        position: position_to_contract(result.position),
        risk_state: risk_state_to_contract(risk_state),
        replayed,
    }
}

fn authenticated_actor(auth: &AuthContext) -> AuthenticatedActor {
    AuthenticatedActor {
        user_id: auth.user_id.clone(),
        session_id: auth.session_id.clone(),
        roles: auth.roles.clone(),
        request_id: auth.request_id.clone(),
        ip: auth.ip.clone(),
        user_agent: auth.user_agent.clone(),
    }
}

fn normalize_callback_connector_name(connector_name: &str) -> polyedge_domain::Result<String> {
    let normalized = connector_name.trim().to_ascii_lowercase();

    if normalized.is_empty() {
        return Err(AppError::invalid_input(
            "CONNECTOR_NAME_REQUIRED",
            "connector_name must not be empty",
        ));
    }

    Ok(normalized)
}

fn validate_callback_event_id(event_id: &str) -> polyedge_domain::Result<String> {
    let normalized = event_id.trim().to_string();

    if normalized.is_empty() {
        return Err(AppError::invalid_input(
            "EXTERNAL_EVENT_ID_REQUIRED",
            "event_id must not be empty",
        ));
    }

    Ok(normalized)
}

fn callback_source(prefix: &str, connector_name: &str) -> String {
    format!("{prefix}.{connector_name}")
}

async fn build_trade_fill_callback_response(
    state: &AppState,
    connector_name: &str,
    external_order_id: &str,
    account_id: &str,
    external_trade_id: &str,
    replayed: bool,
) -> polyedge_domain::Result<ConnectorTradeFillCallbackData> {
    let order = state
        .market_event_service
        .get_order_by_external_ref(connector_name.to_string(), external_order_id.to_string())
        .await?;
    let trades = state
        .execution_service
        .list_trades(TradeListFilters::new(
            Some(order.id.clone()),
            Some(order.signal_id.clone()),
            Some(order.market_id.clone()),
            Some(order.connector_name.clone()),
            Some(100),
        )?)
        .await?;
    let trade = trades
        .into_iter()
        .find(|trade| trade.external_trade_id == external_trade_id)
        .ok_or_else(|| {
            AppError::not_found(
                "EXTERNAL_TRADE_NOT_FOUND",
                "external trade callback replay could not find a matching trade",
            )
        })?;
    let positions = state
        .execution_service
        .list_positions(PositionListFilters::new(
            Some(order.market_id.clone()),
            Some(order.connector_name.clone()),
            Some(order.side),
            Some(100),
        )?)
        .await?;
    let position = positions
        .into_iter()
        .find(|position| position.account_id == account_id)
        .ok_or_else(|| {
            AppError::not_found(
                "POSITION_NOT_FOUND",
                "external trade callback replay could not find a matching position",
            )
        })?;
    let risk_state = state.risk_service.read_state().await?;

    Ok(ConnectorTradeFillCallbackData {
        order: order_to_contract(order),
        trade: trade_to_contract(trade),
        position: position_to_contract(position),
        risk_state: risk_state_to_contract(risk_state),
        replayed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Router,
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use base64::{Engine, engine::general_purpose};
    use ed25519_dalek::{Signer, SigningKey};
    use polyedge_application::{
        AuthenticatedActor, MarkExecutionSubmittedCommand, demo_fixture_bundle,
    };
    use polyedge_domain::{StepUpScope, SystemMode, UserRole};
    use polyedge_infrastructure::{AppState, AuthKeySettings, Runtime, Settings};
    use serde::Serialize;
    use tower::util::ServiceExt;
    use uuid::Uuid;

    #[derive(Serialize)]
    struct TestHeader<'a> {
        alg: &'a str,
        kid: &'a str,
        typ: &'a str,
    }

    #[derive(Serialize)]
    struct TestClaims {
        iss: String,
        aud: String,
        sub: String,
        iat: i64,
        nbf: i64,
        exp: i64,
        jti: String,
        session_id: String,
        roles: Vec<UserRole>,
        auth_time: i64,
        request_id: String,
        step_up_verified: bool,
        step_up_scope: Vec<polyedge_domain::StepUpScope>,
        step_up_until: Option<i64>,
    }

    fn issue_token_with(
        signing_key: &SigningKey,
        kid: &str,
        request_id: &str,
        roles: Vec<UserRole>,
        step_up_scope: Vec<StepUpScope>,
    ) -> String {
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let header = serde_json::to_vec(&TestHeader {
            alg: "EdDSA",
            kid,
            typ: "JWT",
        })
        .expect("serialize header");
        let claims = serde_json::to_vec(&TestClaims {
            iss: "polyedge-nextjs".to_string(),
            aud: "polyedge-rust-api".to_string(),
            sub: "usr_123".to_string(),
            iat: now,
            nbf: now,
            exp: now + 20,
            jti: format!("jit_{}", Uuid::now_v7()),
            session_id: "sess_123".to_string(),
            roles,
            auth_time: now - 30,
            request_id: request_id.to_string(),
            step_up_verified: true,
            step_up_scope,
            step_up_until: Some(now + 120),
        })
        .expect("serialize claims");
        let header_b64 = general_purpose::URL_SAFE_NO_PAD.encode(header);
        let claims_b64 = general_purpose::URL_SAFE_NO_PAD.encode(claims);
        let message = format!("{header_b64}.{claims_b64}");
        let signature = signing_key.sign(message.as_bytes());
        let signature_b64 = general_purpose::URL_SAFE_NO_PAD.encode(signature.to_bytes());
        format!("{message}.{signature_b64}")
    }

    fn issue_token(signing_key: &SigningKey, kid: &str, request_id: &str) -> String {
        issue_token_with(
            signing_key,
            kid,
            request_id,
            vec![UserRole::RiskAdmin],
            vec![StepUpScope::SystemModeSwitch],
        )
    }

    fn test_actor(request_id: &str) -> AuthenticatedActor {
        AuthenticatedActor {
            user_id: "usr_123".to_string(),
            session_id: "sess_123".to_string(),
            roles: vec![UserRole::RiskAdmin],
            request_id: request_id.to_string(),
            ip: None,
            user_agent: Some("api-tests".to_string()),
        }
    }

    async fn approve_and_submit_execution(
        app: Router,
        signing_key: &SigningKey,
        signal_id: &str,
        connector_name: &str,
    ) -> SubmitExecutionData {
        let approve_request_id = format!("req_{}", Uuid::now_v7());
        let approve_token = issue_token_with(
            signing_key,
            "test-key",
            &approve_request_id,
            vec![UserRole::RiskAdmin],
            vec![StepUpScope::SignalApprove],
        );
        let approve_body = serde_json::to_vec(&ApproveSignalRequest {
            reason: "approve before connector callback flow".to_string(),
            expected_version: Some(9),
        })
        .expect("serialize approval body");
        let approve_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/signals/{signal_id}/approve"))
                    .header("Authorization", format!("Bearer {approve_token}"))
                    .header("X-Request-Id", &approve_request_id)
                    .header("Idempotency-Key", format!("idem-approve-{signal_id}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(approve_body))
                    .expect("approve request"),
            )
            .await
            .expect("approve response");
        assert_eq!(approve_response.status(), StatusCode::OK);
        let approve_response_body = to_bytes(approve_response.into_body(), usize::MAX)
            .await
            .expect("read approve body");
        let approve_payload: ApiResponse<ApproveSignalData> =
            serde_json::from_slice(&approve_response_body).expect("deserialize approval response");

        let submit_request_id = format!("req_{}", Uuid::now_v7());
        let submit_token = issue_token_with(
            signing_key,
            "test-key",
            &submit_request_id,
            vec![UserRole::RiskAdmin],
            vec![StepUpScope::ExecutionSubmit],
        );
        let submit_body = serde_json::to_vec(&serde_json::json!({
            "limit_price": "0.48",
            "quantity": "25",
            "reason": "queue manual execution request for connector callback flow",
            "expected_signal_version": approve_payload.data.signal.version,
            "connector_name": connector_name
        }))
        .expect("serialize execution body");
        let submit_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/signals/{signal_id}/execution-requests"))
                    .header("Authorization", format!("Bearer {submit_token}"))
                    .header("X-Request-Id", &submit_request_id)
                    .header("Idempotency-Key", format!("idem-submit-{signal_id}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(submit_body))
                    .expect("submit request"),
            )
            .await
            .expect("submit response");
        assert_eq!(submit_response.status(), StatusCode::OK);
        let submit_response_body = to_bytes(submit_response.into_body(), usize::MAX)
            .await
            .expect("read submit body");
        let submit_payload: ApiResponse<SubmitExecutionData> =
            serde_json::from_slice(&submit_response_body).expect("deserialize submit response");

        submit_payload.data
    }

    async fn dispatch_execution(
        state: &AppState,
        execution_request_id: &str,
        account_id: &str,
        external_order_id: &str,
    ) {
        let request_id = format!("req_dispatch_{}", Uuid::now_v7());
        state
            .execution_service
            .mark_execution_submitted(MarkExecutionSubmittedCommand {
                execution_request_id: execution_request_id.to_string(),
                account_id: account_id.to_string(),
                external_order_id: external_order_id.to_string(),
                request_id: request_id.clone(),
                trace_id: format!("trc_{}", Uuid::now_v7()),
                actor: test_actor(&request_id),
            })
            .await
            .expect("dispatch execution");
    }

    #[tokio::test]
    async fn healthz_is_available_without_authentication() {
        let signing_key = SigningKey::from_bytes(&[11_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let app = build_app(Runtime::test_app_state(settings).expect("state"));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn protected_read_route_requires_valid_token() {
        let signing_key = SigningKey::from_bytes(&[11_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let app = build_app(Runtime::test_app_state(settings).expect("state"));
        let request_id = "req_test_1";
        let token = issue_token(&signing_key, "test-key", request_id);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/system/mode")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn markets_route_returns_seeded_market_list() {
        let signing_key = SigningKey::from_bytes(&[12_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_markets")
            .await
            .expect("seed fixtures");
        let app = build_app(state);
        let request_id = format!("req_{}", Uuid::now_v7());
        let token = issue_token(&signing_key, "test-key", &request_id);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/markets?tradability_status=manual_review")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let payload: ApiResponse<Vec<MarketData>> =
            serde_json::from_slice(&body).expect("deserialize response");
        assert_eq!(payload.data.len(), 1);
        assert_eq!(payload.data[0].id, "mkt_121");
    }

    #[tokio::test]
    async fn events_route_filters_by_status() {
        let signing_key = SigningKey::from_bytes(&[13_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_events")
            .await
            .expect("seed fixtures");
        let app = build_app(state);
        let request_id = format!("req_{}", Uuid::now_v7());
        let token = issue_token(&signing_key, "test-key", &request_id);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/events?status=active")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let payload: ApiResponse<Vec<EventData>> =
            serde_json::from_slice(&body).expect("deserialize response");
        assert_eq!(payload.data.len(), 1);
        assert_eq!(payload.data[0].id, "evt_9001");
    }

    #[tokio::test]
    async fn evidences_route_filters_by_market() {
        let signing_key = SigningKey::from_bytes(&[14_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_evidences")
            .await
            .expect("seed fixtures");
        let app = build_app(state);
        let request_id = format!("req_{}", Uuid::now_v7());
        let token = issue_token(&signing_key, "test-key", &request_id);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/evidences?market_id=mkt_121")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let payload: ApiResponse<Vec<EvidenceData>> =
            serde_json::from_slice(&body).expect("deserialize response");
        assert_eq!(payload.data.len(), 2);
        assert!(payload.data.iter().all(|item| item.market_id == "mkt_121"));
    }

    #[tokio::test]
    async fn signals_route_filters_by_lifecycle_state_alias() {
        let signing_key = SigningKey::from_bytes(&[15_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_signals")
            .await
            .expect("seed fixtures");
        let app = build_app(state);
        let request_id = format!("req_{}", Uuid::now_v7());
        let token = issue_token(&signing_key, "test-key", &request_id);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/signals?status=active")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let payload: ApiResponse<Vec<SignalData>> =
            serde_json::from_slice(&body).expect("deserialize response");
        assert_eq!(payload.data.len(), 1);
        assert_eq!(payload.data[0].id, "sig_2411");
    }

    #[tokio::test]
    async fn risk_state_route_returns_current_snapshot() {
        let signing_key = SigningKey::from_bytes(&[18_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let app = build_app(Runtime::test_app_state(settings).expect("state"));
        let request_id = format!("req_{}", Uuid::now_v7());
        let token = issue_token(&signing_key, "test-key", &request_id);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/risk/state")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let payload: ApiResponse<RiskStateData> =
            serde_json::from_slice(&body).expect("deserialize response");
        assert_eq!(payload.data.mode, SystemMode::ManualConfirm);
        assert!(!payload.data.kill_switch);
        assert_eq!(payload.data.open_alerts, 0);
    }

    #[tokio::test]
    async fn approve_signal_route_requires_signal_approve_scope() {
        let signing_key = SigningKey::from_bytes(&[19_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_approval_scope")
            .await
            .expect("seed fixtures");
        let app = build_app(state);
        let request_id = format!("req_{}", Uuid::now_v7());
        let token = issue_token(&signing_key, "test-key", &request_id);
        let body = serde_json::to_vec(&ApproveSignalRequest {
            reason: "scope check".to_string(),
            expected_version: Some(9),
        })
        .expect("serialize body");

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/signals/sig_2412/approve")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .header("Idempotency-Key", "idem-approve-scope")
                    .header("Content-Type", "application/json")
                    .body(Body::from(body))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn approve_signal_route_is_idempotent() {
        let signing_key = SigningKey::from_bytes(&[20_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_approval")
            .await
            .expect("seed fixtures");
        let app = build_app(state);
        let request_id = format!("req_{}", Uuid::now_v7());
        let token = issue_token_with(
            &signing_key,
            "test-key",
            &request_id,
            vec![UserRole::RiskAdmin],
            vec![StepUpScope::SignalApprove],
        );
        let body = serde_json::to_vec(&ApproveSignalRequest {
            reason: "manual approval after ambiguity review".to_string(),
            expected_version: Some(9),
        })
        .expect("serialize body");

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/signals/sig_2412/approve")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .header("Idempotency-Key", "idem-approve-1")
                    .header("Content-Type", "application/json")
                    .body(Body::from(body.clone()))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let response_body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let payload: ApiResponse<ApproveSignalData> =
            serde_json::from_slice(&response_body).expect("deserialize response");
        assert_eq!(payload.data.signal.id, "sig_2412");
        assert_eq!(
            payload.data.signal.approved_by_user_id.as_deref(),
            Some("usr_123")
        );
        assert!(payload.data.signal.approved_at.is_some());
        assert_eq!(payload.data.risk_state.mode, SystemMode::ManualConfirm);
        assert!(!payload.data.replayed);

        let replay = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/signals/sig_2412/approve")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .header("Idempotency-Key", "idem-approve-1")
                    .header("Content-Type", "application/json")
                    .body(Body::from(body))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(replay.status(), StatusCode::OK);
        let replay_body = to_bytes(replay.into_body(), usize::MAX)
            .await
            .expect("read body");
        let replay_payload: ApiResponse<ApproveSignalData> =
            serde_json::from_slice(&replay_body).expect("deserialize response");
        assert!(replay_payload.data.replayed);
        assert_eq!(
            replay_payload.data.signal.version,
            payload.data.signal.version
        );
    }

    #[tokio::test]
    async fn reject_signal_route_requires_signal_reject_scope() {
        let signing_key = SigningKey::from_bytes(&[23_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_rejection_scope")
            .await
            .expect("seed fixtures");
        let app = build_app(state);
        let request_id = format!("req_{}", Uuid::now_v7());
        let token = issue_token(&signing_key, "test-key", &request_id);
        let body = serde_json::to_vec(&RejectSignalRequest {
            reason: "scope check".to_string(),
            expected_version: Some(9),
        })
        .expect("serialize body");

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/signals/sig_2412/reject")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .header("Idempotency-Key", "idem-reject-scope")
                    .header("Content-Type", "application/json")
                    .body(Body::from(body))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn reject_signal_route_is_idempotent_and_recompute_clears_rejection() {
        let signing_key = SigningKey::from_bytes(&[24_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_rejection")
            .await
            .expect("seed fixtures");
        let app = build_app(state);

        let reject_request_id = format!("req_{}", Uuid::now_v7());
        let reject_token = issue_token_with(
            &signing_key,
            "test-key",
            &reject_request_id,
            vec![UserRole::RiskAdmin],
            vec![StepUpScope::SignalReject],
        );
        let reject_body = serde_json::to_vec(&RejectSignalRequest {
            reason: "manual rejection after operator review".to_string(),
            expected_version: Some(9),
        })
        .expect("serialize body");

        let reject_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/signals/sig_2412/reject")
                    .header("Authorization", format!("Bearer {reject_token}"))
                    .header("X-Request-Id", &reject_request_id)
                    .header("Idempotency-Key", "idem-reject-1")
                    .header("Content-Type", "application/json")
                    .body(Body::from(reject_body.clone()))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(reject_response.status(), StatusCode::OK);
        let reject_response_body = to_bytes(reject_response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let reject_payload: ApiResponse<RejectSignalData> =
            serde_json::from_slice(&reject_response_body).expect("deserialize response");
        assert_eq!(reject_payload.data.signal.id, "sig_2412");
        assert_eq!(
            reject_payload.data.signal.rejected_by_user_id.as_deref(),
            Some("usr_123")
        );
        assert!(reject_payload.data.signal.rejected_at.is_some());
        assert!(reject_payload.data.signal.approved_by_user_id.is_none());
        assert!(!reject_payload.data.replayed);

        let reject_replay = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/signals/sig_2412/reject")
                    .header("Authorization", format!("Bearer {reject_token}"))
                    .header("X-Request-Id", &reject_request_id)
                    .header("Idempotency-Key", "idem-reject-1")
                    .header("Content-Type", "application/json")
                    .body(Body::from(reject_body))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(reject_replay.status(), StatusCode::OK);
        let reject_replay_body = to_bytes(reject_replay.into_body(), usize::MAX)
            .await
            .expect("read body");
        let reject_replay_payload: ApiResponse<RejectSignalData> =
            serde_json::from_slice(&reject_replay_body).expect("deserialize response");
        assert!(reject_replay_payload.data.replayed);

        let approve_request_id = format!("req_{}", Uuid::now_v7());
        let approve_token = issue_token_with(
            &signing_key,
            "test-key",
            &approve_request_id,
            vec![UserRole::RiskAdmin],
            vec![StepUpScope::SignalApprove],
        );
        let approve_body = serde_json::to_vec(&ApproveSignalRequest {
            reason: "should fail after rejection".to_string(),
            expected_version: Some(reject_payload.data.signal.version),
        })
        .expect("serialize body");

        let approve_after_reject = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/signals/sig_2412/approve")
                    .header("Authorization", format!("Bearer {approve_token}"))
                    .header("X-Request-Id", &approve_request_id)
                    .header("Idempotency-Key", "idem-approve-after-reject")
                    .header("Content-Type", "application/json")
                    .body(Body::from(approve_body))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(approve_after_reject.status(), StatusCode::CONFLICT);

        let recompute_request_id = format!("req_{}", Uuid::now_v7());
        let recompute_token = issue_token(&signing_key, "test-key", &recompute_request_id);
        let recompute_body = serde_json::to_vec(&RecomputeSignalRequest {
            reason: "refresh evidence after rejection".to_string(),
        })
        .expect("serialize body");

        let recompute_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/signals/sig_2412/recompute")
                    .header("Authorization", format!("Bearer {recompute_token}"))
                    .header("X-Request-Id", &recompute_request_id)
                    .header("Idempotency-Key", "idem-recompute-after-reject")
                    .header("Content-Type", "application/json")
                    .body(Body::from(recompute_body))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(recompute_response.status(), StatusCode::OK);
        let recompute_response_body = to_bytes(recompute_response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let recompute_payload: ApiResponse<RecomputeSignalData> =
            serde_json::from_slice(&recompute_response_body).expect("deserialize response");
        assert!(recompute_payload.data.signal.rejected_by_user_id.is_none());
        assert!(recompute_payload.data.signal.rejected_at.is_none());
        assert!(recompute_payload.data.signal.approved_by_user_id.is_none());
    }

    #[tokio::test]
    async fn submit_execution_request_requires_execution_submit_scope() {
        let signing_key = SigningKey::from_bytes(&[25_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_execution_scope")
            .await
            .expect("seed fixtures");
        let app = build_app(state);
        let request_id = format!("req_{}", Uuid::now_v7());
        let token = issue_token(&signing_key, "test-key", &request_id);
        let body = serde_json::to_vec(&serde_json::json!({
            "limit_price": "0.48",
            "quantity": "25",
            "reason": "scope check",
            "expected_signal_version": 9,
            "connector_name": "paper_executor"
        }))
        .expect("serialize body");

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/signals/sig_2412/execution-requests")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .header("Idempotency-Key", "idem-execution-scope")
                    .header("Content-Type", "application/json")
                    .body(Body::from(body))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn submit_execution_request_is_idempotent_and_lists_created_records() {
        let signing_key = SigningKey::from_bytes(&[26_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_execution")
            .await
            .expect("seed fixtures");
        let app = build_app(state);

        let approve_request_id = format!("req_{}", Uuid::now_v7());
        let approve_token = issue_token_with(
            &signing_key,
            "test-key",
            &approve_request_id,
            vec![UserRole::RiskAdmin],
            vec![StepUpScope::SignalApprove],
        );
        let approve_body = serde_json::to_vec(&ApproveSignalRequest {
            reason: "approve before queueing execution".to_string(),
            expected_version: Some(9),
        })
        .expect("serialize body");
        let approve_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/signals/sig_2412/approve")
                    .header("Authorization", format!("Bearer {approve_token}"))
                    .header("X-Request-Id", &approve_request_id)
                    .header("Idempotency-Key", "idem-execution-approve")
                    .header("Content-Type", "application/json")
                    .body(Body::from(approve_body))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(approve_response.status(), StatusCode::OK);
        let approve_response_body = to_bytes(approve_response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let approve_payload: ApiResponse<ApproveSignalData> =
            serde_json::from_slice(&approve_response_body).expect("deserialize response");

        let submit_request_id = format!("req_{}", Uuid::now_v7());
        let submit_token = issue_token_with(
            &signing_key,
            "test-key",
            &submit_request_id,
            vec![UserRole::RiskAdmin],
            vec![StepUpScope::ExecutionSubmit],
        );
        let submit_body = serde_json::to_vec(&serde_json::json!({
            "limit_price": "0.48",
            "quantity": "25",
            "reason": "queue manual execution request",
            "expected_signal_version": approve_payload.data.signal.version,
            "connector_name": "paper_executor"
        }))
        .expect("serialize body");

        let submit_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/signals/sig_2412/execution-requests")
                    .header("Authorization", format!("Bearer {submit_token}"))
                    .header("X-Request-Id", &submit_request_id)
                    .header("Idempotency-Key", "idem-execution-submit")
                    .header("Content-Type", "application/json")
                    .body(Body::from(submit_body.clone()))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(submit_response.status(), StatusCode::OK);
        let submit_response_body = to_bytes(submit_response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let submit_payload: ApiResponse<SubmitExecutionData> =
            serde_json::from_slice(&submit_response_body).expect("deserialize response");
        assert_eq!(submit_payload.data.order_draft.signal_id, "sig_2412");
        assert_eq!(submit_payload.data.order_draft.status.as_str(), "queued");
        assert_eq!(
            submit_payload.data.execution_request.status.as_str(),
            "queued"
        );
        assert_eq!(
            submit_payload.data.execution_request.mode,
            SystemMode::ManualConfirm
        );
        assert_eq!(
            submit_payload.data.execution_request.connector_name,
            "paper_executor"
        );
        assert!(!submit_payload.data.replayed);

        let submit_replay = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/signals/sig_2412/execution-requests")
                    .header("Authorization", format!("Bearer {submit_token}"))
                    .header("X-Request-Id", &submit_request_id)
                    .header("Idempotency-Key", "idem-execution-submit")
                    .header("Content-Type", "application/json")
                    .body(Body::from(submit_body))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(submit_replay.status(), StatusCode::OK);
        let submit_replay_body = to_bytes(submit_replay.into_body(), usize::MAX)
            .await
            .expect("read body");
        let submit_replay_payload: ApiResponse<SubmitExecutionData> =
            serde_json::from_slice(&submit_replay_body).expect("deserialize response");
        assert!(submit_replay_payload.data.replayed);

        let list_request_id = format!("req_{}", Uuid::now_v7());
        let list_token = issue_token(&signing_key, "test-key", &list_request_id);

        let order_drafts_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/orders/drafts?signal_id=sig_2412")
                    .header("Authorization", format!("Bearer {list_token}"))
                    .header("X-Request-Id", &list_request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(order_drafts_response.status(), StatusCode::OK);
        let order_drafts_body = to_bytes(order_drafts_response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let order_drafts_payload: ApiResponse<Vec<OrderDraftData>> =
            serde_json::from_slice(&order_drafts_body).expect("deserialize response");
        assert_eq!(order_drafts_payload.data.len(), 1);
        assert_eq!(order_drafts_payload.data[0].signal_id, "sig_2412");

        let execution_requests_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/execution/requests?signal_id=sig_2412")
                    .header("Authorization", format!("Bearer {list_token}"))
                    .header("X-Request-Id", &list_request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(execution_requests_response.status(), StatusCode::OK);
        let execution_requests_body = to_bytes(execution_requests_response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let execution_requests_payload: ApiResponse<Vec<ExecutionRequestData>> =
            serde_json::from_slice(&execution_requests_body).expect("deserialize response");
        assert_eq!(execution_requests_payload.data.len(), 1);
        assert_eq!(execution_requests_payload.data[0].signal_id, "sig_2412");
    }

    #[tokio::test]
    async fn connector_order_status_callback_is_deduplicated_without_idempotency_key() {
        let signing_key = SigningKey::from_bytes(&[27_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_connector_callbacks")
            .await
            .expect("seed fixtures");
        let app = build_app(state.clone());
        let submission =
            approve_and_submit_execution(app.clone(), &signing_key, "sig_2412", "paper_executor")
                .await;
        let external_order_id = "paper_ord_callback_001";

        dispatch_execution(
            &state,
            &submission.execution_request.id,
            "acct_paper_main",
            external_order_id,
        )
        .await;

        let callback_request_id = format!("req_{}", Uuid::now_v7());
        let callback_token = issue_token_with(
            &signing_key,
            "test-key",
            &callback_request_id,
            vec![UserRole::RiskAdmin],
            Vec::new(),
        );
        let callback_body = serde_json::to_vec(&serde_json::json!({
            "event_id": "evt_connector_order_open_1",
            "connector_name": "paper_executor",
            "external_order_id": external_order_id,
            "status": "open"
        }))
        .expect("serialize callback body");

        let first_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/connectors/callbacks/orders/status")
                    .header("Authorization", format!("Bearer {callback_token}"))
                    .header("X-Request-Id", &callback_request_id)
                    .header("Content-Type", "application/json")
                    .body(Body::from(callback_body.clone()))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(first_response.status(), StatusCode::OK);
        let first_response_body = to_bytes(first_response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let first_payload: ApiResponse<ConnectorOrderStatusCallbackData> =
            serde_json::from_slice(&first_response_body).expect("deserialize response");
        assert_eq!(
            first_payload.data.order.external_order_id,
            external_order_id
        );
        assert_eq!(first_payload.data.order.status.as_str(), "open");
        assert!(!first_payload.data.replayed);

        let replay_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/connectors/callbacks/orders/status")
                    .header("Authorization", format!("Bearer {callback_token}"))
                    .header("X-Request-Id", &callback_request_id)
                    .header("Content-Type", "application/json")
                    .body(Body::from(callback_body))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(replay_response.status(), StatusCode::OK);
        let replay_response_body = to_bytes(replay_response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let replay_payload: ApiResponse<ConnectorOrderStatusCallbackData> =
            serde_json::from_slice(&replay_response_body).expect("deserialize response");
        assert!(replay_payload.data.replayed);
        assert_eq!(
            replay_payload.data.order.external_order_id,
            external_order_id
        );
        assert_eq!(replay_payload.data.order.status.as_str(), "open");
    }

    #[tokio::test]
    async fn connector_trade_fill_callback_is_deduplicated_without_duplicate_trades() {
        let signing_key = SigningKey::from_bytes(&[28_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_connector_trade_callback")
            .await
            .expect("seed fixtures");
        let app = build_app(state.clone());
        let submission =
            approve_and_submit_execution(app.clone(), &signing_key, "sig_2412", "paper_executor")
                .await;
        let external_order_id = "paper_ord_callback_002";

        dispatch_execution(
            &state,
            &submission.execution_request.id,
            "acct_paper_main",
            external_order_id,
        )
        .await;

        let open_request_id = format!("req_{}", Uuid::now_v7());
        let open_token = issue_token_with(
            &signing_key,
            "test-key",
            &open_request_id,
            vec![UserRole::RiskAdmin],
            Vec::new(),
        );
        let open_body = serde_json::to_vec(&serde_json::json!({
            "event_id": "evt_connector_order_open_2",
            "connector_name": "paper_executor",
            "external_order_id": external_order_id,
            "status": "open"
        }))
        .expect("serialize order open body");
        let open_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/connectors/callbacks/orders/status")
                    .header("Authorization", format!("Bearer {open_token}"))
                    .header("X-Request-Id", &open_request_id)
                    .header("Content-Type", "application/json")
                    .body(Body::from(open_body))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(open_response.status(), StatusCode::OK);

        let trade_request_id = format!("req_{}", Uuid::now_v7());
        let trade_token = issue_token_with(
            &signing_key,
            "test-key",
            &trade_request_id,
            vec![UserRole::RiskAdmin],
            Vec::new(),
        );
        let trade_body = serde_json::to_vec(&serde_json::json!({
            "event_id": "evt_connector_trade_fill_1",
            "connector_name": "paper_executor",
            "external_order_id": external_order_id,
            "account_id": "acct_paper_main",
            "external_trade_id": "paper_trade_callback_001",
            "fill_price": "0.48",
            "filled_quantity": "1",
            "fee": "0.00"
        }))
        .expect("serialize trade body");

        let first_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/connectors/callbacks/trades/fill")
                    .header("Authorization", format!("Bearer {trade_token}"))
                    .header("X-Request-Id", &trade_request_id)
                    .header("Content-Type", "application/json")
                    .body(Body::from(trade_body.clone()))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(first_response.status(), StatusCode::OK);
        let first_response_body = to_bytes(first_response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let first_payload: ApiResponse<ConnectorTradeFillCallbackData> =
            serde_json::from_slice(&first_response_body).expect("deserialize response");
        assert_eq!(
            first_payload.data.trade.external_trade_id,
            "paper_trade_callback_001"
        );
        assert_eq!(
            first_payload.data.order.external_order_id,
            external_order_id
        );
        assert_eq!(first_payload.data.position.account_id, "acct_paper_main");
        assert!(!first_payload.data.replayed);

        let replay_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/connectors/callbacks/trades/fill")
                    .header("Authorization", format!("Bearer {trade_token}"))
                    .header("X-Request-Id", &trade_request_id)
                    .header("Content-Type", "application/json")
                    .body(Body::from(trade_body))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(replay_response.status(), StatusCode::OK);
        let replay_response_body = to_bytes(replay_response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let replay_payload: ApiResponse<ConnectorTradeFillCallbackData> =
            serde_json::from_slice(&replay_response_body).expect("deserialize response");
        assert!(replay_payload.data.replayed);
        assert_eq!(
            replay_payload.data.trade.external_trade_id,
            "paper_trade_callback_001"
        );

        let trades_request_id = format!("req_{}", Uuid::now_v7());
        let trades_token = issue_token(&signing_key, "test-key", &trades_request_id);
        let trades_response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/v1/trades?order_id={}",
                        first_payload.data.order.id
                    ))
                    .header("Authorization", format!("Bearer {trades_token}"))
                    .header("X-Request-Id", &trades_request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(trades_response.status(), StatusCode::OK);
        let trades_response_body = to_bytes(trades_response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let trades_payload: ApiResponse<Vec<TradeData>> =
            serde_json::from_slice(&trades_response_body).expect("deserialize response");
        assert_eq!(trades_payload.data.len(), 1);
        assert_eq!(
            trades_payload.data[0].external_trade_id,
            "paper_trade_callback_001"
        );
    }

    #[tokio::test]
    async fn polymarket_order_status_callback_normalizes_live_to_open() {
        let signing_key = SigningKey::from_bytes(&[29_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_polymarket_status")
            .await
            .expect("seed fixtures");
        let app = build_app(state.clone());
        let submission = approve_and_submit_execution(
            app.clone(),
            &signing_key,
            "sig_2412",
            polyedge_connectors::POLYMARKET_CONNECTOR_NAME,
        )
        .await;
        let external_order_id = "pm_ord_callback_001";

        dispatch_execution(
            &state,
            &submission.execution_request.id,
            "acct_poly_main",
            external_order_id,
        )
        .await;

        let request_id = format!("req_{}", Uuid::now_v7());
        let token = issue_token_with(
            &signing_key,
            "test-key",
            &request_id,
            vec![UserRole::RiskAdmin],
            Vec::new(),
        );
        let body = serde_json::to_vec(&serde_json::json!({
            "event_id": "evt_pm_order_open_1",
            "order_id": external_order_id,
            "status": "live"
        }))
        .expect("serialize body");

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/connectors/polymarket/callbacks/orders/status")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .header("Content-Type", "application/json")
                    .body(Body::from(body))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let response_body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let payload: ApiResponse<ConnectorOrderStatusCallbackData> =
            serde_json::from_slice(&response_body).expect("deserialize response");
        assert_eq!(
            payload.data.order.connector_name,
            polyedge_connectors::POLYMARKET_CONNECTOR_NAME
        );
        assert_eq!(payload.data.order.status.as_str(), "open");
        assert!(!payload.data.replayed);
    }

    #[tokio::test]
    async fn polymarket_trade_fill_callback_normalizes_trade_payload() {
        let signing_key = SigningKey::from_bytes(&[30_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_polymarket_fill")
            .await
            .expect("seed fixtures");
        let app = build_app(state.clone());
        let submission = approve_and_submit_execution(
            app.clone(),
            &signing_key,
            "sig_2412",
            polyedge_connectors::POLYMARKET_CONNECTOR_NAME,
        )
        .await;
        let external_order_id = "pm_ord_callback_002";

        dispatch_execution(
            &state,
            &submission.execution_request.id,
            "acct_poly_main",
            external_order_id,
        )
        .await;

        let open_request_id = format!("req_{}", Uuid::now_v7());
        let open_token = issue_token_with(
            &signing_key,
            "test-key",
            &open_request_id,
            vec![UserRole::RiskAdmin],
            Vec::new(),
        );
        let open_body = serde_json::to_vec(&serde_json::json!({
            "event_id": "evt_pm_order_open_2",
            "order_id": external_order_id,
            "status": "live"
        }))
        .expect("serialize open body");
        let open_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/connectors/polymarket/callbacks/orders/status")
                    .header("Authorization", format!("Bearer {open_token}"))
                    .header("X-Request-Id", &open_request_id)
                    .header("Content-Type", "application/json")
                    .body(Body::from(open_body))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(open_response.status(), StatusCode::OK);

        let trade_request_id = format!("req_{}", Uuid::now_v7());
        let trade_token = issue_token_with(
            &signing_key,
            "test-key",
            &trade_request_id,
            vec![UserRole::RiskAdmin],
            Vec::new(),
        );
        let trade_body = serde_json::to_vec(&serde_json::json!({
            "event_id": "evt_pm_trade_fill_1",
            "order_id": external_order_id,
            "account_id": "acct_poly_main",
            "trade_id": "pm_trade_callback_001",
            "price": "0.48",
            "size": "1",
            "fee": "0.00"
        }))
        .expect("serialize trade body");

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/connectors/polymarket/callbacks/trades/fill")
                    .header("Authorization", format!("Bearer {trade_token}"))
                    .header("X-Request-Id", &trade_request_id)
                    .header("Content-Type", "application/json")
                    .body(Body::from(trade_body))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let response_body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let payload: ApiResponse<ConnectorTradeFillCallbackData> =
            serde_json::from_slice(&response_body).expect("deserialize response");
        assert_eq!(
            payload.data.order.connector_name,
            polyedge_connectors::POLYMARKET_CONNECTOR_NAME
        );
        assert_eq!(
            payload.data.trade.external_trade_id,
            "pm_trade_callback_001"
        );
        assert_eq!(payload.data.position.account_id, "acct_poly_main");
        assert!(!payload.data.replayed);
    }

    #[tokio::test]
    async fn trigger_kill_switch_requires_specific_scope() {
        let signing_key = SigningKey::from_bytes(&[21_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let app = build_app(Runtime::test_app_state(settings).expect("state"));
        let request_id = format!("req_{}", Uuid::now_v7());
        let token = issue_token_with(
            &signing_key,
            "test-key",
            &request_id,
            vec![UserRole::RiskAdmin],
            vec![StepUpScope::SystemModeSwitch],
        );
        let body = serde_json::to_vec(&TriggerKillSwitchRequest {
            reason: "operator initiated stop".to_string(),
            expected_version: Some(1),
        })
        .expect("serialize body");

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/system/kill-switch/trigger")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .header("Idempotency-Key", "idem-kill-trigger-scope")
                    .header("Content-Type", "application/json")
                    .body(Body::from(body))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn kill_switch_trigger_and_release_are_idempotent() {
        let signing_key = SigningKey::from_bytes(&[22_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let app = build_app(Runtime::test_app_state(settings).expect("state"));

        let trigger_request_id = format!("req_{}", Uuid::now_v7());
        let trigger_token = issue_token_with(
            &signing_key,
            "test-key",
            &trigger_request_id,
            vec![UserRole::RiskAdmin],
            vec![StepUpScope::SystemKillSwitchTrigger],
        );
        let trigger_body = serde_json::to_vec(&TriggerKillSwitchRequest {
            reason: "operator initiated stop".to_string(),
            expected_version: Some(1),
        })
        .expect("serialize body");

        let trigger_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/system/kill-switch/trigger")
                    .header("Authorization", format!("Bearer {trigger_token}"))
                    .header("X-Request-Id", &trigger_request_id)
                    .header("Idempotency-Key", "idem-kill-trigger-1")
                    .header("Content-Type", "application/json")
                    .body(Body::from(trigger_body.clone()))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(trigger_response.status(), StatusCode::OK);
        let trigger_response_body = to_bytes(trigger_response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let trigger_payload: ApiResponse<KillSwitchData> =
            serde_json::from_slice(&trigger_response_body).expect("deserialize response");
        assert_eq!(
            trigger_payload.data.risk_state.mode,
            SystemMode::KillSwitchLocked
        );
        assert!(trigger_payload.data.risk_state.kill_switch);
        assert_eq!(trigger_payload.data.risk_state.version, 2);
        assert!(!trigger_payload.data.replayed);

        let trigger_replay = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/system/kill-switch/trigger")
                    .header("Authorization", format!("Bearer {trigger_token}"))
                    .header("X-Request-Id", &trigger_request_id)
                    .header("Idempotency-Key", "idem-kill-trigger-1")
                    .header("Content-Type", "application/json")
                    .body(Body::from(trigger_body))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(trigger_replay.status(), StatusCode::OK);
        let trigger_replay_body = to_bytes(trigger_replay.into_body(), usize::MAX)
            .await
            .expect("read body");
        let trigger_replay_payload: ApiResponse<KillSwitchData> =
            serde_json::from_slice(&trigger_replay_body).expect("deserialize response");
        assert!(trigger_replay_payload.data.replayed);

        let release_request_id = format!("req_{}", Uuid::now_v7());
        let release_token = issue_token_with(
            &signing_key,
            "test-key",
            &release_request_id,
            vec![UserRole::RiskAdmin],
            vec![StepUpScope::SystemKillSwitchRelease],
        );
        let release_body = serde_json::to_vec(&ReleaseKillSwitchRequest {
            reason: "resume controlled manual operations".to_string(),
            to_mode: SystemMode::ManualConfirm,
            expected_version: Some(2),
        })
        .expect("serialize body");

        let release_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/system/kill-switch/release")
                    .header("Authorization", format!("Bearer {release_token}"))
                    .header("X-Request-Id", &release_request_id)
                    .header("Idempotency-Key", "idem-kill-release-1")
                    .header("Content-Type", "application/json")
                    .body(Body::from(release_body.clone()))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(release_response.status(), StatusCode::OK);
        let release_response_body = to_bytes(release_response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let release_payload: ApiResponse<KillSwitchData> =
            serde_json::from_slice(&release_response_body).expect("deserialize response");
        assert_eq!(
            release_payload.data.risk_state.mode,
            SystemMode::ManualConfirm
        );
        assert!(!release_payload.data.risk_state.kill_switch);
        assert_eq!(release_payload.data.risk_state.version, 3);
        assert!(!release_payload.data.replayed);

        let release_replay = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/system/kill-switch/release")
                    .header("Authorization", format!("Bearer {release_token}"))
                    .header("X-Request-Id", &release_request_id)
                    .header("Idempotency-Key", "idem-kill-release-1")
                    .header("Content-Type", "application/json")
                    .body(Body::from(release_body))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(release_replay.status(), StatusCode::OK);
        let release_replay_body = to_bytes(release_replay.into_body(), usize::MAX)
            .await
            .expect("read body");
        let release_replay_payload: ApiResponse<KillSwitchData> =
            serde_json::from_slice(&release_replay_body).expect("deserialize response");
        assert!(release_replay_payload.data.replayed);
        assert_eq!(release_replay_payload.data.risk_state.version, 3);
    }

    #[tokio::test]
    async fn recompute_signal_route_is_idempotent_and_creates_estimate() {
        let signing_key = SigningKey::from_bytes(&[16_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_recompute")
            .await
            .expect("seed fixtures");
        let app = build_app(state);
        let request_id = format!("req_{}", Uuid::now_v7());
        let token = issue_token(&signing_key, "test-key", &request_id);
        let body = serde_json::to_vec(&RecomputeSignalRequest {
            reason: "manual pricing refresh after official update".to_string(),
        })
        .expect("serialize body");

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/signals/sig_2412/recompute")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .header("Idempotency-Key", "idem-signal-1")
                    .header("Content-Type", "application/json")
                    .body(Body::from(body.clone()))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let first_body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let first_payload: ApiResponse<RecomputeSignalData> =
            serde_json::from_slice(&first_body).expect("deserialize response");
        assert_eq!(first_payload.data.signal.id, "sig_2412");
        assert_eq!(
            first_payload.data.signal.side,
            polyedge_domain::SignalSide::No
        );
        assert_eq!(
            first_payload.data.signal.lifecycle_state,
            polyedge_domain::SignalLifecycleState::New
        );
        assert!(first_payload.data.transition.is_none());
        assert!(!first_payload.data.replayed);

        let replay = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/signals/sig_2412/recompute")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .header("Idempotency-Key", "idem-signal-1")
                    .header("Content-Type", "application/json")
                    .body(Body::from(body))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(replay.status(), StatusCode::OK);
        let replay_body = to_bytes(replay.into_body(), usize::MAX)
            .await
            .expect("read body");
        let replay_payload: ApiResponse<RecomputeSignalData> =
            serde_json::from_slice(&replay_body).expect("deserialize response");
        assert!(replay_payload.data.replayed);
        assert_eq!(
            replay_payload.data.estimate.id,
            first_payload.data.estimate.id
        );

        let estimates_request_id = format!("req_{}", Uuid::now_v7());
        let estimates_token = issue_token(&signing_key, "test-key", &estimates_request_id);
        let estimates_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/pricing/estimates?signal_id=sig_2412")
                    .header("Authorization", format!("Bearer {estimates_token}"))
                    .header("X-Request-Id", &estimates_request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(estimates_response.status(), StatusCode::OK);
        let estimates_body = to_bytes(estimates_response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let estimates_payload: ApiResponse<Vec<ProbabilityEstimateData>> =
            serde_json::from_slice(&estimates_body).expect("deserialize response");
        assert_eq!(estimates_payload.data.len(), 1);
        assert_eq!(
            estimates_payload.data[0].signal_id.as_deref(),
            Some("sig_2412")
        );
    }

    #[tokio::test]
    async fn signal_transitions_route_returns_recompute_transition() {
        let signing_key = SigningKey::from_bytes(&[17_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_transitions")
            .await
            .expect("seed fixtures");
        let app = build_app(state);
        let request_id = format!("req_{}", Uuid::now_v7());
        let token = issue_token(&signing_key, "test-key", &request_id);
        let body = serde_json::to_vec(&RecomputeSignalRequest {
            reason: "refresh transition history".to_string(),
        })
        .expect("serialize body");

        let recompute = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/signals/sig_2411/recompute")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .header("Idempotency-Key", "idem-signal-2")
                    .header("Content-Type", "application/json")
                    .body(Body::from(body))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(recompute.status(), StatusCode::OK);

        let transitions_request_id = format!("req_{}", Uuid::now_v7());
        let transitions_token = issue_token(&signing_key, "test-key", &transitions_request_id);
        let transitions_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/signals/sig_2411/transitions?limit=10")
                    .header("Authorization", format!("Bearer {transitions_token}"))
                    .header("X-Request-Id", &transitions_request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(transitions_response.status(), StatusCode::OK);
        let transitions_body = to_bytes(transitions_response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let payload: ApiResponse<Vec<SignalTransitionData>> =
            serde_json::from_slice(&transitions_body).expect("deserialize response");
        assert_eq!(payload.data.len(), 1);
        assert_eq!(payload.data[0].signal_id, "sig_2411");
        assert_eq!(
            payload.data[0].from_state,
            polyedge_domain::SignalLifecycleState::Active
        );
        assert_eq!(
            payload.data[0].to_state,
            polyedge_domain::SignalLifecycleState::Weakened
        );
    }

    #[tokio::test]
    async fn mode_transition_is_idempotent() {
        let signing_key = SigningKey::from_bytes(&[11_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let app = build_app(Runtime::test_app_state(settings).expect("state"));
        let request_id = "req_test_2";
        let token = issue_token(&signing_key, "test-key", request_id);
        let body = serde_json::to_vec(&TransitionSystemModeRequest {
            to_mode: SystemMode::Research,
            reason: "operator switched to research mode".to_string(),
        })
        .expect("serialize body");

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/system/mode")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", request_id)
                    .header("Idempotency-Key", "idem-1")
                    .header("Content-Type", "application/json")
                    .body(Body::from(body.clone()))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);

        let replay = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/system/mode")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", request_id)
                    .header("Idempotency-Key", "idem-1")
                    .header("Content-Type", "application/json")
                    .body(Body::from(body))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(replay.status(), StatusCode::OK);

        let replay_body = to_bytes(replay.into_body(), usize::MAX)
            .await
            .expect("read body");
        let payload: ApiResponse<SystemModeData> =
            serde_json::from_slice(&replay_body).expect("deserialize response");
        assert!(payload.data.replayed);
    }
}
