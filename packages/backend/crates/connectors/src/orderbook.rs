use async_trait::async_trait;
use polyedge_application::{
    CachedBookLevel, CachedOrderBook, OrderbookCache, OrderbookSubscriptionRegistry,
};
use polyedge_domain::{AppError, Result};
use reqwest::Client;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::str::FromStr;
use std::time::{Duration, Instant};

/// HTTP client that implements `OrderbookCache` by calling the standalone
/// orderbook service. Used by API and Worker processes to read orderbook data.
pub struct OrderbookHttpClient {
    base_url: String,
    client: Client,
    write_token: Option<String>,
}

impl OrderbookHttpClient {
    pub fn new(base_url: &str, write_token: Option<&str>) -> Self {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .build()
            .expect("failed to build orderbook HTTP client");
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client,
            write_token: write_token
                .map(str::trim)
                .filter(|token| !token.is_empty())
                .map(ToString::to_string),
        }
    }

    fn authorize_write(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match self.write_token.as_deref() {
            Some(token) => request.header("x-polyedge-orderbook-token", token),
            None => request,
        }
    }

    async fn fetch_stats(&self) -> Result<OrderbookStatsResponse> {
        let url = format!("{}/orderbook/stats", self.base_url);
        let resp = self.client.get(&url).send().await.map_err(|error| {
            AppError::dependency_unavailable(
                "ORDERBOOK_HTTP_STATS_ERROR",
                format!("failed to fetch orderbook stats: {error}"),
            )
        })?;
        if !resp.status().is_success() {
            return Err(AppError::dependency_unavailable(
                "ORDERBOOK_HTTP_STATS_FAILED",
                format!("orderbook stats returned status {}", resp.status()),
            ));
        }
        resp.json().await.map_err(|error| {
            AppError::dependency_unavailable(
                "ORDERBOOK_HTTP_STATS_DECODE_ERROR",
                format!("failed to decode orderbook stats response: {error}"),
            )
        })
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
struct OrderbookStatsResponse {
    cache_entries: usize,
    registry_sources: usize,
    registry_total_tokens: usize,
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

        let resp = self
            .authorize_write(self.client.post(&url))
            .json(&body)
            .send()
            .await
            .map_err(|error| {
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

        let resp = self
            .authorize_write(self.client.post(&url))
            .json(&body)
            .send()
            .await
            .map_err(|error| {
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

    async fn get_stale_tokens(
        &self,
        _token_ids: &[String],
        _max_age_ms: i64,
    ) -> Result<Vec<String>> {
        // Stale detection is handled internally by the orderbook service.
        Ok(Vec::new())
    }

    async fn entry_count(&self) -> Result<usize> {
        Ok(self.fetch_stats().await?.cache_entries)
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
    async fn register_tokens(&self, source: &str, token_ids: &[String]) -> Result<()> {
        let url = format!("{}/orderbook/register", self.base_url);
        let body = RegisterRequest {
            source: source.to_string(),
            token_ids: token_ids.to_vec(),
        };
        let resp = self
            .authorize_write(self.client.post(&url))
            .json(&body)
            .send()
            .await
            .map_err(|error| {
                AppError::dependency_unavailable(
                    "ORDERBOOK_HTTP_REGISTER_ERROR",
                    format!("failed to register orderbook tokens for source {source}: {error}"),
                )
            })?;
        if !resp.status().is_success() {
            return Err(AppError::dependency_unavailable(
                "ORDERBOOK_HTTP_REGISTER_FAILED",
                format!(
                    "orderbook register returned status {} for source {source}",
                    resp.status()
                ),
            ));
        }
        Ok(())
    }

    async fn unregister_source(&self, source: &str) -> Result<()> {
        let url = format!("{}/orderbook/register/{source}", self.base_url);
        let resp = self
            .authorize_write(self.client.delete(&url))
            .send()
            .await
            .map_err(|error| {
                AppError::dependency_unavailable(
                    "ORDERBOOK_HTTP_UNREGISTER_ERROR",
                    format!("failed to unregister orderbook source {source}: {error}"),
                )
            })?;
        if !resp.status().is_success() {
            return Err(AppError::dependency_unavailable(
                "ORDERBOOK_HTTP_UNREGISTER_FAILED",
                format!(
                    "orderbook unregister returned status {} for source {source}",
                    resp.status()
                ),
            ));
        }
        Ok(())
    }

    async fn unregister_tokens(&self, _source: &str, _token_ids: &[String]) -> Result<()> {
        Err(AppError::invalid_input(
            "ORDERBOOK_HTTP_PARTIAL_UNREGISTER_UNSUPPORTED",
            "remote orderbook registry only supports atomic source replacement",
        ))
    }

    async fn list_all_tokens(&self) -> Vec<String> {
        // Token aggregation is handled by the orderbook service.
        // Remote consumers don't need the full list.
        Vec::new()
    }

    async fn total_token_count(&self) -> usize {
        match self.fetch_stats().await {
            Ok(stats) => stats.registry_total_tokens,
            Err(error) => {
                tracing::warn!(error = %error, "failed to fetch orderbook registry token count");
                0
            }
        }
    }

    async fn source_count(&self) -> usize {
        match self.fetch_stats().await {
            Ok(stats) => stats.registry_sources,
            Err(error) => {
                tracing::warn!(error = %error, "failed to fetch orderbook registry source count");
                0
            }
        }
    }

    async fn has_source(&self, _source: &str) -> bool {
        // Remote clients do not need source-level introspection.
        false
    }

    async fn changed_since(&self, _since: Instant) -> bool {
        // Remote client cannot track server-side registry changes.
        // Return false (conservative) — the orderbook stream uses direct token
        // set comparison instead of this method.
        false
    }
}
