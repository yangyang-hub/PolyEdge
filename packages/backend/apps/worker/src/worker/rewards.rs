const REWARD_WORKER_ADVISORY_LOCK_KEY: i64 = 0x504f_4c59_5245_5744;
const LIVE_EXTERNAL_ORDER_NOT_FOUND_MARKER: &str = "external order lookup returned not found";

async fn run_reward_bot_once(state: &AppState, trace_id: &str) -> Result<RewardBotRunReport> {
    let mut book_history = HashMap::new();
    run_reward_bot_once_with_history(state, trace_id, &mut book_history).await
}

async fn run_reward_bot_once_with_history(
    state: &AppState,
    trace_id: &str,
    book_history: &mut HashMap<String, VecDeque<BookSnapshot>>,
) -> Result<RewardBotRunReport> {
    let Some(lease) = state
        .try_acquire_postgres_advisory_lease(REWARD_WORKER_ADVISORY_LOCK_KEY)
        .await?
    else {
        debug!("skipping rewards full cycle because another worker holds the live lease");
        return Ok(RewardBotRunReport::default());
    };
    let result = async {
        let command_report =
            process_pending_reward_control_commands_unlocked(state, book_history).await?;
        if command_report.processed > 0 {
            return Ok(command_report.report);
        }

        run_reward_bot_tick(state, trace_id, false, book_history).await
    }
    .await;
    finish_reward_worker_lease(lease, result).await
}

async fn run_reward_bot_scheduled_full_cycle(
    state: &AppState,
    trace_id: &str,
    book_history: &mut HashMap<String, VecDeque<BookSnapshot>>,
) -> Result<Option<RewardBotRunReport>> {
    let Some(lease) = state
        .try_acquire_postgres_advisory_lease(REWARD_WORKER_ADVISORY_LOCK_KEY)
        .await?
    else {
        debug!("deferring rewards full cycle because another worker holds the live lease");
        return Ok(None);
    };
    let result = run_reward_bot_tick(state, trace_id, false, book_history).await;
    finish_reward_worker_lease(lease, result).await.map(Some)
}

async fn run_reward_bot_tick(
    state: &AppState,
    trace_id: &str,
    force_orders: bool,
    book_history: &mut HashMap<String, VecDeque<BookSnapshot>>,
) -> Result<RewardBotRunReport> {
    let (markets, books) = fetch_reward_bot_inputs(state).await?;
    record_reward_book_history(book_history, &books);
    run_reward_bot_live_tick(state, markets, books, trace_id, force_orders, book_history).await
}

