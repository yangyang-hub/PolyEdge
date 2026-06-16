async fn poll_reward_bot(
    state: &AppState,
    max_cycles: Option<usize>,
) -> Result<RewardBotRunReport> {
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    poll_reward_bot_loop(state, max_cycles, shutdown_rx, true).await
}

async fn poll_reward_bot_until_shutdown(
    state: &AppState,
    shutdown_rx: watch::Receiver<bool>,
) -> Result<RewardBotRunReport> {
    poll_reward_bot_loop(state, None, shutdown_rx, false).await
}

async fn poll_reward_bot_loop(
    state: &AppState,
    max_cycles: Option<usize>,
    mut shutdown_rx: watch::Receiver<bool>,
    listen_for_ctrl_c: bool,
) -> Result<RewardBotRunReport> {
    let Some(lease) = state
        .try_acquire_postgres_advisory_lease(REWARD_WORKER_ADVISORY_LOCK_KEY)
        .await?
    else {
        info!("rewards poll loop is standing by because another worker owns the live lease");
        return Ok(RewardBotRunReport::default());
    };
    info!("rewards poll loop acquired live lease");
    // Keep exactly one authenticated connector alive for the whole poll loop.
    // A separate guarded task sends the CLOB heartbeat every five seconds.
    let connector = build_live_polymarket_connector(state).await?;
    let _heartbeat_guard = RewardHeartbeatGuard::spawn(connector.clone());
    let orderbook_runtime = RewardOrderbookRuntime::spawn(state);
    let mut orderbook_wake_rx = orderbook_runtime.subscribe();
    let mut total = RewardBotRunReport {
        markets_scanned: 0,
        books_fetched: 0,
        plans_built: 0,
        eligible_plans: 0,
        placed_orders: 0,
        cancelled_orders: 0,
        filled_orders: 0,
        risk_cancelled_orders: 0,
        reward_accrued: rust_decimal::Decimal::ZERO,
    };
    let mut full_cycles = 0usize;
    let mut reconcile_cycles = 0usize;
    let mut last_reconcile_at: Instant;
    let mut book_history: HashMap<String, VecDeque<BookSnapshot>> = HashMap::new();
    let full_interval = Duration::from_secs(state.settings.rewards.poll_interval_secs.max(1));
    // Start with a full cycle immediately.
    let mut last_full_at = Instant::now() - full_interval;
    let mut runtime_revision_rx = state.reward_bot_service.subscribe_runtime_changes();
    let mut command_wake_rx = state.reward_bot_service.subscribe_command_wake();
    let mut config_revision = runtime_revision_rx.borrow().1;

    loop {
        // Read the live config to get the reconcile interval.
        let config = match state.reward_bot_service.read_config().await {
            Ok(config) => config,
            Err(error) => {
                warn!(error = %error, "failed to read rewards config; retrying poll loop");
                tokio::select! {
                    () = tokio::time::sleep(Duration::from_secs(1)) => {}
                    changed = shutdown_rx.changed() => {
                        if changed.is_err() || *shutdown_rx.borrow() {
                            break;
                        }
                    }
                }
                continue;
            }
        };
        if let Err(error) = state
            .reward_bot_service
            .record_worker_heartbeat(&config.account_id, OffsetDateTime::now_utc())
            .await
        {
            warn!(error = %error, "failed to record rewards worker heartbeat");
        }
        let reconcile_interval = Duration::from_secs(config.reconcile_interval_sec.max(1));

        // Always drain queued control commands first.
        let command_report = process_pending_reward_control_commands_unlocked(
            state,
            &connector,
            &mut book_history,
            Some(orderbook_runtime.cache()),
        )
        .await?;
        if command_report.processed > 0 {
            accumulate_report(&mut total, &command_report.report);
            // A RunOnce command already rebuilt quotes, so treat it as a full
            // cycle; cancel/reset-only commands must NOT reset the timer or a
            // steady stream of them would starve quote rebuilding entirely.
            if command_report.ran_full_cycle {
                last_full_at = Instant::now();
            }
        }

        // Then advance the full/reconcile schedule on its own timer, independent
        // of command activity.
        let since_full = Instant::now().duration_since(last_full_at);
        if since_full >= full_interval {
            // --- Full cycle (rebuilds plans) ---
            let trace_id = new_trace_id();
            let report = run_reward_bot_tick(
                state,
                &connector,
                &trace_id,
                false,
                &mut book_history,
                Some(orderbook_runtime.cache()),
            )
            .await?;
            accumulate_report(&mut total, &report);
            full_cycles += 1;
            last_full_at = Instant::now();
            last_reconcile_at = last_full_at;

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
            // --- Fast reconcile-only cycle (risk checks + cancel stale orders) ---
            let trace_id = new_trace_id();
            let report = run_reward_bot_live_reconcile_unlocked(
                state,
                &connector,
                &trace_id,
                &mut book_history,
                Some(orderbook_runtime.cache()),
            )
            .await?;
            accumulate_report(&mut total, &report);
            reconcile_cycles += 1;
            last_reconcile_at = Instant::now();

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
            changed = runtime_revision_rx.changed() => {
                if changed.is_err() {
                    break;
                }
                let next_config_revision = runtime_revision_rx.borrow_and_update().1;
                if next_config_revision != config_revision {
                    config_revision = next_config_revision;
                    last_full_at = Instant::now() - full_interval;
                }
            }
            _ = command_wake_rx.changed() => {}
            changed = orderbook_wake_rx.changed() => {
                if changed.is_err() {
                    break;
                }
                if throttle_reward_orderbook_reconcile(last_reconcile_at, &mut shutdown_rx).await {
                    break;
                }
            }
            changed = shutdown_rx.changed() => {
                if changed.is_err() || *shutdown_rx.borrow() {
                    break;
                }
            }
            shutdown = tokio::signal::ctrl_c(), if listen_for_ctrl_c => {
                if let Err(error) = shutdown {
                    warn!(error = %error, "failed to listen for ctrl-c during reward bot polling");
                }
                break;
            }
        }
    }

    drop(_heartbeat_guard);
    lease.release().await?;
    Ok(total)
}

