use crate::{
    error::{Result, ServerError},
    state::AppState,
    store::IdempotencyBegin,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    routing::{get, post},
};
use polyedge_contracts::{
    ApiResponse, CancelExecutionBatchRequest, CreateCancellationBatchRequest,
    CreateExecutionBatchRequest, CreateMarketStrategyRequest, CreateWalletAccountRequest,
    DependencyStatus, HealthData, ManualTradingListQuery, ReadinessData,
    UpdateMarketStrategyRequest, UpdateSystemRuntimeStateRequest, UpdateWalletAccountRequest,
    WriteOperationData,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tower::ServiceBuilder;
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    limit::RequestBodyLimitLayer,
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use uuid::Uuid;

const EXECUTION_SCOPE: &str = "execution_submit";
const CANCEL_SCOPE: &str = "order_cancel_force";
const WALLET_TRADING_ENABLE_SCOPE: &str = "wallet_trading_enable";
const KILL_SWITCH_TRIGGER_SCOPE: &str = "system_kill_switch_trigger";
const KILL_SWITCH_RELEASE_SCOPE: &str = "system_kill_switch_release";

#[derive(Clone)]
struct RequestContext {
    request_id: String,
    trace_id: String,
    actor_id: String,
}

enum WriteLease {
    Replay(ApiResponse<WriteOperationData>),
    Started { key: String, owner_token: String },
}

pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .route("/wallets", get(list_wallets).post(create_wallet))
        .route("/wallets/{id}", get(get_wallet).patch(update_wallet))
        .route(
            "/market-strategies",
            get(list_strategies).post(create_strategy),
        )
        .route(
            "/market-strategies/{id}",
            get(get_strategy).patch(update_strategy),
        )
        .route(
            "/execution-batches",
            get(list_execution_batches).post(create_execution_batch),
        )
        .route("/execution-batches/{id}", get(get_execution_batch))
        .route(
            "/execution-batches/{id}/cancel",
            post(cancel_execution_batch),
        )
        .route("/cancellation-batches", post(create_cancellation_batch))
        .route("/orders", get(list_orders))
        .route("/positions", get(list_positions))
        .route(
            "/system/runtime-state",
            get(get_runtime_state).patch(update_runtime_state),
        );
    let cors = cors_layer(&state);
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .nest("/api/v1", api)
        .with_state(state.clone())
        .layer(
            ServiceBuilder::new()
                .layer(RequestBodyLimitLayer::new(state.config.max_body_bytes))
                .layer(TraceLayer::new_for_http())
                .layer(TimeoutLayer::with_status_code(
                    StatusCode::REQUEST_TIMEOUT,
                    std::time::Duration::from_secs(15),
                ))
                .layer(cors),
        )
}

async fn healthz(headers: HeaderMap) -> Json<ApiResponse<HealthData>> {
    let context = request_context(&headers);
    response(
        HealthData {
            status: "ok".to_string(),
        },
        &context,
    )
}

