use polymarket_client_sdk::clob::ws::BookUpdate;
use polymarket_client_sdk::clob::ws::Client as ClobWsClient;
use polymarket_client_sdk::ws::config::Config as WsConfig;
use polymarket_client_sdk::types::U256;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct OrderbookStreamReport {
    subscribed_tokens: usize,
    ws_snapshots_received: usize,
    poll_reconciliations: usize,
    poll_failures: usize,
}

async fn consume_orderbook_stream(state: &AppState) -> Result<OrderbookStreamReport> {
    let settings = &state.settings.orderbook_stream;
    let cache = state.orderbook_cache.clone();
    let mut report = OrderbookStreamReport::default();

    // 1. Collect token IDs from multiple sources
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
    let stream = ws_client.subscribe_orderbook(u256_ids.clone()).map_err(|error| {
        AppError::internal(
            "ORDERBOOK_WS_SUBSCRIBE_FAILED",
            format!("failed to subscribe to orderbook websocket: {error}"),
        )
    })?;
    let mut stream = Box::pin(stream);

    info!(
        subscribed_tokens = u256_ids.len(),
        "orderbook stream subscribed to market channel"
    );

    // 4. Spawn poll reconciler as a background companion task
    let poll_cache = cache.clone();
    let poll_token_ids = token_ids.clone();
    let poll_interval = settings.poll_reconcile_interval_secs;
    let stale_threshold_ms = settings.stale_threshold_ms as i64;
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

            let stale = match poll_cache
                .get_stale_tokens(&poll_token_ids, stale_threshold_ms)
                .await
            {
                Ok(tokens) => tokens,
                Err(error) => {
                    warn!(error = %error, "orderbook poll reconciler failed to get stale tokens");
                    continue;
                }
            };

            if stale.is_empty() {
                continue;
            }

            debug!(stale_count = stale.len(), "orderbook poll reconciler checking stale tokens");

            let fetch_limit = stale.len().min(poll_max_tokens);
            let to_fetch = &stale[..fetch_limit];

            match connector.fetch_order_books(to_fetch).await {
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
    });

    // 5. Consume WS stream
    while let Some(message) = stream.next().await {
        match message {
            Ok(book_update) => {
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
            Err(error) => {
                warn!(error = %error, "orderbook WS stream error, poll reconciler will cover gaps");
            }
        }
    }

    poll_handle.abort();

    report.poll_reconciliations = poll_reconciliations.load(Ordering::Relaxed);
    report.poll_failures = poll_failures.load(Ordering::Relaxed);

    info!(
        subscribed_tokens = report.subscribed_tokens,
        ws_snapshots_received = report.ws_snapshots_received,
        "orderbook stream consumer stopped"
    );

    Ok(report)
}

async fn collect_orderbook_subscription_tokens(state: &AppState) -> Result<Vec<String>> {
    let max_tokens = state.settings.orderbook_stream.max_tokens;
    let mut seen = HashSet::new();
    let mut tokens = Vec::new();

    // Source 1: Active orders (Submitted/Open/PartiallyFilled)
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
                return Ok(tokens);
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

            if !seen.contains(&market_refs.yes_asset_id) {
                seen.insert(market_refs.yes_asset_id.clone());
                tokens.push(market_refs.yes_asset_id.clone());
            }
            if tokens.len() >= max_tokens {
                return Ok(tokens);
            }
            if !seen.contains(&market_refs.no_asset_id) {
                seen.insert(market_refs.no_asset_id.clone());
                tokens.push(market_refs.no_asset_id.clone());
            }
        }
    }

    // Source 2: Open markets with polymarket refs (for arbitrage/general monitoring)
    let market_limit = u16::try_from((max_tokens / 2).min(usize::from(u16::MAX)))
        .unwrap_or(u16::MAX);
    let open_markets = state
        .market_event_service
        .list_markets(MarketListFilters::new(
            Some(MarketStatus::Open),
            None,
            None,
            None,
            None,
            None,
            Some(market_limit),
        )?)
        .await?;

    for market in open_markets {
        if tokens.len() >= max_tokens {
            return Ok(tokens);
        }
        let market_refs = match polymarket_market_refs(&market) {
            Ok(refs) => refs,
            Err(_) => continue,
        };
        if !seen.contains(&market_refs.yes_asset_id) {
            seen.insert(market_refs.yes_asset_id.clone());
            tokens.push(market_refs.yes_asset_id);
        }
        if tokens.len() >= max_tokens {
            return Ok(tokens);
        }
        if !seen.contains(&market_refs.no_asset_id) {
            seen.insert(market_refs.no_asset_id.clone());
            tokens.push(market_refs.no_asset_id);
        }
    }

    // Source 3: Reward markets (for reward bot simulation)
    if let Ok(reward_markets) = state
        .reward_bot_service
        .list_active_reward_markets()
        .await
    {
        let reward_token_ids = select_reward_book_token_ids(&reward_markets);
        for token_id in reward_token_ids {
            if tokens.len() >= max_tokens {
                return Ok(tokens);
            }
            if !seen.contains(&token_id) {
                seen.insert(token_id.clone());
                tokens.push(token_id);
            }
        }
    }

    Ok(tokens)
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
