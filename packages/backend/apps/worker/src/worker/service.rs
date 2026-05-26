async fn run_worker_service(state: AppState) -> Result<()> {
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handles = spawn_worker_tasks(&state, shutdown_rx);

    if handles.is_empty() {
        warn!("polyedge-worker service started with no enabled jobs");
    } else {
        info!(jobs = handles.len(), "polyedge-worker service started");
    }

    worker_shutdown_signal().await;
    let _ = shutdown_tx.send(true);

    for handle in handles {
        if let Err(error) = handle.await {
            warn!(error = %error, "worker task failed to join");
        }
    }

    info!("polyedge-worker service stopped");
    Ok(())
}

fn spawn_worker_tasks(state: &AppState, shutdown_rx: watch::Receiver<bool>) -> Vec<JoinHandle<()>> {
    let mut handles = Vec::new();
    let settings = &state.settings.worker;

    if settings.poll_news {
        if state.settings.news.enabled {
            let job_state = state.clone();
            handles.push(spawn_interval_job(
                "poll-news",
                state.settings.news.poll_interval_secs,
                shutdown_rx.clone(),
                move || {
                    let state = job_state.clone();
                    async move {
                        let trace_id = new_trace_id();
                        match ingest_news_once(&state, &trace_id).await {
                            Ok(report) => info!(
                                trace_id = %trace_id,
                                sources_scanned = report.sources_scanned,
                                sources_succeeded = report.sources_succeeded,
                                sources_failed = report.sources_failed,
                                fetched = report.fetched,
                                inserted = report.inserted,
                                deduped = report.deduped,
                                "completed worker news ingestion cycle",
                            ),
                            Err(error) => {
                                warn!(trace_id = %trace_id, error = %error, "worker news ingestion cycle failed");
                            }
                        }
                    }
                },
            ));
        } else {
            warn!(
                "worker poll-news is enabled but news ingestion is disabled; set POLYEDGE_NEWS__ENABLED=true"
            );
        }
    }

    if settings.promote_news_events {
        let job_state = state.clone();
        handles.push(spawn_interval_job(
            "promote-news-events",
            settings.news_promotion_interval_secs,
            shutdown_rx.clone(),
            move || {
                let state = job_state.clone();
                async move {
                    let trace_id = new_trace_id();
                    match promote_news_events(&state, task_limit(&state), &trace_id).await {
                        Ok(report) => info!(
                            trace_id = %trace_id,
                            scanned = report.scanned,
                            promoted = report.promoted,
                            evidences_promoted = report.evidences_promoted,
                            skipped_unmatched = report.skipped_unmatched,
                            "completed worker news promotion cycle",
                        ),
                        Err(error) => {
                            warn!(trace_id = %trace_id, error = %error, "worker news promotion cycle failed");
                        }
                    }
                }
            },
        ));
    }

    if settings.poll_arbitrage_radar {
        if state.settings.arbitrage.enabled {
            let job_state = state.clone();
            handles.push(spawn_interval_job(
                "poll-arbitrage-radar",
                state.settings.arbitrage.poll_interval_secs,
                shutdown_rx.clone(),
                move || {
                    let state = job_state.clone();
                    async move {
                        let trace_id = new_trace_id();
                        match scan_arbitrage_once(&state, &trace_id).await {
                            Ok(report) => info!(
                                trace_id = %trace_id,
                                markets_scanned = report.markets_scanned,
                                snapshots_recorded = report.snapshots_recorded,
                                opportunities_recorded = report.opportunities_recorded,
                                validations_recorded = report.validations_recorded,
                                validation_books_refetched = report.validation_books_refetched,
                                validation_book_failures = report.validation_book_failures,
                                opportunities_expired = report.opportunities_expired,
                                events_pruned = report.events_pruned,
                                failed_books = report.failed_books,
                                "completed worker arbitrage radar cycle",
                            ),
                            Err(error) => {
                                warn!(trace_id = %trace_id, error = %error, "worker arbitrage radar cycle failed");
                            }
                        }
                    }
                },
            ));
        } else {
            warn!(
                "worker poll-arbitrage-radar is enabled but arbitrage is disabled; set POLYEDGE_ARBITRAGE__ENABLED=true"
            );
        }
    }

    if settings.analyze_arbitrage_opportunities {
        if state.settings.arbitrage.enabled {
            let job_state = state.clone();
            handles.push(spawn_interval_job(
                "analyze-arbitrage-opportunities",
                settings.arbitrage_analysis_interval_secs,
                shutdown_rx.clone(),
                move || {
                    let state = job_state.clone();
                    async move {
                        let trace_id = new_trace_id();
                        match analyze_arbitrage_opportunities(
                            &state,
                            state.settings.arbitrage.analysis_lookback_hours,
                            &trace_id,
                        )
                        .await
                        {
                            Ok(analysis) => info!(
                                trace_id = %trace_id,
                                analysis_id = %analysis.id,
                                lookback_hours = analysis.lookback_hours,
                                opportunity_count = analysis.opportunity_count,
                                market_count = analysis.market_count,
                                "completed worker arbitrage analysis cycle",
                            ),
                            Err(error) => {
                                warn!(trace_id = %trace_id, error = %error, "worker arbitrage analysis cycle failed");
                            }
                        }
                    }
                },
            ));
        } else {
            warn!(
                "worker arbitrage analysis is enabled but arbitrage is disabled; set POLYEDGE_ARBITRAGE__ENABLED=true"
            );
        }
    }

    if settings.poll_reward_bot {
        if state.settings.rewards.enabled {
            let job_state = state.clone();
            handles.push(spawn_interval_job(
                "poll-reward-bot",
                state.settings.rewards.poll_interval_secs,
                shutdown_rx.clone(),
                move || {
                    let state = job_state.clone();
                    async move {
                        let trace_id = new_trace_id();
                        match run_reward_bot_once(&state, &trace_id).await {
                            Ok(report) => info!(
                                trace_id = %trace_id,
                                markets_scanned = report.markets_scanned,
                                books_fetched = report.books_fetched,
                                plans_built = report.plans_built,
                                eligible_plans = report.eligible_plans,
                                simulated_orders = report.simulated_orders,
                                cancelled_orders = report.cancelled_orders,
                                "completed worker reward bot simulation cycle",
                            ),
                            Err(error) => {
                                warn!(trace_id = %trace_id, error = %error, "worker reward bot simulation cycle failed");
                            }
                        }
                    }
                },
            ));
        } else {
            warn!(
                "worker poll-reward-bot is enabled but rewards bot is disabled; set POLYEDGE_REWARDS__ENABLED=true"
            );
        }
    }

    if settings.drain_execution_queue {
        let job_state = state.clone();
        handles.push(spawn_interval_job(
            "drain-execution-queue",
            settings.execution_drain_interval_secs,
            shutdown_rx.clone(),
            move || {
                let state = job_state.clone();
                async move {
                    drain_execution_queue_for_connector(&state, PAPER_EXECUTOR_NAME).await;
                    drain_execution_queue_for_connector(&state, POLYMARKET_CONNECTOR_NAME).await;
                }
            },
        ));
    }

    if settings.poll_paper_order_statuses {
        let job_state = state.clone();
        handles.push(spawn_interval_job(
            "poll-paper-order-statuses",
            settings.order_status_poll_interval_secs,
            shutdown_rx.clone(),
            move || {
                let state = job_state.clone();
                async move {
                    match poll_paper_order_statuses(
                        &state,
                        Some(PAPER_EXECUTOR_NAME.to_string()),
                        task_limit(&state),
                    )
                    .await
                    {
                        Ok(report) => info!(
                            scanned = report.scanned,
                            opened = report.opened,
                            "completed worker paper order status poll",
                        ),
                        Err(error) => {
                            warn!(error = %error, "worker paper order status poll failed");
                        }
                    }
                }
            },
        ));
    }

    if settings.reconcile_paper_fills {
        let job_state = state.clone();
        handles.push(spawn_interval_job(
            "reconcile-paper-fills",
            settings.fill_reconciliation_interval_secs,
            shutdown_rx.clone(),
            move || {
                let state = job_state.clone();
                async move {
                    match reconcile_paper_fills(
                        &state,
                        Some(PAPER_EXECUTOR_NAME.to_string()),
                        task_limit(&state),
                    )
                    .await
                    {
                        Ok(report) => info!(
                            scanned = report.scanned,
                            reconciled = report.reconciled,
                            "completed worker paper fill reconciliation",
                        ),
                        Err(error) => {
                            warn!(error = %error, "worker paper fill reconciliation failed");
                        }
                    }
                }
            },
        ));
    }

    if settings.poll_polymarket_order_statuses {
        let job_state = state.clone();
        handles.push(spawn_interval_job(
            "poll-polymarket-order-statuses",
            settings.order_status_poll_interval_secs,
            shutdown_rx.clone(),
            move || {
                let state = job_state.clone();
                async move {
                    let limit = polymarket_order_status_limit(&state, task_limit(&state));
                    match poll_polymarket_order_statuses(
                        &state,
                        Some(POLYMARKET_CONNECTOR_NAME.to_string()),
                        limit,
                    )
                    .await
                    {
                        Ok(report) => info!(
                            scanned = report.scanned,
                            opened = report.opened,
                            "completed worker polymarket order status poll",
                        ),
                        Err(error) => {
                            warn!(error = %error, "worker polymarket order status poll failed");
                        }
                    }
                }
            },
        ));
    }

    if settings.reconcile_polymarket_fills {
        let job_state = state.clone();
        handles.push(spawn_interval_job(
            "reconcile-polymarket-fills",
            settings.fill_reconciliation_interval_secs,
            shutdown_rx.clone(),
            move || {
                let state = job_state.clone();
                async move {
                    let limit = polymarket_fill_limit(&state, task_limit(&state));
                    match reconcile_polymarket_fills(
                        &state,
                        Some(POLYMARKET_CONNECTOR_NAME.to_string()),
                        limit,
                    )
                    .await
                    {
                        Ok(report) => info!(
                            scanned = report.scanned,
                            reconciled = report.reconciled,
                            "completed worker polymarket fill reconciliation",
                        ),
                        Err(error) => {
                            warn!(error = %error, "worker polymarket fill reconciliation failed");
                        }
                    }
                }
            },
        ));
    }

    if settings.consume_polymarket_user_events {
        let job_state = state.clone();
        handles.push(spawn_restarting_job(
            "consume-polymarket-user-events",
            settings.polymarket_user_event_restart_interval_secs,
            shutdown_rx,
            move || {
                let state = job_state.clone();
                async move {
                    match consume_polymarket_user_events(
                        &state,
                        Some(POLYMARKET_CONNECTOR_NAME.to_string()),
                        None,
                    )
                    .await
                    {
                        Ok(report) => info!(
                            subscribed_markets = report.subscribed_markets,
                            consumed = report.consumed,
                            order_updates_applied = report.order_updates_applied,
                            trade_updates_applied = report.trade_updates_applied,
                            skipped_unknown_orders = report.skipped_unknown_orders,
                            skipped_duplicate_trades = report.skipped_duplicate_trades,
                            "polymarket user event consumer stopped",
                        ),
                        Err(error) => {
                            warn!(error = %error, "polymarket user event consumer failed");
                        }
                    }
                }
            },
        ));
    }

    handles
}

