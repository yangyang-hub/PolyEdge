use crate::{
    metrics::OrderbookRuntimeMetrics,
    stream::{
        ORDERBOOK_UPSTREAM_BATCH_DELAY_MS, ORDERBOOK_UPSTREAM_BATCH_TIMEOUT_SECS,
        current_unix_millis, effective_orderbook_ws_chunk_size, normalized_cached_book,
        reconcile_poll_book, reward_book_to_cached,
    },
    updates::OrderbookUpdateBroadcaster,
};
use axum::{
    Json,
    extract::{
        Path, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use polyedge_application::{BookSource, CachedBookLevel, CachedOrderBook, OrderbookStreamReason};
use polyedge_connectors::PolymarketRewardsConnector;
use polyedge_infrastructure::AppState;
use polymarket_client_sdk::types::U256;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, str::FromStr, sync::Arc, time::Duration};
use tokio::sync::{Mutex, Semaphore, broadcast};
use tracing::{debug, warn};

const MAX_REGISTRY_SOURCES: usize = 32;
const MAX_SOURCE_LEN: usize = 64;
const MAX_INTERNAL_STREAM_CONNECTIONS: usize = 64;
const INTERNAL_STREAM_SEND_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_INGEST_CLOCK_SKEW_MS: i64 = 30_000;
const MAX_INGEST_OBSERVED_AGE_MS: i64 = 24 * 60 * 60 * 1_000;

type ApiError = (StatusCode, Json<MessageResponse>);
type ApiResult<T> = Result<Json<T>, ApiError>;

#[derive(Clone)]
pub struct OrderbookApiState {
    pub app: AppState,
    pub broadcaster: OrderbookUpdateBroadcaster,
    upstream_request_gate: Arc<Mutex<()>>,
    stream_connections: Arc<Semaphore>,
    runtime_metrics: Arc<OrderbookRuntimeMetrics>,
}

impl OrderbookApiState {
    pub fn new(
        app: AppState,
        broadcaster: OrderbookUpdateBroadcaster,
        upstream_request_gate: Arc<Mutex<()>>,
        runtime_metrics: Arc<OrderbookRuntimeMetrics>,
    ) -> Self {
        Self {
            app,
            broadcaster,
            upstream_request_gate,
            stream_connections: Arc::new(Semaphore::new(MAX_INTERNAL_STREAM_CONNECTIONS)),
            runtime_metrics,
        }
    }
}

// ── Response types ──────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct OrderbookResponse {
    pub token_id: String,
    pub bids: Vec<LevelResponse>,
    pub asks: Vec<LevelResponse>,
    pub observed_at: i64,
    pub confirmed_at: i64,
    pub source: String,
}

#[derive(Serialize, Deserialize)]
pub struct LevelResponse {
    pub price: String,
    pub size: String,
}

#[derive(Serialize)]
pub struct OrderbookBatchResponse {
    pub books: Vec<OrderbookResponse>,
}

#[derive(Serialize)]
pub struct OrderbookStatsResponse {
    pub cache_entries: usize,
    pub registry_sources: usize,
    pub registry_total_tokens: usize,
    pub configured_ws_chunk_size: usize,
    pub effective_ws_chunk_size: usize,
    pub ws_max_connections: usize,
    pub estimated_ws_connections: usize,
    pub stale_cache_entries: usize,
    pub oldest_confirmation_age_ms: i64,
    #[serde(flatten)]
    pub runtime: crate::metrics::OrderbookRuntimeMetricsSnapshot,
}

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub source: String,
    pub token_ids: Vec<String>,
}

#[derive(Deserialize)]
pub struct BatchRequest {
    pub token_ids: Vec<String>,
    #[serde(default)]
    pub refresh_if_stale_ms: Option<i64>,
}

#[derive(Deserialize)]
pub struct IngestRequest {
    pub books: Vec<IngestBook>,
}

#[derive(Deserialize)]
pub struct StreamQuery {
    pub source: Option<String>,
}

#[derive(Deserialize)]
pub struct IngestBook {
    pub token_id: String,
    pub bids: Vec<LevelResponse>,
    pub asks: Vec<LevelResponse>,
    pub observed_at: i64,
    pub source: String,
}

#[derive(Serialize)]
pub struct MessageResponse {
    pub message: String,
}

// ── Handlers ────────────────────────────────────────────────────────────────

