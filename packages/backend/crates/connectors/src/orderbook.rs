use async_trait::async_trait;
use polyedge_application::{CachedBookLevel, CachedOrderBook, OrderbookCache, OrderbookSubscriptionRegistry};
use polyedge_domain::{AppError, Result};
use reqwest::Client;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::str::FromStr;
use std::time::Instant;

/// HTTP client that implements `OrderbookCache` by calling the standalone
/// orderbook service. Used by API and Worker processes to read orderbook data.
pub struct OrderbookHttpClient {
    base_url: String,
    client: Client,
}

impl OrderbookHttpClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: Client::new(),
        }
    }
}

#[derive(Deserialize)]
struct OrderbookResponse {
    token_id: String,
    bids: Vec<LevelResponse>,
    asks: Vec<LevelResponse>,
    observed_at: i64,
    source: String,
}

#[derive(Deserialize)]
struct LevelResponse {
    price: String,
    size: String,
}

#[derive(serde::Serialize)]
struct IngestRequest {
    books: Vec<IngestBook>,
}

#[derive(serde::Serialize)]
struct IngestBook {
    token_id: String,
    bids: Vec<IngestLevel>,
    asks: Vec<IngestLevel>,
    observed_at: i64,
    source: String,
}

#[derive(serde::Serialize)]
struct IngestLevel {
    price: String,
    size: String,
}

fn to_cached(resp: OrderbookResponse) -> CachedOrderBook {
    CachedOrderBook {
        token_id: resp.token_id,
        bids: resp
            .bids
            .into_iter()
            .map(|l| CachedBookLevel {
                price: Decimal::from_str(&l.price).unwrap_or_default(),
                size: Decimal::from_str(&l.size).unwrap_or_default(),
            })
            .collect(),
        asks: resp
            .asks
            .into_iter()
            .map(|l| CachedBookLevel {
                price: Decimal::from_str(&l.price).unwrap_or_default(),
                size: Decimal::from_str(&l.size).unwrap_or_default(),
            })
            .collect(),
        observed_at: resp.observed_at,
        source: match resp.source.as_str() {
            "ws" => polyedge_application::BookSource::Ws,
            _ => polyedge_application::BookSource::Poll,
        },
    }
}

#[async_trait]
impl OrderbookCache for OrderbookHttpClient {
    async fn get_book(&self, token_id: &str) -> Result<Option<CachedOrderBook>> {
        let url = format!("{}/orderbook/{}", self.base_url, token_id);
        let resp = self.client.get(&url).send().await.map_err(|error| {
            AppError::dependency_unavailable(
                "ORDERBOOK_HTTP_ERROR",
                format!("failed to fetch orderbook for {token_id}: {error}"),
            )
        })?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let book: OrderbookResponse = resp.json().await.map_err(|error| {
            AppError::dependency_unavailable(
                "ORDERBOOK_HTTP_DECODE_ERROR",
                format!("failed to decode orderbook response for {token_id}: {error}"),
            )
        })?;

        Ok(Some(to_cached(book)))
    }

    async fn set_book(&self, book: &CachedOrderBook) -> Result<()> {
        // Remote clients can push books to the orderbook service via the ingest endpoint.
        let url = format!("{}/orderbook/ingest", self.base_url);
        let body = IngestRequest {
            books: vec![IngestBook {
                token_id: book.token_id.clone(),
                bids: book
                    .bids
                    .iter()
                    .map(|l| IngestLevel {
                        price: l.price.to_string(),
                        size: l.size.to_string(),
                    })
                    .collect(),
                asks: book
                    .asks
                    .iter()
                    .map(|l| IngestLevel {
                        price: l.price.to_string(),
                        size: l.size.to_string(),
                    })
                    .collect(),
                observed_at: book.observed_at,
                source: book.source.to_string(),
            }],
        };

        let resp = self.client.post(&url).json(&body).send().await.map_err(|error| {
            AppError::dependency_unavailable(
                "ORDERBOOK_HTTP_INGEST_ERROR",
                format!("failed to ingest orderbook for {}: {error}", book.token_id),
            )
        })?;
        if !resp.status().is_success() {
            return Err(AppError::dependency_unavailable(
                "ORDERBOOK_HTTP_INGEST_FAILED",
                format!(
                    "orderbook ingest returned status {} for {}",
                    resp.status(),
                    book.token_id
                ),
            ));
        }
        Ok(())
    }

