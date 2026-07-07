pub struct WorkerRuntime {
    shutdown_tx: watch::Sender<bool>,
    handles: Vec<JoinHandle<()>>,
}

impl WorkerRuntime {
    #[must_use]
    pub fn start(state: &AppState) -> Self {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let handles = spawn_worker_tasks(state, shutdown_rx);

        if handles.is_empty() {
            warn!("embedded worker runtime started with no enabled jobs");
        } else {
            info!(jobs = handles.len(), "embedded worker runtime started");
        }

        Self {
            shutdown_tx,
            handles,
        }
    }

    pub async fn shutdown(self) {
        let _ = self.shutdown_tx.send(true);
        for handle in self.handles {
            if let Err(error) = handle.await {
                warn!(error = %error, "worker task failed to join");
            }
        }
        info!("embedded worker runtime stopped");
    }
}

async fn run_worker_service(state: AppState) -> Result<()> {
    let runtime = WorkerRuntime::start(&state);

    worker_shutdown_signal().await;
    runtime.shutdown().await;
    Ok(())
}

fn spawn_worker_tasks(state: &AppState, shutdown_rx: watch::Receiver<bool>) -> Vec<JoinHandle<()>> {
    let mut handles = Vec::new();
    let settings = &state.settings.worker;
    let live_polymarket_status = live_polymarket_config_status(state);

    if settings.database_maintenance {
        let job_state = state.clone();
        handles.push(spawn_interval_job(
            "database-maintenance",
            settings.database_maintenance_interval_secs,
            shutdown_rx.clone(),
            move || {
                let state = job_state.clone();
                async move {
                    match run_database_maintenance_once(&state).await {
                        Ok(report) => {
                            log_database_maintenance_report(
                                report,
                                "completed worker database maintenance cycle",
                            );
                        }
                        Err(error) => {
                            warn!(error = %error, "worker database maintenance cycle failed");
                        }
                    }
                }
            },
        ));
    }

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

    // Market sync is now handled by the standalone polyedge-orderbook service.
    // Worker registers its token interests (exec orders + reward markets) with
    // the orderbook service so it subscribes to the right markets.
    {
        let job_state = state.clone();
        let registration_state = Arc::new(tokio::sync::Mutex::new(
            OrderbookRegistrationState::default(),
        ));
        handles.push(spawn_interval_job(
            "register-orderbook-tokens",
            settings.market_sync_interval_secs.clamp(10, 60),
            shutdown_rx.clone(),
            move || {
                let state = job_state.clone();
                let registration_state = registration_state.clone();
                async move {
                    let mut registration_state = registration_state.lock().await;
                    register_orderbook_tokens(&state, &mut registration_state).await;
                }
            },
        ));
    }

    if settings.poll_reward_bot {
        if state.settings.rewards.enabled {
            if live_polymarket_status.is_ready() {
                info!(
                    poll_interval_secs = state.settings.rewards.poll_interval_secs,
                    ai_openai_key_configured = state
                        .settings
                        .rewards
                        .ai_openai_api_key
                        .as_ref()
                        .is_some_and(|value| !value.trim().is_empty()),
                    ai_anthropic_key_configured = state
                        .settings
                        .rewards
                        .ai_anthropic_api_key
                        .as_ref()
                        .is_some_and(|value| !value.trim().is_empty()),
                    ai_model = %state.settings.rewards.ai_model,
                    "spawning worker reward bot poll loop",
                );
                let job_state = state.clone();
                let job_shutdown_rx = shutdown_rx.clone();
                handles.push(spawn_restarting_job(
                    "poll-reward-bot",
                    1,
                    shutdown_rx.clone(),
                    move || {
                        let state = job_state.clone();
                        let shutdown_rx = job_shutdown_rx.clone();
                        async move {
                            match poll_reward_bot_until_shutdown(&state, shutdown_rx).await {
                                Ok(report) => info!(
                                    markets_scanned = report.markets_scanned,
                                    books_fetched = report.books_fetched,
                                    plans_built = report.plans_built,
                                    eligible_plans = report.eligible_plans,
                                    placed_orders = report.placed_orders,
                                    cancelled_orders = report.cancelled_orders,
                                    filled_orders = report.filled_orders,
                                    risk_cancelled_orders = report.risk_cancelled_orders,
                                    reward_accrued = %report.reward_accrued,
                                    "completed worker reward bot cycle",
                                ),
                                Err(error) => {
                                    warn!(error = %error, "worker reward bot polling failed");
                                }
                            }
                        }
                    },
                ));
            } else {
                warn_live_polymarket_config_incomplete("poll-reward-bot", live_polymarket_status);
            }
        } else {
            warn!(
                "worker poll-reward-bot is enabled but rewards bot is disabled; set POLYEDGE_REWARDS__ENABLED=true"
            );
        }
    } else {
        info!(
            "worker reward bot poll loop is disabled; set POLYEDGE_WORKER__POLL_REWARD_BOT=true"
        );
    }

    maybe_spawn_reward_info_risk_task(state, shutdown_rx.clone(), &mut handles);

    if settings.drain_execution_queue {
        let drain_polymarket = live_polymarket_status.is_ready();
        if !drain_polymarket {
            warn_live_polymarket_config_incomplete(
                "drain-execution-queue:polymarket",
                live_polymarket_status,
            );
        }
        let job_state = state.clone();
        handles.push(spawn_interval_job(
            "drain-execution-queue",
            settings.execution_drain_interval_secs,
            shutdown_rx.clone(),
            move || {
                let state = job_state.clone();
                async move {
                    drain_execution_queue_for_connector(&state, PAPER_EXECUTOR_NAME).await;
                    if drain_polymarket {
                        drain_execution_queue_for_connector(&state, POLYMARKET_CONNECTOR_NAME)
                            .await;
                    }
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
        if live_polymarket_status.is_ready() {
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
        } else {
            warn_live_polymarket_config_incomplete(
                "poll-polymarket-order-statuses",
                live_polymarket_status,
            );
        }
    }

    if settings.reconcile_polymarket_fills {
        if live_polymarket_status.is_ready() {
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
        } else {
            warn_live_polymarket_config_incomplete(
                "reconcile-polymarket-fills",
                live_polymarket_status,
            );
        }
    }

    if settings.consume_polymarket_user_events {
        if live_polymarket_status.is_ready() {
            let job_state = state.clone();
            handles.push(spawn_restarting_job(
                "consume-polymarket-user-events",
                settings.polymarket_user_event_restart_interval_secs,
                shutdown_rx.clone(),
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
        } else {
            warn_live_polymarket_config_incomplete(
                "consume-polymarket-user-events",
                live_polymarket_status,
            );
        }
    }

    // Orderbook stream is now handled by the standalone polyedge-orderbook service.
    // Workers connect to it via HTTP (configured by POLYEDGE_ORDERBOOK__SERVICE_URL).

    handles
}

#[derive(Debug, Clone, Copy)]
struct LivePolymarketConfigStatus {
    account_id_configured: bool,
    private_key_configured: bool,
    api_credentials_complete: bool,
}

impl LivePolymarketConfigStatus {
    fn is_ready(self) -> bool {
        self.account_id_configured
            && self.private_key_configured
            && self.api_credentials_complete
    }
}

fn live_polymarket_config_status(state: &AppState) -> LivePolymarketConfigStatus {
    let settings = &state.settings.polymarket;
    let api_key_configured = settings
        .api_key
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let api_secret_configured = settings
        .api_secret
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let api_passphrase_configured = settings
        .api_passphrase
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let configured_api_credentials = [
        api_key_configured,
        api_secret_configured,
        api_passphrase_configured,
    ]
    .into_iter()
    .filter(|configured| *configured)
    .count();

    LivePolymarketConfigStatus {
        account_id_configured: !polymarket_account_id(state).is_empty(),
        private_key_configured: settings
            .private_key
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty()),
        api_credentials_complete: configured_api_credentials == 0
            || configured_api_credentials == 3,
    }
}

fn warn_live_polymarket_config_incomplete(job: &'static str, status: LivePolymarketConfigStatus) {
    warn!(
        job,
        account_id_configured = status.account_id_configured,
        private_key_configured = status.private_key_configured,
        api_credentials_complete = status.api_credentials_complete,
        "skipping live Polymarket worker job because configuration is incomplete; set POLYEDGE_POLYMARKET__ACCOUNT_ID and POLYEDGE_POLYMARKET__PRIVATE_KEY, and set all API credential fields or none"
    );
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