pub async fn get_orderbook(
    State(state): State<OrderbookApiState>,
    Path(token_id): Path<String>,
) -> Result<Json<OrderbookResponse>, StatusCode> {
    match state.app.orderbook_cache.get_book(&token_id).await {
        Ok(Some(book)) => Ok(Json(to_response(book))),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn get_orderbook_batch(
    State(state): State<OrderbookApiState>,
    headers: HeaderMap,
    Json(req): Json<BatchRequest>,
) -> ApiResult<OrderbookBatchResponse> {
    let max_tokens = state.app.settings.orderbook_stream.max_tokens;
    let token_ids = validate_token_ids(req.token_ids, max_tokens)?;
    if let Some(max_age_ms) = req.refresh_if_stale_ms.filter(|max_age_ms| *max_age_ms > 0) {
        authorize_write(&state.app, &headers)?;
        if let Err(error) = refresh_stale_orderbook_batch(&state, &token_ids, max_age_ms).await {
            warn!(
                error = %error,
                requested = token_ids.len(),
                max_age_ms,
                "failed to refresh stale orderbook batch before cache read"
            );
        }
    }
    let books = state
        .app
        .orderbook_cache
        .get_books(&token_ids)
        .await
        .map_err(|error| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to read orderbook batch: {error}"),
            )
        })?
        .into_iter()
        .map(to_response)
        .collect();
    Ok(Json(OrderbookBatchResponse { books }))
}

async fn refresh_stale_orderbook_batch(
    state: &OrderbookApiState,
    token_ids: &[String],
    max_age_ms: i64,
) -> polyedge_domain::Result<usize> {
    let initially_stale = state
        .app
        .orderbook_cache
        .get_stale_tokens(token_ids, max_age_ms)
        .await?;
    if initially_stale.is_empty() {
        return Ok(0);
    }

    let connector = PolymarketRewardsConnector::new(&state.app.settings.polymarket.clob_host)?;
    let max_levels = state.app.settings.orderbook_stream.max_levels_per_side;
    let mut refreshed = 0usize;
    for (chunk_index, chunk) in initially_stale.chunks(100).enumerate() {
        if chunk_index > 0 {
            tokio::time::sleep(Duration::from_millis(ORDERBOOK_UPSTREAM_BATCH_DELAY_MS)).await;
        }
        let books = {
            let _request_guard = state.upstream_request_gate.lock().await;
            // A background poll or an earlier caller may have refreshed this
            // chunk while it waited for the shared upstream gate. Recheck
            // under the gate to avoid sending duplicate CLOB requests.
            let stale = state
                .app
                .orderbook_cache
                .get_stale_tokens(chunk, max_age_ms)
                .await?;
            if stale.is_empty() {
                continue;
            }
            tokio::time::timeout(
                Duration::from_secs(ORDERBOOK_UPSTREAM_BATCH_TIMEOUT_SECS),
                connector.fetch_order_books(&stale),
            )
            .await
            .map_err(|_| {
                polyedge_domain::AppError::dependency_unavailable(
                    "ORDERBOOK_ON_DEMAND_BATCH_TIMEOUT",
                    format!(
                        "orderbook on-demand batch exceeded {} seconds",
                        ORDERBOOK_UPSTREAM_BATCH_TIMEOUT_SECS
                    ),
                )
            })??
        };
        let poll_confirmed_at = current_unix_millis();
        state
            .runtime_metrics
            .observe_poll_success(poll_confirmed_at);
        for book in books {
            let cached =
                normalized_cached_book(reward_book_to_cached(&book, poll_confirmed_at), max_levels);
            reconcile_poll_book(
                &state.app.orderbook_cache,
                &state.broadcaster,
                &cached,
                &state.runtime_metrics,
            )
            .await?;
            if state
                .app
                .orderbook_cache
                .get_book(&cached.token_id)
                .await?
                .is_some_and(|current| current.confirmation_time_ms() >= poll_confirmed_at)
            {
                refreshed += 1;
            }
        }
    }
    debug!(
        requested = token_ids.len(),
        stale = initially_stale.len(),
        refreshed,
        max_age_ms,
        "refreshed stale orderbook batch on demand"
    );
    Ok(refreshed)
}

