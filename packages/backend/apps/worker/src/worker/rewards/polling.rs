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
    let mut external_sync_throttle = RewardExternalSyncThrottle::default();
    let mut book_history: HashMap<String, VecDeque<BookSnapshot>> = HashMap::new();
    let mut low_competition_probe = LowCompetitionProbeState::default();
    let full_interval = Duration::from_secs(state.settings.rewards.poll_interval_secs.max(1));
    let history_prune_interval = Duration::from_secs(REWARD_HISTORY_PRUNE_INTERVAL_SECS);
    // Start with a full cycle immediately.
    let mut last_full_at = Instant::now() - full_interval;
    let mut last_history_prune_at = Instant::now() - history_prune_interval;
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
        maybe_prune_reward_history(state, &mut last_history_prune_at, history_prune_interval)
            .await;
        let reconcile_interval = Duration::from_secs(config.reconcile_interval_sec.max(1));

        // Always drain queued control commands first.
        let command_report = process_pending_reward_control_commands_unlocked(
            state,
            &connector,
            &mut book_history,
            Some(orderbook_runtime.cache()),
            Some(&mut low_competition_probe),
        )
        .await?;
        if command_report.processed > 0 {
            accumulate_report(&mut total, &command_report.report);
            // A RunOnce command already rebuilt quotes, so treat it as a full
            // cycle; cancel/reset-only commands must NOT reset the timer or a
            // steady stream of them would starve quote rebuilding entirely.
            if command_report.ran_full_cycle {
                let now = Instant::now();
                last_full_at = now;
                external_sync_throttle.mark_full_sync(now);
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
                Some(&mut low_competition_probe),
            )
            .await?;
            accumulate_report(&mut total, &report);
            full_cycles += 1;
            let now = Instant::now();
            last_full_at = now;
            external_sync_throttle.mark_full_sync(now);

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
            // NOTE: pre-warming managed orderbook books here (in the synchronous
            // fast-reconcile path under the advisory lease) was reverted — it
            // stretched each reconcile cycle from ~3s to ~14s (DB candidate scan
            // + HTTP batch) and triggered a storm of AI-advisory batch flushes,
            // while only partially lowering book age (max stayed ~27-32s). To
            // keep fast reconcile at 3s, this must run as an independent
            // throttled background task instead. See refresh_reward_managed_orderbook_cache.
            let sync_policy = external_sync_throttle.fast_reconcile_policy(&config, Instant::now());
            let report = run_reward_bot_live_reconcile_unlocked(
                state,
                &connector,
                &trace_id,
                &mut book_history,
                Some(orderbook_runtime.cache()),
                sync_policy,
            )
            .await?;
            accumulate_report(&mut total, &report);
            reconcile_cycles += 1;
            let now = Instant::now();
            external_sync_throttle.mark_fast_reconcile(sync_policy, now);

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
                    low_competition_probe.reset();
                    last_full_at = Instant::now() - full_interval;
                }
            }
            _ = command_wake_rx.changed() => {}
            changed = orderbook_wake_rx.changed() => {
                if changed.is_err() {
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

async fn maybe_prune_reward_history(
    state: &AppState,
    last_history_prune_at: &mut Instant,
    interval: Duration,
) {
    let now = Instant::now();
    if now.duration_since(*last_history_prune_at) < interval {
        return;
    }
    *last_history_prune_at = now;

    let cutoff = OffsetDateTime::now_utc() - TimeDuration::seconds(REWARD_HISTORY_RETENTION_SECS);
    match state.reward_bot_service.prune_history(cutoff).await {
        Ok(report)
            if report.terminal_orders_deleted > 0
                || report.risk_events_deleted > 0
                || report.low_competition_observations_deleted > 0 =>
        {
            info!(
                terminal_orders_deleted = report.terminal_orders_deleted,
                risk_events_deleted = report.risk_events_deleted,
                low_competition_observations_deleted =
                    report.low_competition_observations_deleted,
                cutoff = %cutoff,
                "pruned reward history",
            );
        }
        Ok(_) => {
            debug!(cutoff = %cutoff, "reward history prune found no old rows");
        }
        Err(error) => {
            warn!(error = %error, cutoff = %cutoff, "failed to prune reward history");
        }
    }
}

const REWARD_FAST_ORDER_SYNC_MIN_SECS: u64 = 5;
const REWARD_FAST_OPEN_ORDER_SYNC_MIN_SECS: u64 = 15;
const REWARD_FAST_REWARD_EARNINGS_SYNC_SECS: u64 = 60;
const REWARD_FAST_ACCOUNT_SNAPSHOT_SYNC_SECS: u64 = 60;

#[derive(Debug, Clone, Copy, Default)]
struct RewardFastReconcileSyncPolicy {
    order_statuses: bool,
    reward_earnings: bool,
    managed_scoring: bool,
    open_orders: bool,
    account_snapshot: bool,
}

#[derive(Debug, Default)]
struct RewardExternalSyncThrottle {
    last_order_status_sync_at: Option<Instant>,
    last_reward_earnings_sync_at: Option<Instant>,
    last_managed_scoring_sync_at: Option<Instant>,
    last_open_order_sync_at: Option<Instant>,
    last_account_snapshot_sync_at: Option<Instant>,
}

impl RewardExternalSyncThrottle {
    fn mark_full_sync(&mut self, now: Instant) {
        self.last_order_status_sync_at = Some(now);
        self.last_reward_earnings_sync_at = Some(now);
        self.last_managed_scoring_sync_at = Some(now);
        self.last_open_order_sync_at = Some(now);
        self.last_account_snapshot_sync_at = Some(now);
    }

    fn fast_reconcile_policy(
        &self,
        config: &RewardBotConfig,
        now: Instant,
    ) -> RewardFastReconcileSyncPolicy {
        let order_status_interval = Duration::from_secs(
            config
                .reconcile_interval_sec
                .clamp(REWARD_FAST_ORDER_SYNC_MIN_SECS, 60),
        );
        let open_order_interval = Duration::from_secs(
            config
                .reconcile_interval_sec
                .clamp(REWARD_FAST_OPEN_ORDER_SYNC_MIN_SECS, 60),
        );
        let scoring_interval = Duration::from_secs(
            config
                .min_scoring_check_sec
                .clamp(REWARD_FAST_OPEN_ORDER_SYNC_MIN_SECS, 600),
        );

        RewardFastReconcileSyncPolicy {
            order_statuses: reward_sync_due(
                self.last_order_status_sync_at,
                now,
                order_status_interval,
            ),
            reward_earnings: reward_sync_due(
                self.last_reward_earnings_sync_at,
                now,
                Duration::from_secs(REWARD_FAST_REWARD_EARNINGS_SYNC_SECS),
            ),
            managed_scoring: reward_sync_due(
                self.last_managed_scoring_sync_at,
                now,
                scoring_interval,
            ),
            open_orders: reward_sync_due(
                self.last_open_order_sync_at,
                now,
                open_order_interval,
            ),
            account_snapshot: reward_sync_due(
                self.last_account_snapshot_sync_at,
                now,
                Duration::from_secs(REWARD_FAST_ACCOUNT_SNAPSHOT_SYNC_SECS),
            ),
        }
    }

    fn mark_fast_reconcile(&mut self, policy: RewardFastReconcileSyncPolicy, now: Instant) {
        if policy.order_statuses {
            self.last_order_status_sync_at = Some(now);
        }
        if policy.reward_earnings {
            self.last_reward_earnings_sync_at = Some(now);
        }
        if policy.managed_scoring {
            self.last_managed_scoring_sync_at = Some(now);
        }
        if policy.open_orders {
            self.last_open_order_sync_at = Some(now);
        }
        if policy.account_snapshot {
            self.last_account_snapshot_sync_at = Some(now);
        }
    }
}

fn reward_sync_due(last_synced_at: Option<Instant>, now: Instant, interval: Duration) -> bool {
    last_synced_at.is_none_or(|last_synced_at| now.duration_since(last_synced_at) >= interval)
}

struct RewardHeartbeatGuard {
    handle: tokio::task::JoinHandle<()>,
}

impl RewardHeartbeatGuard {
    fn spawn(connector: LivePolymarketConnector) -> Self {
        let handle = tokio::spawn(async move {
            let mut heartbeat_id: Option<String> = None;
            let mut consecutive_failures = 0u32;
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
                    Ok(Ok(next_heartbeat_id)) => {
                        if consecutive_failures > 0 {
                            info!(
                                recovered_after_failures = consecutive_failures,
                                "restored Polymarket rewards heartbeat",
                            );
                        }
                        consecutive_failures = 0;
                        heartbeat_id = Some(next_heartbeat_id);
                    }
                    Ok(Err(error)) => {
                        // Resetting the id lets the next request establish a new
                        // heartbeat chain after an expired or invalid id.
                        heartbeat_id = None;
                        consecutive_failures = consecutive_failures.saturating_add(1);
                        let retry_after = reward_heartbeat_retry_backoff(consecutive_failures);
                        if reward_heartbeat_failure_should_warn(consecutive_failures) {
                            warn!(
                                error = %error,
                                consecutive_failures,
                                retry_after_secs = retry_after.as_secs(),
                                "failed to maintain Polymarket rewards heartbeat",
                            );
                        } else {
                            debug!(
                                error = %error,
                                consecutive_failures,
                                retry_after_secs = retry_after.as_secs(),
                                "failed to maintain Polymarket rewards heartbeat",
                            );
                        }
                        tokio::time::sleep(retry_after).await;
                    }
                    Err(_) => {
                        heartbeat_id = None;
                        consecutive_failures = consecutive_failures.saturating_add(1);
                        let retry_after = reward_heartbeat_retry_backoff(consecutive_failures);
                        if reward_heartbeat_failure_should_warn(consecutive_failures) {
                            warn!(
                                consecutive_failures,
                                retry_after_secs = retry_after.as_secs(),
                                "timed out while maintaining Polymarket rewards heartbeat",
                            );
                        } else {
                            debug!(
                                consecutive_failures,
                                retry_after_secs = retry_after.as_secs(),
                                "timed out while maintaining Polymarket rewards heartbeat",
                            );
                        }
                        tokio::time::sleep(retry_after).await;
                    }
                }
            }
        });
        Self { handle }
    }
}

