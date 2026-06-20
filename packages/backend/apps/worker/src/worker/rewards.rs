const REWARD_WORKER_ADVISORY_LOCK_KEY: i64 = 0x504f_4c59_5245_5744;
const LIVE_EXTERNAL_ORDER_NOT_FOUND_MARKER: &str = "external order lookup returned not found";
const LIVE_EXTERNAL_ORDER_NOT_FOUND_CLOSE_AFTER_SECS: i64 = 300;
static REWARD_AI_PROVIDER_REQUEST_SEMAPHORE: tokio::sync::Semaphore =
    tokio::sync::Semaphore::const_new(1);
static REWARD_MARKET_PROVIDER_REFRESH_RUNNING: AtomicBool = AtomicBool::new(false);

async fn run_reward_bot_once(state: &AppState, trace_id: &str) -> Result<RewardBotRunReport> {
    let connector = build_live_polymarket_connector(state).await?;
    let _heartbeat_guard = RewardHeartbeatGuard::spawn(connector.clone());
    let mut book_history = HashMap::new();
    run_reward_bot_once_with_history(state, &connector, trace_id, &mut book_history).await
}

async fn run_reward_bot_once_with_history(
    state: &AppState,
    connector: &LivePolymarketConnector,
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
            process_pending_reward_control_commands_unlocked(state, connector, book_history, None)
                .await?;
        if command_report.processed > 0 {
            return Ok(command_report.report);
        }

        run_reward_bot_tick(state, connector, trace_id, false, book_history, None).await
    }
    .await;
    finish_reward_worker_lease(lease, result).await
}

async fn run_reward_bot_tick(
    state: &AppState,
    connector: &LivePolymarketConnector,
    trace_id: &str,
    force_orders: bool,
    book_history: &mut HashMap<String, VecDeque<BookSnapshot>>,
    orderbook_cache: Option<&RewardOrderbookLocalCache>,
) -> Result<RewardBotRunReport> {
    let (markets, books) = fetch_reward_bot_inputs(state, orderbook_cache).await?;
    record_reward_book_history(book_history, &books);
    run_reward_bot_live_tick(
        state,
        connector,
        markets,
        books,
        trace_id,
        force_orders,
        book_history,
    )
    .await
}

fn mark_pre_ai_eligible_quote_plans(
    plans: &mut [RewardQuotePlan],
    pre_ai_eligible_condition_ids: &mut Vec<String>,
) {
    pre_ai_eligible_condition_ids.clear();
    for plan in plans {
        plan.pre_ai_eligible = plan.eligible;
        if plan.pre_ai_eligible {
            if plan.orderbook_token_ids.is_empty() {
                plan.orderbook_token_ids = quote_plan_leg_token_ids(&plan.legs);
            }
            pre_ai_eligible_condition_ids.push(plan.condition_id.clone());
        }
    }
}

