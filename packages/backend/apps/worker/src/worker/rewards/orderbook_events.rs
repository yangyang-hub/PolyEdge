const REWARD_ORDERBOOK_ACTIVE_TOKEN_REFRESH: Duration = Duration::from_secs(5);
const REWARD_ORDERBOOK_MIN_IDLE_TIMEOUT: Duration = Duration::from_secs(5);

struct RewardOrderbookRuntime {
    cache: Arc<RewardOrderbookLocalCache>,
    handle: JoinHandle<()>,
    batch_handle: JoinHandle<()>,
    prewarm_handle: JoinHandle<()>,
}

impl RewardOrderbookRuntime {
    fn spawn(state: &AppState) -> Self {
        let (cache, ready_rx) = RewardOrderbookLocalCache::new(
            state.settings.orderbook_stream.max_levels_per_side,
            state.settings.orderbook_stream.book_ttl_ms,
        );
        let cache = Arc::new(cache);
        let stream_state = state.clone();
        let stream_cache = Arc::clone(&cache);
        let handle = tokio::spawn(async move {
            consume_reward_orderbook_stream(stream_state, stream_cache).await;
        });
        let batch_state = state.clone();
        let batch_cache = Arc::clone(&cache);
        let batch_handle = tokio::spawn(async move {
            run_reward_ai_advisory_batch_worker(batch_state, batch_cache, ready_rx).await;
        });
        let prewarm_state = state.clone();
        let prewarm_cache = Arc::clone(&cache);
        let prewarm_handle = tokio::spawn(async move {
            run_reward_managed_orderbook_cache_prewarm(prewarm_state, prewarm_cache).await;
        });
        Self {
            cache,
            handle,
            batch_handle,
            prewarm_handle,
        }
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
        self.batch_handle.abort();
        self.prewarm_handle.abort();
    }
}

struct RewardOrderbookLocalCache {
    books: RwLock<HashMap<String, RewardOrderbookLocalEntry>>,
    wake_tx: watch::Sender<u64>,
    max_levels_per_side: usize,
    ttl_ms: i64,
    condition_tokens: RwLock<HashMap<String, Vec<String>>>,
    token_to_condition: RwLock<HashMap<String, String>>,
    notified_ready: RwLock<HashSet<String>>,
    ready_tx: tokio::sync::mpsc::Sender<String>,
}

struct RewardOrderbookLocalEntry {
    book: CachedOrderBook,
    expires_at_ms: i64,
}

const REWARD_ORDERBOOK_CONDITION_TOKEN_REFRESH: Duration = Duration::from_secs(5);
const REWARD_ORDERBOOK_READY_CHANNEL_CAPACITY: usize = 256;

