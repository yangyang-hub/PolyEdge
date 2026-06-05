use futures::StreamExt;
use polyedge_application::{BookSource, CachedBookLevel, CachedOrderBook, OrderbookCache};
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
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

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
pub async fn run_orderbook_stream(state: &AppState) -> Result<OrderbookStreamReport> {
    let settings = &state.settings.orderbook_stream;
    let cache = state.orderbook_cache.clone();
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

    // 3. Create WS client and subscribe.
    let ws_client = ClobWsClient::new(&state.settings.polymarket.ws_host, WsConfig::default())
        .map_err(|error| {
            AppError::internal(
                "ORDERBOOK_WS_INIT_FAILED",
                format!("failed to create orderbook websocket client: {error}"),
            )
        })?;
    let book_stream = ws_client
        .subscribe_orderbook(u256_ids.clone())
        .map_err(|error| {
            AppError::internal(
                "ORDERBOOK_WS_SUBSCRIBE_FAILED",
                format!("failed to subscribe to orderbook websocket: {error}"),
            )
        })?;
    let price_stream = ws_client
        .subscribe_prices(u256_ids.clone())
        .map_err(|error| {
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

    // 4. Shared token list for poll reconciler.
    let shared_tokens: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(token_ids.clone()));
    let ws_token_set: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(token_ids));

    // 5. Spawn poll reconciler.
    let poll_cache = cache.clone();
    let poll_tokens_ref = shared_tokens.clone();
    let poll_interval = settings.poll_reconcile_interval_secs;
    let stale_threshold_ms = settings.stale_threshold_ms as i64;
    let clob_host = state.settings.polymarket.clob_host.clone();
    let poll_max_tokens = settings.max_tokens;
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
                            let cached = reward_book_to_cached(&book);
                            if let Err(error) = poll_cache.set_book(&cached).await {
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

    // 6. Consume WS stream with periodic token set check.
    //    When other services register new tokens via the HTTP API, the registry
    //    changes. We periodically check and reconnect if the set changed.
    let refresh_interval = Duration::from_secs(settings.token_refresh_interval_secs.max(10));
    let mut refresh_timer = tokio::time::interval(refresh_interval);
    refresh_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    refresh_timer.tick().await; // skip first immediate tick

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
                // Check if the registry token set changed (other services may
                // have registered new tokens via HTTP API).
                let new_tokens = collect_orderbook_subscription_tokens(state).await;
                let new_count = new_tokens.len();
                {
                    let mut shared = shared_tokens.write().await;
                    *shared = new_tokens.clone();
                }

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

async fn collect_orderbook_subscription_tokens(state: &AppState) -> Vec<String> {
    let max_tokens = state.settings.orderbook_stream.max_tokens;
    let all = state.orderbook_registry.list_all_tokens().await;
    all.into_iter().take(max_tokens).collect()
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
        cache.set_book(&book).await?;
    }
    Ok(())
}

fn reward_book_to_cached(book: &polyedge_connectors::PolymarketRewardOrderBook) -> CachedOrderBook {
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

        apply_price_change_to_cache(&cache, &update)
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
}
