use crate::updates::OrderbookUpdateBroadcaster;
use futures::StreamExt;
use polyedge_application::{
    BookSource, CachedBookLevel, CachedOrderBook, OrderbookCache, OrderbookStreamReason,
};
use polyedge_connectors::PolymarketRewardsConnector;
use polyedge_domain::{AppError, Result};
use polyedge_infrastructure::AppState;
use polymarket_client_sdk::clob::ws::Client as ClobWsClient;
use polymarket_client_sdk::clob::{
    types::Side,
    ws::{BookUpdate, PriceChange},
};
use polymarket_client_sdk::types::U256;
use polymarket_client_sdk::ws::config::Config as WsConfig;
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::{RwLock, mpsc};
use tokio::task::JoinSet;
use tracing::{debug, info, warn};

const ORDERBOOK_WS_HEARTBEAT_INTERVAL_SECS: u64 = 15;
const ORDERBOOK_WS_HEARTBEAT_TIMEOUT_SECS: u64 = 60;
const ORDERBOOK_WS_RECONNECT_DEBOUNCE_SECS: u64 = 5;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct OrderbookStreamReport {
    pub subscribed_tokens: usize,
    pub ws_snapshots_received: usize,
    pub ws_price_changes_received: usize,
    pub poll_reconciliations: usize,
    pub poll_failures: usize,
}

