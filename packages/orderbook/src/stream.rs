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
use time::OffsetDateTime;
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

    // 3. WS consumers are sharded into chunks of `ws_chunk_size` tokens (one WS
    //    connection per chunk) so high-volume market updates don't lag one shared
    //    receiver. The actual chunk tasks are spawned per-path below.
    let ws_chunk_size = settings.ws_chunk_size.max(1);
    let ws_snapshots_received = Arc::new(AtomicUsize::new(0));
    let ws_price_changes_received = Arc::new(AtomicUsize::new(0));

    // 4. Shared token list for the poll reconciler (`shared_tokens`, updated
    //    eagerly) and the live WS subscription set (`ws_token_set`, updated on
    //    each reconcile).
    let shared_tokens: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(token_ids.clone()));
    let ws_token_set: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(token_ids.clone()));

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
                        let poll_confirmed_at = current_unix_millis();
                        for book in books {
                            let cached = normalized_cached_book(
                                reward_book_to_cached(&book, poll_confirmed_at),
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

    // 6. Keep WS consumers alive with immediate registry change checks and a
    //    periodic fallback. Two strategies, gated by
    //    `orderbook_ws_incremental_reconcile`:
    //      - incremental (default): keep WS connections alive across membership
    //        changes, applying subscribe/unsubscribe diffs in place; rebuild a
    //        connection only when it actually dies.
    //      - legacy (rollback): tear down and rebuild the whole connection on
    //        every membership change.
    let refresh_interval = Duration::from_secs(settings.token_refresh_interval_secs.max(1));
    let mut refresh_timer = tokio::time::interval(refresh_interval);
    refresh_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    refresh_timer.tick().await; // skip first immediate tick
    let mut registry_changes = state.orderbook_registry.subscribe_changes();

    let incremental = settings.orderbook_ws_incremental_reconcile;
    let ws_host = state.settings.polymarket.ws_host.clone();

    if incremental {
        // === Incremental reconcile path ===
        let context_template = OrderbookWsChunkContext {
            ws_host,
            cache: cache.clone(),
            broadcaster: broadcaster.clone(),
            max_levels_per_side,
            snapshots_received: ws_snapshots_received.clone(),
            price_changes_received: ws_price_changes_received.clone(),
        };
        let mut session_tasks: JoinSet<ChunkExit> = JoinSet::new();
        let mut session_cmds: Vec<mpsc::Sender<ChunkCommand>> = Vec::new();
        for (index, chunk_tokens) in partition_tokens_into_chunks(&token_ids, ws_chunk_size)
            .into_iter()
            .enumerate()
        {
            let tokens_u256: Vec<U256> = chunk_tokens
                .iter()
                .filter_map(|token| U256::from_str(token).ok())
                .collect();
            let (command_tx, command_rx) = mpsc::channel::<ChunkCommand>(8);
            session_tasks.spawn(run_orderbook_chunk_session(
                index,
                context_template.clone(),
                tokens_u256,
                command_rx,
            ));
            session_cmds.push(command_tx);
        }
        info!(
            subscribed_tokens = u256_ids.len(),
            ws_connections = session_cmds.len(),
            ws_chunk_size,
            incremental = true,
            "orderbook stream subscribed to market channel"
        );

        let full_resync_interval = settings.orderbook_full_resync_interval_secs;
        let mut full_resync_timer: Option<tokio::time::Interval> = (full_resync_interval > 0)
            .then(|| tokio::time::interval(Duration::from_secs(full_resync_interval)));
        if let Some(timer) = full_resync_timer.as_mut() {
            timer.tick().await; // skip the immediate first tick
        }

        loop {
            tokio::select! {
                exit_res = session_tasks.join_next() => {
                    match exit_res {
                        Some(Ok(exit)) => match exit.reason {
                            ChunkExitReason::Shutdown => {
                                debug!(ws_chunk = exit.chunk_index, "orderbook WS chunk session shut down");
                            }
                            ChunkExitReason::ReaderEnded => {
                                // Connection died: rebuild this single chunk in
                                // place using the current partition; other chunks
                                // are unaffected.
                                let full: Vec<String> = ws_token_set.read().await.clone();
                                let chunks = partition_tokens_into_chunks(&full, ws_chunk_size);
                                if let Some(chunk_tokens) = chunks.get(exit.chunk_index) {
                                    let tokens_u256: Vec<U256> = chunk_tokens
                                        .iter()
                                        .filter_map(|token| U256::from_str(token).ok())
                                        .collect();
                                    let (command_tx, command_rx) = mpsc::channel::<ChunkCommand>(8);
                                    session_tasks.spawn(run_orderbook_chunk_session(
                                        exit.chunk_index,
                                        context_template.clone(),
                                        tokens_u256,
                                        command_rx,
                                    ));
                                    if exit.chunk_index < session_cmds.len() {
                                        session_cmds[exit.chunk_index] = command_tx;
                                    } else {
                                        session_cmds.push(command_tx);
                                    }
                                    info!(ws_chunk = exit.chunk_index, "orderbook WS chunk session rebuilt after reader end");
                                } else {
                                    debug!(ws_chunk = exit.chunk_index, "orderbook WS chunk session ended; chunk no longer in partition");
                                }
                            }
                        },
                        Some(Err(error)) => {
                            warn!(error = %error, "orderbook WS chunk session task panicked; restarting stream");
                            break;
                        }
                        None => {
                            info!("all orderbook WS chunk sessions ended");
                            break;
                        }
                    }
                }
                _ = wait_for_registry_change(&mut registry_changes) => {
                    if let Some(new_tokens) = refresh_tokens_and_reconcile(
                        state,
                        &shared_tokens,
                        &ws_token_set,
                        &mut report,
                    )
                    .await
                    {
                        apply_membership_diff(
                            &mut session_cmds,
                            &mut session_tasks,
                            &ws_token_set,
                            &context_template,
                            new_tokens,
                            ws_chunk_size,
                        )
                        .await;
                    }
                }
                _ = refresh_timer.tick() => {
                    if let Some(new_tokens) = refresh_tokens_and_reconcile(
                        state,
                        &shared_tokens,
                        &ws_token_set,
                        &mut report,
                    )
                    .await
                    {
                        apply_membership_diff(
                            &mut session_cmds,
                            &mut session_tasks,
                            &ws_token_set,
                            &context_template,
                            new_tokens,
                            ws_chunk_size,
                        )
                        .await;
                    }
                }
                _ = async {
                    match full_resync_timer.as_mut() {
                        Some(timer) => {
                            let _ = timer.tick().await;
                        }
                        None => std::future::pending::<()>().await,
                    }
                } => {
                    warn!(
                        interval_secs = full_resync_interval,
                        "orderbook WS emergency full resync triggered"
                    );
                    for command_tx in &session_cmds {
                        let _ = command_tx.send(ChunkCommand::Shutdown).await;
                    }
                    while session_tasks.join_next().await.is_some() {}
                    session_cmds.clear();
                    let full: Vec<String> = ws_token_set.read().await.clone();
                    for (index, chunk_tokens) in
                        partition_tokens_into_chunks(&full, ws_chunk_size).into_iter().enumerate()
                    {
                        let tokens_u256: Vec<U256> = chunk_tokens
                            .iter()
                            .filter_map(|token| U256::from_str(token).ok())
                            .collect();
                        let (command_tx, command_rx) = mpsc::channel::<ChunkCommand>(8);
                        session_tasks.spawn(run_orderbook_chunk_session(
                            index,
                            context_template.clone(),
                            tokens_u256,
                            command_rx,
                        ));
                        session_cmds.push(command_tx);
                    }
                    info!(ws_connections = session_cmds.len(), "orderbook WS sessions rebuilt after full resync");
                }
            }
        }

        for command_tx in &session_cmds {
            let _ = command_tx.send(ChunkCommand::Shutdown).await;
        }
        while session_tasks.join_next().await.is_some() {}
    } else {
        // === Legacy path: tear down and rebuild the whole connection on every
        // membership change. Kept verbatim as an emergency rollback lever. ===
        let mut ws_tasks = JoinSet::new();
        let mut ws_connection_count = 0usize;
        for (chunk_index, chunk) in u256_ids.chunks(ws_chunk_size).enumerate() {
            ws_connection_count += 1;
            let chunk_token_ids = chunk.to_vec();
            let context = OrderbookWsChunkContext {
                ws_host: ws_host.clone(),
                cache: cache.clone(),
                broadcaster: broadcaster.clone(),
                max_levels_per_side,
                snapshots_received: ws_snapshots_received.clone(),
                price_changes_received: ws_price_changes_received.clone(),
            };
            ws_tasks.spawn(async move {
                run_orderbook_ws_chunk(chunk_index, chunk_token_ids, context).await
            });
        }
        info!(
            subscribed_tokens = u256_ids.len(),
            ws_connections = ws_connection_count,
            ws_chunk_size,
            incremental = false,
            "orderbook stream subscribed to market channel"
        );

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
                    if refresh_tokens_and_reconcile(
                        state,
                        &shared_tokens,
                        &ws_token_set,
                        &mut report,
                    )
                    .await
                    .is_some()
                    {
                        break;
                    }
                }
                _ = refresh_timer.tick() => {
                    if refresh_tokens_and_reconcile(
                        state,
                        &shared_tokens,
                        &ws_token_set,
                        &mut report,
                    )
                    .await
                    .is_some()
                    {
                        break;
                    }
                }
            }
        }
        ws_tasks.abort_all();
        while ws_tasks.join_next().await.is_some() {}
    }

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

