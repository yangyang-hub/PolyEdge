async fn run_reward_bot_once(state: &AppState, trace_id: &str) -> Result<RewardBotRunReport> {
    let mut book_history = HashMap::new();
    run_reward_bot_once_with_history(state, trace_id, &mut book_history).await
}

async fn run_reward_bot_once_with_history(
    state: &AppState,
    trace_id: &str,
    book_history: &mut HashMap<String, VecDeque<BookSnapshot>>,
) -> Result<RewardBotRunReport> {
    let command_report = process_pending_reward_control_commands(state, book_history).await?;
    if command_report.processed > 0 {
        return Ok(command_report.report);
    }

    run_reward_bot_tick(state, trace_id, false, book_history).await
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

async fn process_pending_reward_control_commands(
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
    let mut report = RewardBotRunReport {
        markets_scanned: cycle.markets.len(),
        books_fetched,
        plans_built: cycle.plans.len(),
        eligible_plans: cycle.plans.iter().filter(|plan| plan.eligible).count(),
        simulated_orders: 0,
        cancelled_orders: 0,
        filled_orders: 0,
        risk_cancelled_orders: 0,
        reward_accrued: Decimal::ZERO,
    };

    if !cycle.should_execute && cycle.open_orders.is_empty() {
        return Ok(report);
    }

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

    let mut open_orders = cycle.open_orders.clone();
    let mut changed_orders = Vec::new();
    let mut events = Vec::new();
    let mut cancel_rejected = false;

    submit_deferred_live_exit_orders(
        &connector,
        &mut open_orders,
        &books,
        &mut changed_orders,
        &mut events,
        &mut report,
    )
    .await?;

    for (order_id, reason) in
        live_cancel_candidates(&cycle.config, &cycle.plans, &open_orders, &books, book_history)
    {
        let Some(index) = open_orders.iter().position(|order| order.id == order_id) else {
            continue;
        };
        let order = open_orders[index].clone();
        match cancel_one_live_reward_order(&connector, order, &reason, trace_id).await? {
            LiveRewardOrderUpdate::Changed(updated, event) => {
                open_orders[index] = updated.clone();
                changed_orders.push(updated);
                events.push(event);
                report.cancelled_orders += 1;
            }
            LiveRewardOrderUpdate::Unchanged(event) => {
                events.push(event);
                cancel_rejected = true;
            }
        }
    }

    if cancel_rejected {
        if !changed_orders.is_empty() || !events.is_empty() {
            let mut account = cycle.account;
            account.tick_index += 1;
            account.updated_at = OffsetDateTime::now_utc();
            let outcome = RewardSimulationOutcome {
                account,
                markets: cycle.markets,
                plans: cycle.plans,
                orders: changed_orders,
                positions: cycle.positions,
                fills: Vec::new(),
                events,
                report: report.clone(),
            };
            state
                .reward_bot_service
                .apply_live_tick_outcome(&outcome, trace_id)
                .await?;
        }
        return Ok(report);
    }

    if !cycle.should_execute {
        if !changed_orders.is_empty() || !events.is_empty() {
            let mut account = cycle.account;
            account.tick_index += 1;
            account.updated_at = OffsetDateTime::now_utc();
            let outcome = RewardSimulationOutcome {
                account,
                markets: cycle.markets,
                plans: cycle.plans,
                orders: changed_orders,
                positions: cycle.positions,
                fills: Vec::new(),
                events,
                report: report.clone(),
            };
            state
                .reward_bot_service
                .apply_live_tick_outcome(&outcome, trace_id)
                .await?;
        }
        return Ok(report);
    }

    let placement_orders = live_placement_orders(
        &cycle.config,
        &cycle.account.account_id,
        &cycle.plans,
        &books,
        &open_orders,
        &cycle.positions,
        trace_id,
    );

    for mut order in placement_orders {
        match submit_one_live_reward_order(&connector, &mut order).await? {
            LiveRewardOrderUpdate::Changed(updated, event) => {
                open_orders.push(updated.clone());
                match updated.status {
                    ManagedRewardOrderStatus::Open => report.simulated_orders += 1,
                    ManagedRewardOrderStatus::Cancelled => report.cancelled_orders += 1,
                    _ => {}
                }
                changed_orders.push(updated);
                events.push(event);
            }
            LiveRewardOrderUpdate::Unchanged(event) => {
                order.status = ManagedRewardOrderStatus::Error;
                order.reason = event.message.clone();
                order.updated_at = OffsetDateTime::now_utc();
                changed_orders.push(order);
                events.push(event);
            }
        }
    }

    if changed_orders.is_empty() && events.is_empty() {
        return Ok(report);
    }

    let mut account = cycle.account;
    account.tick_index += 1;
    account.updated_at = OffsetDateTime::now_utc();
    let outcome = RewardSimulationOutcome {
        account,
        markets: cycle.markets,
        plans: cycle.plans,
        orders: changed_orders,
        positions: cycle.positions,
        fills: Vec::new(),
        events,
        report: report.clone(),
    };
    state
        .reward_bot_service
        .apply_live_tick_outcome(&outcome, trace_id)
        .await?;

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
    let mut changed_orders = Vec::new();
    let mut events = Vec::new();
    let mut report = LiveCancelReport::default();

    for order in target_orders {
        match cancel_one_live_reward_order(&connector, order, reason, trace_id).await? {
            LiveRewardOrderUpdate::Changed(updated, event) => {
                changed_orders.push(updated);
                events.push(event);
                report.cancelled += 1;
            }
            LiveRewardOrderUpdate::Unchanged(event) => {
                events.push(event);
                report.rejected += 1;
            }
        }
    }

    if changed_orders.is_empty() && events.is_empty() {
        return Ok(report);
    }

    let mut account = cycle.account;
    account.tick_index += 1;
    account.updated_at = OffsetDateTime::now_utc();
    let outcome = RewardSimulationOutcome {
        account,
        markets: cycle.markets,
        plans: cycle.plans,
        orders: changed_orders,
        positions: cycle.positions,
        fills: Vec::new(),
        events,
        report: report.as_run_report(),
    };
    state
        .reward_bot_service
        .apply_live_tick_outcome(&outcome, trace_id)
        .await?;
    Ok(report)
}

include!("rewards/live_sync.rs");
include!("rewards/live_orders.rs");
include!("rewards/live_risk.rs");
include!("rewards/polling.rs");
