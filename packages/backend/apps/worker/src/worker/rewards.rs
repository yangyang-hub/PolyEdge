const REWARD_WORKER_ADVISORY_LOCK_KEY: i64 = 0x504f_4c59_5245_5744;
const LIVE_EXTERNAL_ORDER_NOT_FOUND_MARKER: &str = "external order lookup returned not found";
const LIVE_EXTERNAL_ORDER_NOT_FOUND_CLOSE_AFTER_SECS: i64 = 300;
// A submission whose result is unknown (e.g. CLOB 5xx / response without an order id) is
// recovered every tick via `find_matching_open_token_order`. Once that lookup confirms there
// is no live Polymarket order, the local intent is closed after this grace so the global
// reconciliation lock (which pauses new buy placements) self-clears instead of requiring a
// manual DB fix. Mirrors the 404-lock close above; tunable here if needed.
const LIVE_SUBMISSION_UNKNOWN_CLOSE_AFTER_SECS: i64 = 600;
const REWARD_HISTORY_PRUNE_INTERVAL_SECS: u64 = 5 * 24 * 60 * 60;
const REWARD_HISTORY_RETENTION_SECS: i64 = 5 * 24 * 60 * 60;
static REWARD_PROVIDER_REQUEST_SEMAPHORE: tokio::sync::Semaphore =
    tokio::sync::Semaphore::const_new(1);
