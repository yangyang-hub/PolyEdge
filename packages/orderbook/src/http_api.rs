use crate::updates::OrderbookUpdateBroadcaster;
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
use polyedge_infrastructure::AppState;
use polymarket_client_sdk::types::U256;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, str::FromStr};
use tokio::sync::broadcast;
use tracing::warn;

const MAX_REGISTRY_SOURCES: usize = 32;
const MAX_SOURCE_LEN: usize = 64;

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<MessageResponse>)>;

#[derive(Clone)]
pub struct OrderbookApiState {
    pub app: AppState,
    pub broadcaster: OrderbookUpdateBroadcaster,
}

impl OrderbookApiState {
    pub fn new(app: AppState, broadcaster: OrderbookUpdateBroadcaster) -> Self {
        Self { app, broadcaster }
    }
}

// ── Response types ──────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct OrderbookResponse {
    pub token_id: String,
    pub bids: Vec<LevelResponse>,
    pub asks: Vec<LevelResponse>,
    pub observed_at: i64,
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
}

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub source: String,
    pub token_ids: Vec<String>,
}

#[derive(Deserialize)]
pub struct BatchRequest {
    pub token_ids: Vec<String>,
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
    Json(req): Json<BatchRequest>,
) -> ApiResult<OrderbookBatchResponse> {
    let max_tokens = state.app.settings.orderbook_stream.max_tokens;
    let token_ids = validate_token_ids(req.token_ids, max_tokens)?;
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

    Json(OrderbookStatsResponse {
        cache_entries,
        registry_sources,
        registry_total_tokens: total_tokens,
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
    Ok(Json(MessageResponse {
        message: format!(
            "registered {} tokens for source '{}'",
            token_ids.len(),
            source
        ),
    }))
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
    Ok(Json(MessageResponse {
        message: format!("unregistered source '{source}'"),
    }))
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
    for book in req.books {
        let token_id = validate_token_id(book.token_id)?;
        cached_books.push(CachedOrderBook {
            token_id,
            bids: parse_levels(book.bids, max_levels, true)?,
            asks: parse_levels(book.asks, max_levels, false)?,
            observed_at: book.observed_at,
            source: match book.source.as_str() {
                "ws" => BookSource::Ws,
                _ => BookSource::Poll,
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
    Ok(Json(MessageResponse {
        message: format!("ingested {count} books"),
    }))
}

async fn publish_ingested_books(state: &OrderbookApiState, books: &[CachedOrderBook]) {
    for book in books {
        match state.app.orderbook_cache.get_book(&book.token_id).await {
            Ok(Some(current))
                if current.observed_at == book.observed_at && current.source == book.source =>
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
    Query(query): Query<StreamQuery>,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, (StatusCode, Json<MessageResponse>)> {
    let source = query.source.map(validate_source).transpose()?;
    Ok(ws.on_upgrade(move |socket| stream_orderbooks_socket(socket, state, source)))
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

        if socket.send(Message::Text(payload.into())).await.is_err() {
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

fn validate_source(source: String) -> Result<String, (StatusCode, Json<MessageResponse>)> {
    let source = source.trim();
    if source.is_empty() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "orderbook source must not be empty",
        ));
    }
    if source.len() > MAX_SOURCE_LEN {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            format!("orderbook source must be at most {MAX_SOURCE_LEN} bytes"),
        ));
    }
    if !source
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b':'))
    {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "orderbook source contains unsupported characters",
        ));
    }
    Ok(source.to_string())
}

fn authorize_write(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(), (StatusCode, Json<MessageResponse>)> {
    let Some(expected) = state
        .settings
        .orderbook
        .write_token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
    else {
        return Err(error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "orderbook write endpoints are disabled until POLYEDGE_ORDERBOOK__WRITE_TOKEN is configured",
        ));
    };
    let actual = headers
        .get("x-polyedge-orderbook-token")
        .and_then(|value| value.to_str().ok());
    if actual != Some(expected) {
        return Err(error_response(
            StatusCode::UNAUTHORIZED,
            "invalid orderbook write token",
        ));
    }
    Ok(())
}

fn validate_token_ids(
    token_ids: Vec<String>,
    max_tokens: usize,
) -> Result<Vec<String>, (StatusCode, Json<MessageResponse>)> {
    if token_ids.len() > max_tokens {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            format!("orderbook request supports at most {max_tokens} token ids"),
        ));
    }

    let mut seen = HashSet::new();
    let mut normalized = Vec::with_capacity(token_ids.len());
    for token_id in token_ids {
        let token_id = validate_token_id(token_id)?;
        if seen.insert(token_id.clone()) {
            normalized.push(token_id);
        }
    }
    Ok(normalized)
}