/// Run a single orderbook stream lifecycle (WS + poll reconciler).
/// Subscribes to tokens currently registered in the subscription registry.
/// Returns when the WS ends or the token set changes; the caller should restart.
pub async fn run_orderbook_stream(
    state: &AppState,
    broadcaster: &OrderbookUpdateBroadcaster,
) -> Result<OrderbookStreamReport> {
    let settings = &state.settings.orderbook_stream;
    let cache = state.orderbook_cache.clone();
    let max_levels_per_side = settings.max_levels_per_side;
    let mut report = OrderbookStreamReport::default();

    // 1. Collect aggregated tokens from the registry.
    let token_ids = collect_orderbook_subscription_tokens(state).await;
    report.subscribed_tokens = token_ids.len();

    if token_ids.is_empty() {
        info!("no tokens registered, skipping orderbook stream");
        return Ok(report);
    }

    // 2. Convert to U256 for SDK.
    let u256_ids: Vec<U256> = token_ids
        .iter()
        .filter_map(|id| U256::from_str(id).ok())
        .collect();

    if u256_ids.is_empty() {
        warn!("no valid U256 token IDs found for orderbook subscription");
        return Ok(report);
    }

    // 3. Start WS consumers in token chunks. The SDK uses a fixed-size
    // broadcast queue per WS connection; splitting large subscriptions keeps
    // high-volume market updates from lagging one shared receiver.
    let ws_chunk_size = settings.ws_chunk_size.max(1);
    let ws_snapshots_received = Arc::new(AtomicUsize::new(0));
    let ws_price_changes_received = Arc::new(AtomicUsize::new(0));
    let mut ws_tasks = JoinSet::new();
    let mut ws_connection_count = 0usize;
    for (chunk_index, chunk) in u256_ids.chunks(ws_chunk_size).enumerate() {
        ws_connection_count += 1;
        let chunk_token_ids = chunk.to_vec();
        let ws_host = state.settings.polymarket.ws_host.clone();
        let ws_cache = cache.clone();
        let ws_broadcaster = broadcaster.clone();
        let chunk_snapshots = ws_snapshots_received.clone();
        let chunk_price_changes = ws_price_changes_received.clone();
        let context = OrderbookWsChunkContext {
            ws_host,
            cache: ws_cache,
            broadcaster: ws_broadcaster,
            max_levels_per_side,
            snapshots_received: chunk_snapshots,
            price_changes_received: chunk_price_changes,
        };
        ws_tasks.spawn(async move {
            run_orderbook_ws_chunk(chunk_index, chunk_token_ids, context).await
        });
    }

    info!(
        subscribed_tokens = u256_ids.len(),
        ws_connections = ws_connection_count,
        ws_chunk_size,
        "orderbook stream subscribed to market channel"
    );

    // 4. Shared token list for poll reconciler.
    let shared_tokens: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(token_ids.clone()));
    let ws_token_set: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(token_ids));

    // 5. Spawn poll reconciler.
    let poll_cache = cache.clone();
    let poll_broadcaster = broadcaster.clone();
    let poll_tokens_ref = shared_tokens.clone();
    let poll_interval = settings.poll_reconcile_interval_secs;
    let stale_threshold_ms = settings.stale_threshold_ms as i64;
    let clob_host = state.settings.polymarket.clob_host.clone();
    let poll_max_tokens = settings.max_tokens;
    let poll_max_levels = max_levels_per_side;
    let poll_reconciliations = Arc::new(AtomicUsize::new(0));
    let poll_failures = Arc::new(AtomicUsize::new(0));
    let poll_rec_clone = poll_reconciliations.clone();
    let poll_fail_clone = poll_failures.clone();

    let poll_handle = tokio::spawn(async move {
        let connector = match PolymarketRewardsConnector::new(&clob_host) {
            Ok(c) => c,
            Err(error) => {
                warn!(error = %error, "orderbook poll reconciler failed to create connector");
                return;
            }
        };

        loop {
            tokio::time::sleep(Duration::from_secs(poll_interval.max(1))).await;

            let current_tokens = poll_tokens_ref.read().await.clone();
            let stale = match poll_cache
                .get_stale_tokens(&current_tokens, stale_threshold_ms)
                .await
            {
                Ok(tokens) => tokens,
                Err(error) => {
                    warn!(error = %error, "poll reconciler failed to get stale tokens");
                    Vec::new()
                }
            };
            let targets = poll_reconcile_targets(&current_tokens, &stale, poll_max_tokens);

            if targets.is_empty() {
                continue;
            }

            debug!(
                stale_count = stale.len(),
                target_count = targets.len(),
                "poll reconciler refreshing registered tokens"
            );

            for chunk in targets.chunks(100) {
                match connector.fetch_order_books(chunk).await {
                    Ok(books) => {
                        for book in books {
                            let cached = normalized_cached_book(
                                reward_book_to_cached(&book),
                                poll_max_levels,
                            );
                            if let Err(error) = set_book_and_publish_if_current(
                                &poll_cache,
                                &poll_broadcaster,
                                OrderbookStreamReason::PollReconcile,
                                &cached,
                            )
                            .await
                            {
                                warn!(
                                    token_id = %cached.token_id,
                                    error = %error,
                                    "poll reconciler failed to write book to cache"
                                );
                            }
                        }
                        poll_rec_clone.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(error) => {
                        poll_fail_clone.fetch_add(1, Ordering::Relaxed);
                        warn!(error = %error, "poll reconciler failed to fetch books");
                    }
                }
            }
        }
    });

    // 6. Keep WS chunk consumers alive with immediate registry change checks
    //    and a periodic fallback.
    //    When other services register new tokens via the HTTP API, the registry
    //    changes. We reconnect only when the token membership changed.
    let refresh_interval = Duration::from_secs(settings.token_refresh_interval_secs.max(1));
    let mut refresh_timer = tokio::time::interval(refresh_interval);
    refresh_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    refresh_timer.tick().await; // skip first immediate tick
    let mut registry_changes = state.orderbook_registry.subscribe_changes();

    loop {
        tokio::select! {
            result = ws_tasks.join_next() => {
                match result {
                    Some(Ok(Ok(()))) => {
                        info!("orderbook WS chunk ended, restarting stream");
                    }
                    Some(Ok(Err(error))) => {
                        warn!(error = %error, "orderbook WS chunk failed, restarting stream");
                    }
                    Some(Err(error)) => {
                        warn!(error = %error, "orderbook WS chunk task failed, restarting stream");
                    }
                    None => {
                        info!("all orderbook WS chunks ended");
                    }
                }
                break;
            }
            _ = wait_for_registry_change(&mut registry_changes) => {
                if refresh_tokens_and_should_reconnect(
                    state,
                    &shared_tokens,
                    &ws_token_set,
                    &mut report,
                )
                .await
                {
                    break;
                }
            }
            _ = refresh_timer.tick() => {
                // Check if the registry token set changed (other services may
                // have registered new tokens via HTTP API).
                if refresh_tokens_and_should_reconnect(
                    state,
                    &shared_tokens,
                    &ws_token_set,
                    &mut report,
                )
                .await
                {
                    break;
                }
            }
        }
    }

    ws_tasks.abort_all();
    while ws_tasks.join_next().await.is_some() {}
    poll_handle.abort();

    report.ws_snapshots_received = ws_snapshots_received.load(Ordering::Relaxed);
    report.ws_price_changes_received = ws_price_changes_received.load(Ordering::Relaxed);
    report.poll_reconciliations = poll_reconciliations.load(Ordering::Relaxed);
    report.poll_failures = poll_failures.load(Ordering::Relaxed);

    info!(
        subscribed_tokens = report.subscribed_tokens,
        ws_snapshots_received = report.ws_snapshots_received,
        ws_price_changes_received = report.ws_price_changes_received,
        "orderbook stream consumer stopped"
    );

    Ok(report)
}

