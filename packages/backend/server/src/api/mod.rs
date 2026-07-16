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
    ActivateUserRequest, ApiResponse, CancelExecutionBatchRequest, CreateCancellationBatchRequest,
    CreateExecutionBatchRequest, CreateMarketStrategyRequest, CreateStrategySubscriptionRequest,
    CreateUserRequest, CreateWalletAccountRequest, DependencyStatus, HealthData, LoginRequest,
    ManualTradingListQuery, ReadinessData, ReauthenticateRequest, ReissueActivationTokenRequest,
    UpdateMarketStrategyRequest, UpdateStrategySubscriptionRequest,
    UpdateSystemRuntimeStateRequest, UpdateUserRequest, UpdateWalletAccountRequest,
    WriteOperationData,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
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
const WALLET_SECRET_ROTATE_SCOPE: &str = "wallet_secret_rotate";
const CASH_FLOW_RECORD_SCOPE: &str = "cash_flow_record";
const KILL_SWITCH_TRIGGER_SCOPE: &str = "system_kill_switch_trigger";
const KILL_SWITCH_RELEASE_SCOPE: &str = "system_kill_switch_release";

#[derive(Clone)]
struct RequestContext {
    request_id: String,
    trace_id: String,
    actor: polyedge_domain::ActorScope,
    session_id: Uuid,
}

enum WriteLease {
    Replay(ApiResponse<WriteOperationData>),
    Started { key: String, owner_token: String },
}

pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/activate", post(activate_user))
        .route("/auth/reauth", post(reauthenticate))
        .route("/auth/me", get(current_user))
        .route("/admin/users", get(list_users).post(create_user))
        .route("/admin/users/{id}", axum::routing::patch(update_user))
        .route(
            "/admin/users/{id}/activation-token",
            post(reissue_activation_token),
        )
        .route("/admin/finance", get(admin_finance))
        .route(
            "/security/wallet-import-contexts",
            post(create_wallet_import_context),
        )
        .route("/wallets", get(list_wallets).post(create_wallet))
        .route("/wallets/{id}", get(get_wallet).patch(update_wallet))
        .route(
            "/market-strategies",
            get(list_strategies).post(create_strategy),
        )
        .route("/market-strategies/discover", get(discover_strategies))
        .route(
            "/market-strategies/{id}",
            get(get_strategy).patch(update_strategy),
        )
        .route(
            "/strategy-subscriptions",
            get(list_strategy_subscriptions).post(create_strategy_subscription),
        )
        .route(
            "/strategy-subscriptions/{id}",
            axum::routing::patch(update_strategy_subscription),
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
        .route("/cash-flows", get(list_cash_flows).post(record_cash_flow))
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

include!("identity.rs");
include!("finance.rs");
include!("subscriptions.rs");
include!("wallet_security.rs");

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
    let context = authorize(&state, &headers, None).await?;
    Ok(response(
        state.store.list_wallets(context.actor, &query).await?,
        &context,
    ))
}

async fn get_wallet(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(wallet_id): Path<i64>,
) -> Result<Json<ApiResponse<polyedge_contracts::WalletAccountData>>> {
    let context = authorize(&state, &headers, None).await?;
    Ok(response(
        state.store.get_wallet(context.actor, wallet_id).await?,
        &context,
    ))
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
    )
    .await?;
    let lease = begin_write(&state, &headers, "wallet.create", &request).await?;
    if let WriteLease::Replay(replay) = lease {
        return Ok(Json(replay));
    }
    let wallet = state
        .store
        .create_wallet(
            context.actor,
            &request,
            &state.wallet_crypto,
            &context.request_id,
        )
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
        if request.encrypted_secret.is_some() {
            Some(WALLET_SECRET_ROTATE_SCOPE)
        } else {
            (request.trading_enabled == Some(true)).then_some(WALLET_TRADING_ENABLE_SCOPE)
        },
    )
    .await?;
    let scope = format!("wallet.update:{wallet_id}");
    let lease = begin_write(&state, &headers, &scope, &request).await?;
    if let WriteLease::Replay(replay) = lease {
        return Ok(Json(replay));
    }
    state
        .store
        .update_wallet(
            context.actor,
            wallet_id,
            &request,
            &state.wallet_crypto,
            &context.request_id,
        )
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
    let context = authorize(&state, &headers, None).await?;
    Ok(response(
        state.store.list_strategies(&query, context.actor).await?,
        &context,
    ))
}