fn validate_token_id(token_id: String) -> Result<String, (StatusCode, Json<MessageResponse>)> {
    let token_id = token_id.trim();
    if token_id.is_empty() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "orderbook token id must not be empty",
        ));
    }
    if U256::from_str(token_id).is_err() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            format!("invalid orderbook token id '{token_id}'"),
        ));
    }
    Ok(token_id.to_string())
}

fn parse_levels(
    levels: Vec<LevelResponse>,
    max_levels: usize,
    descending: bool,
) -> Result<Vec<CachedBookLevel>, (StatusCode, Json<MessageResponse>)> {
    let mut parsed = levels
        .into_iter()
        .map(|level| {
            Ok(CachedBookLevel {
                price: Decimal::from_str(&level.price).map_err(|error| {
                    error_response(
                        StatusCode::BAD_REQUEST,
                        format!("invalid orderbook level price '{}': {error}", level.price),
                    )
                })?,
                size: Decimal::from_str(&level.size).map_err(|error| {
                    error_response(
                        StatusCode::BAD_REQUEST,
                        format!("invalid orderbook level size '{}': {error}", level.size),
                    )
                })?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    // Keep the BEST levels (bids descending, asks ascending) before trimming, so
    // an unsorted ingest payload never drops top-of-book.
    if descending {
        parsed.sort_by(|a, b| b.price.cmp(&a.price));
    } else {
        parsed.sort_by(|a, b| a.price.cmp(&b.price));
    }
    parsed.truncate(max_levels.max(1));
    Ok(parsed)
}

fn error_response(
    status: StatusCode,
    message: impl Into<String>,
) -> (StatusCode, Json<MessageResponse>) {
    (
        status,
        Json(MessageResponse {
            message: message.into(),
        }),
    )
}

fn registry_error_response(
    error: polyedge_domain::AppError,
) -> (StatusCode, Json<MessageResponse>) {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("orderbook registry update failed: {error}"),
    )
}

fn cache_error_response(error: polyedge_domain::AppError) -> (StatusCode, Json<MessageResponse>) {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("orderbook cache update failed: {error}"),
    )
}

fn to_response(book: polyedge_application::CachedOrderBook) -> OrderbookResponse {
    OrderbookResponse {
        token_id: book.token_id,
        bids: book
            .bids
            .into_iter()
            .map(|l| LevelResponse {
                price: l.price.to_string(),
                size: l.size.to_string(),
            })
            .collect(),
        asks: book
            .asks
            .into_iter()
            .map(|l| LevelResponse {
                price: l.price.to_string(),
                size: l.size.to_string(),
            })
            .collect(),
        observed_at: book.observed_at,
        source: book.source.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polyedge_domain::SystemMode;
    use polyedge_infrastructure::{Runtime, Settings};

    fn test_state(write_token: Option<&str>) -> AppState {
        let mut settings = Settings::for_test(SystemMode::LiveAuto, "test", Vec::new());
        settings.orderbook.write_token = write_token.map(ToString::to_string);
        Runtime::test_app_state(settings).expect("test app state")
    }

    #[tokio::test]
    async fn orderbook_write_auth_is_disabled_without_configured_token() {
        let error = authorize_write(&test_state(None), &HeaderMap::new())
            .expect_err("write auth must reject missing configuration");

        assert_eq!(error.0, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn orderbook_write_auth_rejects_wrong_token() {
        let state = test_state(Some("secret"));
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-polyedge-orderbook-token",
            "wrong".parse().expect("header"),
        );

        let error =
            authorize_write(&state, &headers).expect_err("write auth must reject wrong token");

        assert_eq!(error.0, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn orderbook_write_auth_accepts_matching_token() {
        let state = test_state(Some("secret"));
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-polyedge-orderbook-token",
            "secret".parse().expect("header"),
        );

        assert!(authorize_write(&state, &headers).is_ok());
    }
}