/// Reads the latest registry token set, writes it to `shared_tokens` (the poll
/// reconciler's view), and returns `Some(new_tokens)` when the WS subscription
/// membership changed (after debounce), or `None` when unchanged. The caller
/// either reconciles incrementally (new path) or tears down and reconnects
/// (legacy path) based on whether membership changed.
async fn refresh_tokens_and_reconcile(
    state: &AppState,
    shared_tokens: &Arc<RwLock<Vec<String>>>,
    ws_token_set: &Arc<RwLock<Vec<String>>>,
    report: &mut OrderbookStreamReport,
) -> Option<Vec<String>> {
    let debounce = Duration::from_secs(ORDERBOOK_WS_RECONNECT_DEBOUNCE_SECS);
    let mut new_tokens = collect_orderbook_subscription_tokens(state).await;
    *shared_tokens.write().await = new_tokens.clone();
    if token_set_matches_current_ws(ws_token_set, &new_tokens).await {
        return None;
    }

    if !debounce.is_zero() {
        debug!(
            old = report.subscribed_tokens,
            new = new_tokens.len(),
            debounce_ms = debounce.as_millis(),
            "orderbook token set changed, debouncing WS reconcile"
        );
        tokio::time::sleep(debounce).await;
        new_tokens = collect_orderbook_subscription_tokens(state).await;
        *shared_tokens.write().await = new_tokens.clone();
        if token_set_matches_current_ws(ws_token_set, &new_tokens).await {
            return None;
        }
    }

    let new_count = new_tokens.len();
    info!(
        old = report.subscribed_tokens,
        new = new_count,
        debounce_ms = debounce.as_millis(),
        "orderbook token list changed, reconciling WS subscriptions"
    );
    report.subscribed_tokens = new_count;
    *ws_token_set.write().await = new_tokens.clone();
    Some(new_tokens)
}

