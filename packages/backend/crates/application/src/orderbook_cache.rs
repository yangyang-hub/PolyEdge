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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    #[serde(default)]
    pub confirmed_at: i64,
    pub source: BookSource,
}

impl CachedOrderBook {
    pub fn confirmation_time_ms(&self) -> i64 {
        if self.confirmed_at > 0 {
            self.confirmed_at
        } else {
            self.observed_at
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderbookStreamReason {
    Book,
    PriceChange,
    PollReconcile,
    Ingest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderbookStreamEvent {
    pub sequence: u64,
    pub reason: OrderbookStreamReason,
    pub book: CachedOrderBook,
}

#[async_trait]
pub trait OrderbookCache: Send + Sync {
    async fn get_book(&self, token_id: &str) -> Result<Option<CachedOrderBook>>;
    async fn get_books(&self, token_ids: &[String]) -> Result<Vec<CachedOrderBook>> {
        let mut books = Vec::new();
        for token_id in token_ids {
            if let Some(book) = self.get_book(token_id).await? {
                books.push(book);
            }
        }
        Ok(books)
    }
    async fn get_books_with_max_age(
        &self,
        token_ids: &[String],
        _max_age_ms: i64,
    ) -> Result<Vec<CachedOrderBook>> {
        self.get_books(token_ids).await
    }
    async fn set_book(&self, book: &CachedOrderBook) -> Result<()>;
    async fn set_books(&self, books: &[CachedOrderBook]) -> Result<()>;
    async fn get_stale_tokens(&self, token_ids: &[String], max_age_ms: i64) -> Result<Vec<String>>;
    async fn entry_count(&self) -> Result<usize>;
    /// Advance confirmation for the exact cached content version. This is used
    /// by poll reconciliation after it has independently checked compatibility;
    /// it never replaces levels or confirms a version that changed concurrently.
    async fn confirm_book_version(
        &self,
        _token_id: &str,
        _expected_observed_at: i64,
        _confirmed_at: i64,
    ) -> Result<bool> {
        Ok(false)
    }

    /// Replace a cached book only if it exists and the new content is not older
    /// than the current cached value. Implementations may merge a newer
    /// confirmation time from an otherwise rejected replacement. Returns `true`
    /// only if the book content was replaced.
    async fn replace_book(&self, book: &CachedOrderBook) -> Result<bool> {
        let existed = self.get_book(&book.token_id).await?.is_some();
        if !existed {
            return Ok(false);
        }
        self.set_book(book).await?;
        Ok(true)
    }
}
