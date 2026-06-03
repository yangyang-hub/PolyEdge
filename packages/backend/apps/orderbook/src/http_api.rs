use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use polyedge_application::{BookSource, CachedBookLevel, CachedOrderBook};
use polyedge_infrastructure::AppState;
use polymarket_client_sdk::types::U256;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, str::FromStr};

const MAX_REGISTRY_SOURCES: usize = 32;
const MAX_SOURCE_LEN: usize = 64;

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<MessageResponse>)>;

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
    State(state): State<AppState>,
    Path(token_id): Path<String>,
) -> Result<Json<OrderbookResponse>, StatusCode> {
    match state.orderbook_cache.get_book(&token_id).await {
        Ok(Some(book)) => Ok(Json(to_response(book))),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn get_orderbook_batch(
    State(state): State<AppState>,
    Json(req): Json<BatchRequest>,
) -> ApiResult<OrderbookBatchResponse> {
    let max_tokens = state.settings.orderbook_stream.max_tokens;
    let token_ids = validate_token_ids(req.token_ids, max_tokens)?;
    let mut books = Vec::new();
    for token_id in &token_ids {
        if let Ok(Some(book)) = state.orderbook_cache.get_book(token_id).await {
            books.push(to_response(book));
        }
    }
    Ok(Json(OrderbookBatchResponse { books }))
}

pub async fn get_orderbook_stats(State(state): State<AppState>) -> Json<OrderbookStatsResponse> {
    let all_tokens = state.orderbook_registry.list_all_tokens().await;
    let cache_entries = state
        .orderbook_cache
        .entry_count()
        .await
        .unwrap_or_default();
    let registry_sources = state.orderbook_registry.source_count().await;

    Json(OrderbookStatsResponse {
        cache_entries,
        registry_sources,
        registry_total_tokens: all_tokens.len(),
    })
}

pub async fn register_tokens(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> ApiResult<MessageResponse> {
    let source = validate_source(req.source)?;
    let token_ids = validate_token_ids(req.token_ids, state.settings.orderbook_stream.max_tokens)?;
    if !state.orderbook_registry.has_source(&source).await
        && state.orderbook_registry.source_count().await >= MAX_REGISTRY_SOURCES
    {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            format!("orderbook registry supports at most {MAX_REGISTRY_SOURCES} sources"),
        ));
    }

    state.orderbook_registry.unregister_source(&source).await;
    state
        .orderbook_registry
        .register_tokens(&source, &token_ids)
        .await;
    Ok(Json(MessageResponse {
        message: format!(
            "registered {} tokens for source '{}'",
            token_ids.len(),
            source
        ),
    }))
}

pub async fn unregister_source(
    State(state): State<AppState>,
    Path(source): Path<String>,
) -> ApiResult<MessageResponse> {
    let source = validate_source(source)?;
    state.orderbook_registry.unregister_source(&source).await;
    Ok(Json(MessageResponse {
        message: format!("unregistered source '{source}'"),
    }))
}

pub async fn ingest_books(
    State(state): State<AppState>,
    Json(req): Json<IngestRequest>,
) -> ApiResult<MessageResponse> {
    let max_tokens = state.settings.orderbook_stream.max_tokens;
    if req.books.len() > max_tokens {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            format!("orderbook ingest supports at most {max_tokens} books per request"),
        ));
    }
    let max_levels = state.settings.orderbook_stream.max_levels_per_side;
    let count = req.books.len();
    for book in req.books {
        let token_id = validate_token_id(book.token_id)?;
        let cached = CachedOrderBook {
            token_id,
            bids: parse_levels(book.bids, max_levels)?,
            asks: parse_levels(book.asks, max_levels)?,
            observed_at: book.observed_at,
            source: match book.source.as_str() {
                "ws" => BookSource::Ws,
                _ => BookSource::Poll,
            },
        };
        let _ = state.orderbook_cache.set_book(&cached).await;
    }
    Ok(Json(MessageResponse {
        message: format!("ingested {count} books"),
    }))
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
) -> Result<Vec<CachedBookLevel>, (StatusCode, Json<MessageResponse>)> {
    levels
        .into_iter()
        .take(max_levels.max(1))
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
        .collect()
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