async fn token_set_matches_current_ws(
    ws_token_set: &Arc<RwLock<Vec<String>>>,
    new_tokens: &[String],
) -> bool {
    let old_tokens = ws_token_set.read().await;
    token_lists_have_same_members(&old_tokens, new_tokens)
}

#[derive(Clone)]
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

/// Apply a single WS event (book snapshot or price change) to the cache. Shared
/// by both the legacy chunk consumer and the incremental chunk session.
async fn handle_orderbook_ws_event(
    event: OrderbookWsEvent,
    context: &OrderbookWsChunkContext,
    chunk_index: usize,
) {
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
                    received, "orderbook stream processing snapshots"
                );
            }
        }
        OrderbookWsEvent::PriceChange(price_change) => {
            if let Err(error) =
                apply_price_change_to_cache(&context.cache, &context.broadcaster, &price_change)
                    .await
            {
                warn!(
                    ws_chunk = chunk_index,
                    error = %error,
                    "failed to apply orderbook price change"
                );
            }
            context
                .price_changes_received
                .fetch_add(1, Ordering::Relaxed);
        }
    }
}

fn make_orderbook_ws_client(ws_host: &str) -> Result<ClobWsClient> {
    let mut ws_config = WsConfig::default();
    ws_config.heartbeat_interval = Duration::from_secs(ORDERBOOK_WS_HEARTBEAT_INTERVAL_SECS);
    ws_config.heartbeat_timeout = Duration::from_secs(ORDERBOOK_WS_HEARTBEAT_TIMEOUT_SECS);
    ClobWsClient::new(ws_host, ws_config).map_err(|error| {
        AppError::internal(
            "ORDERBOOK_WS_INIT_FAILED",
            format!("failed to create orderbook websocket client: {error}"),
        )
    })
}