async fn discover_strategies(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ManualTradingListQuery>,
) -> Result<Json<ApiResponse<Vec<polyedge_contracts::MarketStrategyData>>>> {
    let context = authorize(&state, &headers, None).await?;
    Ok(response(
        state
            .store
            .discover_strategies(&query, context.actor)
            .await?,
        &context,
    ))
}

async fn get_strategy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(strategy_id): Path<i64>,
) -> Result<Json<ApiResponse<polyedge_contracts::MarketStrategyData>>> {
    let context = authorize(&state, &headers, None).await?;
    Ok(response(
        state.store.get_strategy(strategy_id, context.actor).await?,
        &context,
    ))
}

async fn create_strategy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateMarketStrategyRequest>,
) -> Result<Json<ApiResponse<WriteOperationData>>> {
    let context = authorize(&state, &headers, None).await?;
    let lease = begin_write(&state, &headers, "strategy.create", &request).await?;
    if let WriteLease::Replay(replay) = lease {
        return Ok(Json(replay));
    }
    let strategy = state
        .store
        .create_strategy(&request, context.actor, &context.request_id)
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
    let context = authorize(&state, &headers, None).await?;
    let scope = format!("strategy.update:{strategy_id}");
    let lease = begin_write(&state, &headers, &scope, &request).await?;
    if let WriteLease::Replay(replay) = lease {
        return Ok(Json(replay));
    }
    state
        .store
        .update_strategy(strategy_id, &request, context.actor, &context.request_id)
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
    let context = authorize(&state, &headers, None).await?;
    Ok(response(
        state
            .store
            .list_execution_batches(&query, context.actor)
            .await?,
        &context,
    ))
}

async fn get_execution_batch(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(batch_id): Path<i64>,
) -> Result<Json<ApiResponse<polyedge_contracts::ExecutionBatchData>>> {
    let context = authorize(&state, &headers, None).await?;
    Ok(response(
        state
            .store
            .get_execution_batch(batch_id, context.actor)
            .await?,
        &context,
    ))
}

async fn create_execution_batch(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateExecutionBatchRequest>,
) -> Result<Json<ApiResponse<WriteOperationData>>> {
    let context = authorize(&state, &headers, Some(EXECUTION_SCOPE)).await?;
    let lease = begin_write(&state, &headers, "execution_batch.create", &request).await?;
    if let WriteLease::Replay(replay) = lease {
        return Ok(Json(replay));
    }
    let batch = state
        .store
        .create_execution_batch(&request, context.actor, &context.request_id)
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
    let context = authorize(&state, &headers, Some(CANCEL_SCOPE)).await?;
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
            context.actor,
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
    let context = authorize(&state, &headers, Some(CANCEL_SCOPE)).await?;
    let lease = begin_write(&state, &headers, "cancellation_batch.create", &request).await?;
    if let WriteLease::Replay(replay) = lease {
        return Ok(Json(replay));
    }
    let batch_ids = state
        .store
        .create_cancellation_batches(&request, context.actor, &context.request_id)
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
    let context = authorize(&state, &headers, None).await?;
    Ok(response(
        state.store.list_orders(&query, context.actor).await?,
        &context,
    ))
}

async fn list_positions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ManualTradingListQuery>,
) -> Result<Json<ApiResponse<Vec<polyedge_domain::ManagedPosition>>>> {
    let context = authorize(&state, &headers, None).await?;
    Ok(response(
        state.store.list_positions(&query, context.actor).await?,
        &context,
    ))
}

