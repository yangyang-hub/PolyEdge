use polyedge_application::{CachedOrderBook, OrderbookCache};
use tracing::debug;

// ---------------------------------------------------------------------------
// In-memory implementation with TTL and periodic cleanup
// ---------------------------------------------------------------------------

struct BookEntry {
    book: CachedOrderBook,
    expires_at_ms: i64,
}

pub struct InMemoryOrderbookCache {
    books: RwLock<HashMap<String, BookEntry>>,
    ttl_ms: i64,
    max_levels_per_side: usize,
}

impl InMemoryOrderbookCache {
    pub fn new(ttl_ms: u64, max_levels_per_side: usize) -> Self {
        Self {
            books: RwLock::new(HashMap::new()),
            ttl_ms: (ttl_ms.max(1_000)) as i64, // minimum 1 second
            max_levels_per_side: max_levels_per_side.max(1),
        }
    }

    /// Spawn a background task that periodically removes expired entries.
    pub fn spawn_cleanup(self: &Arc<Self>, interval: std::time::Duration) {
        let cache = Arc::clone(self);
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                let now = now_millis();
                let mut books = cache.books.write().await;
                let before = books.len();
                books.retain(|_, entry| entry.expires_at_ms > now);
                let dropped = before.saturating_sub(books.len());
                let remaining = books.len();
                drop(books);
                if dropped > 0 {
                    debug!(
                        dropped,
                        remaining, "orderbook cache cleanup removed expired entries"
                    );
                }
            }
        });
    }

    fn bounded_book(&self, book: &CachedOrderBook) -> CachedOrderBook {
        let mut bounded = book.clone();
        // Keep the BEST levels regardless of the writer's ordering: bids by
        // descending price, asks by ascending price. Sorting here — the single
        // choke-point every writer (WS, poll, ingest) passes through — guarantees
        // the depth trim never discards top-of-book.
        bounded.bids.sort_by(|a, b| b.price.cmp(&a.price));
        bounded.asks.sort_by(|a, b| a.price.cmp(&b.price));
        bounded.bids.truncate(self.max_levels_per_side);
        bounded.asks.truncate(self.max_levels_per_side);
        bounded
    }

    fn rejects_replacement(current: &CachedOrderBook, replacement: &CachedOrderBook) -> bool {
        current.observed_at > replacement.observed_at
            || (current.observed_at == replacement.observed_at
                && current.source == polyedge_application::BookSource::Ws
                && replacement.source == polyedge_application::BookSource::Poll)
    }

    fn merge_confirmation_if_newer(
        entry: &mut BookEntry,
        replacement: &CachedOrderBook,
        expires_at_ms: i64,
    ) -> bool {
        if entry.book.bids != replacement.bids || entry.book.asks != replacement.asks {
            return false;
        }
        let replacement_confirmed_at = replacement.confirmation_time_ms();
        if replacement_confirmed_at > entry.book.confirmation_time_ms() {
            entry.book.confirmed_at = replacement_confirmed_at;
            entry.expires_at_ms = expires_at_ms;
            return true;
        }
        false
    }
}

#[async_trait]
impl OrderbookCache for InMemoryOrderbookCache {
    async fn get_book(&self, token_id: &str) -> Result<Option<CachedOrderBook>> {
        let books = self.books.read().await;
        let now = now_millis();
        Ok(books
            .get(token_id)
            .filter(|entry| entry.expires_at_ms > now)
            .map(|entry| entry.book.clone()))
    }

    async fn get_books(&self, token_ids: &[String]) -> Result<Vec<CachedOrderBook>> {
        let books = self.books.read().await;
        let now = now_millis();
        Ok(token_ids
            .iter()
            .filter_map(|token_id| {
                books
                    .get(token_id)
                    .filter(|entry| entry.expires_at_ms > now)
                    .map(|entry| entry.book.clone())
            })
            .collect())
    }

    async fn set_book(&self, book: &CachedOrderBook) -> Result<()> {
        let book = self.bounded_book(book);
        let mut books = self.books.write().await;
        let now = now_millis();
        let expires_at_ms = now + self.ttl_ms;
        if let Some(entry) = books.get_mut(&book.token_id) {
            if entry.expires_at_ms > now && Self::rejects_replacement(&entry.book, &book) {
                Self::merge_confirmation_if_newer(entry, &book, expires_at_ms);
                return Ok(());
            }
        }
        books.insert(
            book.token_id.clone(),
            BookEntry {
                book,
                expires_at_ms,
            },
        );
        Ok(())
    }