async fn throttle_reward_orderbook_reconcile(
    last_reconcile_at: Instant,
    shutdown_rx: &mut watch::Receiver<bool>,
) -> bool {
    let min_interval = Duration::from_secs(1);
    let elapsed = Instant::now().duration_since(last_reconcile_at);
    if elapsed >= min_interval {
        return false;
    }
    tokio::select! {
        () = tokio::time::sleep(min_interval - elapsed) => false,
        changed = shutdown_rx.changed() => {
            changed.is_err() || *shutdown_rx.borrow()
        }
    }
}

struct RewardHeartbeatGuard {
    handle: tokio::task::JoinHandle<()>,
}

impl RewardHeartbeatGuard {
    fn spawn(connector: LivePolymarketConnector) -> Self {
        let handle = tokio::spawn(async move {
            let mut heartbeat_id: Option<String> = None;
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

            loop {
                interval.tick().await;
                match tokio::time::timeout(
                    Duration::from_secs(4),
                    connector.post_heartbeat(heartbeat_id.as_deref()),
                )
                .await
                {
                    Ok(Ok(next_heartbeat_id)) => heartbeat_id = Some(next_heartbeat_id),
                    Ok(Err(error)) => {
                        // Resetting the id lets the next request establish a new
                        // heartbeat chain after an expired or invalid id.
                        heartbeat_id = None;
                        warn!(error = %error, "failed to maintain Polymarket rewards heartbeat");
                    }
                    Err(_) => {
                        heartbeat_id = None;
                        warn!("timed out while maintaining Polymarket rewards heartbeat");
                    }
                }
            }
        });
        Self { handle }
    }
}