async fn readyz(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> (StatusCode, Json<ApiResponse<ReadinessData>>) {
    let context = request_context(&headers);
    let postgres = state.store.ping().await;
    let cached_books = state.orderbooks.snapshot().await.len();
    let ready = postgres;
    let data = ReadinessData {
        status: if ready { "ready" } else { "not_ready" }.to_string(),
        postgres: DependencyStatus {
            status: if postgres { "ok" } else { "unavailable" }.to_string(),
            detail: None,
        },
        orderbook: DependencyStatus {
            status: "running".to_string(),
            detail: Some(format!("{cached_books} targeted books cached")),
        },
    };
    (
        if ready {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        response(data, &context),
    )
}

async fn list_wallets(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ManualTradingListQuery>,
) -> Result<Json<ApiResponse<Vec<polyedge_contracts::WalletAccountData>>>> {
    let context = authorize(&state, &headers, None)?;
    Ok(response(state.store.list_wallets(&query).await?, &context))
}

async fn get_wallet(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(wallet_id): Path<i64>,
) -> Result<Json<ApiResponse<polyedge_contracts::WalletAccountData>>> {
    let context = authorize(&state, &headers, None)?;
    Ok(response(state.store.get_wallet(wallet_id).await?, &context))
}

async fn create_wallet(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateWalletAccountRequest>,
) -> Result<Json<ApiResponse<WriteOperationData>>> {
    let context = authorize(
        &state,
        &headers,
        request
            .trading_enabled
            .then_some(WALLET_TRADING_ENABLE_SCOPE),
    )?;
    let lease = begin_write(&state, &headers, "wallet.create", &request).await?;
    if let WriteLease::Replay(replay) = lease {
        return Ok(Json(replay));
    }
    let wallet = state
        .store
        .create_wallet(&request, &context.actor_id, &context.request_id)
        .await?;
    finish_write(
        &state,
        "wallet.create",
        lease,
        completed_operation("wallet", wallet.account.id),
        &context,
    )
    .await
}

async fn update_wallet(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(wallet_id): Path<i64>,
    Json(request): Json<UpdateWalletAccountRequest>,
) -> Result<Json<ApiResponse<WriteOperationData>>> {
    let context = authorize(
        &state,
        &headers,
        (request.trading_enabled == Some(true)).then_some(WALLET_TRADING_ENABLE_SCOPE),
    )?;
    let scope = format!("wallet.update:{wallet_id}");
    let lease = begin_write(&state, &headers, &scope, &request).await?;
    if let WriteLease::Replay(replay) = lease {
        return Ok(Json(replay));
    }
    state
        .store
        .update_wallet(wallet_id, &request, &context.actor_id, &context.request_id)
        .await?;
    finish_write(
        &state,
        &scope,
        lease,
        completed_operation("wallet", wallet_id),
        &context,
    )
    .await
}

async fn list_strategies(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ManualTradingListQuery>,
) -> Result<Json<ApiResponse<Vec<polyedge_contracts::MarketStrategyData>>>> {
    let context = authorize(&state, &headers, None)?;
    Ok(response(
        state.store.list_strategies(&query).await?,
        &context,
    ))
}

async fn get_strategy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(strategy_id): Path<i64>,
) -> Result<Json<ApiResponse<polyedge_contracts::MarketStrategyData>>> {
    let context = authorize(&state, &headers, None)?;
    Ok(response(
        state.store.get_strategy(strategy_id).await?,
        &context,
    ))
}

async fn create_strategy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateMarketStrategyRequest>,
) -> Result<Json<ApiResponse<WriteOperationData>>> {
    let context = authorize(&state, &headers, None)?;
    let lease = begin_write(&state, &headers, "strategy.create", &request).await?;
    if let WriteLease::Replay(replay) = lease {
        return Ok(Json(replay));
    }
    let strategy = state
        .store
        .create_strategy(&request, &context.actor_id, &context.request_id)
        .await?;
    finish_write(
        &state,
        "strategy.create",
        lease,
        completed_operation("strategy", strategy.strategy.id),
        &context,
    )
    .await
}

async fn update_strategy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(strategy_id): Path<i64>,
    Json(request): Json<UpdateMarketStrategyRequest>,
) -> Result<Json<ApiResponse<WriteOperationData>>> {
    let context = authorize(&state, &headers, None)?;
    let scope = format!("strategy.update:{strategy_id}");
    let lease = begin_write(&state, &headers, &scope, &request).await?;
    if let WriteLease::Replay(replay) = lease {
        return Ok(Json(replay));
    }
    state
        .store
        .update_strategy(
            strategy_id,
            &request,
            &context.actor_id,
            &context.request_id,
        )
        .await?;
    finish_write(
        &state,
        &scope,
        lease,
        completed_operation("strategy", strategy_id),
        &context,
    )
    .await
}

async fn list_execution_batches(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ManualTradingListQuery>,
) -> Result<Json<ApiResponse<Vec<polyedge_contracts::ExecutionBatchData>>>> {
    let context = authorize(&state, &headers, None)?;
    Ok(response(
        state.store.list_execution_batches(&query).await?,
        &context,
    ))
}

async fn get_execution_batch(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(batch_id): Path<i64>,
) -> Result<Json<ApiResponse<polyedge_contracts::ExecutionBatchData>>> {
    let context = authorize(&state, &headers, None)?;
    Ok(response(
        state.store.get_execution_batch(batch_id).await?,
        &context,
    ))
}

async fn create_execution_batch(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateExecutionBatchRequest>,
) -> Result<Json<ApiResponse<WriteOperationData>>> {
    let context = authorize(&state, &headers, Some(EXECUTION_SCOPE))?;
    let lease = begin_write(&state, &headers, "execution_batch.create", &request).await?;
    if let WriteLease::Replay(replay) = lease {
        return Ok(Json(replay));
    }
    let batch = state
        .store
        .create_execution_batch(&request, &context.actor_id, &context.request_id)
        .await?;
    finish_write(
        &state,
        "execution_batch.create",
        lease,
        queued_operation("execution_batch", batch.batch.id),
        &context,
    )
    .await
}