fn spawn_interval_job<F, Fut>(
    name: &'static str,
    interval_secs: u64,
    mut shutdown_rx: watch::Receiver<bool>,
    mut job: F,
) -> JoinHandle<()>
where
    F: FnMut() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(async move {
        loop {
            if *shutdown_rx.borrow() {
                break;
            }

            job().await;

            if wait_for_worker_interval(&mut shutdown_rx, interval_secs).await {
                break;
            }
        }

        info!(job = name, "worker interval job stopped");
    })
}

fn spawn_restarting_job<F, Fut>(
    name: &'static str,
    restart_interval_secs: u64,
    mut shutdown_rx: watch::Receiver<bool>,
    mut job: F,
) -> JoinHandle<()>
where
    F: FnMut() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(async move {
        loop {
            if *shutdown_rx.borrow() {
                break;
            }

            tokio::select! {
                () = job() => {}
                changed = shutdown_rx.changed() => {
                    if changed.is_err() || *shutdown_rx.borrow() {
                        break;
                    }
                }
            }

            if wait_for_worker_interval(&mut shutdown_rx, restart_interval_secs).await {
                break;
            }
        }

        info!(job = name, "worker restarting job stopped");
    })
}

async fn wait_for_worker_interval(
    shutdown_rx: &mut watch::Receiver<bool>,
    interval_secs: u64,
) -> bool {
    let interval = Duration::from_secs(interval_secs.max(1));

    tokio::select! {
        () = tokio::time::sleep(interval) => false,
        changed = shutdown_rx.changed() => changed.is_err() || *shutdown_rx.borrow(),
    }
}

async fn worker_shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            let _ = signal.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}