async fn wait_for_registry_change(change_rx: &mut Option<tokio::sync::watch::Receiver<u64>>) {
    let Some(rx) = change_rx else {
        std::future::pending::<()>().await;
        return;
    };
    let _ = rx.changed().await;
}

async fn refresh_tokens_and_should_reconnect(
    state: &AppState,
    shared_tokens: &Arc<RwLock<Vec<String>>>,
    ws_token_set: &Arc<RwLock<Vec<String>>>,
    report: &mut OrderbookStreamReport,
) -> bool {
    let debounce = Duration::from_secs(ORDERBOOK_WS_RECONNECT_DEBOUNCE_SECS);
    let mut new_tokens = collect_orderbook_subscription_tokens(state).await;
    *shared_tokens.write().await = new_tokens.clone();
    if token_set_matches_current_ws(ws_token_set, &new_tokens).await {
        return false;
    }

    if !debounce.is_zero() {
        debug!(
            old = report.subscribed_tokens,
            new = new_tokens.len(),
            debounce_ms = debounce.as_millis(),
            "orderbook token set changed, debouncing WS reconnect"
        );
        tokio::time::sleep(debounce).await;
        new_tokens = collect_orderbook_subscription_tokens(state).await;
        *shared_tokens.write().await = new_tokens.clone();
        if token_set_matches_current_ws(ws_token_set, &new_tokens).await {
            return false;
        }
    }

    let new_count = new_tokens.len();
    info!(
        old = report.subscribed_tokens,
        new = new_count,
        debounce_ms = debounce.as_millis(),
        "orderbook token list changed, reconnecting WS with new set"
    );
    report.subscribed_tokens = new_count;
    *ws_token_set.write().await = new_tokens;
    true
}

async fn token_set_matches_current_ws(
    ws_token_set: &Arc<RwLock<Vec<String>>>,
    new_tokens: &[String],
) -> bool {
    let old_tokens = ws_token_set.read().await;
    token_lists_have_same_members(&old_tokens, new_tokens)
}

struct OrderbookWsChunkContext {
    ws_host: String,
    cache: Arc<dyn OrderbookCache>,
    broadcaster: OrderbookUpdateBroadcaster,
    max_levels_per_side: usize,
    snapshots_received: Arc<AtomicUsize>,
    price_changes_received: Arc<AtomicUsize>,
}

enum OrderbookWsEvent {
    Book(BookUpdate),
    PriceChange(PriceChange),
}