async fn cancel_execution_batch(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(batch_id): Path<i64>,
    Json(request): Json<CancelExecutionBatchRequest>,
) -> Result<Json<ApiResponse<WriteOperationData>>> {
    let context = authorize(&state, &headers, Some(CANCEL_SCOPE))?;
    let scope = format!("execution_batch.cancel:{batch_id}");
    let lease = begin_write(&state, &headers, &scope, &request).await?;
    if let WriteLease::Replay(replay) = lease {
        return Ok(Json(replay));
    }
    state
        .store
        .cancel_execution_batch(
            batch_id,
            request.operator_note.as_deref(),
            &context.actor_id,
            &context.request_id,
        )
        .await?;
    finish_write(
        &state,
        &scope,
        lease,
        completed_operation("execution_batch", batch_id),
        &context,
    )
    .await
}

async fn create_cancellation_batch(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateCancellationBatchRequest>,
) -> Result<Json<ApiResponse<WriteOperationData>>> {
    let context = authorize(&state, &headers, Some(CANCEL_SCOPE))?;
    let lease = begin_write(&state, &headers, "cancellation_batch.create", &request).await?;
    if let WriteLease::Replay(replay) = lease {
        return Ok(Json(replay));
    }
    let batch_ids = state
        .store
        .create_cancellation_batches(&request, &context.actor_id, &context.request_id)
        .await?;
    let resource_id = if batch_ids.is_empty() {
        "none".to_string()
    } else {
        batch_ids
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    };
    finish_write(
        &state,
        "cancellation_batch.create",
        lease,
        WriteOperationData {
            accepted: true,
            operation_id: format!("op_cancel_{}", Uuid::now_v7()),
            resource_id,
            status: "queued".to_string(),
        },
        &context,
    )
    .await
}

async fn list_orders(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ManualTradingListQuery>,
) -> Result<Json<ApiResponse<Vec<polyedge_domain::ManagedOrder>>>> {
    let context = authorize(&state, &headers, None)?;
    Ok(response(state.store.list_orders(&query).await?, &context))
}

async fn list_positions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ManualTradingListQuery>,
) -> Result<Json<ApiResponse<Vec<polyedge_domain::ManagedPosition>>>> {
    let context = authorize(&state, &headers, None)?;
    Ok(response(
        state.store.list_positions(&query).await?,
        &context,
    ))
}

async fn get_runtime_state(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<polyedge_contracts::SystemRuntimeStateData>>> {
    let context = authorize(&state, &headers, None)?;
    Ok(response(
        state.store.system_runtime_state().await?,
        &context,
    ))
}

async fn update_runtime_state(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpdateSystemRuntimeStateRequest>,
) -> Result<Json<ApiResponse<WriteOperationData>>> {
    let required_scope = if request.kill_switch_locked {
        KILL_SWITCH_TRIGGER_SCOPE
    } else {
        KILL_SWITCH_RELEASE_SCOPE
    };
    let context = authorize(&state, &headers, Some(required_scope))?;
    let lease = begin_write(&state, &headers, "system.runtime_state.update", &request).await?;
    if let WriteLease::Replay(replay) = lease {
        return Ok(Json(replay));
    }
    state
        .store
        .update_system_runtime_state(&request, &context.actor_id, &context.request_id)
        .await?;
    finish_write(
        &state,
        "system.runtime_state.update",
        lease,
        completed_operation("system_runtime_state", 1),
        &context,
    )
    .await
}

fn request_context(headers: &HeaderMap) -> RequestContext {
    let request_id = headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("req_{}", Uuid::now_v7()));
    let actor_id = headers
        .get("x-polyedge-console-user")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("console")
        .chars()
        .take(120)
        .collect();
    RequestContext {
        request_id,
        trace_id: format!("trc_{}", Uuid::now_v7()),
        actor_id,
    }
}