impl RewardOrderbookLocalCache {
    fn new(
        max_levels_per_side: usize,
        ttl_ms: u64,
    ) -> (Self, tokio::sync::mpsc::Receiver<String>) {
        let (wake_tx, _) = watch::channel(0);
        let (ready_tx, ready_rx) =
            tokio::sync::mpsc::channel(REWARD_ORDERBOOK_READY_CHANNEL_CAPACITY);
        let cache = Self {
            books: RwLock::new(HashMap::new()),
            wake_tx,
            max_levels_per_side: max_levels_per_side.max(1),
            ttl_ms: ttl_ms.max(1_000) as i64,
            condition_tokens: RwLock::new(HashMap::new()),
            token_to_condition: RwLock::new(HashMap::new()),
            notified_ready: RwLock::new(HashSet::new()),
            ready_tx,
        };
        (cache, ready_rx)
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

    /// Returns the condition_id the first time every one of its tokens has a
    /// populated, non-expired book in the local cache. Subsequent triggers for
    /// the same condition are suppressed until [`Self::clear_notified_ready`]
    /// drops the marker (after the batch worker consumes it), so an advisory
    /// TTL expiry can re-trigger evaluation.
    async fn check_condition_readiness(&self, token_id: &str) -> Option<String> {
        let condition_id = {
            let token_map = self.token_to_condition.read().await;
            token_map.get(token_id)?.clone()
        };
        let tokens = {
            let condition_map = self.condition_tokens.read().await;
            condition_map.get(&condition_id)?.clone()
        };
        if tokens.is_empty() {
            return None;
        }
        let now = reward_orderbook_now_millis();
        let all_ready = {
            let books = self.books.read().await;
            tokens.iter().all(|tid| {
                books.get(tid).is_some_and(|entry| {
                    entry.expires_at_ms > now
                        && !entry.book.bids.is_empty()
                        && !entry.book.asks.is_empty()
                })
            })
        };
        if !all_ready {
            return None;
        }
        let mut notified = self.notified_ready.write().await;
        if notified.insert(condition_id.clone()) {
            Some(condition_id)
        } else {
            None
        }
    }

    fn notify_condition_ready(&self, condition_id: &str) {
        let _ = self.ready_tx.try_send(condition_id.to_string());
    }

    /// Atomically replace the condition<->token maps and drop notified markers
    /// for conditions that are no longer candidates (bounds memory growth).
    /// Markers for conditions that remain candidates are preserved so the same
    /// refresh cycle does not re-enqueue them.
    async fn replace_condition_tokens(
        &self,
        condition_tokens: HashMap<String, Vec<String>>,
        token_to_condition: HashMap<String, String>,
        active_condition_ids: &HashSet<String>,
    ) {
        {
            let mut map = self.condition_tokens.write().await;
            *map = condition_tokens;
        }
        {
            let mut map = self.token_to_condition.write().await;
            *map = token_to_condition;
        }
        let mut notified = self.notified_ready.write().await;
        notified.retain(|condition_id| active_condition_ids.contains(condition_id));
    }

    async fn clear_notified_ready(&self, condition_ids: &[String]) {
        let mut notified = self.notified_ready.write().await;
        for condition_id in condition_ids {
            notified.remove(condition_id);
        }
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
    let mut condition_tokens_last_refresh =
        Instant::now() - REWARD_ORDERBOOK_CONDITION_TOKEN_REFRESH;
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
                    refresh_reward_condition_tokens(&state, &cache, &mut condition_tokens_last_refresh)
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
                    if accepted {
                        if active_tokens.contains(&token_id) {
                            cache.notify_active_book_update();
                        }
                        if let Some(condition_id) =
                            cache.check_condition_readiness(&token_id).await
                        {
                            cache.notify_condition_ready(&condition_id);
                        }
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

async fn refresh_reward_condition_tokens(
    state: &AppState,
    cache: &RewardOrderbookLocalCache,
    last_refresh: &mut Instant,
) {
    if last_refresh.elapsed() < REWARD_ORDERBOOK_CONDITION_TOKEN_REFRESH {
        return;
    }
    *last_refresh = Instant::now();
    let markets = match refresh_reward_condition_token_markets(state).await {
        Ok(markets) => markets,
        Err(error) => {
            warn!(error = %error, "failed to refresh reward condition token map");
            return;
        }
    };
    let mut condition_tokens: HashMap<String, Vec<String>> = HashMap::new();
    let mut token_to_condition: HashMap<String, String> = HashMap::new();
    let mut active_condition_ids: HashSet<String> = HashSet::new();
    for market in markets {
        let token_ids: Vec<String> = market
            .tokens
            .iter()
            .map(|token| token.token_id.clone())
            .collect();
        active_condition_ids.insert(market.condition_id.clone());
        for token_id in &token_ids {
            token_to_condition.insert(token_id.clone(), market.condition_id.clone());
        }
        condition_tokens.insert(market.condition_id, token_ids);
    }
    cache
        .replace_condition_tokens(condition_tokens, token_to_condition, &active_condition_ids)
        .await;
}

async fn refresh_reward_condition_token_markets(state: &AppState) -> Result<Vec<RewardMarket>> {
    let mut markets = state
        .reward_bot_service
        .list_reward_run_candidate_markets()
        .await?;
    let active = state
        .reward_bot_service
        .list_active_reward_markets()
        .await?;
    let mut seen: HashSet<String> = markets
        .iter()
        .map(|market| market.condition_id.clone())
        .collect();
    for market in active {
        if seen.insert(market.condition_id.clone()) {
            markets.push(market);
        }
    }
    Ok(markets)
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
        let (cache, _ready_rx) = RewardOrderbookLocalCache::new(10, 1_000);
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

    fn both_sided_book(token_id: &str) -> CachedOrderBook {
        CachedOrderBook {
            token_id: token_id.to_string(),
            bids: vec![CachedBookLevel {
                price: Decimal::new(50, 2),
                size: Decimal::from(10_u64),
            }],
            asks: vec![CachedBookLevel {
                price: Decimal::new(52, 2),
                size: Decimal::from(10_u64),
            }],
            observed_at: reward_orderbook_now_millis(),
            source: BookSource::Poll,
        }
    }

    #[tokio::test]
    async fn check_condition_readiness_fires_once_when_all_tokens_ready() {
        let (cache, _ready_rx) = RewardOrderbookLocalCache::new(10, 60_000);
        let condition_id = "cond_a".to_string();
        let token_yes = "yes_token".to_string();
        let token_no = "no_token".to_string();
        let mut condition_tokens = HashMap::new();
        condition_tokens.insert(
            condition_id.clone(),
            vec![token_yes.clone(), token_no.clone()],
        );
        let mut token_to_condition = HashMap::new();
        token_to_condition.insert(token_yes.clone(), condition_id.clone());
        token_to_condition.insert(token_no.clone(), condition_id.clone());
        let active = HashSet::from([condition_id.clone()]);
        cache
            .replace_condition_tokens(condition_tokens, token_to_condition, &active)
            .await;

        // Only one token ready -> condition not ready.
        cache.apply_book(both_sided_book(&token_yes)).await;
        assert!(cache.check_condition_readiness(&token_yes).await.is_none());

        // Second token ready -> condition ready, fires once.
        cache.apply_book(both_sided_book(&token_no)).await;
        assert_eq!(
            cache.check_condition_readiness(&token_yes).await.as_deref(),
            Some("cond_a"),
        );
        // Notified marker suppresses the second trigger.
        assert!(cache.check_condition_readiness(&token_no).await.is_none());

        // Clearing the marker lets the next orderbook change re-fire.
        cache
            .clear_notified_ready(std::slice::from_ref(&condition_id))
            .await;
        assert_eq!(
            cache.check_condition_readiness(&token_yes).await.as_deref(),
            Some("cond_a"),
        );
    }

    #[tokio::test]
    async fn replace_condition_tokens_drops_notified_markers_for_exited_conditions() {
        let (cache, _ready_rx) = RewardOrderbookLocalCache::new(10, 60_000);
        let condition_id = "cond_b".to_string();
        let token_id = "token_b".to_string();
        let mut condition_tokens = HashMap::new();
        condition_tokens.insert(condition_id.clone(), vec![token_id.clone()]);
        let mut token_to_condition = HashMap::new();
        token_to_condition.insert(token_id.clone(), condition_id.clone());
        let active = HashSet::from([condition_id.clone()]);
        cache
            .replace_condition_tokens(condition_tokens, token_to_condition, &active)
            .await;
        cache.apply_book(both_sided_book(&token_id)).await;
        // First readiness check sets the notified marker.
        assert_eq!(
            cache.check_condition_readiness(&token_id).await.as_deref(),
            Some("cond_b"),
        );

        // Condition exits the candidate set (empty maps + empty active set).
        cache
            .replace_condition_tokens(HashMap::new(), HashMap::new(), &HashSet::new())
            .await;
        // Re-inserting the condition lets it fire again because the marker was dropped.
        let mut condition_tokens = HashMap::new();
        condition_tokens.insert(condition_id.clone(), vec![token_id.clone()]);
        let mut token_to_condition = HashMap::new();
        token_to_condition.insert(token_id.clone(), condition_id.clone());
        let active = HashSet::from([condition_id.clone()]);
        cache
            .replace_condition_tokens(condition_tokens, token_to_condition, &active)
            .await;
        assert_eq!(
            cache.check_condition_readiness(&token_id).await.as_deref(),
            Some("cond_b"),
        );
    }
}
