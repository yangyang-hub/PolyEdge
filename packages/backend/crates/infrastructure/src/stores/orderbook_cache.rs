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
}

impl InMemoryOrderbookCache {
    pub fn new(ttl_ms: u64) -> Self {
        Self {
            books: RwLock::new(HashMap::new()),
            ttl_ms: (ttl_ms.max(1_000)) as i64, // minimum 1 second
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
                    debug!(dropped, remaining, "orderbook cache cleanup removed expired entries");
                }
            }
        });
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
        let mut books = self.books.write().await;
        books.insert(
            book.token_id.clone(),
            BookEntry {
                book: book.clone(),
                expires_at_ms: now_millis() + self.ttl_ms,
            },
        );
        Ok(())
    }

    async fn set_books(&self, books_slice: &[CachedOrderBook]) -> Result<()> {
        let mut books = self.books.write().await;
        let expires_at_ms = now_millis() + self.ttl_ms;
        for book in books_slice {
            books.insert(
                book.token_id.clone(),
                BookEntry {
                    book: book.clone(),
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
                    entry.expires_at_ms <= now || now - entry.book.observed_at > max_age_ms
                }
                None => true,
            };
            if is_stale {
                stale.push(token_id.clone());
            }
        }
        Ok(stale)
    }
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
