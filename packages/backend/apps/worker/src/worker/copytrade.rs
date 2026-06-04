async fn run_copytrade_once(
    state: &AppState,
    trace_id: &str,
) -> Result<CopyTradeRunReport> {
    let command_report = process_pending_copytrade_control_commands(state).await?;
    if command_report.processed > 0 {
        return Ok(command_report.report);
    }

    run_copytrade_tick(state, trace_id).await
}

async fn run_copytrade_tick(state: &AppState, trace_id: &str) -> Result<CopyTradeRunReport> {
    let (wallet_feeds, books) = fetch_copytrade_inputs(state).await?;
    state
        .copytrade_service
        .run_copy_cycle(wallet_feeds, books, trace_id)
        .await
}

#[derive(Debug, Default)]
struct CopyCommandProcessReport {
    processed: usize,
    report: CopyTradeRunReport,
}

async fn process_pending_copytrade_control_commands(
    state: &AppState,
) -> Result<CopyCommandProcessReport> {
    let mut total = CopyCommandProcessReport::default();
    let max_commands = usize::from(task_limit(state).unwrap_or(10).max(1));

    for _ in 0..max_commands {
        let trace_id = new_trace_id();
        let Some(command) = state
            .copytrade_service
            .claim_next_control_command(&trace_id)
            .await?
        else {
            break;
        };

        let result = execute_copytrade_control_command(state, &command, &trace_id).await;
        match result {
            Ok(report) => {
                state
                    .copytrade_service
                    .complete_control_command(&command, &trace_id)
                    .await?;
                accumulate_copytrade_report(&mut total.report, &report);
                total.processed += 1;
                info!(
                    trace_id = %trace_id,
                    command_id = %command.id,
                    action = command.action.as_str(),
                    "completed queued copytrade control command",
                );
            }
            Err(error) => {
                state
                    .copytrade_service
                    .fail_control_command(&command, &trace_id, &error)
                    .await?;
                total.processed += 1;
                warn!(
                    trace_id = %trace_id,
                    command_id = %command.id,
                    action = command.action.as_str(),
                    error = %error,
                    "queued copytrade control command failed",
                );
            }
        }
    }

    Ok(total)
}

async fn execute_copytrade_control_command(
    state: &AppState,
    command: &CopyControlCommand,
    trace_id: &str,
) -> Result<CopyTradeRunReport> {
    match command.action {
        CopyControlAction::RunOnce => run_copytrade_tick(state, trace_id).await,
        CopyControlAction::AnalyzeWallets => {
            let wallet_feeds = fetch_wallet_analysis_inputs(state).await?;
            let analyzed = state.copytrade_service.analyze_wallets(wallet_feeds).await?;
            info!(
                trace_id = %trace_id,
                wallets_analyzed = analyzed,
                "analyzed copytrade wallets from queued control command",
            );
            Ok(CopyTradeRunReport::default())
        }
        CopyControlAction::CancelAll => {
            state
                .copytrade_service
                .cancel_all_orders(
                    command.account_id.as_deref(),
                    "worker processed queued copytrade cancel-all command",
                    trace_id,
                )
                .await?;
            Ok(CopyTradeRunReport::default())
        }
        CopyControlAction::Reset => {
            state.copytrade_service.reset_simulation(trace_id).await?;
            Ok(CopyTradeRunReport::default())
        }
    }
}

async fn poll_copytrade(
    state: &AppState,
    max_cycles: Option<usize>,
) -> Result<CopyTradeRunReport> {
    let mut total = CopyTradeRunReport::default();
    let mut cycles = 0usize;
    let interval = Duration::from_secs(state.settings.copytrade.poll_interval_secs.max(1));

    loop {
        let trace_id = new_trace_id();
        let report = run_copytrade_once(state, &trace_id).await?;
        accumulate_copytrade_report(&mut total, &report);
        cycles += 1;

        info!(
            trace_id = %trace_id,
            cycle = cycles,
            wallets_scanned = report.wallets_scanned,
            trades_detected = report.trades_detected,
            orders_placed = report.orders_placed,
            orders_filled = report.orders_filled,
            "completed copytrade polling cycle",
        );

        if max_cycles.is_some_and(|limit| cycles >= limit) {
            break;
        }

        tokio::select! {
            () = tokio::time::sleep(interval) => {}
            shutdown = tokio::signal::ctrl_c() => {
                if let Err(error) = shutdown {
                    warn!(error = %error, "failed to listen for ctrl-c during copytrade polling");
                }
                break;
            }
        }
    }

    Ok(total)
}