fn u256_sets_same_members(left: &[U256], right: &[U256]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let left: HashSet<String> = left.iter().map(|v| v.to_string()).collect();
    right.iter().all(|v| left.contains(&v.to_string()))
}

fn u256_to_token_strings(tokens: &[U256]) -> Vec<String> {
    tokens.iter().map(|v| v.to_string()).collect()
}

/// Subscribe (book + price) for a token set on an existing client, spawn the two
/// reader tasks, and return a guard (which unsubscribes the set on drop) plus the
/// reader JoinSet. The SDK's per-asset refcounting means re-subscribing already
/// subscribed tokens only bumps refcounts and sends no server frames; only
/// genuinely-new assets trigger subscribe frames. Returns an empty guard+readers
/// for an empty token slice.
#[allow(clippy::too_many_arguments)]
async fn subscribe_orderbook_token_set(
    client: &ClobWsClient,
    tokens: &[U256],
    event_tx: mpsc::UnboundedSender<OrderbookWsEvent>,
    reader_died_tx: mpsc::UnboundedSender<()>,
    chunk_index: usize,
) -> Result<(OrderbookWsSubscriptionGuard, JoinSet<()>)> {
    let mut guard = OrderbookWsSubscriptionGuard::new(client.clone(), tokens.to_vec());
    if tokens.is_empty() {
        return Ok((guard, JoinSet::new()));
    }
    let book_stream = client
        .subscribe_orderbook(tokens.to_vec())
        .map_err(|error| {
            AppError::internal(
                "ORDERBOOK_WS_SUBSCRIBE_FAILED",
                format!("failed to subscribe to orderbook websocket: {error}"),
            )
        })?;
    guard.mark_subscribed();
    let price_stream = client.subscribe_prices(tokens.to_vec()).map_err(|error| {
        AppError::internal(
            "ORDERBOOK_WS_PRICE_SUBSCRIBE_FAILED",
            format!("failed to subscribe to orderbook price changes: {error}"),
        )
    })?;
    guard.mark_subscribed();

    let mut readers = JoinSet::new();
    {
        let event_tx = event_tx.clone();
        let reader_died_tx = reader_died_tx.clone();
        readers.spawn(async move {
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
            let _ = reader_died_tx.send(());
        });
    }
    {
        let event_tx = event_tx.clone();
        let reader_died_tx = reader_died_tx.clone();
        readers.spawn(async move {
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
            let _ = reader_died_tx.send(());
        });
    }
    Ok((guard, readers))
}

