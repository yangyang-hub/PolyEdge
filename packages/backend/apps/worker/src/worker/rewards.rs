async fn run_reward_bot_once(state: &AppState, trace_id: &str) -> Result<RewardBotRunReport> {
    let (markets, books) = fetch_reward_bot_inputs(state).await?;
    state
        .reward_bot_service
        .run_simulation(markets, books, trace_id)
        .await
}

async fn poll_reward_bot(
    state: &AppState,
    max_cycles: Option<usize>,
) -> Result<RewardBotRunReport> {
    let mut total = RewardBotRunReport {
        markets_scanned: 0,
        books_fetched: 0,
        plans_built: 0,
        eligible_plans: 0,
        simulated_orders: 0,
        cancelled_orders: 0,
        filled_orders: 0,
        risk_cancelled_orders: 0,
        reward_accrued: rust_decimal::Decimal::ZERO,
    };
    let mut full_cycles = 0usize;
    let mut reconcile_cycles = 0usize;
    let full_interval = Duration::from_secs(state.settings.rewards.poll_interval_secs.max(1));
    // Start with a full cycle immediately.
    let mut last_full_at = Instant::now() - full_interval;

    loop {
        // Read the live config to get the reconcile interval.
        let config = state.reward_bot_service.read_config().await.unwrap_or_default();
        let reconcile_interval = Duration::from_secs(config.reconcile_interval_sec.max(1));
        let now = Instant::now();
        let since_full = now.duration_since(last_full_at);

        if since_full >= full_interval {
            // --- Full simulation cycle (rebuilds plans) ---
            let trace_id = new_trace_id();
            let report = run_reward_bot_once(state, &trace_id).await?;
            accumulate_report(&mut total, &report);
            full_cycles += 1;
            last_full_at = Instant::now();

            info!(
                trace_id = %trace_id,
                full_cycle = full_cycles,
                markets_scanned = report.markets_scanned,
                eligible_plans = report.eligible_plans,
                cancelled = report.cancelled_orders,
                risk_cancelled = report.risk_cancelled_orders,
                "completed full reward bot cycle",
            );

            if max_cycles.is_some_and(|limit| full_cycles >= limit) {
                break;
            }
        } else {
            // --- Fast reconcile-only cycle (risk checks + fills + quotes) ---
            let trace_id = new_trace_id();
            let books = fetch_reward_bot_active_books(state).await?;
            let report = state
                .reward_bot_service
                .run_reconcile_only(books, &trace_id)
                .await?;
            accumulate_report(&mut total, &report);
            reconcile_cycles += 1;

            if report.risk_cancelled_orders > 0 || report.filled_orders > 0 {
                info!(
                    trace_id = %trace_id,
                    reconcile_cycle = reconcile_cycles,
                    risk_cancelled = report.risk_cancelled_orders,
                    filled = report.filled_orders,
                    "fast reconcile cycle",
                );
            }
        }

        // Sleep until the next reconcile tick or the next full cycle, whichever
        // comes first. Also check for shutdown.
        let elapsed_since_full = Instant::now().duration_since(last_full_at);
        let next_full_in = full_interval.checked_sub(elapsed_since_full).unwrap_or(reconcile_interval);
        let sleep_dur = reconcile_interval.min(next_full_in);

        tokio::select! {
            () = tokio::time::sleep(sleep_dur) => {}
            shutdown = tokio::signal::ctrl_c() => {
                if let Err(error) = shutdown {
                    warn!(error = %error, "failed to listen for ctrl-c during reward bot polling");
                }
                break;
            }
        }
    }

    Ok(total)
}

fn accumulate_report(total: &mut RewardBotRunReport, report: &RewardBotRunReport) {
    total.markets_scanned += report.markets_scanned;
    total.books_fetched += report.books_fetched;
    total.plans_built += report.plans_built;
    total.eligible_plans += report.eligible_plans;
    total.simulated_orders += report.simulated_orders;
    total.cancelled_orders += report.cancelled_orders;
    total.filled_orders += report.filled_orders;
    total.risk_cancelled_orders += report.risk_cancelled_orders;
    total.reward_accrued += report.reward_accrued;
}

async fn fetch_reward_bot_inputs(
    state: &AppState,
) -> Result<(Vec<RewardMarket>, HashMap<String, RewardOrderBook>)> {
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

/// Lightweight book fetch for the fast reconcile loop: only reads books for
/// tokens where the bot currently has open orders or positions (not the full
/// candidate market set).
async fn fetch_reward_bot_active_books(
    state: &AppState,
) -> Result<HashMap<String, RewardOrderBook>> {
    let token_ids = state
        .reward_bot_service
        .list_active_reward_book_token_ids()
        .await?;

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

    Ok(books)
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
                .map(|dt| dt + TimeDuration::nanoseconds(i64::from(nsecs)))
                .unwrap_or_else(|_| OffsetDateTime::now_utc())
        },
    }
}

fn reward_market_from_connector(market: PolymarketRewardMarket) -> RewardMarket {
    RewardMarket {
        condition_id: market.condition_id,
        question: market.question,
        market_slug: market.market_slug,
        event_slug: market.event_slug,
        image: market.image,
        rewards_max_spread: market.rewards_max_spread,
        rewards_min_size: market.rewards_min_size,
        total_daily_rate: market.total_daily_rate,
        tokens: market
            .tokens
            .into_iter()
            .map(|token| RewardToken {
                token_id: token.token_id,
                outcome: token.outcome,
                price: token.price,
            })
            .collect(),
        active: market.active,
        updated_at: market.updated_at,
    }
}