fn quote_plan_leg_token_ids(legs: &[RewardQuoteLeg]) -> Vec<String> {
    let mut token_ids = Vec::new();
    let mut seen = HashSet::new();
    for leg in legs {
        if leg.token_id.trim().is_empty() || !seen.insert(leg.token_id.clone()) {
            continue;
        }
        token_ids.push(leg.token_id.clone());
    }
    token_ids
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

#[allow(clippy::too_many_arguments)]
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

async fn process_pending_reward_control_commands_unlocked(
    state: &AppState,
    connector: &LivePolymarketConnector,
    book_history: &mut HashMap<String, VecDeque<BookSnapshot>>,
    orderbook_cache: Option<&RewardOrderbookLocalCache>,
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

        let result = execute_reward_control_command(
            state,
            connector,
            &command,
            &trace_id,
            book_history,
            orderbook_cache,
        )
        .await;
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
    connector: &LivePolymarketConnector,
    command: &RewardControlCommand,
    trace_id: &str,
    book_history: &mut HashMap<String, VecDeque<BookSnapshot>>,
    orderbook_cache: Option<&RewardOrderbookLocalCache>,
) -> Result<RewardBotRunReport> {
    match command.action {
        RewardControlAction::RunOnce => {
            run_reward_bot_tick(
                state,
                connector,
                trace_id,
                true,
                book_history,
                orderbook_cache,
            )
            .await
        }
        RewardControlAction::CancelAll => {
            let cancel_report = cancel_live_reward_orders(
                state,
                connector,
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
                connector,
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
    connector: &LivePolymarketConnector,
    markets: Vec<RewardCandidateMarket>,
    books: HashMap<String, RewardOrderBook>,
    trace_id: &str,
    force_orders: bool,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
) -> Result<RewardBotRunReport> {
    let books_fetched = books.len();
    let ai_min_confidence = reward_ai_min_confidence(state.settings.rewards.ai_min_confidence_bps);
    let ai_model = state.settings.rewards.ai_model.trim().to_string();
    let mut cycle = state
        .reward_bot_service
        .prepare_live_cycle(
            markets,
            books.clone(),
            trace_id,
            force_orders,
            ai_min_confidence,
            &ai_model,
        )
        .await?;
    apply_low_competition_metrics_to_quote_plans(
        &mut cycle.plans,
        &books,
        book_history,
        &cycle.open_orders,
        &cycle.config,
    );
    mark_pre_ai_eligible_quote_plans(&mut cycle.plans, &mut cycle.pre_ai_eligible_condition_ids);
    info!(
        trace_id = %trace_id,
        markets = cycle.markets.len(),
        books = books_fetched,
        plans = cycle.plans.len(),
        pre_ai_eligible_plans = cycle.pre_ai_eligible_condition_ids.len(),
        eligible_plans = cycle.plans.iter().filter(|plan| plan.eligible).count(),
        open_orders = cycle.open_orders.len(),
        positions = cycle.positions.len(),
        ai_advisory_enabled = cycle.config.ai_advisory_enabled,
        ai_provider = cycle.config.ai_provider.as_str(),
        ai_request_format = cycle.config.ai_request_format.as_str(),
        info_risk_enabled = cycle.config.info_risk_enabled,
        info_risk_mode = cycle.config.info_risk_mode.as_str(),
        "prepared rewards live cycle",
    );
    spawn_reward_market_provider_refresh(state, &cycle, &books, trace_id);
    apply_cached_reward_ai_advisories_to_cycle(state, &mut cycle, &books, trace_id).await?;
    apply_cached_reward_info_risks_to_cycle(state, &mut cycle, trace_id).await?;
    let low_competition_observations = build_low_competition_observations(
        &cycle.account.account_id,
        &cycle.plans,
        &cycle.config,
        OffsetDateTime::now_utc(),
    );
    if !low_competition_observations.is_empty() {
        state
            .reward_bot_service
            .record_low_competition_observations(&low_competition_observations)
            .await?;
        info!(
            trace_id = %trace_id,
            observations = low_competition_observations.len(),
            "recorded low-competition sleeve observations",
        );
    }
    state.reward_bot_service.save_quote_plans(&cycle.plans).await?;
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

    if !cycle.open_orders.is_empty() {
        let sync_report =
            sync_live_reward_orders(state, connector, &cycle.open_orders, &books, trace_id).await?;
        accumulate_report(&mut report, &sync_report);
        let latest = state.reward_bot_service.current_live_cycle_state().await?;
        cycle.account = latest.account;
        cycle.open_orders = latest.open_orders;
        cycle.positions = latest.positions;
    }

    // Reward earnings sync always runs — it queries Polymarket's authoritative
    // daily total and does not risk double-counting fills.
    sync_reward_earnings(state, connector, &mut cycle.account, trace_id).await;

    // Always reconcile the authoritative open-order list so venue-side automatic
    // cancels release local open-order capacity immediately. Balance/position
    // replacement still waits when a fresh fill may not have propagated yet.
    sync_external_account_state(
        state,
        connector,
        &mut cycle.account,
        &mut cycle.positions,
        &mut cycle.open_orders,
        trace_id,
        can_refresh_external_account_after_order_sync(&report),
    )
    .await;

    let mut account = cycle.account.clone();
    let mut open_orders = cycle.open_orders.clone();

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
        match cancel_one_live_reward_order(connector, order, &reason, trace_id).await? {
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
    let pending_plan_index: HashMap<&str, &RewardQuotePlan> = cycle
        .plans
        .iter()
        .map(|plan| (plan.condition_id.as_str(), plan))
        .collect();
    let pending_buy_submit_risk = LiveBuySubmitRiskContext {
        config: &cycle.config,
        plans: &pending_plan_index,
        book_history,
        kill_switch,
    };
    submit_pending_live_reward_orders(
        connector,
        &mut open_orders,
        &books,
        Some(pending_buy_submit_risk),
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

    let (placement_orders, plans_changed) = live_placement_orders(
        &cycle.config,
        &account,
        &mut cycle.plans,
        &books,
        book_history,
        &open_orders,
        &cycle.positions,
        kill_switch,
        trace_id,
    );
    if plans_changed {
        state.reward_bot_service.save_quote_plans(&cycle.plans).await?;
    }

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
        let placement_plan_index: HashMap<&str, &RewardQuotePlan> = cycle
            .plans
            .iter()
            .map(|plan| (plan.condition_id.as_str(), plan))
            .collect();
        let placement_buy_submit_risk = LiveBuySubmitRiskContext {
            config: &cycle.config,
            plans: &placement_plan_index,
            book_history,
            kill_switch,
        };
        submit_pending_live_reward_orders(
            connector,
            &mut open_orders,
            &books,
            Some(placement_buy_submit_risk),
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
    connector: &LivePolymarketConnector,
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

    let mut account = cycle.account.clone();
    let mut report = LiveCancelReport::default();

    for order in target_orders {
        match cancel_one_live_reward_order(connector, order, reason, trace_id).await? {
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
include!("rewards/orderbook_events.rs");
include!("rewards/polling.rs");
include!("rewards/provider_advisory.rs");
include!("rewards/provider_refresh.rs");
include!("rewards/info_risk.rs");
include!("rewards/provider_batch.rs");