fn accumulate_copytrade_report(total: &mut CopyTradeRunReport, report: &CopyTradeRunReport) {
    total.wallets_scanned += report.wallets_scanned;
    total.trades_detected += report.trades_detected;
    total.orders_placed += report.orders_placed;
    total.orders_filled += report.orders_filled;
    total.orders_skipped += report.orders_skipped;
}

async fn analyze_wallets_once(state: &AppState, trace_id: &str) -> Result<usize> {
    let wallet_feeds = fetch_wallet_analysis_inputs(state).await?;
    let analyzed = state
        .copytrade_service
        .analyze_wallets(wallet_feeds)
        .await?;

    state
        .copytrade_service
        .read_config()
        .await
        .ok();

    info!(
        trace_id = %trace_id,
        wallets_analyzed = analyzed,
        "analyzed copytrade wallets once",
    );

    Ok(analyzed)
}

// ── Input fetching ──────────────────────────────────────────────────────────

async fn fetch_copytrade_inputs(
    state: &AppState,
) -> Result<(Vec<WalletFeedInput>, HashMap<String, CopyOrderBook>)> {
    let wallet_feeds = fetch_wallet_analysis_inputs(state).await?;

    // Collect token IDs from wallet trade activities.
    let mut token_ids_set = std::collections::HashSet::new();
    for feed in &wallet_feeds {
        for activity in &feed.activities {
            if activity.kind.eq_ignore_ascii_case("TRADE") && !activity.asset.is_empty() {
                token_ids_set.insert(activity.asset.clone());
            }
        }
    }
    let token_ids: Vec<String> = token_ids_set.into_iter().collect();

    // Replace copytrade's token set so historic wallet activity does not keep
    // stale orderbook subscriptions alive forever.
    state
        .orderbook_registry
        .register_tokens("copytrade", &token_ids)
        .await?;

    // Read from shared cache first; collect cache misses for fallback.
    let mut books = HashMap::new();
    let mut missing = Vec::new();

    for token_id in &token_ids {
        match state.orderbook_cache.get_book(token_id).await? {
            Some(cached) => {
                books.insert(token_id.clone(), cached_to_copy_book(cached));
            }
            None => {
                missing.push(token_id.clone());
            }
        }
    }

    // Fallback: cache miss tokens are fetched directly from CLOB REST API
    // (cold start / subscription delay). Results are written into the cache so
    // subsequent reads succeed without a network call.
    if !missing.is_empty() {
        debug!(
            missing_count = missing.len(),
            "copytrade orderbook cache miss, falling back to direct CLOB poll"
        );
        let connector =
            PolymarketRewardsConnector::new(&state.settings.polymarket.clob_host)?;
        match connector.fetch_order_books(&missing).await {
            Ok(polled) => {
                for book in polled {
                    let cached = polled_book_to_cached(&book);
                    if let Err(error) = state.orderbook_cache.set_book(&cached).await {
                        warn!(
                            token_id = %cached.token_id,
                            error = %error,
                            "copytrade failed to write fallback book to cache"
                        );
                    }
                    books.insert(book.token_id.clone(), polled_book_to_copy_book(book));
                }
            }
            Err(error) => {
                warn!(
                    error = %error,
                    "copytrade fallback CLOB poll failed, some markets will lack orderbook data"
                );
            }
        }
    }

    Ok((wallet_feeds, books))
}

