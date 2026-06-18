use polyedge_application::OrderbookCache;
use polymarket_client_sdk::clob::{
    types::Side,
    ws::{BookUpdate, PriceChange},
};
use polymarket_client_sdk::clob::ws::Client as ClobWsClient;
use polymarket_client_sdk::ws::config::Config as WsConfig;
use polymarket_client_sdk::types::U256;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct OrderbookStreamReport {
    subscribed_tokens: usize,
    ws_snapshots_received: usize,
    ws_price_changes_received: usize,
    poll_reconciliations: usize,
    poll_failures: usize,
}

async fn consume_orderbook_stream(state: &AppState) -> Result<OrderbookStreamReport> {
    let settings = &state.settings.orderbook_stream;
    let cache = state.orderbook_cache.clone();
    let mut report = OrderbookStreamReport::default();

    // 1. Register token sources into the subscription registry, then collect the
    //    aggregated & deduplicated set.
    register_exec_order_tokens(state).await?;
    register_reward_tokens(state).await?;

    let token_ids = collect_orderbook_subscription_tokens(state).await?;
    report.subscribed_tokens = token_ids.len();

    if token_ids.is_empty() {
        info!(
            "skipping orderbook stream because there are no markets to subscribe to"
        );
        return Ok(report);
    }

    // 2. Convert to U256 for SDK
    let u256_ids: Vec<U256> = token_ids
        .iter()
        .filter_map(|id| U256::from_str(id).ok())
        .collect();

    if u256_ids.is_empty() {
        warn!("no valid U256 token IDs found for orderbook subscription");
        return Ok(report);
    }

    // 3. Create unauthenticated WS client (market channel is public)
    let ws_client = ClobWsClient::new(
        &state.settings.polymarket.ws_host,
        WsConfig::default(),
    ).map_err(|error| {
        AppError::internal(
            "ORDERBOOK_WS_INIT_FAILED",
            format!("failed to create orderbook websocket client: {error}"),
        )
    })?;
    let book_stream = ws_client.subscribe_orderbook(u256_ids.clone()).map_err(|error| {
        AppError::internal(
            "ORDERBOOK_WS_SUBSCRIBE_FAILED",
            format!("failed to subscribe to orderbook websocket: {error}"),
        )
    })?;
    let price_stream = ws_client.subscribe_prices(u256_ids.clone()).map_err(|error| {
        AppError::internal(
            "ORDERBOOK_WS_PRICE_SUBSCRIBE_FAILED",
            format!("failed to subscribe to orderbook price changes: {error}"),
        )
    })?;
    let mut book_stream = Box::pin(book_stream);
    let mut price_stream = Box::pin(price_stream);

    info!(
        subscribed_tokens = u256_ids.len(),
        "orderbook stream subscribed to market channel"
    );

    // 4. Shared token list: the poll reconciler reads from this; the refresh
    //    timer updates it when new reward markets appear.
    let shared_tokens: Arc<RwLock<Vec<String>>> =
        Arc::new(RwLock::new(token_ids.clone()));
    let ws_token_set: Arc<RwLock<Vec<String>>> =
        Arc::new(RwLock::new(token_ids));

    // 5. Spawn poll reconciler as a background companion task.
    //    It reads the token list from `shared_tokens` each cycle so newly
    //    added reward markets are picked up without a WS reconnect.
    let poll_cache = cache.clone();
    let poll_tokens_ref = shared_tokens.clone();
    let poll_interval = settings.poll_reconcile_interval_secs;
    let clob_host = state.settings.polymarket.clob_host.clone();
    let poll_max_tokens = settings.max_tokens;
    let poll_reconciliations = Arc::new(AtomicUsize::new(0));
    let poll_failures = Arc::new(AtomicUsize::new(0));
    let poll_reconciliations_clone = poll_reconciliations.clone();
    let poll_failures_clone = poll_failures.clone();

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

            // Read the latest token list (may have been updated by refresh timer).
            let current_tokens = poll_tokens_ref.read().await.clone();
            let fetch_limit = current_tokens.len().min(poll_max_tokens);
            if fetch_limit == 0 {
                continue;
            }

            debug!(
                target_count = fetch_limit,
                "orderbook poll reconciler refreshing registered tokens"
            );
            for chunk in current_tokens[..fetch_limit].chunks(100) {
                match connector.fetch_order_books(chunk).await {
                    Ok(books) => {
                        for book in books {
                            let cached = reward_book_to_cached(&book);
                            if let Err(error) = poll_cache.set_book(&cached).await {
                                warn!(
                                    token_id = %cached.token_id,
                                    error = %error,
                                    "orderbook poll reconciler failed to write book to cache"
                                );
                            }
                        }
                        poll_reconciliations_clone.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(error) => {
                        poll_failures_clone.fetch_add(1, Ordering::Relaxed);
                        warn!(
                            error = %error,
                            "orderbook poll reconciler failed to fetch books"
                        );
                    }
                }
            }
        }
    });

    // 6. Consume WS stream with periodic token refresh.
    //    When new reward markets appear the poll reconciler picks them up
    //    immediately (via `shared_tokens`). If the WS-subscribed set also
    //    changed, we break the loop so `spawn_restarting_job` reconnects
    //    with the updated token list.
    let refresh_interval = Duration::from_secs(
        settings.token_refresh_interval_secs.max(10),
    );
    let mut refresh_timer = tokio::time::interval(refresh_interval);
    refresh_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    // The first tick fires immediately; skip it (we just subscribed).
    refresh_timer.tick().await;

    loop {
        tokio::select! {
            message = book_stream.next() => {
                match message {
                    Some(Ok(book_update)) => {
                        let cached = book_update_to_cached(&book_update);
                        if let Err(error) = cache.set_book(&cached).await {
                            warn!(
                                token_id = %cached.token_id,
                                error = %error,
                                "failed to write orderbook snapshot to cache"
                            );
                        }
                        report.ws_snapshots_received += 1;

                        if report.ws_snapshots_received % 100 == 0 {
                            debug!(
                                received = report.ws_snapshots_received,
                                "orderbook stream processing snapshots"
                            );
                        }
                    }
                    Some(Err(error)) => {
                        warn!(error = %error, "orderbook WS stream error, poll reconciler will cover gaps");
                    }
                    None => {
                        info!("orderbook WS stream ended");
                        break;
                    }
                }
            }
            message = price_stream.next() => {
                match message {
                    Some(Ok(price_change)) => {
                        if let Err(error) = apply_price_change_to_cache(&cache, &price_change).await {
                            warn!(error = %error, "failed to apply orderbook price change");
                        }
                        report.ws_price_changes_received += 1;
                    }
                    Some(Err(error)) => {
                        warn!(error = %error, "orderbook price-change WS stream error, poll reconciler will cover gaps");
                    }
                    None => {
                        info!("orderbook price-change WS stream ended");
                        break;
                    }
                }
            }
            _ = refresh_timer.tick() => {
                // Re-register token sources so the registry reflects latest state.
                let _ = register_exec_order_tokens(state).await;
                let _ = register_reward_tokens(state).await;

                // Always update the shared list so the poll reconciler picks up
                // new markets immediately.
                let new_tokens = collect_orderbook_subscription_tokens(state).await
                    .unwrap_or_default();
                let new_count = new_tokens.len();
                {
                    let mut shared = shared_tokens.write().await;
                    *shared = new_tokens.clone();
                }

                // Check if the WS-subscribed set changed.
                let old_set = ws_token_set.read().await;
                let changed = *old_set != new_tokens;
                drop(old_set);

                if changed {
                    info!(
                        old = report.subscribed_tokens,
                        new = new_count,
                        "orderbook token list changed, reconnecting WS with new set"
                    );
                    report.subscribed_tokens = new_count;
                    *ws_token_set.write().await = new_tokens;
                    break;
                }
            }
        }
    }

    poll_handle.abort();

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

