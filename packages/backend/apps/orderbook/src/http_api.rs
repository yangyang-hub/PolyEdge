use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use polyedge_application::{BookSource, CachedBookLevel, CachedOrderBook};
use polyedge_infrastructure::AppState;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

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
) -> Json<OrderbookBatchResponse> {
    let mut books = Vec::new();
    for token_id in &req.token_ids {
        if let Ok(Some(book)) = state.orderbook_cache.get_book(token_id).await {
            books.push(to_response(book));
        }
    }
    Json(OrderbookBatchResponse { books })
}

pub async fn get_orderbook_stats(
    State(state): State<AppState>,
) -> Json<OrderbookStatsResponse> {
    let all_tokens = state.orderbook_registry.list_all_tokens().await;
    // Count distinct sources by reading the registry internals is not exposed;
    // report total tokens as the primary metric.
    Json(OrderbookStatsResponse {
        cache_entries: all_tokens.len(), // approximate: registry ≈ cache
        registry_sources: 0,             // not exposed by trait
        registry_total_tokens: all_tokens.len(),
    })
}

pub async fn register_tokens(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Json<MessageResponse> {
    state
        .orderbook_registry
        .register_tokens(&req.source, &req.token_ids)
        .await;
    Json(MessageResponse {
        message: format!(
            "registered {} tokens for source '{}'",
            req.token_ids.len(),
            req.source
        ),
    })
}

pub async fn unregister_source(
    State(state): State<AppState>,
    Path(source): Path<String>,
) -> Json<MessageResponse> {
    state
        .orderbook_registry
        .unregister_source(&source)
        .await;
    Json(MessageResponse {
        message: format!("unregistered source '{source}'"),
    })
}

pub async fn ingest_books(
    State(state): State<AppState>,
    Json(req): Json<IngestRequest>,
) -> Json<MessageResponse> {
    let count = req.books.len();
    for book in req.books {
        let cached = CachedOrderBook {
            token_id: book.token_id,
            bids: book
                .bids
                .into_iter()
                .map(|l| CachedBookLevel {
                    price: Decimal::from_str(&l.price).unwrap_or_default(),
                    size: Decimal::from_str(&l.size).unwrap_or_default(),
                })
                .collect(),
            asks: book
                .asks
                .into_iter()
                .map(|l| CachedBookLevel {
                    price: Decimal::from_str(&l.price).unwrap_or_default(),
                    size: Decimal::from_str(&l.size).unwrap_or_default(),
                })
                .collect(),
            observed_at: book.observed_at,
            source: match book.source.as_str() {
                "ws" => BookSource::Ws,
                _ => BookSource::Poll,
            },
        };
        let _ = state.orderbook_cache.set_book(&cached).await;
    }
    Json(MessageResponse {
        message: format!("ingested {count} books"),
    })
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