/// Convert a `CachedOrderBook` (from in-memory cache) to `CopyOrderBook`.
fn cached_to_copy_book(cached: CachedOrderBook) -> CopyOrderBook {
    CopyOrderBook {
        token_id: cached.token_id,
        bids: cached
            .bids
            .into_iter()
            .map(|l| CopyBookLevel {
                price: l.price,
                size: l.size,
            })
            .collect(),
        asks: cached
            .asks
            .into_iter()
            .map(|l| CopyBookLevel {
                price: l.price,
                size: l.size,
            })
            .collect(),
        observed_at: OffsetDateTime::from_unix_timestamp(cached.observed_at / 1000)
            .unwrap_or(OffsetDateTime::UNIX_EPOCH)
            + TimeDuration::milliseconds(cached.observed_at % 1000),
    }
}

/// Convert a `PolymarketRewardOrderBook` (from CLOB REST) to `CachedOrderBook`
/// for writing into the shared cache.
fn polled_book_to_cached(book: &PolymarketRewardOrderBook) -> CachedOrderBook {
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

/// Convert a `PolymarketRewardOrderBook` (from CLOB REST) to `CopyOrderBook`.
fn polled_book_to_copy_book(book: PolymarketRewardOrderBook) -> CopyOrderBook {
    let now = OffsetDateTime::now_utc();
    CopyOrderBook {
        token_id: book.token_id,
        bids: book
            .bids
            .into_iter()
            .map(|l| CopyBookLevel {
                price: l.price,
                size: l.size,
            })
            .collect(),
        asks: book
            .asks
            .into_iter()
            .map(|l| CopyBookLevel {
                price: l.price,
                size: l.size,
            })
            .collect(),
        observed_at: now,
    }
}

async fn fetch_wallet_analysis_inputs(state: &AppState) -> Result<Vec<WalletFeedInput>> {
    let wallets = state.copytrade_service.snapshot().await?.wallets;
    let active_wallets: Vec<_> = wallets
        .into_iter()
        .filter(|w| w.status == polyedge_application::TrackedWalletStatus::Active)
        .collect();

    if active_wallets.is_empty() {
        return Ok(Vec::new());
    }

    let connector =
        PolymarketDataApiConnector::new(&state.settings.polymarket.data_api_host)?;

    let mut feeds = Vec::new();
    for wallet in active_wallets {
        let limit = state.settings.copytrade.wallet_activity_limit;
        let activities = match connector
            .fetch_wallet_activity(&wallet.address, limit)
            .await
        {
            Ok(raws) => raws
                .into_iter()
                .map(|raw| WalletActivityInput {
                    kind: raw.kind,
                    side: raw.side,
                    asset: raw.asset,
                    condition_id: raw.condition_id,
                    outcome: raw.outcome,
                    title: raw.title,
                    slug: raw.slug,
                    price: raw.price,
                    size: raw.size,
                    usdc_size: raw.usdc_size,
                    transaction_hash: raw.transaction_hash,
                    timestamp: raw.timestamp,
                })
                .collect::<Vec<_>>(),
            Err(error) => {
                warn!(
                    wallet = %wallet.address,
                    error = %error,
                    "failed to fetch wallet activity from Polymarket Data API"
                );
                Vec::new()
            }
        };

        let positions = match connector
            .fetch_wallet_positions(&wallet.address)
            .await
        {
            Ok(raws) => raws
                .into_iter()
                .map(|raw| WalletPositionInput {
                    asset: raw.asset,
                    condition_id: raw.condition_id,
                    outcome: raw.outcome,
                    title: raw.title,
                    slug: raw.slug,
                    size: raw.size,
                    avg_price: raw.avg_price,
                    cur_price: raw.cur_price,
                    realized_pnl: raw.realized_pnl,
                    percent_pnl: raw.percent_pnl,
                })
                .collect::<Vec<_>>(),
            Err(error) => {
                warn!(
                    wallet = %wallet.address,
                    error = %error,
                    "failed to fetch wallet positions from Polymarket Data API"
                );
                Vec::new()
            }
        };

        feeds.push(WalletFeedInput {
            address: wallet.address,
            activities,
            positions,
        });
    }

    Ok(feeds)
}