pub async fn get_orderbook_stats(
    State(state): State<OrderbookApiState>,
) -> Json<OrderbookStatsResponse> {
    let total_tokens = state.app.orderbook_registry.total_token_count().await;
    let cache_entries = state
        .app
        .orderbook_cache
        .entry_count()
        .await
        .unwrap_or_default();
    let registry_sources = state.app.orderbook_registry.source_count().await;
    let configured_ws_chunk_size = state.app.settings.orderbook_stream.ws_chunk_size.max(1);
    let ws_max_connections = state
        .app
        .settings
        .orderbook_stream
        .ws_max_connections
        .max(1);
    let effective_ws_chunk_size = effective_orderbook_ws_chunk_size(
        configured_ws_chunk_size,
        state.app.settings.orderbook_stream.max_tokens,
        ws_max_connections,
    );
    let estimated_ws_connections = total_tokens
        .saturating_add(effective_ws_chunk_size.saturating_sub(1))
        / effective_ws_chunk_size;
    let registered_tokens = state.app.orderbook_registry.list_all_tokens().await;
    let now = current_unix_millis();
    let books = state
        .app
        .orderbook_cache
        .get_books(&registered_tokens)
        .await
        .unwrap_or_default();
    let stale_threshold_ms = state.app.settings.orderbook_stream.stale_threshold_ms as i64;
    let stale_cache_entries = if stale_threshold_ms > 0 {
        books
            .iter()
            .filter(|book| now.saturating_sub(book.confirmation_time_ms()) > stale_threshold_ms)
            .count()
    } else {
        0
    } + registered_tokens.len().saturating_sub(books.len());
    let oldest_confirmation_age_ms = books
        .iter()
        .map(|book| now.saturating_sub(book.confirmation_time_ms()))
        .max()
        .unwrap_or_default();

    Json(OrderbookStatsResponse {
        cache_entries,
        registry_sources,
        registry_total_tokens: total_tokens,
        configured_ws_chunk_size,
        effective_ws_chunk_size,
        ws_max_connections,
        estimated_ws_connections,
        stale_cache_entries,
        oldest_confirmation_age_ms,
        runtime: state.runtime_metrics.snapshot(),
    })
}

pub async fn register_tokens(
    State(state): State<OrderbookApiState>,
    headers: HeaderMap,
    Json(req): Json<RegisterRequest>,
) -> ApiResult<MessageResponse> {
    authorize_write(&state.app, &headers)?;
    let source = validate_source(req.source)?;
    let token_ids = validate_token_ids(
        req.token_ids,
        state.app.settings.orderbook_stream.max_tokens,
    )?;
    if !token_ids.is_empty()
        && !state.app.orderbook_registry.has_source(&source).await
        && state.app.orderbook_registry.source_count().await >= MAX_REGISTRY_SOURCES
    {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            format!("orderbook registry supports at most {MAX_REGISTRY_SOURCES} sources"),
        ));
    }

    state
        .app
        .orderbook_registry
        .register_tokens(&source, &token_ids)
        .await
        .map_err(registry_error_response)?;
    Ok(message_response(format!(
        "registered {} tokens for source '{}'",
        token_ids.len(),
        source
    )))
}

pub async fn unregister_source(
    State(state): State<OrderbookApiState>,
    headers: HeaderMap,
    Path(source): Path<String>,
) -> ApiResult<MessageResponse> {
    authorize_write(&state.app, &headers)?;
    let source = validate_source(source)?;
    state
        .app
        .orderbook_registry
        .unregister_source(&source)
        .await
        .map_err(registry_error_response)?;
    Ok(message_response(format!("unregistered source '{source}'")))
}