    async fn set_books(&self, books: &[CachedOrderBook]) -> Result<()> {
        if books.is_empty() {
            return Ok(());
        }
        let url = format!("{}/orderbook/ingest", self.base_url);
        let body = IngestRequest {
            books: books
                .iter()
                .map(|book| IngestBook {
                    token_id: book.token_id.clone(),
                    bids: book
                        .bids
                        .iter()
                        .map(|l| IngestLevel {
                            price: l.price.to_string(),
                            size: l.size.to_string(),
                        })
                        .collect(),
                    asks: book
                        .asks
                        .iter()
                        .map(|l| IngestLevel {
                            price: l.price.to_string(),
                            size: l.size.to_string(),
                        })
                        .collect(),
                    observed_at: book.observed_at,
                    source: book.source.to_string(),
                })
                .collect(),
        };

        let resp = self.client.post(&url).json(&body).send().await.map_err(|error| {
            AppError::dependency_unavailable(
                "ORDERBOOK_HTTP_INGEST_ERROR",
                format!("failed to batch ingest orderbooks: {error}"),
            )
        })?;
        if !resp.status().is_success() {
            return Err(AppError::dependency_unavailable(
                "ORDERBOOK_HTTP_INGEST_FAILED",
                format!("orderbook batch ingest returned status {}", resp.status()),
            ));
        }
        Ok(())
    }

    async fn get_stale_tokens(&self, _token_ids: &[String], _max_age_ms: i64) -> Result<Vec<String>> {
        // Stale detection is handled internally by the orderbook service.
        Ok(Vec::new())
    }
}

// ── Register request types ──────────────────────────────────────────────────

#[derive(serde::Serialize)]
struct RegisterRequest {
    source: String,
    token_ids: Vec<String>,
}

// ── OrderbookSubscriptionRegistry implementation ────────────────────────────

#[async_trait]
impl OrderbookSubscriptionRegistry for OrderbookHttpClient {
    async fn register_tokens(&self, source: &str, token_ids: &[String]) {
        let url = format!("{}/orderbook/register", self.base_url);
        let body = RegisterRequest {
            source: source.to_string(),
            token_ids: token_ids.to_vec(),
        };
        match self.client.post(&url).json(&body).send().await {
            Ok(resp) if !resp.status().is_success() => {
                tracing::warn!(
                    source,
                    status = %resp.status(),
                    "orderbook register tokens returned non-success status"
                );
            }
            Err(error) => {
                tracing::warn!(
                    source,
                    error = %error,
                    "orderbook register tokens HTTP call failed"
                );
            }
            _ => {}
        }
    }

    async fn unregister_source(&self, source: &str) {
        let url = format!("{}/orderbook/register/{source}", self.base_url);
        match self.client.delete(&url).send().await {
            Ok(resp) if !resp.status().is_success() => {
                tracing::warn!(
                    source,
                    status = %resp.status(),
                    "orderbook unregister source returned non-success status"
                );
            }
            Err(error) => {
                tracing::warn!(
                    source,
                    error = %error,
                    "orderbook unregister source HTTP call failed"
                );
            }
            _ => {}
        }
    }

    async fn unregister_tokens(&self, _source: &str, _token_ids: &[String]) {
        // Not implemented — use unregister_source + re-register instead.
    }

    async fn list_all_tokens(&self) -> Vec<String> {
        // Token aggregation is handled by the orderbook service.
        // Remote consumers don't need the full list.
        Vec::new()
    }

    async fn changed_since(&self, _since: Instant) -> bool {
        // Remote client cannot track server-side registry changes.
        // Return false (conservative) — the orderbook stream uses direct token
        // set comparison instead of this method.
        false
    }
}