/// Commands sent by the parent loop to a long-lived chunk session.
enum ChunkCommand {
    /// Reconcile the session's subscription to this new token set (subscribe-new,
    /// then unsubscribe-old on the persistent client; the SDK sends only diff frames).
    Reconcile(Vec<U256>),
    /// Tear the session down (chunk no longer needed).
    Shutdown,
}

#[derive(Debug, Clone, Copy)]
enum ChunkExitReason {
    /// A reader ended naturally — the underlying WS connection died; caller rebuilds.
    ReaderEnded,
    /// The session was asked to shut down via `ChunkCommand::Shutdown`.
    Shutdown,
}

#[derive(Debug)]
struct ChunkExit {
    chunk_index: usize,
    reason: ChunkExitReason,
}

/// A long-lived chunk session: owns a persistent `ClobWsClient`, applies
/// incremental reconcile commands without dropping the connection, and exits
/// (returning `ChunkExit`) only when a reader ends (connection death) or it is
/// shut down. The parent owns the `command_rx` and rebuilds the session on exit.
async fn run_orderbook_chunk_session(
    chunk_index: usize,
    context: OrderbookWsChunkContext,
    initial_tokens: Vec<U256>,
    mut command_rx: mpsc::Receiver<ChunkCommand>,
) -> ChunkExit {
    let client = match make_orderbook_ws_client(&context.ws_host) {
        Ok(client) => client,
        Err(error) => {
            warn!(
                ws_chunk = chunk_index,
                error = %error,
                "orderbook WS chunk session failed to create client"
            );
            return ChunkExit {
                chunk_index,
                reason: ChunkExitReason::ReaderEnded,
            };
        }
    };

    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<OrderbookWsEvent>();
    let (reader_died_tx, mut reader_died_rx) = mpsc::unbounded_channel::<()>();

    let mut current_tokens = initial_tokens.clone();
    let (mut current_guard, mut readers) = match subscribe_orderbook_token_set(
        &client,
        &initial_tokens,
        event_tx.clone(),
        reader_died_tx.clone(),
        chunk_index,
    )
    .await
    {
        Ok(pair) => {
            info!(
                ws_chunk = chunk_index,
                subscribed_tokens = initial_tokens.len(),
                "orderbook WS chunk session subscribed"
            );
            pair
        }
        Err(error) => {
            warn!(
                ws_chunk = chunk_index,
                error = %error,
                "orderbook WS chunk session failed initial subscribe"
            );
            return ChunkExit {
                chunk_index,
                reason: ChunkExitReason::ReaderEnded,
            };
        }
    };

    loop {
        tokio::select! {
            Some(event) = event_rx.recv() => {
                handle_orderbook_ws_event(event, &context, chunk_index).await;
            }
            command = command_rx.recv() => match command {
                Some(ChunkCommand::Reconcile(new_tokens)) => {
                    if u256_sets_same_members(&current_tokens, &new_tokens) {
                        continue;
                    }
                    match subscribe_orderbook_token_set(
                        &client,
                        &new_tokens,
                        event_tx.clone(),
                        reader_died_tx.clone(),
                        chunk_index,
                    )
                    .await
                    {
                        Ok((new_guard, new_readers)) => {
                            // Drop the old guard first so it unsubscribes the old
                            // set AFTER the new set is subscribed — this keeps the
                            // shared Market channel non-empty so the connection
                            // survives. Old readers are then aborted; aborted
                            // readers never signal reader_died (only natural stream
                            // end does), so a connection death is still detected.
                            let old_guard = std::mem::replace(&mut current_guard, new_guard);
                            drop(old_guard);
                            readers.abort_all();
                            while readers.join_next().await.is_some() {}
                            readers = new_readers;
                            let diff = compute_chunk_diff(
                                &u256_to_token_strings(&current_tokens),
                                &u256_to_token_strings(&new_tokens),
                            );
                            current_tokens = new_tokens;
                            info!(
                                ws_chunk = chunk_index,
                                added = diff.added.len(),
                                removed = diff.removed.len(),
                                "orderbook WS chunk session reconciled in place"
                            );
                        }
                        Err(error) => {
                            warn!(
                                ws_chunk = chunk_index,
                                error = %error,
                                "orderbook WS chunk session reconcile subscribe failed; keeping previous subscription"
                            );
                        }
                    }
                }
                Some(ChunkCommand::Shutdown) | None => {
                    return ChunkExit {
                        chunk_index,
                        reason: ChunkExitReason::Shutdown,
                    };
                }
            },
            Some(()) = reader_died_rx.recv() => {
                info!(
                    ws_chunk = chunk_index,
                    "orderbook WS chunk session reader ended; connection will be rebuilt"
                );
                return ChunkExit {
                    chunk_index,
                    reason: ChunkExitReason::ReaderEnded,
                };
            }
        }
    }
}