fn reward_heartbeat_retry_backoff(consecutive_failures: u32) -> Duration {
    let exponent = consecutive_failures.saturating_sub(1).min(4);
    Duration::from_secs((5u64 * (1u64 << exponent)).min(60))
}

fn reward_heartbeat_failure_should_warn(consecutive_failures: u32) -> bool {
    consecutive_failures == 1 || consecutive_failures.is_multiple_of(6)
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
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    low_competition_probe: Option<&mut LowCompetitionProbeState>,
    trace_id: &str,
) -> Result<(Vec<RewardCandidateMarket>, HashMap<String, RewardOrderBook>)> {
    // Read a bounded candidate pool from database (synced by the sync-markets worker).
    let candidates = state
        .reward_bot_service
        .list_reward_run_candidate_market_profiles()
        .await?;
    let markets = candidates
        .iter()
        .map(|candidate| candidate.market.clone())
        .collect::<Vec<_>>();
    if let Some(probe) = low_competition_probe {
        probe
            .refresh_registration(state, &candidates, book_history, trace_id)
            .await?;
    }

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
        candidates,
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

/// Pre-warm the worker-local orderbook cache for every token the bot may act
/// on next — active orders/positions plus eligible quote plans and candidates
/// — so quiet markets stay fresh between full ticks. The reconcile step only
/// refreshes *active* tokens, and the orderbook stream only pushes tokens
/// whose books actually change, so without this a quiet eligible market's
/// local age grows until the next full tick and then fails the placement
/// freshness check. The orderbook service keeps these books fresh via its poll
/// reconciler, so an HTTP batch refresh always reads recent data. Reuses
/// `fetch_cached_reward_books`, which HTTP-fetches only tokens whose local age
/// already exceeds the placement threshold, so an active token that the
/// reconcile step refreshes right after is not double-fetched.
async fn refresh_reward_managed_orderbook_cache(
    state: &AppState,
    orderbook_cache: &RewardOrderbookLocalCache,
) -> Result<usize> {
    let token_ids = reward_orderbook_bootstrap_tokens(state).await?;
    if token_ids.is_empty() {
        return Ok(0);
    }
    Ok(fetch_cached_reward_books(state, Some(orderbook_cache), &token_ids).await?.len())
}

const REWARD_ORDERBOOK_PREWARM_INTERVAL: Duration = Duration::from_secs(5);
const REWARD_ORDERBOOK_REMOTE_REFRESH_PLACEMENT_HEADROOM_MS: i128 = 10_000;

/// Background task that keeps the worker-local orderbook cache fresh for every
/// token the bot may place orders on next (active + eligible + candidate), so
/// quiet markets whose books rarely change stay below the placement freshness
/// threshold between full ticks. Spawned by `RewardOrderbookRuntime` as its own
/// task, fully independent of the poll loop: it never blocks fast reconcile and
/// never holds the advisory lease. `fetch_cached_reward_books` only HTTP-fetches
/// tokens whose local age already exceeds the placement threshold, so it stays
/// cheap when books are already fresh. Aborted when the runtime is dropped.
async fn run_reward_managed_orderbook_cache_prewarm(
    state: AppState,
    cache: Arc<RewardOrderbookLocalCache>,
) {
    loop {
        tokio::time::sleep(REWARD_ORDERBOOK_PREWARM_INTERVAL).await;
        if let Err(error) = refresh_reward_managed_orderbook_cache(&state, cache.as_ref()).await {
            warn!(error = %error, "background orderbook cache pre-warm failed");
        }
    }
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
        let remote_refresh_tokens =
            reward_orderbook_remote_refresh_tokens(state, token_ids, &cached_books).await?;
        if !remote_refresh_tokens.is_empty() {
            let remote_books = fetch_remote_cached_orderbooks(state, &remote_refresh_tokens).await?;
            let remote_token_ids = remote_books
                .iter()
                .map(|book| book.token_id.as_str())
                .collect::<HashSet<_>>();
            cached_books.retain(|book| !remote_token_ids.contains(book.token_id.as_str()));
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

async fn reward_orderbook_remote_refresh_tokens(
    state: &AppState,
    token_ids: &[String],
    cached_books: &[CachedOrderBook],
) -> Result<Vec<String>> {
    let config = state.reward_bot_service.read_config().await?;
    let stale_book_ms = config.stale_book_ms;
    let max_placement_age_ms = live_orderbook_max_placement_age_ms(&config);
    let now_ms = reward_orderbook_now_millis();
    let present = cached_books
        .iter()
        .map(|book| book.token_id.as_str())
        .collect::<HashSet<_>>();
    let stale = cached_books
        .iter()
        .filter(|book| {
            reward_orderbook_book_needs_remote_refresh(
                book,
                now_ms,
                stale_book_ms,
                max_placement_age_ms,
            )
        })
        .map(|book| book.token_id.as_str())
        .collect::<HashSet<_>>();

    let mut seen = HashSet::new();
    let mut refresh = Vec::new();
    for token_id in token_ids {
        if (!present.contains(token_id.as_str()) || stale.contains(token_id.as_str()))
            && seen.insert(token_id.as_str())
        {
            refresh.push(token_id.clone());
        }
    }
    Ok(refresh)
}

fn reward_orderbook_book_is_stale(
    book: &CachedOrderBook,
    now_ms: i64,
    stale_book_ms: u64,
) -> bool {
    if stale_book_ms == 0 {
        return false;
    }
    let age_ms = now_ms.saturating_sub(book.observed_at);
    book.observed_at > now_ms || age_ms > i64::try_from(stale_book_ms).unwrap_or(i64::MAX)
}

fn reward_orderbook_book_needs_remote_refresh(
    book: &CachedOrderBook,
    now_ms: i64,
    stale_book_ms: u64,
    max_placement_age_ms: i128,
) -> bool {
    if reward_orderbook_book_is_stale(book, now_ms, stale_book_ms) {
        return true;
    }
    if stale_book_ms == 0 || max_placement_age_ms == i128::MAX {
        return false;
    }
    let age_ms = i128::from(now_ms.saturating_sub(book.observed_at));
    age_ms > reward_orderbook_remote_refresh_age_ms(max_placement_age_ms)
}

fn reward_orderbook_remote_refresh_age_ms(max_placement_age_ms: i128) -> i128 {
    let headroom = REWARD_ORDERBOOK_REMOTE_REFRESH_PLACEMENT_HEADROOM_MS
        .min(max_placement_age_ms.saturating_div(2));
    max_placement_age_ms.saturating_sub(headroom)
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
