async fn run_copytrade_once(
    state: &AppState,
    trace_id: &str,
) -> Result<CopyTradeRunReport> {
    let command_report = process_pending_copytrade_control_commands(state).await?;
    if command_report.processed > 0 {
        return Ok(command_report.report);
    }

    // Detect and record source trades from tracked wallets.
    let config = state.copytrade_service.read_config().await?;
    if !config.enabled {
        return Ok(CopyTradeRunReport::default());
    }
    let wallet_feeds = fetch_wallet_analysis_inputs(state).await?;
    let detected = state
        .copytrade_service
        .detect_and_record_source_trades(&config, &wallet_feeds, trace_id)
        .await?;
    Ok(CopyTradeRunReport {
        wallets_scanned: wallet_feeds.len(),
        trades_detected: detected,
        ..CopyTradeRunReport::default()
    })
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
        CopyControlAction::RunOnce => {
            // Simulation engine removed — run-once is a no-op.
            Ok(CopyTradeRunReport::default())
        }
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
            // Simulation engine removed — cancel-all is a no-op.
            Ok(CopyTradeRunReport::default())
        }
        CopyControlAction::Reset => {
            // Simulation engine removed — reset is a no-op.
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
