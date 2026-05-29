async fn run_copytrade_once(
    state: &AppState,
    trace_id: &str,
) -> Result<CopyTradeRunReport> {
    let (wallet_feeds, books) = fetch_copytrade_inputs(state).await?;
    state
        .copytrade_service
        .run_copy_cycle(wallet_feeds, books, trace_id)
        .await
}

async fn poll_copytrade(
    state: &AppState,
    max_cycles: Option<usize>,
) -> Result<CopyTradeRunReport> {
    let mut total = CopyTradeRunReport {
        wallets_scanned: 0,
        trades_detected: 0,
        orders_placed: 0,
        orders_filled: 0,
        orders_skipped: 0,
    };
    let mut cycles = 0usize;
    let interval = Duration::from_secs(state.settings.copytrade.poll_interval_secs.max(1));

    loop {
        let trace_id = new_trace_id();
        let report = run_copytrade_once(state, &trace_id).await?;
        total.wallets_scanned += report.wallets_scanned;
        total.trades_detected += report.trades_detected;
        total.orders_placed += report.orders_placed;
        total.orders_filled += report.orders_filled;
        total.orders_skipped += report.orders_skipped;
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

    let mut token_ids = std::collections::HashSet::new();
    for feed in &wallet_feeds {
        for activity in &feed.activities {
            if activity.kind.eq_ignore_ascii_case("TRADE") && !activity.asset.is_empty() {
                token_ids.insert(activity.asset.clone());
            }
        }
    }

    let connector = PolymarketRewardsConnector::new(&state.settings.polymarket.clob_host)?;
    let books = connector
        .fetch_order_books(&token_ids.into_iter().collect::<Vec<_>>())
        .await?
        .into_iter()
        .map(|book| {
            (
                book.token_id.clone(),
                CopyOrderBook {
                    token_id: book.token_id,
                    bids: book
                        .bids
                        .into_iter()
                        .map(|level| CopyBookLevel {
                            price: level.price,
                            size: level.size,
                        })
                        .collect(),
                    asks: book
                        .asks
                        .into_iter()
                        .map(|level| CopyBookLevel {
                            price: level.price,
                            size: level.size,
                        })
                        .collect(),
                    observed_at: book.observed_at,
                },
            )
        })
        .collect::<HashMap<_, _>>();

    Ok((wallet_feeds, books))
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
        let activities = connector
            .fetch_wallet_activity(&wallet.address, limit)
            .await
            .map(|raws| {
                raws.into_iter()
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
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let positions = connector
            .fetch_wallet_positions(&wallet.address)
            .await
            .map(|raws| {
                raws.into_iter()
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
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        feeds.push(WalletFeedInput {
            address: wallet.address,
            activities,
            positions,
        });
    }

    Ok(feeds)
}
