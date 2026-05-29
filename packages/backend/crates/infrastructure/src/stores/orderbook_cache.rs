use polyedge_application::{BookSource, CachedBookLevel, CachedOrderBook, OrderbookCache};

fn cache_error(context: impl Into<String>) -> AppError {
    AppError::dependency_unavailable("ORDERBOOK_CACHE_ERROR", context.into())
}

// ---------------------------------------------------------------------------
// Redis-backed implementation
// ---------------------------------------------------------------------------

pub struct RedisOrderbookCache {
    client: redis::Client,
    ttl_secs: u64,
}

impl RedisOrderbookCache {
    pub fn new(client: redis::Client, ttl_secs: u64) -> Self {
        Self { client, ttl_secs }
    }

    fn redis_key(token_id: &str) -> String {
        format!("ob:{token_id}")
    }
}

#[async_trait]
impl OrderbookCache for RedisOrderbookCache {
    async fn get_book(&self, token_id: &str) -> Result<Option<CachedOrderBook>> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| cache_error(format!("redis connection failed: {e}")))?;

        let key = Self::redis_key(token_id);
        let result: HashMap<String, String> = redis::cmd("HGETALL")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .map_err(|e| cache_error(format!("redis HGETALL failed for {key}: {e}")))?;

        if result.is_empty() {
            return Ok(None);
        }

        let bids: Vec<CachedBookLevel> = serde_json::from_str(
            result.get("bids").map(String::as_str).unwrap_or("[]"),
        )
        .map_err(|e| cache_error(format!("failed to deserialize bids for {key}: {e}")))?;

        let asks: Vec<CachedBookLevel> = serde_json::from_str(
            result.get("asks").map(String::as_str).unwrap_or("[]"),
        )
        .map_err(|e| cache_error(format!("failed to deserialize asks for {key}: {e}")))?;

        let observed_at: i64 = result
            .get("ts")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        let source = match result.get("source").map(String::as_str).unwrap_or("ws") {
            "poll" => BookSource::Poll,
            _ => BookSource::Ws,
        };

        Ok(Some(CachedOrderBook {
            token_id: token_id.to_string(),
            bids,
            asks,
            observed_at,
            source,
        }))
    }

    async fn set_book(&self, book: &CachedOrderBook) -> Result<()> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| cache_error(format!("redis connection failed: {e}")))?;

        let key = Self::redis_key(&book.token_id);
        let bids_json = serde_json::to_string(&book.bids)
            .map_err(|e| cache_error(format!("failed to serialize bids: {e}")))?;
        let asks_json = serde_json::to_string(&book.asks)
            .map_err(|e| cache_error(format!("failed to serialize asks: {e}")))?;

        redis::cmd("HSET")
            .arg(&key)
            .arg("bids")
            .arg(&bids_json)
            .arg("asks")
            .arg(&asks_json)
            .arg("ts")
            .arg(book.observed_at.to_string())
            .arg("source")
            .arg(book.source.to_string())
            .query_async::<i64>(&mut conn)
            .await
            .map_err(|e| cache_error(format!("redis HSET failed for {key}: {e}")))?;

        redis::cmd("EXPIRE")
            .arg(&key)
            .arg(self.ttl_secs)
            .query_async::<i64>(&mut conn)
            .await
            .map_err(|e| cache_error(format!("redis EXPIRE failed for {key}: {e}")))?;

        Ok(())
    }

    async fn set_books(&self, books: &[CachedOrderBook]) -> Result<()> {
        if books.is_empty() {
            return Ok(());
        }

        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| cache_error(format!("redis connection failed: {e}")))?;

        let mut pipe = redis::pipe();
        for book in books {
            let key = Self::redis_key(&book.token_id);
            let bids_json = serde_json::to_string(&book.bids)
                .map_err(|e| cache_error(format!("failed to serialize bids: {e}")))?;
            let asks_json = serde_json::to_string(&book.asks)
                .map_err(|e| cache_error(format!("failed to serialize asks: {e}")))?;

            pipe.cmd("HSET")
                .arg(&key)
                .arg("bids")
                .arg(&bids_json)
                .arg("asks")
                .arg(&asks_json)
                .arg("ts")
                .arg(book.observed_at.to_string())
                .arg("source")
                .arg(book.source.to_string())
                .ignore();

            pipe.cmd("EXPIRE")
                .arg(&key)
                .arg(self.ttl_secs)
                .ignore();
        }

        pipe.query_async::<()>(&mut conn)
            .await
            .map_err(|e| cache_error(format!("redis pipeline failed: {e}")))?;

        Ok(())
    }

    async fn get_stale_tokens(&self, token_ids: &[String], max_age_ms: i64) -> Result<Vec<String>> {
        if token_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| cache_error(format!("redis connection failed: {e}")))?;

        let now_ms = now_millis();

        let mut pipe = redis::pipe();
        for token_id in token_ids {
            pipe.cmd("HGET")
                .arg(Self::redis_key(token_id))
                .arg("ts")
                .ignore();
        }

        let results: Vec<Option<String>> = pipe
            .query_async(&mut conn)
            .await
            .map_err(|e| cache_error(format!("redis pipeline HGET ts failed: {e}")))?;

        let mut stale = Vec::new();
        for (token_id, ts_opt) in token_ids.iter().zip(results.into_iter()) {
            let is_stale = match ts_opt {
                Some(ts_str) => {
                    let ts: i64 = ts_str.parse().unwrap_or(0);
                    now_ms - ts > max_age_ms
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

// ---------------------------------------------------------------------------
// In-memory implementation (for tests and no-Redis environments)
// ---------------------------------------------------------------------------

pub struct InMemoryOrderbookCache {
    books: RwLock<HashMap<String, CachedOrderBook>>,
}

impl InMemoryOrderbookCache {
    pub fn new() -> Self {
        Self {
            books: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl OrderbookCache for InMemoryOrderbookCache {
    async fn get_book(&self, token_id: &str) -> Result<Option<CachedOrderBook>> {
        let books = self.books.read().await;
        Ok(books.get(token_id).cloned())
    }

    async fn set_book(&self, book: &CachedOrderBook) -> Result<()> {
        let mut books = self.books.write().await;
        books.insert(book.token_id.clone(), book.clone());
        Ok(())
    }

    async fn set_books(&self, books_slice: &[CachedOrderBook]) -> Result<()> {
        let mut books = self.books.write().await;
        for book in books_slice {
            books.insert(book.token_id.clone(), book.clone());
        }
        Ok(())
    }

    async fn get_stale_tokens(&self, token_ids: &[String], max_age_ms: i64) -> Result<Vec<String>> {
        let books = self.books.read().await;
        let now_ms = now_millis();
        let mut stale = Vec::new();
        for token_id in token_ids {
            let is_stale = match books.get(token_id) {
                Some(book) => now_ms - book.observed_at > max_age_ms,
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