    async fn set_books(&self, books_slice: &[CachedOrderBook]) -> Result<()> {
        let mut books = self.books.write().await;
        let now = now_millis();
        let expires_at_ms = now + self.ttl_ms;
        for book in books_slice {
            let book = self.bounded_book(book);
            if let Some(entry) = books.get_mut(&book.token_id) {
                if entry.expires_at_ms > now && Self::rejects_replacement(&entry.book, &book) {
                    Self::merge_confirmation_if_newer(entry, &book, expires_at_ms);
                    continue;
                }
            }
            books.insert(
                book.token_id.clone(),
                BookEntry {
                    book,
                    expires_at_ms,
                },
            );
        }
        Ok(())
    }

    async fn get_stale_tokens(&self, token_ids: &[String], max_age_ms: i64) -> Result<Vec<String>> {
        let books = self.books.read().await;
        let now = now_millis();
        let mut stale = Vec::new();
        for token_id in token_ids {
            let is_stale = match books.get(token_id) {
                Some(entry) => {
                    // A non-positive max_age_ms disables the age-based check (only
                    // TTL expiry counts); otherwise a 0 threshold would mark every
                    // cached book stale on every poll and refetch the whole set.
                    entry.expires_at_ms <= now
                        || (max_age_ms > 0
                            && now - entry.book.confirmation_time_ms() > max_age_ms)
                }
                None => true,
            };
            if is_stale {
                stale.push(token_id.clone());
            }
        }
        Ok(stale)
    }

    async fn replace_book(&self, book: &CachedOrderBook) -> Result<bool> {
        let book = self.bounded_book(book);
        let mut books = self.books.write().await;
        let now = now_millis();
        let Some(entry) = books.get_mut(&book.token_id) else {
            return Ok(false);
        };
        if entry.expires_at_ms <= now {
            return Ok(false);
        }
        if Self::rejects_replacement(&entry.book, &book) {
            Self::merge_confirmation_if_newer(entry, &book, now + self.ttl_ms);
            return Ok(false);
        }
        entry.book = book;
        entry.expires_at_ms = now + self.ttl_ms;
        Ok(true)
    }

    async fn entry_count(&self) -> Result<usize> {
        let books = self.books.read().await;
        Ok(books.len())
    }

    async fn confirm_book_version(
        &self,
        token_id: &str,
        expected_observed_at: i64,
        confirmed_at: i64,
    ) -> Result<bool> {
        let mut books = self.books.write().await;
        let now = now_millis();
        let Some(entry) = books.get_mut(token_id) else {
            return Ok(false);
        };
        if entry.expires_at_ms <= now || entry.book.observed_at != expected_observed_at {
            return Ok(false);
        }
        if confirmed_at > entry.book.confirmation_time_ms() {
            entry.book.confirmed_at = confirmed_at;
            entry.expires_at_ms = now + self.ttl_ms;
            return Ok(true);
        }
        Ok(false)
    }
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use polyedge_application::{BookSource, CachedBookLevel};
    use rust_decimal::Decimal;

    fn level(price: i64, size: i64) -> CachedBookLevel {
        CachedBookLevel {
            price: Decimal::new(price, 2),
            size: Decimal::new(size, 0),
        }
    }

    fn book(observed_at: i64, confirmed_at: i64, source: BookSource, bid_size: i64) -> CachedOrderBook {
        CachedOrderBook {
            token_id: "1".to_string(),
            bids: vec![level(40, bid_size)],
            asks: vec![level(60, 1)],
            observed_at,
            confirmed_at,
            source,
        }
    }

    #[tokio::test]
    async fn stale_divergent_poll_does_not_advance_confirmation() {
        let cache = InMemoryOrderbookCache::new(60_000, 10);
        cache.set_book(&book(200, 300, BookSource::Ws, 1)).await.unwrap();
        cache.set_book(&book(100, 400, BookSource::Poll, 2)).await.unwrap();
        let current = cache.get_book("1").await.unwrap().unwrap();
        assert_eq!(current.confirmed_at, 300);
        assert_eq!(current.bids[0].size, Decimal::ONE);
    }

    #[tokio::test]
    async fn explicit_confirmation_is_version_fenced() {
        let cache = InMemoryOrderbookCache::new(60_000, 10);
        cache.set_book(&book(200, 300, BookSource::Ws, 1)).await.unwrap();
        assert!(!cache.confirm_book_version("1", 199, 400).await.unwrap());
        assert!(cache.confirm_book_version("1", 200, 400).await.unwrap());
        assert_eq!(cache.get_book("1").await.unwrap().unwrap().confirmed_at, 400);
    }
}