fn authorize(
    state: &AppState,
    headers: &HeaderMap,
    required_scope: Option<&str>,
) -> Result<RequestContext> {
    let context = request_context(headers);
    if !state.config.auth_disabled {
        let actual = headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .unwrap_or_default();
        let expected = state.config.api_token.as_deref().unwrap_or_default();
        if !constant_time_equal(actual, expected) {
            return Err(ServerError::Unauthorized);
        }
    }
    if let Some(required_scope) = required_scope {
        let scopes = headers
            .get("x-polyedge-step-up-scopes")
            .or_else(|| headers.get("x-step-up-scope"))
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        if !scopes
            .split(',')
            .map(str::trim)
            .any(|scope| scope == required_scope)
        {
            return Err(ServerError::Forbidden);
        }
        if let Some(expected_code) = state.config.step_up_code.as_deref() {
            let actual_code = headers
                .get("x-polyedge-step-up-code")
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default();
            if !constant_time_equal(actual_code, expected_code) {
                return Err(ServerError::Forbidden);
            }
        }
    }
    Ok(context)
}

async fn begin_write<T: Serialize>(
    state: &AppState,
    headers: &HeaderMap,
    scope: &str,
    request: &T,
) -> Result<WriteLease> {
    let key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ServerError::InvalidInput("Idempotency-Key is required".to_string()))?
        .to_string();
    if key.len() > 200 || key.contains(['\r', '\n']) {
        return Err(ServerError::InvalidInput(
            "Idempotency-Key is invalid".to_string(),
        ));
    }
    let request_hash = hash_json(request)?;
    match state
        .store
        .begin_idempotency(scope, &key, &request_hash)
        .await?
    {
        IdempotencyBegin::Replay(value) => {
            let response = serde_json::from_value(value).map_err(|error| {
                ServerError::Internal(format!("invalid stored idempotent response: {error}"))
            })?;
            Ok(WriteLease::Replay(response))
        }
        IdempotencyBegin::Started { owner_token } => Ok(WriteLease::Started { key, owner_token }),
    }
}

async fn finish_write(
    state: &AppState,
    scope: &str,
    lease: WriteLease,
    data: WriteOperationData,
    context: &RequestContext,
) -> Result<Json<ApiResponse<WriteOperationData>>> {
    let response = ApiResponse::new(data, &context.request_id, &context.trace_id);
    if let WriteLease::Started { key, owner_token } = lease {
        let value = serde_json::to_value(&response).map_err(|error| {
            ServerError::Internal(format!("failed to encode idempotent response: {error}"))
        })?;
        state
            .store
            .complete_idempotency(scope, &key, &owner_token, &value)
            .await?;
    }
    Ok(Json(response))
}

fn response<T>(data: T, context: &RequestContext) -> Json<ApiResponse<T>> {
    Json(ApiResponse::new(
        data,
        &context.request_id,
        &context.trace_id,
    ))
}

fn completed_operation(resource: &str, id: i64) -> WriteOperationData {
    WriteOperationData {
        accepted: true,
        operation_id: format!("op_{resource}_{}", Uuid::now_v7()),
        resource_id: id.to_string(),
        status: "completed".to_string(),
    }
}

fn queued_operation(resource: &str, id: i64) -> WriteOperationData {
    WriteOperationData {
        accepted: true,
        operation_id: format!("op_{resource}_{}", Uuid::now_v7()),
        resource_id: id.to_string(),
        status: "queued".to_string(),
    }
}

fn hash_json<T: Serialize>(value: &T) -> Result<String> {
    let bytes = serde_json::to_vec(value).map_err(|error| {
        ServerError::InvalidInput(format!("request cannot be serialized: {error}"))
    })?;
    let digest = Sha256::digest(bytes);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn constant_time_equal(actual: &str, expected: &str) -> bool {
    let actual = Sha256::digest(actual.as_bytes());
    let expected = Sha256::digest(expected.as_bytes());
    bool::from(actual.as_slice().ct_eq(expected.as_slice()))
}

fn cors_layer(state: &AppState) -> CorsLayer {
    let origins = state
        .config
        .cors_origins
        .iter()
        .filter_map(|origin| HeaderValue::from_str(origin).ok())
        .collect::<Vec<_>>();
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([Method::GET, Method::POST, Method::PATCH])
        .allow_headers([
            header::ACCEPT,
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::HeaderName::from_static("x-request-id"),
            header::HeaderName::from_static("idempotency-key"),
            header::HeaderName::from_static("x-polyedge-step-up-code"),
            header::HeaderName::from_static("x-polyedge-step-up-scopes"),
            header::HeaderName::from_static("x-polyedge-console-user"),
        ])
}
