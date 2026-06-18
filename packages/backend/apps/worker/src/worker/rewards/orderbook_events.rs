const REWARD_ORDERBOOK_ACTIVE_TOKEN_REFRESH: Duration = Duration::from_secs(5);
const REWARD_ORDERBOOK_MIN_IDLE_TIMEOUT: Duration = Duration::from_secs(5);

struct RewardOrderbookRuntime {
    cache: Arc<RewardOrderbookLocalCache>,
    handle: JoinHandle<()>,
}

impl RewardOrderbookRuntime {
    fn spawn(state: &AppState) -> Self {
        let cache = Arc::new(RewardOrderbookLocalCache::new(
            state.settings.orderbook_stream.max_levels_per_side,
            state.settings.orderbook_stream.book_ttl_ms,
        ));
        let task_cache = Arc::clone(&cache);
        let state = state.clone();
        let handle = tokio::spawn(async move {
            consume_reward_orderbook_stream(state, task_cache).await;
        });
        Self { cache, handle }
    }

    fn cache(&self) -> &RewardOrderbookLocalCache {
        self.cache.as_ref()
    }

    fn subscribe(&self) -> watch::Receiver<u64> {
        self.cache.subscribe()
    }
}

impl Drop for RewardOrderbookRuntime {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

struct RewardOrderbookLocalCache {
    books: RwLock<HashMap<String, RewardOrderbookLocalEntry>>,
    wake_tx: watch::Sender<u64>,
    max_levels_per_side: usize,
    ttl_ms: i64,
}

struct RewardOrderbookLocalEntry {
    book: CachedOrderBook,
    expires_at_ms: i64,
}

impl RewardOrderbookLocalCache {
    fn new(max_levels_per_side: usize, ttl_ms: u64) -> Self {
        let (wake_tx, _) = watch::channel(0);
        Self {
            books: RwLock::new(HashMap::new()),
            wake_tx,
            max_levels_per_side: max_levels_per_side.max(1),
            ttl_ms: ttl_ms.max(1_000) as i64,
        }
    }

    fn subscribe(&self) -> watch::Receiver<u64> {
        self.wake_tx.subscribe()
    }

    async fn get_books(&self, token_ids: &[String]) -> Vec<CachedOrderBook> {
        let books = self.books.read().await;
        let now = reward_orderbook_now_millis();
        token_ids
            .iter()
            .filter_map(|token_id| {
                books
                    .get(token_id)
                    .filter(|entry| entry.expires_at_ms > now)
                    .map(|entry| entry.book.clone())
            })
            .collect()
    }

    async fn apply_book(&self, book: CachedOrderBook) -> bool {
        let book = self.bounded_book(book);
        let mut books = self.books.write().await;
        let now = reward_orderbook_now_millis();
        books.retain(|_, current| current.expires_at_ms > now);
        if books
            .get(&book.token_id)
            .is_some_and(|entry| Self::rejects_replacement(&entry.book, &book))
        {
            return false;
        }
        books.insert(
            book.token_id.clone(),
            RewardOrderbookLocalEntry {
                book,
                expires_at_ms: now + self.ttl_ms,
            },
        );
        true
    }

    async fn apply_books(&self, books: Vec<CachedOrderBook>) -> usize {
        let mut accepted = 0usize;
        for book in books {
            if self.apply_book(book).await {
                accepted += 1;
            }
        }
        accepted
    }

    fn notify_active_book_update(&self) {
        let next = self.wake_tx.borrow().wrapping_add(1);
        let _ = self.wake_tx.send(next);
    }

    fn bounded_book(&self, mut book: CachedOrderBook) -> CachedOrderBook {
        book.bids.sort_by(|a, b| b.price.cmp(&a.price));
        book.asks.sort_by(|a, b| a.price.cmp(&b.price));
        book.bids.truncate(self.max_levels_per_side);
        book.asks.truncate(self.max_levels_per_side);
        book
    }