impl Drop for RewardHeartbeatGuard {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

fn accumulate_report(total: &mut RewardBotRunReport, report: &RewardBotRunReport) {
    total.markets_scanned += report.markets_scanned;
    total.books_fetched += report.books_fetched;
    total.plans_built += report.plans_built;
    total.eligible_plans += report.eligible_plans;
    total.placed_orders += report.placed_orders;
    total.cancelled_orders += report.cancelled_orders;
    total.filled_orders += report.filled_orders;
    total.risk_cancelled_orders += report.risk_cancelled_orders;
    total.reward_accrued += report.reward_accrued;
}

async fn fetch_reward_bot_inputs(
    state: &AppState,
    orderbook_cache: Option<&RewardOrderbookLocalCache>,
) -> Result<(Vec<RewardMarket>, HashMap<String, RewardOrderBook>)> {
    // Read a bounded candidate pool from database (synced by the sync-markets worker).
    let markets = state
        .reward_bot_service
        .list_reward_run_candidate_markets()
        .await?;

    // Read order books from the worker-local cache maintained by orderbook-stream.
    let active_token_ids = state
        .reward_bot_service
        .list_active_reward_book_token_ids()
        .await?;
    let mut seen = HashSet::new();
    let mut token_ids = Vec::new();
    for token_id in active_token_ids
        .into_iter()
        .chain(select_reward_book_token_ids(&markets))
    {
        if seen.insert(token_id.clone()) {
            token_ids.push(token_id);
        }
    }
    Ok((
        markets,
        fetch_cached_reward_books(state, orderbook_cache, &token_ids).await?,
    ))
}

/// Lightweight book fetch for the fast reconcile loop: only reads books for
/// tokens where the bot currently has open orders or positions (not the full
/// candidate market set).
async fn fetch_reward_bot_active_books(
    state: &AppState,
    orderbook_cache: Option<&RewardOrderbookLocalCache>,
) -> Result<HashMap<String, RewardOrderBook>> {
    let token_ids = state
        .reward_bot_service
        .list_active_reward_book_token_ids()
        .await?;

    fetch_cached_reward_books(state, orderbook_cache, &token_ids).await
}

async fn fetch_cached_reward_books(
    state: &AppState,
    orderbook_cache: Option<&RewardOrderbookLocalCache>,
    token_ids: &[String],
) -> Result<HashMap<String, RewardOrderBook>> {
    let batch_size = state.settings.orderbook_stream.max_tokens;
    if batch_size == 0 || token_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let cached_books = if let Some(orderbook_cache) = orderbook_cache {
        let mut cached_books = orderbook_cache.get_books(token_ids).await;
        let present = cached_books
            .iter()
            .map(|book| book.token_id.clone())
            .collect::<HashSet<_>>();
        let missing = token_ids
            .iter()
            .filter(|token_id| !present.contains(*token_id))
            .cloned()
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            let remote_books = fetch_remote_cached_orderbooks(state, &missing).await?;
            for book in &remote_books {
                cached_books.push(book.clone());
            }
            orderbook_cache.apply_books(remote_books).await;
        }
        cached_books
    } else {
        fetch_remote_cached_orderbooks(state, token_ids).await?
    };

    let mut books = HashMap::new();
    for cached in cached_books {
        books.insert(cached.token_id.clone(), cached_order_book_to_reward(&cached));
    }
    Ok(books)
}

fn record_reward_book_history(
    history: &mut HashMap<String, VecDeque<BookSnapshot>>,
    books: &HashMap<String, RewardOrderBook>,
) {
    for book in books.values() {
        let snapshots = history
            .entry(book.token_id.clone())
            .or_default();
        if snapshots
            .back()
            .is_some_and(|snapshot| snapshot.observed_at >= book.observed_at)
        {
            continue;
        }
        snapshots.push_back(BookSnapshot {
            bids: book.bids.clone(),
            asks: book.asks.clone(),
            observed_at: book.observed_at,
        });
        while snapshots.len() > REWARD_BOOK_HISTORY_LIMIT {
            snapshots.pop_front();
        }
    }
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
        category: String::new(),
        image: market.image,
        rewards_max_spread: market.rewards_max_spread,
        rewards_min_size: market.rewards_min_size,
        total_daily_rate: market.total_daily_rate,
        liquidity_usd: Decimal::ZERO,
        volume_24h_usd: Decimal::ZERO,
        market_spread_cents: Decimal::ZERO,
        end_at: None,
        ambiguity_level: "unknown".to_string(),
        market_synced_at: None,
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
