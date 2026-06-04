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

    async fn set_book(&self, book: &CachedOrderBook) -> Result<()> {
        let book = self.bounded_book(book);
        let mut books = self.books.write().await;
        books.insert(
            book.token_id.clone(),
            BookEntry {
                book,
                expires_at_ms: now_millis() + self.ttl_ms,
            },
        );
        Ok(())
    }

    async fn set_books(&self, books_slice: &[CachedOrderBook]) -> Result<()> {
        let mut books = self.books.write().await;
        let expires_at_ms = now_millis() + self.ttl_ms;
        for book in books_slice {
            let book = self.bounded_book(book);
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
                        || (max_age_ms > 0 && now - entry.book.observed_at > max_age_ms)
                }
                None => true,
            };
            if is_stale {
                stale.push(token_id.clone());
            }
        }
        Ok(stale)
    }

    async fn entry_count(&self) -> Result<usize> {
        let books = self.books.read().await;
        Ok(books.len())
    }
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
