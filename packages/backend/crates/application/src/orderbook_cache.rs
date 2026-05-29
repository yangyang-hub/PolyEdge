use async_trait::async_trait;
use polyedge_domain::Result;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BookSource {
    Ws,
    Poll,
}

impl std::fmt::Display for BookSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BookSource::Ws => write!(f, "ws"),
            BookSource::Poll => write!(f, "poll"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedBookLevel {
    pub price: Decimal,
    pub size: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedOrderBook {
    pub token_id: String,
    pub bids: Vec<CachedBookLevel>,
    pub asks: Vec<CachedBookLevel>,
    pub observed_at: i64,
    pub source: BookSource,
}

#[async_trait]
pub trait OrderbookCache: Send + Sync {
    async fn get_book(&self, token_id: &str) -> Result<Option<CachedOrderBook>>;
    async fn set_book(&self, book: &CachedOrderBook) -> Result<()>;
    async fn set_books(&self, books: &[CachedOrderBook]) -> Result<()>;
    async fn get_stale_tokens(&self, token_ids: &[String], max_age_ms: i64) -> Result<Vec<String>>;
}