async fn get_runtime_state(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<polyedge_contracts::SystemRuntimeStateData>>> {
    let context = authorize(&state, &headers, None).await?;
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
    let context = authorize(&state, &headers, Some(required_scope)).await?;
    if !context.actor.is_admin() {
        return Err(ServerError::Forbidden);
    }
    let lease = begin_write(&state, &headers, "system.runtime_state.update", &request).await?;
    if let WriteLease::Replay(replay) = lease {
        return Ok(Json(replay));
    }
    state
        .store
        .update_system_runtime_state(&request, context.actor, &context.request_id)
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
    RequestContext {
        request_id: request_id(headers),
        trace_id: trace_id(headers),
        actor: polyedge_domain::ActorScope {
            user_id: 0,
            role: polyedge_domain::UserRole::ReadOnly,
        },
        session_id: Uuid::nil(),
    }
}

fn request_id(headers: &HeaderMap) -> String {
    headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("req_{}", Uuid::now_v7()))
}

fn trace_id(headers: &HeaderMap) -> String {
    headers
        .get("x-trace-id")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("trc_{}", Uuid::now_v7()))
}

async fn authorize(
    state: &AppState,
    headers: &HeaderMap,
    required_scope: Option<&str>,
) -> Result<RequestContext> {
    let write_request = headers.contains_key("idempotency-key") || required_scope.is_some();
    authorize_request(state, headers, required_scope, write_request, write_request).await
}

async fn authorize_mutation(
    state: &AppState,
    headers: &HeaderMap,
    required_scope: Option<&str>,
) -> Result<RequestContext> {
    authorize_request(state, headers, required_scope, true, true).await
}

async fn authorize_account_mutation(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<RequestContext> {
    authorize_request(state, headers, None, true, false).await
}

async fn authorize_request(
    state: &AppState,
    headers: &HeaderMap,
    required_scope: Option<&str>,
    write_request: bool,
    enforce_writer_role: bool,
) -> Result<RequestContext> {
    let token = session_cookie(headers).ok_or(ServerError::Unauthorized)?;
    if write_request {
        let origin = headers
            .get(header::ORIGIN)
            .and_then(|value| value.to_str().ok())
            .ok_or(ServerError::Forbidden)?;
        if origin != state.config.public_origin {
            return Err(ServerError::Forbidden);
        }
    }
    let session = state
        .store
        .authenticate_session(
            token,
            csrf_header(headers),
            write_request,
            time::Duration::try_from(state.config.session_idle_ttl)
                .map_err(|_| ServerError::Configuration("session idle ttl is invalid".into()))?,
        )
        .await?;
    if enforce_writer_role && session.user.role == polyedge_domain::UserRole::ReadOnly {
        return Err(ServerError::Forbidden);
    }
    if required_scope.is_some() {
        let recent = session.recent_auth_at.ok_or(ServerError::Forbidden)?;
        let recent_ttl = time::Duration::try_from(state.config.recent_auth_ttl)
            .map_err(|_| ServerError::Configuration("recent auth ttl is invalid".into()))?;
        if OffsetDateTime::now_utc() - recent > recent_ttl {
            return Err(ServerError::Forbidden);
        }
    }
    Ok(RequestContext {
        request_id: request_id(headers),
        trace_id: trace_id(headers),
        actor: session.actor(),
        session_id: session.session_id,
    })
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
    let authenticated = state
        .store
        .authenticate_session(
            session_cookie(headers).ok_or(ServerError::Unauthorized)?,
            csrf_header(headers),
            true,
            time::Duration::try_from(state.config.session_idle_ttl)
                .map_err(|_| ServerError::Configuration("session idle ttl is invalid".into()))?,
        )
        .await?;
    match state
        .store
        .begin_idempotency(authenticated.user.id, scope, &key, &request_hash)
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
            .complete_idempotency(context.actor.user_id, scope, &key, &owner_token, &value)
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
        .allow_credentials(true)
        .allow_headers([
            header::ACCEPT,
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::HeaderName::from_static("x-request-id"),
            header::HeaderName::from_static("idempotency-key"),
            header::HeaderName::from_static("x-polyedge-csrf-token"),
            header::HeaderName::from_static("x-csrf-token"),
        ])
}