pub async fn ingest_books(
    State(state): State<OrderbookApiState>,
    headers: HeaderMap,
    Json(req): Json<IngestRequest>,
) -> ApiResult<MessageResponse> {
    authorize_write(&state.app, &headers)?;
    let max_tokens = state.app.settings.orderbook_stream.max_tokens;
    if req.books.len() > max_tokens {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            format!("orderbook ingest supports at most {max_tokens} books per request"),
        ));
    }
    let max_levels = state.app.settings.orderbook_stream.max_levels_per_side;
    let count = req.books.len();
    let mut cached_books = Vec::with_capacity(count);
    let mut seen_tokens = HashSet::with_capacity(count);
    let confirmed_at = current_unix_millis();
    for book in req.books {
        let token_id = validate_token_id(book.token_id)?;
        if !seen_tokens.insert(token_id.clone()) {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                format!("duplicate orderbook token '{token_id}' in ingest batch"),
            ));
        }
        validate_ingest_observed_at(book.observed_at, confirmed_at)?;
        let bids = parse_levels(book.bids, max_levels, true)?;
        let asks = parse_levels(book.asks, max_levels, false)?;
        if bids
            .first()
            .zip(asks.first())
            .is_some_and(|(bid, ask)| bid.price >= ask.price)
        {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                format!("crossed or locked orderbook for token '{token_id}'"),
            ));
        }
        cached_books.push(CachedOrderBook {
            token_id,
            bids,
            asks,
            observed_at: book.observed_at,
            confirmed_at,
            source: match book.source.as_str() {
                "ws" => BookSource::Ws,
                "poll" => BookSource::Poll,
                _ => {
                    return Err(error_response(
                        StatusCode::BAD_REQUEST,
                        format!("unsupported orderbook source '{}'", book.source),
                    ));
                }
            },
        });
    }
    state
        .app
        .orderbook_cache
        .set_books(&cached_books)
        .await
        .map_err(cache_error_response)?;
    publish_ingested_books(&state, &cached_books).await;
    Ok(message_response(format!("ingested {count} books")))
}

async fn publish_ingested_books(state: &OrderbookApiState, books: &[CachedOrderBook]) {
    for book in books {
        match state.app.orderbook_cache.get_book(&book.token_id).await {
            Ok(Some(current))
                if current.observed_at == book.observed_at
                    && current.confirmation_time_ms() == book.confirmation_time_ms()
                    && current.source == book.source =>
            {
                state
                    .broadcaster
                    .publish(OrderbookStreamReason::Ingest, current);
            }
            Ok(_) => {}
            Err(error) => {
                warn!(
                    token_id = %book.token_id,
                    error = %error,
                    "failed to confirm ingested orderbook before broadcast"
                );
            }
        }
    }
}

pub async fn stream_orderbooks(
    State(state): State<OrderbookApiState>,
    headers: HeaderMap,
    Query(query): Query<StreamQuery>,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, ApiError> {
    authorize_write(&state.app, &headers)?;
    let source = query.source.map(validate_source).transpose()?;
    let permit = Arc::clone(&state.stream_connections)
        .try_acquire_owned()
        .map_err(|_| {
            error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "orderbook stream connection limit reached",
            )
        })?;
    Ok(ws.on_upgrade(move |socket| async move {
        let _permit = permit;
        stream_orderbooks_socket(socket, state, source).await;
    }))
}

async fn stream_orderbooks_socket(
    mut socket: WebSocket,
    state: OrderbookApiState,
    source: Option<String>,
) {
    let mut rx = state.broadcaster.subscribe();
    let mut source_tokens = match source.as_deref() {
        Some(source) => state
            .app
            .orderbook_registry
            .list_source_tokens(source)
            .await
            .into_iter()
            .collect::<HashSet<_>>(),
        None => HashSet::new(),
    };
    let mut registry_changes = state.app.orderbook_registry.subscribe_changes();
    loop {
        let event = tokio::select! {
            change = wait_for_registry_change(&mut registry_changes), if source.is_some() => {
                if change {
                    if let Some(source) = source.as_deref() {
                        source_tokens = state
                            .app
                            .orderbook_registry
                            .list_source_tokens(source)
                            .await
                            .into_iter()
                            .collect::<HashSet<_>>();
                    }
                }
                continue;
            }
            received = rx.recv() => {
                match received {
                    Ok(event) => event,
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!(
                            skipped,
                            "orderbook stream client lagged behind broadcast channel"
                        );
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        };

        if source.is_some() && !source_tokens.contains(&event.book.token_id) {
            continue;
        }

        let payload = match serde_json::to_string(&event) {
            Ok(payload) => payload,
            Err(error) => {
                warn!(error = %error, "failed to encode orderbook stream event");
                continue;
            }
        };

        if !matches!(
            tokio::time::timeout(
                INTERNAL_STREAM_SEND_TIMEOUT,
                socket.send(Message::Text(payload.into())),
            )
            .await,
            Ok(Ok(()))
        ) {
            break;
        }
    }
}

async fn wait_for_registry_change(
    change_rx: &mut Option<tokio::sync::watch::Receiver<u64>>,
) -> bool {
    let Some(rx) = change_rx else {
        std::future::pending::<()>().await;
        return false;
    };
    rx.changed().await.is_ok()
}

include!("http_api/helpers.rs");

#[cfg(test)]
include!("http_api/tests.rs");