#[derive(Debug, Default)]
struct RewardCommandProcessReport {
    processed: usize,
    /// True if at least one processed command ran a full quote-rebuilding tick
    /// (RunOnce). Lets the poll loop avoid an immediately-redundant full cycle
    /// without resetting the full-cycle timer for cancel/reset-only commands.
    ran_full_cycle: bool,
    report: RewardBotRunReport,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct LiveCancelReport {
    cancelled: usize,
    rejected: usize,
}

impl LiveCancelReport {
    fn as_run_report(self) -> RewardBotRunReport {
        RewardBotRunReport {
            cancelled_orders: self.cancelled,
            ..RewardBotRunReport::default()
        }
    }
}

async fn persist_live_reward_updates(
    state: &AppState,
    account: &mut RewardAccountState,
    positions: Vec<RewardPosition>,
    orders: Vec<ManagedRewardOrder>,
    fills: Vec<RewardFill>,
    events: Vec<RewardRiskEvent>,
    report: &RewardBotRunReport,
    trace_id: &str,
) -> Result<()> {
    if orders.is_empty() && fills.is_empty() && events.is_empty() {
        return Ok(());
    }

    account.tick_index += 1;
    account.updated_at = OffsetDateTime::now_utc();
    state
        .reward_bot_service
        .apply_live_tick_outcome(
            &RewardTickOutcome {
                account: account.clone(),
                markets: Vec::new(),
                plans: Vec::new(),
                orders,
                positions,
                fills,
                events,
                report: report.clone(),
            },
            trace_id,
        )
        .await
}

async fn process_pending_reward_control_commands(
    state: &AppState,
    book_history: &mut HashMap<String, VecDeque<BookSnapshot>>,
) -> Result<RewardCommandProcessReport> {
    let Some(lease) = state
        .try_acquire_postgres_advisory_lease(REWARD_WORKER_ADVISORY_LOCK_KEY)
        .await?
    else {
        debug!("skipping rewards commands because another worker holds the live lease");
        return Ok(RewardCommandProcessReport::default());
    };
    let result = process_pending_reward_control_commands_unlocked(state, book_history).await;
    finish_reward_worker_lease(lease, result).await
}

async fn process_pending_reward_control_commands_unlocked(
    state: &AppState,
    book_history: &mut HashMap<String, VecDeque<BookSnapshot>>,
) -> Result<RewardCommandProcessReport> {
    let mut total = RewardCommandProcessReport::default();
    let max_commands = usize::from(task_limit(state).unwrap_or(10).max(1));

    for _ in 0..max_commands {
        let trace_id = new_trace_id();
        let Some(command) = state
            .reward_bot_service
            .claim_next_control_command(&trace_id)
            .await?
        else {
            break;
        };

        let result = execute_reward_control_command(state, &command, &trace_id, book_history).await;
        match result {
            Ok(report) => {
                state
                    .reward_bot_service
                    .complete_control_command(&command, &trace_id)
                    .await?;
                accumulate_report(&mut total.report, &report);
                total.processed += 1;
                if command.action == RewardControlAction::RunOnce {
                    total.ran_full_cycle = true;
                }
                info!(
                    trace_id = %trace_id,
                    command_id = %command.id,
                    action = command.action.as_str(),
                    "completed queued reward bot control command",
                );
            }
            Err(error) => {
                state
                    .reward_bot_service
                    .fail_control_command(&command, &trace_id, &error)
                    .await?;
                total.processed += 1;
                warn!(
                    trace_id = %trace_id,
                    command_id = %command.id,
                    action = command.action.as_str(),
                    error = %error,
                    "queued reward bot control command failed",
                );
            }
        }
    }

    Ok(total)
}

async fn finish_reward_worker_lease<T>(
    lease: polyedge_infrastructure::PostgresAdvisoryLease,
    result: Result<T>,
) -> Result<T> {
    if let Err(release_error) = lease.release().await {
        if result.is_ok() {
            return Err(release_error);
        }
        warn!(error = %release_error, "failed to release rewards worker advisory lease");
    }
    result
}

async fn execute_reward_control_command(
    state: &AppState,
    command: &RewardControlCommand,
    trace_id: &str,
    book_history: &mut HashMap<String, VecDeque<BookSnapshot>>,
) -> Result<RewardBotRunReport> {
    match command.action {
        RewardControlAction::RunOnce => run_reward_bot_tick(state, trace_id, true, book_history).await,
        RewardControlAction::CancelAll => {
            let cancel_report = cancel_live_reward_orders(
                state,
                command.account_id.as_deref(),
                "worker processed queued rewards live cancel-all command",
                trace_id,
            )
            .await?;
            if cancel_report.rejected > 0 {
                return Err(AppError::conflict(
                    "REWARD_LIVE_CANCEL_REJECTED",
                    format!(
                        "live cancel-all left {} managed Polymarket orders open",
                        cancel_report.rejected
                    ),
                ));
            }
            Ok(cancel_report.as_run_report())
        }
        RewardControlAction::Reset => {
            let cancel_report = cancel_live_reward_orders(
                state,
                command.account_id.as_deref(),
                "worker processed queued rewards live reset as cancel-all command",
                trace_id,
            )
            .await?;
            if cancel_report.rejected > 0 {
                return Err(AppError::conflict(
                    "REWARD_LIVE_RESET_CANCEL_REJECTED",
                    format!(
                        "live reset refused to clear local state because {} managed Polymarket orders could not be cancelled",
                        cancel_report.rejected
                    ),
                ));
            }
            state
                .reward_bot_service
                .record_live_reset_cancel_all(trace_id)
                .await?;
            Ok(cancel_report.as_run_report())
        }
    }
}

async fn run_reward_bot_live_tick(
    state: &AppState,
    markets: Vec<RewardMarket>,
    books: HashMap<String, RewardOrderBook>,
    trace_id: &str,
    force_orders: bool,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
) -> Result<RewardBotRunReport> {
    let books_fetched = books.len();
    let mut cycle = state
        .reward_bot_service
        .prepare_live_cycle(markets, books.clone(), trace_id, force_orders)
        .await?;
    let kill_switch = state.risk_service.read_state().await?.kill_switch;
    if kill_switch {
        cycle.should_execute = false;
    }
    let mut report = RewardBotRunReport {
        markets_scanned: cycle.markets.len(),
        books_fetched,
        plans_built: cycle.plans.len(),
        eligible_plans: cycle.plans.iter().filter(|plan| plan.eligible).count(),
        placed_orders: 0,
        cancelled_orders: 0,
        filled_orders: 0,
        risk_cancelled_orders: 0,
        reward_accrued: Decimal::ZERO,
    };

    let connector = build_live_polymarket_connector(state).await?;

    if !cycle.open_orders.is_empty() {
        let sync_report =
            sync_live_reward_orders(state, &connector, &cycle.open_orders, &books, trace_id).await?;
        accumulate_report(&mut report, &sync_report);
        let latest = state.reward_bot_service.current_live_cycle_state().await?;
        cycle.account = latest.account;
        cycle.open_orders = latest.open_orders;
        cycle.positions = latest.positions;
    }

    // Reward earnings sync always runs — it queries Polymarket's authoritative
    // daily total and does not risk double-counting fills.
    sync_reward_earnings(state, &connector, &mut cycle.account, trace_id).await;

    // A newly confirmed fill was just applied to the local ledger and may not yet
    // be visible in the eventually-consistent Data API snapshot. Refresh only on
    // cycles without new fills so the same fill cannot be counted twice.
    if can_refresh_external_account_after_order_sync(&report) {
        sync_external_account_state(
            state,
            &connector,
            &mut cycle.account,
            &mut cycle.positions,
            &mut cycle.open_orders,
            trace_id,
        )
        .await;
    }

    let mut account = cycle.account.clone();
    let mut open_orders = cycle.open_orders.clone();

    // Auto-expire stale stuck-reconciliation orders after configured threshold.
    // Runs after sync (gives Polymarket one last chance) and before cancel
    // candidates so that `has_unresolved_live_reconciliation` sees the
    // cleaned-up order list.
    {
        let stale_candidates = live_stale_auto_cancel_candidates(
            &cycle.config,
            &open_orders,
            OffsetDateTime::now_utc(),
        );
        if !stale_candidates.is_empty() {
            for (order_id, reason) in &stale_candidates {
                let Some(index) = open_orders.iter().position(|o| o.id == *order_id) else {
                    continue;
                };
                let (updated, event) =
                    force_cancel_stale_live_reward_order(open_orders[index].clone(), reason);
                open_orders[index] = updated.clone();
                report.cancelled_orders += 1;
                report.risk_cancelled_orders += 1;
                persist_live_reward_updates(
                    state,
                    &mut account,
                    Vec::new(),
                    vec![updated],
                    Vec::new(),
                    vec![event],
                    &report,
                    trace_id,
                )
                .await?;
            }
            info!(
                trace_id = %trace_id,
                stale_auto_cancelled = stale_candidates.len(),
                "auto-cancelled stale stuck-reconciliation orders",
            );
        }
    }

    if !cycle.should_execute && cycle.open_orders.is_empty() {
        return Ok(report);
    }

    let mut cancel_rejected = false;

    for (order_id, reason) in
        live_cancel_candidates(
            &cycle.config,
            &cycle.plans,
            &open_orders,
            &books,
            book_history,
            kill_switch,
        )
    {
        let Some(index) = open_orders.iter().position(|order| order.id == order_id) else {
            continue;
        };
        let order = open_orders[index].clone();
        match cancel_one_live_reward_order(&connector, order, &reason, trace_id).await? {
            LiveRewardOrderUpdate::Changed(updated, event) => {
                open_orders[index] = updated.clone();
                if live_cancel_result_is_unknown(&updated) {
                    cancel_rejected = true;
                } else {
                    report.cancelled_orders += 1;
                }
                persist_live_reward_updates(
                    state,
                    &mut account,
                    Vec::new(), // positions unchanged during cancel
                    vec![updated],
                    Vec::new(),
                    vec![event],
                    &report,
                    trace_id,
                )
                .await?;
            }
            LiveRewardOrderUpdate::Unchanged(event)
            | LiveRewardOrderUpdate::Retryable(event) => {
                cancel_rejected = true;
                persist_live_reward_updates(
                    state,
                    &mut account,
                    Vec::new(), // positions unchanged during cancel
                    Vec::new(),
                    Vec::new(),
                    vec![event],
                    &report,
                    trace_id,
                )
                .await?;
            }
        }
    }

    // Validate and cancel stale persisted intents before submitting them.
    // Unknown submissions are protected by live_cancel_reason and recovered
    // here without issuing a duplicate order.
    let unresolved_before_recovery = has_unresolved_live_reconciliation(&open_orders);
    submit_pending_live_reward_orders(
        &connector,
        &mut open_orders,
        &books,
        state,
        &mut account,
        &cycle.positions,
        &mut report,
        trace_id,
        cycle.should_execute && !cancel_rejected && !unresolved_before_recovery,
    )
    .await?;

    if cancel_rejected || has_unresolved_live_reconciliation(&open_orders) {
        return Ok(report);
    }

    if !cycle.should_execute {
        return Ok(report);
    }

    let placement_orders = live_placement_orders(
        &cycle.config,
        &account.account_id,
        &cycle.plans,
        &books,
        &open_orders,
        &cycle.positions,
        account.available_usd,
        trace_id,
    );

    if !placement_orders.is_empty() {
        let events = placement_orders
            .iter()
            .map(|order| {
                reward_live_event(
                    order,
                    "reward_live_order_planned",
                    RewardRiskSeverity::Info,
                    "persisted rewards quote intent before live submission",
                    json!({
                        "token_id": order.token_id,
                        "side": order.side.as_str(),
                        "size": order.size,
                        "price": order.price,
                    }),
                )
            })
            .collect();
        persist_live_reward_updates(
            state,
            &mut account,
            Vec::new(), // positions unchanged during placement
            placement_orders.clone(),
            Vec::new(),
            events,
            &report,
            trace_id,
        )
        .await?;
        open_orders.extend(placement_orders);
        submit_pending_live_reward_orders(
            &connector,
            &mut open_orders,
            &books,
            state,
            &mut account,
            &cycle.positions,
            &mut report,
            trace_id,
            true,
        )
        .await?;
    }

    Ok(report)
}

async fn cancel_live_reward_orders(
    state: &AppState,
    account_id: Option<&str>,
    reason: &str,
    trace_id: &str,
) -> Result<LiveCancelReport> {
    let cycle = state.reward_bot_service.current_live_cycle_state().await?;
    let target_orders = cycle
        .open_orders
        .iter()
        .filter(|order| account_id.is_none_or(|id| id == order.account_id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if target_orders.is_empty() {
        return Ok(LiveCancelReport::default());
    }

    let connector = build_live_polymarket_connector(state).await?;
    let mut account = cycle.account.clone();
    let mut report = LiveCancelReport::default();

    for order in target_orders {
        match cancel_one_live_reward_order(&connector, order, reason, trace_id).await? {
            LiveRewardOrderUpdate::Changed(updated, event) => {
                if live_cancel_result_is_unknown(&updated) {
                    report.rejected += 1;
                } else {
                    report.cancelled += 1;
                }
                persist_live_reward_updates(
                    state,
                    &mut account,
                    Vec::new(), // positions unchanged during cancel
                    vec![updated],
                    Vec::new(),
                    vec![event],
                    &report.as_run_report(),
                    trace_id,
                )
                .await?;
            }
            LiveRewardOrderUpdate::Unchanged(event)
            | LiveRewardOrderUpdate::Retryable(event) => {
                report.rejected += 1;
                persist_live_reward_updates(
                    state,
                    &mut account,
                    Vec::new(), // positions unchanged during cancel
                    Vec::new(),
                    Vec::new(),
                    vec![event],
                    &report.as_run_report(),
                    trace_id,
                )
                .await?;
            }
        }
    }
    Ok(report)
}

include!("rewards/account_sync.rs");
include!("rewards/live_sync.rs");
include!("rewards/live_orders.rs");
include!("rewards/live_submission.rs");
include!("rewards/live_pending.rs");
include!("rewards/live_helpers.rs");
include!("rewards/live_risk.rs");
include!("rewards/polling.rs");