async fn collect_orderbook_subscription_tokens(state: &AppState) -> Result<Vec<String>> {
    let max_tokens = state.settings.orderbook_stream.max_tokens;
    let all = state.orderbook_registry.list_all_tokens().await;
    Ok(all.into_iter().take(max_tokens).collect())
}

/// Register tokens from active execution orders into the subscription registry.
async fn register_exec_order_tokens(state: &AppState) -> Result<()> {
    let max_tokens = state.settings.orderbook_stream.max_tokens;
    let mut tokens = Vec::new();

    for status in [
        OrderStatus::Submitted,
        OrderStatus::Open,
        OrderStatus::PartiallyFilled,
    ] {
        let fetch_limit = u16::try_from(max_tokens.saturating_mul(2).min(usize::from(u16::MAX)))
            .unwrap_or(u16::MAX);
        let orders = state
            .execution_service
            .list_orders(OrderListFilters::new(
                None,
                None,
                Some(POLYMARKET_CONNECTOR_NAME.to_string()),
                Some(status),
                Some(fetch_limit),
            )?)
            .await?;

        for order in orders {
            if tokens.len() >= max_tokens {
                break;
            }
            let market = match state
                .market_event_service
                .get_market(&order.market_id)
                .await
            {
                Ok(m) => m,
                Err(_) => continue,
            };
            let market_refs = match polymarket_market_refs(&market) {
                Ok(refs) => refs,
                Err(_) => continue,
            };
            tokens.push(market_refs.yes_asset_id);
            tokens.push(market_refs.no_asset_id);
        }
    }

    state
        .orderbook_registry
        .register_tokens("exec_orders", &tokens)
        .await?;
    Ok(())
}

/// Register tokens from all reward candidate markets into the subscription registry.
async fn register_reward_tokens(state: &AppState) -> Result<()> {
    let cap = state.settings.orderbook_stream.reward_candidate_token_cap;
    if cap == 0 {
        state.orderbook_registry.register_tokens("rewards", &[]).await?;
        return Ok(());
    }
    if let Ok(reward_token_ids) = state
        .reward_bot_service
        .list_all_reward_candidate_token_ids()
        .await
    {
        let reward_token_ids: Vec<String> = reward_token_ids.into_iter().take(cap).collect();
        state
            .orderbook_registry
            .register_tokens("rewards", &reward_token_ids)
            .await?;
    }
    Ok(())
}

fn book_update_to_cached(update: &BookUpdate) -> CachedOrderBook {
    let bids: Vec<CachedBookLevel> = update
        .bids
        .iter()
        .map(|level| CachedBookLevel {
            price: level.price,
            size: level.size,
        })
        .collect();

    let asks: Vec<CachedBookLevel> = update
        .asks
        .iter()
        .map(|level| CachedBookLevel {
            price: level.price,
            size: level.size,
        })
        .collect();

    CachedOrderBook {
        token_id: update.asset_id.to_string(),
        bids,
        asks,
        observed_at: update.timestamp,
        source: BookSource::Ws,
    }
}

async fn apply_price_change_to_cache(
    cache: &Arc<dyn OrderbookCache>,
    update: &PriceChange,
) -> Result<()> {
    for change in &update.price_changes {
        let token_id = change.asset_id.to_string();
        let Some(mut book) = cache.get_book(&token_id).await? else {
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
        if size <= Decimal::ZERO {
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
        cache.set_book(&book).await?;
    }
    Ok(())
}

fn reward_book_to_cached(book: &PolymarketRewardOrderBook) -> CachedOrderBook {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

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
        observed_at: now_ms,
        source: BookSource::Poll,
    }
}
