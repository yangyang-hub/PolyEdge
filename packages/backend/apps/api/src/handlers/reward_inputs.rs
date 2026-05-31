async fn fetch_reward_bot_inputs(
    state: &AppState,
    _trace_id: &str,
) -> polyedge_domain::Result<(Vec<RewardMarket>, HashMap<String, RewardOrderBook>)> {
    // Read a bounded candidate pool from database (synced by the sync-markets worker).
    let markets = state
        .reward_bot_service
        .list_reward_run_candidate_markets()
        .await?;

    // Read order books from Redis cache (written by orderbook-stream worker).
    let token_ids = select_reward_book_token_ids(&markets);
    let mut books = HashMap::new();
    let cache = state.orderbook_cache.clone();
    let cached_books = stream::iter(token_ids)
        .map(|token_id| {
            let cache = cache.clone();
            async move { cache.get_book(&token_id).await }
        })
        .buffer_unordered(32)
        .collect::<Vec<_>>()
        .await;

    for cached in cached_books {
        if let Some(cached) = cached? {
            books.insert(cached.token_id.clone(), cached_order_book_to_reward(&cached));
        }
    }

    Ok((markets, books))
}

fn cached_order_book_to_reward(book: &CachedOrderBook) -> RewardOrderBook {
    RewardOrderBook {
        token_id: book.token_id.clone(),
        bids: book
            .bids
            .iter()
            .map(|level| RewardBookLevel {
                price: level.price,
                size: level.size,
            })
            .collect(),
        asks: book
            .asks
            .iter()
            .map(|level| RewardBookLevel {
                price: level.price,
                size: level.size,
            })
            .collect(),
        observed_at: {
            let secs = book.observed_at / 1000;
            let nsecs = ((book.observed_at % 1000) * 1_000_000) as u32;
            OffsetDateTime::from_unix_timestamp(secs)
                .map(|dt| dt + time::Duration::nanoseconds(i64::from(nsecs)))
                .unwrap_or_else(|_| OffsetDateTime::now_utc())
        },
    }
}