async fn run_orderbook_ws_chunk(
    chunk_index: usize,
    token_ids: Vec<U256>,
    context: OrderbookWsChunkContext,
) -> Result<()> {
    // Polymarket occasionally delays or drops text PONGs while data still
    // flows. Keep the SDK heartbeat useful without logging on short stalls.
    let mut ws_config = WsConfig::default();
    ws_config.heartbeat_interval = Duration::from_secs(ORDERBOOK_WS_HEARTBEAT_INTERVAL_SECS);
    ws_config.heartbeat_timeout = Duration::from_secs(ORDERBOOK_WS_HEARTBEAT_TIMEOUT_SECS);
    let ws_client = ClobWsClient::new(&context.ws_host, ws_config).map_err(|error| {
        AppError::internal(
            "ORDERBOOK_WS_INIT_FAILED",
            format!("failed to create orderbook websocket client: {error}"),
        )
    })?;
    let mut subscription_guard =
        OrderbookWsSubscriptionGuard::new(ws_client.clone(), token_ids.clone());
    let book_stream = ws_client
        .subscribe_orderbook(token_ids.clone())
        .map_err(|error| {
            AppError::internal(
                "ORDERBOOK_WS_SUBSCRIBE_FAILED",
                format!("failed to subscribe to orderbook websocket: {error}"),
            )
        })?;
    subscription_guard.mark_subscribed();
    let price_stream = ws_client
        .subscribe_prices(token_ids.clone())
        .map_err(|error| {
            AppError::internal(
                "ORDERBOOK_WS_PRICE_SUBSCRIBE_FAILED",
                format!("failed to subscribe to orderbook price changes: {error}"),
            )
        })?;
    subscription_guard.mark_subscribed();
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let mut reader_tasks = JoinSet::new();

    {
        let event_tx = event_tx.clone();
        reader_tasks.spawn(async move {
            let mut book_stream = Box::pin(book_stream);
            while let Some(message) = book_stream.next().await {
                match message {
                    Ok(book_update) => {
                        if event_tx.send(OrderbookWsEvent::Book(book_update)).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        warn!(
                            ws_chunk = chunk_index,
                            error = %error,
                            "orderbook WS stream error, poll reconciler will cover gaps"
                        );
                    }
                }
            }
            Ok::<(), AppError>(())
        });
    }

    {
        let event_tx = event_tx.clone();
        reader_tasks.spawn(async move {
            let mut price_stream = Box::pin(price_stream);
            while let Some(message) = price_stream.next().await {
                match message {
                    Ok(price_change) => {
                        if event_tx
                            .send(OrderbookWsEvent::PriceChange(price_change))
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(error) => {
                        warn!(
                            ws_chunk = chunk_index,
                            error = %error,
                            "orderbook price-change WS stream error, poll reconciler will cover gaps"
                        );
                    }
                }
            }
            Ok::<(), AppError>(())
        });
    }
    drop(event_tx);

    info!(
        ws_chunk = chunk_index,
        subscribed_tokens = token_ids.len(),
        "orderbook WS chunk subscribed"
    );

    loop {
        tokio::select! {
            Some(event) = event_rx.recv() => {
                match event {
                    OrderbookWsEvent::Book(book_update) => {
                        let cached = normalized_cached_book(
                            book_update_to_cached(&book_update),
                            context.max_levels_per_side,
                        );
                        if let Err(error) = set_book_and_publish_if_current(
                            &context.cache,
                            &context.broadcaster,
                            OrderbookStreamReason::Book,
                            &cached,
                        )
                        .await
                        {
                            warn!(
                                ws_chunk = chunk_index,
                                token_id = %cached.token_id,
                                error = %error,
                                "failed to write orderbook snapshot to cache"
                            );
                        }
                        let received = context.snapshots_received.fetch_add(1, Ordering::Relaxed) + 1;

                        if received.is_multiple_of(100) {
                            debug!(
                                ws_chunk = chunk_index,
                                received,
                                "orderbook stream processing snapshots"
                            );
                        }
                    }
                    OrderbookWsEvent::PriceChange(price_change) => {
                        if let Err(error) = apply_price_change_to_cache(
                            &context.cache,
                            &context.broadcaster,
                            &price_change,
                        )
                        .await
                        {
                            warn!(
                                ws_chunk = chunk_index,
                                error = %error,
                                "failed to apply orderbook price change"
                            );
                        }
                        context.price_changes_received.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
            result = reader_tasks.join_next() => {
                match result {
                    Some(Ok(Ok(()))) => {
                        info!(ws_chunk = chunk_index, "orderbook WS reader ended");
                    }
                    Some(Ok(Err(error))) => {
                        warn!(ws_chunk = chunk_index, error = %error, "orderbook WS reader failed");
                    }
                    Some(Err(error)) => {
                        warn!(ws_chunk = chunk_index, error = %error, "orderbook WS reader task failed");
                    }
                    None => {
                        info!(ws_chunk = chunk_index, "orderbook WS readers ended");
                    }
                }
                break;
            }
        }
    }

    reader_tasks.abort_all();
    while reader_tasks.join_next().await.is_some() {}

    Ok(())
}

struct OrderbookWsSubscriptionGuard {
    client: ClobWsClient,
    token_ids: Vec<U256>,
    subscriptions: u8,
}

impl OrderbookWsSubscriptionGuard {
    fn new(client: ClobWsClient, token_ids: Vec<U256>) -> Self {
        Self {
            client,
            token_ids,
            subscriptions: 0,
        }
    }

    fn mark_subscribed(&mut self) {
        self.subscriptions = self.subscriptions.saturating_add(1);
    }
}

impl Drop for OrderbookWsSubscriptionGuard {
    fn drop(&mut self) {
        for _ in 0..self.subscriptions {
            if let Err(error) = self.client.unsubscribe_orderbook(&self.token_ids) {
                debug!(error = %error, "failed to unsubscribe orderbook WS stream");
                break;
            }
        }
        self.subscriptions = 0;
    }
}

async fn collect_orderbook_subscription_tokens(state: &AppState) -> Vec<String> {
    let max_tokens = state.settings.orderbook_stream.max_tokens;
    let all = state.orderbook_registry.list_all_tokens().await;
    all.into_iter().take(max_tokens).collect()
}

fn token_lists_have_same_members(left: &[String], right: &[String]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let left = left.iter().map(String::as_str).collect::<HashSet<_>>();
    let right = right.iter().map(String::as_str).collect::<HashSet<_>>();
    left == right
}

fn poll_reconcile_targets(
    current_tokens: &[String],
    stale_tokens: &[String],
    max_tokens: usize,
) -> Vec<String> {
    if max_tokens == 0 {
        return Vec::new();
    }
    let current = current_tokens.iter().collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    let mut targets = Vec::with_capacity(current_tokens.len().min(max_tokens));
    for token_id in stale_tokens.iter().chain(current_tokens) {
        if current.contains(token_id) && seen.insert(token_id.as_str()) {
            targets.push(token_id.clone());
            if targets.len() >= max_tokens {
                break;
            }
        }
    }
    targets
}

fn book_update_to_cached(update: &BookUpdate) -> CachedOrderBook {
    CachedOrderBook {
        token_id: update.asset_id.to_string(),
        bids: update
            .bids
            .iter()
            .map(|level| CachedBookLevel {
                price: level.price,
                size: level.size,
            })
            .collect(),
        asks: update
            .asks
            .iter()
            .map(|level| CachedBookLevel {
                price: level.price,
                size: level.size,
            })
            .collect(),
        observed_at: update.timestamp,
        source: BookSource::Ws,
    }
}

async fn set_book_and_publish_if_current(
    cache: &Arc<dyn OrderbookCache>,
    broadcaster: &OrderbookUpdateBroadcaster,
    reason: OrderbookStreamReason,
    book: &CachedOrderBook,
) -> Result<()> {
    cache.set_book(book).await?;
    publish_if_current(cache, broadcaster, reason, book).await
}

async fn publish_if_current(
    cache: &Arc<dyn OrderbookCache>,
    broadcaster: &OrderbookUpdateBroadcaster,
    reason: OrderbookStreamReason,
    candidate: &CachedOrderBook,
) -> Result<()> {
    let Some(current) = cache.get_book(&candidate.token_id).await? else {
        return Ok(());
    };
    if current.observed_at == candidate.observed_at && current.source == candidate.source {
        broadcaster.publish(reason, current);
    }
    Ok(())
}

async fn apply_price_change_to_cache(
    cache: &Arc<dyn OrderbookCache>,
    broadcaster: &OrderbookUpdateBroadcaster,
    update: &PriceChange,
) -> Result<()> {
    for change in &update.price_changes {
        let token_id = change.asset_id.to_string();
        let Some(mut book) = cache.get_book(&token_id).await? else {
            debug!(token_id, "price change skipped: book not in cache");
            continue;
        };
        if update.timestamp < book.observed_at {
            continue;
        }

        let levels = match change.side {
            Side::Buy => &mut book.bids,
            Side::Sell => &mut book.asks,
            _ => continue,
        };
        let Some(size) = change.size else {
            continue;
        };
        if size <= rust_decimal::Decimal::ZERO {
            levels.retain(|level| level.price != change.price);
        } else if let Some(level) = levels.iter_mut().find(|level| level.price == change.price) {
            level.size = size;
        } else {
            levels.push(CachedBookLevel {
                price: change.price,
                size,
            });
        }
        book.observed_at = update.timestamp;
        book.source = BookSource::Ws;
        // Use replace_book which checks freshness atomically under the lock,
        // preventing the race where a poll reconciler writes a newer snapshot
        // between our get_book and set_book.
        if cache.replace_book(&book).await? {
            broadcaster.publish(OrderbookStreamReason::PriceChange, book);
        }
    }
    Ok(())
}

fn normalized_cached_book(
    mut book: CachedOrderBook,
    max_levels_per_side: usize,
) -> CachedOrderBook {
    let max_levels = max_levels_per_side.max(1);
    book.bids.sort_by(|a, b| b.price.cmp(&a.price));
    book.asks.sort_by(|a, b| a.price.cmp(&b.price));
    book.bids.truncate(max_levels);
    book.asks.truncate(max_levels);
    book
}

fn reward_book_to_cached(book: &polyedge_connectors::PolymarketRewardOrderBook) -> CachedOrderBook {
    let observed_at_ms = book
        .observed_at
        .unix_timestamp_nanos()
        .div_euclid(1_000_000);
    let observed_at = i64::try_from(observed_at_ms).unwrap_or_else(|_| {
        if observed_at_ms.is_negative() {
            i64::MIN
        } else {
            i64::MAX
        }
    });

    CachedOrderBook {
        token_id: book.token_id.clone(),
        bids: book
            .bids
            .iter()
            .map(|l| CachedBookLevel {
                price: l.price,
                size: l.size,
            })
            .collect(),
        asks: book
            .asks
            .iter()
            .map(|l| CachedBookLevel {
                price: l.price,
                size: l.size,
            })
            .collect(),
        observed_at,
        source: BookSource::Poll,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polyedge_infrastructure::stores::InMemoryOrderbookCache;
    use rust_decimal::Decimal;

    #[tokio::test]
    async fn price_change_updates_and_removes_levels() {
        let cache: Arc<dyn OrderbookCache> = Arc::new(InMemoryOrderbookCache::new(60_000, 10));
        cache
            .set_book(&CachedOrderBook {
                token_id: "123".to_string(),
                bids: vec![CachedBookLevel {
                    price: Decimal::new(49, 2),
                    size: Decimal::from(10_u64),
                }],
                asks: vec![CachedBookLevel {
                    price: Decimal::new(52, 2),
                    size: Decimal::from(10_u64),
                }],
                observed_at: 100,
                source: BookSource::Poll,
            })
            .await
            .expect("seed book");
        let update: PriceChange = serde_json::from_value(serde_json::json!({
            "market": format!("0x{:064x}", 1),
            "timestamp": "200",
            "price_changes": [
                {"asset_id": "123", "price": "0.50", "size": "7", "side": "BUY"},
                {"asset_id": "123", "price": "0.49", "size": "0", "side": "BUY"}
            ]
        }))
        .expect("decode price change");

        let broadcaster = OrderbookUpdateBroadcaster::new(16);

        apply_price_change_to_cache(&cache, &broadcaster, &update)
            .await
            .expect("apply price change");
        let book = cache
            .get_book("123")
            .await
            .expect("get book")
            .expect("book present");

        assert_eq!(book.observed_at, 200);
        assert_eq!(book.bids.len(), 1);
        assert_eq!(book.bids[0].price, Decimal::new(50, 2));
        assert_eq!(book.bids[0].size, Decimal::from(7_u64));
    }

    #[test]
    fn poll_reconcile_targets_include_fresh_tokens_after_stale_priority() {
        let current = vec!["fresh".to_string(), "stale".to_string()];
        let stale = vec!["stale".to_string()];

        assert_eq!(
            poll_reconcile_targets(&current, &stale, 2),
            vec!["stale".to_string(), "fresh".to_string()]
        );
    }

    #[test]
    fn token_list_member_comparison_ignores_order_only_changes() {
        let left = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let reordered = vec!["c".to_string(), "a".to_string(), "b".to_string()];
        let changed = vec!["a".to_string(), "b".to_string(), "d".to_string()];

        assert!(token_lists_have_same_members(&left, &reordered));
        assert!(!token_lists_have_same_members(&left, &changed));
    }
}