static REWARD_PROVIDER_REFRESH_RUNNING: AtomicBool = AtomicBool::new(false);

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
    let (markets, books) =
        fetch_reward_bot_inputs(state, orderbook_cache, book_history, trace_id).await?;
    record_reward_book_history(book_history, &books);
    run_reward_bot_live_tick(
        state,
        connector,
        markets,
        books,
        trace_id,
        force_orders,
        book_history,
        orderbook_cache,
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

fn push_reward_live_action_token(
    token_ids: &mut Vec<String>,
    seen: &mut HashSet<String>,
    token_id: &str,
) {
    let token_id = token_id.trim();
    if token_id.is_empty() || !seen.insert(token_id.to_string()) {
        return;
    }
    token_ids.push(token_id.to_string());
}

fn reward_live_action_orderbook_tokens(
    plans: &[RewardQuotePlan],
    open_orders: &[ManagedRewardOrder],
) -> Vec<String> {
    let mut token_ids = Vec::new();
    let mut seen = HashSet::new();

    for order in open_orders
        .iter()
        .filter(|order| order.status.is_open_like())
    {
        push_reward_live_action_token(&mut token_ids, &mut seen, &order.token_id);
    }

    for plan in plans.iter().filter(|plan| plan.eligible) {
        for token_id in &plan.orderbook_token_ids {
            push_reward_live_action_token(&mut token_ids, &mut seen, token_id);
        }
        for leg in &plan.legs {
            push_reward_live_action_token(&mut token_ids, &mut seen, &leg.token_id);
        }
    }

    token_ids
}

async fn refresh_reward_live_action_books(
    state: &AppState,
    orderbook_cache: Option<&RewardOrderbookLocalCache>,
    books: &mut HashMap<String, RewardOrderBook>,
    plans: &[RewardQuotePlan],
    open_orders: &[ManagedRewardOrder],
    trace_id: &str,
) -> Result<usize> {
    let token_ids = reward_live_action_orderbook_tokens(plans, open_orders);
    if token_ids.is_empty() {
        return Ok(0);
    }

    let refreshed = fetch_cached_reward_books(state, orderbook_cache, &token_ids).await?;
    let refreshed_count = refreshed.len();
    for (token_id, book) in refreshed {
        books.insert(token_id, book);
    }
    debug!(
        trace_id = %trace_id,
        tokens = token_ids.len(),
        refreshed_books = refreshed_count,
        "refreshed reward live action orderbooks before live actions"
    );
    Ok(refreshed_count)
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
    persist_live_reward_updates_with_merge_intents(
        state,
        account,
        positions,
        orders,
        fills,
        Vec::new(),
        events,
        report,
        trace_id,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn persist_live_reward_updates_with_merge_intents(
    state: &AppState,
    account: &mut RewardAccountState,
    positions: Vec<RewardPosition>,
    orders: Vec<ManagedRewardOrder>,
    fills: Vec<RewardFill>,
    merge_intents: Vec<RewardMergeIntent>,
    events: Vec<RewardRiskEvent>,
    report: &RewardBotRunReport,
    trace_id: &str,
) -> Result<()> {
    if orders.is_empty() && fills.is_empty() && merge_intents.is_empty() && events.is_empty() {
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
                merge_intents,
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
    mut books: HashMap<String, RewardOrderBook>,
    trace_id: &str,
    force_orders: bool,
    book_history: &mut HashMap<String, VecDeque<BookSnapshot>>,
    orderbook_cache: Option<&RewardOrderbookLocalCache>,
) -> Result<RewardBotRunReport> {
    let books_fetched = books.len();
    let mut cycle = state
        .reward_bot_service
        .prepare_live_cycle(
            markets,
            books.clone(),
            trace_id,
            force_orders,
        )
        .await?;
    apply_reward_opportunity_metrics_to_quote_plans(
        &mut cycle.plans,
        &books,
        book_history,
        &cycle.open_orders,
        &cycle.account,
        &cycle.config,
    );
    let funding_precheck_blocked = apply_live_funding_precheck(
        &cycle.config,
        &cycle.account,
        &mut cycle.plans,
        &books,
        &cycle.open_orders,
        &cycle.positions,
    );
    mark_pre_ai_eligible_quote_plans(&mut cycle.plans, &mut cycle.pre_ai_eligible_condition_ids);
    info!(
        trace_id = %trace_id,
        markets = cycle.markets.len(),
        books = books_fetched,
        plans = cycle.plans.len(),
        funding_precheck_blocked,
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
    let provider_refresh_cycle = cycle.clone();
    apply_cached_reward_ai_advisories_to_cycle(state, &mut cycle, &books, trace_id).await?;
    apply_cached_reward_info_risks_to_cycle(state, &mut cycle, trace_id).await?;
    spawn_reward_market_provider_refresh(state, &provider_refresh_cycle, &books, trace_id);
    if apply_first_quote_entry_gates(
        &mut cycle.plans,
        &cycle.previous_plans,
        &cycle.open_orders,
        &cycle.positions,
        &cycle.config,
        OffsetDateTime::now_utc(),
    ) {
        debug!(
            trace_id = %trace_id,
            "applied first-quote entry gates to reward quote plans"
        );
    }
    register_reward_eligible_orderbook_tokens_from_plans(state, &cycle.plans, trace_id).await;
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

    let mut live_order_sync_reliable = true;
    if !cycle.open_orders.is_empty() {
        let sync_report =
            sync_live_reward_orders(state, connector, &cycle.open_orders, &books, trace_id).await?;
        live_order_sync_reliable = sync_report.reconciliation_reliable;
        accumulate_report(&mut report, &sync_report.report);
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
        live_order_sync_reliable,
    )
    .await;

    let mut account = cycle.account.clone();
    let mut open_orders = cycle.open_orders.clone();

    let refreshed_action_books = refresh_reward_live_action_books(
        state,
        orderbook_cache,
        &mut books,
        &cycle.plans,
        &open_orders,
        trace_id,
    )
    .await?;
    if refreshed_action_books > 0 {
        record_reward_book_history(book_history, &books);
        report.books_fetched = report.books_fetched.max(books.len());
    }

    let readiness_changed =
        refresh_live_quote_plan_readiness(&cycle.config, &mut cycle.plans, &books);
    refresh_reward_opportunity_metrics_for_quote_plans(
        &mut cycle.plans,
        &books,
        book_history,
        &open_orders,
        &account,
        &cycle.config,
    );
    state
        .reward_bot_service
        .save_quote_plans(&cycle.plans)
        .await?;
    if readiness_changed {
        debug!(
            trace_id = %trace_id,
            "refreshed reward quote plan readiness before saving snapshot"
        );
    }

    if !cycle.should_execute && cycle.open_orders.is_empty() {
        return Ok(report);
    }

    let mut cancel_rejected = false;

    for (order_id, reason) in live_cancel_candidates_with_account(
        &cycle.config,
        &cycle.plans,
        &open_orders,
        &books,
        book_history,
        &account,
        kill_switch,
    ) {
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
            LiveRewardOrderUpdate::Unchanged(event) | LiveRewardOrderUpdate::Retryable(event) => {
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
    let pending_open_orders_snapshot = open_orders.clone();
    let pending_account_snapshot = account.clone();
    let pending_buy_submit_risk = LiveBuySubmitRiskContext {
        config: &cycle.config,
        plans: &pending_plan_index,
        book_history,
        open_orders: &pending_open_orders_snapshot,
        positions: &cycle.positions,
        account: &pending_account_snapshot,
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
        state
            .reward_bot_service
            .save_quote_plans(&cycle.plans)
            .await?;
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
        register_reward_active_orderbook_tokens(state, trace_id).await;
        let placement_plan_index: HashMap<&str, &RewardQuotePlan> = cycle
            .plans
            .iter()
            .map(|plan| (plan.condition_id.as_str(), plan))
            .collect();
        let placement_open_orders_snapshot = open_orders.clone();
        let placement_account_snapshot = account.clone();
        let placement_buy_submit_risk = LiveBuySubmitRiskContext {
            config: &cycle.config,
            plans: &placement_plan_index,
            book_history,
            open_orders: &placement_open_orders_snapshot,
            positions: &cycle.positions,
            account: &placement_account_snapshot,
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
            LiveRewardOrderUpdate::Unchanged(event) | LiveRewardOrderUpdate::Retryable(event) => {
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
include!("rewards/live_orderbook_risk.rs");
include!("rewards/live_requote.rs");
include!("rewards/live_risk.rs");
include!("rewards/event_cancel.rs");
include!("rewards/orderbook_events.rs");
include!("rewards/polling.rs");
include!("rewards/provider_advisory.rs");
include!("rewards/provider_fallback.rs");
include!("rewards/provider_content_filter.rs");
include!("rewards/provider_refresh_orderbook.rs");
include!("rewards/provider_refresh.rs");
include!("rewards/provider_refresh_candidates.rs");
include!("rewards/info_risk.rs");