    fn rejects_replacement(current: &CachedOrderBook, replacement: &CachedOrderBook) -> bool {
        current.observed_at > replacement.observed_at
            || (current.observed_at == replacement.observed_at
                && current.source == BookSource::Ws
                && replacement.source == BookSource::Poll)
    }
}

fn reward_orderbook_now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

async fn consume_reward_orderbook_stream(
    state: AppState,
    cache: Arc<RewardOrderbookLocalCache>,
) {
    let client = OrderbookStreamClient::new(&state.settings.orderbook.service_url);
    let mut active_tokens = HashSet::new();
    let mut last_active_refresh = Instant::now() - REWARD_ORDERBOOK_ACTIVE_TOKEN_REFRESH;
    let reconnect_delay =
        Duration::from_secs(state.settings.orderbook_stream.restart_interval_secs.max(1));
    let idle_timeout = reward_orderbook_stream_idle_timeout(&state);

    loop {
        if let Err(error) = bootstrap_reward_orderbook_cache(&state, &cache).await {
            warn!(error = %error, "failed to bootstrap reward orderbook local cache");
        }

        match client.connect().await {
            Ok(mut connection) => {
                info!(
                    stream_url = client.stream_url(),
                    "connected to orderbook internal stream"
                );
                loop {
                    refresh_reward_orderbook_active_tokens(
                        &state,
                        &mut active_tokens,
                        &mut last_active_refresh,
                    )
                    .await;

                    let next_event =
                        tokio::time::timeout(idle_timeout, connection.next_event()).await;
                    let event = match next_event {
                        Err(_) => {
                            warn!(
                                idle_timeout_ms = idle_timeout.as_millis() as u64,
                                "orderbook internal stream idle timeout; reconnecting"
                            );
                            break;
                        }
                        Ok(Ok(Some(event))) => event,
                        Ok(Ok(None)) => {
                            info!("orderbook internal stream closed by server");
                            break;
                        }
                        Ok(Err(error)) => {
                            warn!(error = %error, "orderbook internal stream receive failed");
                            break;
                        }
                    };
                    refresh_reward_orderbook_active_tokens(
                        &state,
                        &mut active_tokens,
                        &mut last_active_refresh,
                    )
                    .await;
                    let token_id = event.book.token_id.clone();
                    let accepted = cache.apply_book(event.book).await;
                    if accepted && active_tokens.contains(&token_id) {
                        cache.notify_active_book_update();
                    }
                }
            }
            Err(error) => {
                warn!(
                    stream_url = client.stream_url(),
                    error = %error,
                    "failed to connect orderbook internal stream"
                );
            }
        }

        tokio::time::sleep(reconnect_delay).await;
    }
}

fn reward_orderbook_stream_idle_timeout(state: &AppState) -> Duration {
    let fallback_ms = state
        .settings
        .orderbook_stream
        .poll_reconcile_interval_secs
        .max(1)
        .saturating_mul(3)
        .saturating_mul(1_000);
    let ttl_ms = state.settings.orderbook_stream.book_ttl_ms.max(1_000);
    Duration::from_millis(fallback_ms.min(ttl_ms)).max(REWARD_ORDERBOOK_MIN_IDLE_TIMEOUT)
}

async fn refresh_reward_orderbook_active_tokens(
    state: &AppState,
    active_tokens: &mut HashSet<String>,
    last_refresh: &mut Instant,
) {
    if last_refresh.elapsed() < REWARD_ORDERBOOK_ACTIVE_TOKEN_REFRESH {
        return;
    }
    *last_refresh = Instant::now();
    match state
        .reward_bot_service
        .list_active_reward_book_token_ids()
        .await
    {
        Ok(tokens) => {
            active_tokens.clear();
            active_tokens.extend(tokens);
        }
        Err(error) => {
            warn!(error = %error, "failed to refresh reward active orderbook tokens");
        }
    }
}

async fn bootstrap_reward_orderbook_cache(
    state: &AppState,
    cache: &RewardOrderbookLocalCache,
) -> Result<()> {
    let token_ids = reward_orderbook_bootstrap_tokens(state).await?;
    let books = fetch_remote_cached_orderbooks(state, &token_ids).await?;
    let accepted = cache.apply_books(books).await;
    if accepted > 0 {
        debug!(accepted, "bootstrapped reward orderbook local cache");
    }
    Ok(())
}

async fn reward_orderbook_bootstrap_tokens(state: &AppState) -> Result<Vec<String>> {
    let max_tokens = state.settings.orderbook_stream.max_tokens;
    let reward_candidate_token_cap = state.settings.orderbook_stream.reward_candidate_token_cap;
    if max_tokens == 0 {
        return Ok(Vec::new());
    }
    let mut seen = HashSet::new();
    let mut token_ids = Vec::with_capacity(max_tokens);

    let active = state
        .reward_bot_service
        .list_active_reward_book_token_ids()
        .await?;
    push_reward_orderbook_tokens(&mut token_ids, &mut seen, active, max_tokens);

    if token_ids.len() < max_tokens {
        let eligible = state
            .reward_bot_service
            .list_eligible_reward_book_token_ids()
            .await?;
        push_reward_orderbook_tokens(&mut token_ids, &mut seen, eligible, max_tokens);
    }

    if token_ids.len() < max_tokens && reward_candidate_token_cap > 0 {
        let candidates = state
            .reward_bot_service
            .list_all_reward_candidate_token_ids()
            .await?;
        let candidate_limit = token_ids.len()
            + max_tokens
                .saturating_sub(token_ids.len())
                .min(reward_candidate_token_cap);
        push_reward_orderbook_tokens(&mut token_ids, &mut seen, candidates, candidate_limit);
    }

    Ok(token_ids)
}

fn push_reward_orderbook_tokens(
    token_ids: &mut Vec<String>,
    seen: &mut HashSet<String>,
    candidates: Vec<String>,
    max_tokens: usize,
) {
    for token_id in candidates {
        if token_ids.len() >= max_tokens {
            break;
        }
        if seen.insert(token_id.clone()) {
            token_ids.push(token_id);
        }
    }
}

async fn fetch_remote_cached_orderbooks(
    state: &AppState,
    token_ids: &[String],
) -> Result<Vec<CachedOrderBook>> {
    let batch_size = state.settings.orderbook_stream.max_tokens;
    if batch_size == 0 || token_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut books = Vec::new();
    for chunk in token_ids.chunks(batch_size) {
        books.extend(state.orderbook_cache.get_books(chunk).await?);
    }
    Ok(books)
}

#[cfg(test)]
mod reward_orderbook_local_cache_tests {
    use super::*;
    use polyedge_application::CachedBookLevel;
    use rust_decimal::Decimal;

    #[tokio::test]
    async fn local_cache_ttl_uses_receive_time_not_future_observed_at() {
        let cache = RewardOrderbookLocalCache::new(10, 1_000);
        let token_id = "123".to_string();
        let future_observed_at = reward_orderbook_now_millis() + 60_000;

        assert!(
            cache
                .apply_book(CachedOrderBook {
                    token_id: token_id.clone(),
                    bids: vec![CachedBookLevel {
                        price: Decimal::new(50, 2),
                        size: Decimal::from(10_u64),
                    }],
                    asks: Vec::new(),
                    observed_at: future_observed_at,
                    source: BookSource::Poll,
                })
                .await
        );

        assert_eq!(cache.get_books(std::slice::from_ref(&token_id)).await.len(), 1);
        tokio::time::sleep(Duration::from_millis(1_100)).await;
        assert!(cache.get_books(&[token_id]).await.is_empty());
    }
}