/// Apply a new aggregated token set to the live chunk sessions: reconcile
/// same-index sessions in place, spawn new sessions for grown chunk counts, and
/// shut down sessions beyond the new chunk count. Updates `ws_token_set` to the
/// new set. Chunk index alignment is preserved (only the tail grows/shrinks).
async fn apply_membership_diff(
    session_cmds: &mut Vec<mpsc::Sender<ChunkCommand>>,
    session_tasks: &mut JoinSet<ChunkExit>,
    ws_token_set: &Arc<RwLock<Vec<String>>>,
    context_template: &OrderbookWsChunkContext,
    new_tokens: Vec<String>,
    ws_chunk_size: usize,
) {
    let new_chunks = partition_tokens_into_chunks(&new_tokens, ws_chunk_size);
    for (index, chunk_tokens) in new_chunks.iter().enumerate() {
        let target_u256: Vec<U256> = chunk_tokens
            .iter()
            .filter_map(|token| U256::from_str(token).ok())
            .collect();
        if index < session_cmds.len() {
            let _ = session_cmds[index]
                .send(ChunkCommand::Reconcile(target_u256))
                .await;
        } else {
            let (command_tx, command_rx) = mpsc::channel::<ChunkCommand>(8);
            session_tasks.spawn(run_orderbook_chunk_session(
                index,
                context_template.clone(),
                target_u256,
                command_rx,
            ));
            session_cmds.push(command_tx);
        }
    }
    while session_cmds.len() > new_chunks.len() {
        if let Some(command_tx) = session_cmds.pop() {
            let _ = command_tx.send(ChunkCommand::Shutdown).await;
        }
    }
    *ws_token_set.write().await = new_tokens;
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

/// Partition an ordered token list into fixed-size chunks (one per WS connection).
/// A `chunk_size` of 0 is treated as 1.
fn partition_tokens_into_chunks(tokens: &[String], chunk_size: usize) -> Vec<Vec<String>> {
    let size = chunk_size.max(1);
    tokens.chunks(size).map(|slice| slice.to_vec()).collect()
}

/// Membership diff between two token sets, used for logging/observability during
/// incremental reconcile. The actual subscribe/unsubscribe diff is handled by the
/// SDK's per-asset refcounting; this is purely informational.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct ChunkDiff {
    added: Vec<String>,
    removed: Vec<String>,
}

fn compute_chunk_diff(current: &[String], target: &[String]) -> ChunkDiff {
    let current_set: HashSet<&str> = current.iter().map(String::as_str).collect();
    let target_set: HashSet<&str> = target.iter().map(String::as_str).collect();
    let added = target
        .iter()
        .filter(|token| !current_set.contains(token.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let removed = current
        .iter()
        .filter(|token| !target_set.contains(token.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    ChunkDiff { added, removed }
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
    let confirmed_at = current_unix_millis();
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
        confirmed_at,
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
    if (current.observed_at == candidate.observed_at && current.source == candidate.source)
        || current.confirmation_time_ms() == candidate.confirmation_time_ms()
    {
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
        book.confirmed_at = current_unix_millis();
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

fn reward_book_to_cached(
    book: &polyedge_connectors::PolymarketRewardOrderBook,
    confirmed_at: i64,
) -> CachedOrderBook {
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
        observed_at: offset_datetime_to_unix_millis(book.observed_at),
        confirmed_at,
        source: BookSource::Poll,
    }
}

fn offset_datetime_to_unix_millis(time: OffsetDateTime) -> i64 {
    let millis = time.unix_timestamp_nanos().div_euclid(1_000_000);
    i64::try_from(millis).unwrap_or_else(|_| {
        if millis.is_negative() {
            i64::MIN
        } else {
            i64::MAX
        }
    })
}

fn current_unix_millis() -> i64 {
    let millis = OffsetDateTime::now_utc()
        .unix_timestamp_nanos()
        .div_euclid(1_000_000);
    i64::try_from(millis).unwrap_or_else(|_| {
        if millis.is_negative() {
            i64::MIN
        } else {
            i64::MAX
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use polyedge_connectors::{PolymarketRewardBookLevel, PolymarketRewardOrderBook};
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
                confirmed_at: 100,
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
    fn poll_cached_book_uses_upstream_observation_and_local_confirmation_time() {
        let upstream_observed_at = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let book = PolymarketRewardOrderBook {
            token_id: "123".to_string(),
            bids: vec![PolymarketRewardBookLevel {
                price: Decimal::new(49, 2),
                size: Decimal::from(10_u64),
            }],
            asks: vec![PolymarketRewardBookLevel {
                price: Decimal::new(51, 2),
                size: Decimal::from(11_u64),
            }],
            observed_at: upstream_observed_at,
        };

        let cached = reward_book_to_cached(&book, 1_800_000_000_123);

        assert_eq!(cached.observed_at, 1_700_000_000_000);
        assert_eq!(cached.confirmed_at, 1_800_000_000_123);
        assert_eq!(cached.source, BookSource::Poll);
        assert_eq!(cached.bids[0].price, Decimal::new(49, 2));
        assert_eq!(cached.asks[0].price, Decimal::new(51, 2));
    }

    #[test]
    fn token_list_member_comparison_ignores_order_only_changes() {
        let left = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let reordered = vec!["c".to_string(), "a".to_string(), "b".to_string()];
        let changed = vec!["a".to_string(), "b".to_string(), "d".to_string()];

        assert!(token_lists_have_same_members(&left, &reordered));
        assert!(!token_lists_have_same_members(&left, &changed));
    }

    #[test]
    fn partition_tokens_into_chunks_respects_size() {
        let tokens = vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
            "e".to_string(),
        ];
        let chunks = partition_tokens_into_chunks(&tokens, 2);
        assert_eq!(
            chunks,
            vec![
                vec!["a".to_string(), "b".to_string()],
                vec!["c".to_string(), "d".to_string()],
                vec!["e".to_string()],
            ]
        );
        // zero size is treated as 1; empty input yields no chunks.
        assert_eq!(
            partition_tokens_into_chunks(&[], 100),
            Vec::<Vec<String>>::new()
        );
        assert_eq!(partition_tokens_into_chunks(&tokens, 0).len(), 5);
    }

    #[test]
    fn compute_chunk_diff_reports_added_and_removed() {
        let current = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let target = vec!["b".to_string(), "c".to_string(), "d".to_string()];
        let diff = compute_chunk_diff(&current, &target);
        assert_eq!(diff.added, vec!["d".to_string()]);
        assert_eq!(diff.removed, vec!["a".to_string()]);

        // identical sets produce an empty diff.
        let same = compute_chunk_diff(&current, &current);
        assert!(same.added.is_empty());
        assert!(same.removed.is_empty());
    }
}
