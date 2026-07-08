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

async fn record_reward_fair_value_estimates(
    state: &AppState,
    config: &RewardBotConfig,
    estimates: &[RewardFairValueEstimate],
    trace_id: &str,
) {
    if !config.fair_value_enabled
        || !config.fair_value_record_history_enabled
        || estimates.is_empty()
    {
        return;
    }
    if let Err(error) = state
        .reward_bot_service
        .record_fair_value_estimates(estimates)
        .await
    {
        warn!(
            trace_id = %trace_id,
            error = %error,
            estimates = estimates.len(),
            "failed to record reward fair-value estimates"
        );
    }
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

async fn execute_pending_balanced_merge_intents(
    state: &AppState,
    config: &RewardBotConfig,
    account: &mut RewardAccountState,
    positions: &[RewardPosition],
    open_orders: &[ManagedRewardOrder],
    report: &RewardBotRunReport,
    trace_id: &str,
) -> Result<usize> {
    if !config.balanced_merge_enabled || !config.balanced_merge_auto_execute_enabled {
        return Ok(0);
    }
    let private_key = normalize_optional_config_string(state.settings.polymarket.private_key.as_deref());
    let Some(private_key) = private_key else {
        warn!(
            trace_id = %trace_id,
            "balanced merge auto execution enabled but polymarket private key is not configured"
        );
        return Ok(0);
    };
    let proxy_wallet_address = account
        .wallet_address
        .as_deref()
        .and_then(|value| normalize_optional_config_string(Some(value)))
        .or_else(|| normalize_optional_config_string(state.settings.polymarket.funder.as_deref()))
        .unwrap_or_else(|| config.account_id.clone());
    let chain = PolymarketChainConnector::new(&state.settings.polymarket.polygon_rpc_url)?;
    let intents = state
        .reward_bot_service
        .list_executable_reward_merge_intents(&config.account_id, 10)
        .await?;
    if intents.is_empty() {
        return Ok(0);
    }

    let mut executed = 0usize;
    for intent in intents {
        let mut event_account = account.clone();
        let maybe_error = balanced_merge_intent_preflight_error(&intent, positions, open_orders);
        if let Some(reason) = maybe_error {
            debug!(
                trace_id = %trace_id,
                merge_intent_id = %intent.id,
                condition_id = %intent.condition_id,
                reason = %reason,
                "skipped balanced merge intent execution for this cycle"
            );
            continue;
        }

        match chain
            .submit_merge_positions(
                &private_key,
                state.settings.polymarket.chain_id,
                PolymarketMergePositionsRequest {
                    proxy_wallet_address: proxy_wallet_address.clone(),
                    condition_id: intent.condition_id.clone(),
                    amount: intent.merge_size,
                },
            )
            .await
        {
            Ok(receipt) => {
                executed += 1;
                let now = OffsetDateTime::now_utc();
                let reason = format!(
                    "submitted balanced merge transaction {} for {} shares",
                    receipt.tx_hash, receipt.amount
                );
                state
                    .reward_bot_service
                    .mark_reward_merge_intent_submitted(
                        &intent.id,
                        &receipt.tx_hash,
                        now,
                        &reason,
                    )
                    .await?;
                persist_live_reward_updates(
                    state,
                    &mut event_account,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    vec![new_risk_event(
                        Some(intent.account_id.clone()),
                        Some(intent.condition_id.clone()),
                        None,
                        "reward_live_balanced_merge_submitted",
                        RewardRiskSeverity::Info,
                        reason,
                        json!({
                            "merge_intent_id": intent.id,
                            "tx_hash": receipt.tx_hash,
                            "owner_address": receipt.owner_address,
                            "proxy_wallet_address": receipt.proxy_wallet_address,
                            "merge_size": receipt.amount,
                            "amount_units": receipt.amount_units,
                            "safe_nonce": receipt.safe_nonce,
                        }),
                    )],
                    report,
                    trace_id,
                )
                .await?;
            }
            Err(error) => {
                let now = OffsetDateTime::now_utc();
                let reason = format!("balanced merge transaction failed before broadcast: {error}");
                state
                    .reward_bot_service
                    .mark_reward_merge_intent_failed(&intent.id, &reason, now)
                    .await?;
                persist_live_reward_updates(
                    state,
                    &mut event_account,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    vec![new_risk_event(
                        Some(intent.account_id.clone()),
                        Some(intent.condition_id.clone()),
                        None,
                        "reward_live_balanced_merge_failed",
                        RewardRiskSeverity::Critical,
                        reason,
                        json!({
                            "merge_intent_id": intent.id,
                            "yes_token_id": intent.yes_token_id,
                            "no_token_id": intent.no_token_id,
                            "merge_size": intent.merge_size,
                        }),
                    )],
                    report,
                    trace_id,
                )
                .await?;
            }
        }
    }
    Ok(executed)
}

fn balanced_merge_intent_preflight_error(
    intent: &RewardMergeIntent,
    positions: &[RewardPosition],
    open_orders: &[ManagedRewardOrder],
) -> Option<String> {
    if open_orders.iter().any(|order| {
        order.status.is_open_like()
            && order.side == RewardOrderSide::Sell
            && (order.token_id == intent.yes_token_id || order.token_id == intent.no_token_id)
    }) {
        return Some("balanced merge skipped because a SELL order is open for one of the paired tokens".to_string());
    }
    let yes_size = positions
        .iter()
        .find(|position| {
            position.account_id == intent.account_id && position.token_id == intent.yes_token_id
        })
        .map_or(Decimal::ZERO, |position| position.size);
    let no_size = positions
        .iter()
        .find(|position| {
            position.account_id == intent.account_id && position.token_id == intent.no_token_id
        })
        .map_or(Decimal::ZERO, |position| position.size);
    if yes_size < intent.merge_size || no_size < intent.merge_size {
        return Some(format!(
            "balanced merge skipped because current paired inventory is insufficient: yes={yes_size}, no={no_size}, required={}",
            intent.merge_size
        ));
    }
    None
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

async fn start_reward_strategy_run_for_cycle(
    state: &AppState,
    cycle: &RewardLiveCycle,
    trace_id: &str,
    force_orders: bool,
    books_fetched: usize,
) -> Result<i64> {
    let config_json = serde_json::to_value(&cycle.config).map_err(|error| {
        AppError::internal(
            "REWARD_STRATEGY_RUN_CONFIG_SERIALIZE_FAILED",
            format!("failed to serialize reward strategy config for run ledger: {error}"),
        )
    })?;
    state
        .reward_bot_service
        .start_strategy_run(&RewardStrategyRunStart {
            account_id: cycle.config.account_id.clone(),
            trace_id: trace_id.to_string(),
            trigger_type: if force_orders {
                RewardStrategyRunTrigger::RunOnce
            } else {
                RewardStrategyRunTrigger::Poll
            },
            config_hash: reward_config_hash(&cycle.config),
            config_json,
            input_summary: reward_strategy_run_input_summary(cycle, books_fetched, force_orders),
            started_at: OffsetDateTime::now_utc(),
        })
        .await
}

async fn save_reward_quote_plans_for_run(
    state: &AppState,
    run_id: i64,
    plans: &mut [RewardQuotePlan],
) -> Result<()> {
    for plan in plans.iter_mut() {
        plan.latest_run_id = Some(run_id);
    }
    let decisions =
        reward_strategy_decisions_from_plans(run_id, plans, OffsetDateTime::now_utc());
    state.reward_bot_service.save_quote_plans(plans).await?;
    state
        .reward_bot_service
        .record_strategy_decisions(&decisions)
        .await
}

fn reward_strategy_run_input_summary(
    cycle: &RewardLiveCycle,
    books_fetched: usize,
    force_orders: bool,
) -> Value {
    json!({
        "force_orders": force_orders,
        "should_execute": cycle.should_execute,
        "markets": cycle.markets.len(),
        "plans": cycle.plans.len(),
        "pre_ai_eligible_plans": cycle.pre_ai_eligible_condition_ids.len(),
        "books_fetched": books_fetched,
        "open_orders": cycle.open_orders.len(),
        "positions": cycle.positions.len(),
        "account": {
            "available_usd": cycle.account.available_usd,
            "reserved_usd": cycle.account.reserved_usd,
            "external_buy_notional": cycle.account.external_buy_notional,
            "unmanaged_external_buy_notional": cycle.account.unmanaged_external_buy_notional,
            "tick_index": cycle.account.tick_index,
        },
    })
}

fn reward_strategy_run_metrics_from_report(report: &RewardBotRunReport) -> Value {
    json!({
        "markets_scanned": report.markets_scanned,
        "books_fetched": report.books_fetched,
        "plans_built": report.plans_built,
        "eligible_plans": report.eligible_plans,
        "placed_orders": report.placed_orders,
        "cancelled_orders": report.cancelled_orders,
        "filled_orders": report.filled_orders,
        "risk_cancelled_orders": report.risk_cancelled_orders,
        "reward_accrued": report.reward_accrued,
    })
}

async fn run_reward_bot_live_tick(
    state: &AppState,
    connector: &LivePolymarketConnector,
    markets: Vec<RewardCandidateMarket>,
    books: HashMap<String, RewardOrderBook>,
    trace_id: &str,
    force_orders: bool,
    book_history: &mut HashMap<String, VecDeque<BookSnapshot>>,
    orderbook_cache: Option<&RewardOrderbookLocalCache>,
) -> Result<RewardBotRunReport> {
    let books_fetched = books.len();
    let cycle = state
        .reward_bot_service
        .prepare_live_cycle(
            markets,
            books.clone(),
            trace_id,
            force_orders,
        )
        .await?;
    let run_id =
        start_reward_strategy_run_for_cycle(state, &cycle, trace_id, force_orders, books_fetched)
            .await?;
    let result = run_reward_bot_live_tick_prepared(
        state,
        connector,
        cycle,
        books,
        books_fetched,
        run_id,
        trace_id,
        book_history,
        orderbook_cache,
    )
    .await;
    let completed_at = OffsetDateTime::now_utc();
    match result {
        Ok(report) => {
            state
                .reward_bot_service
                .complete_strategy_run(
                    run_id,
                    reward_strategy_run_metrics_from_report(&report),
                    completed_at,
                )
                .await?;
            Ok(report)
        }
        Err(error) => {
            let error_message = error.to_string();
            if let Err(fail_error) = state
                .reward_bot_service
                .fail_strategy_run(
                    run_id,
                    error.code(),
                    &error_message,
                    json!({ "failed": true }),
                    completed_at,
                )
                .await
            {
                warn!(
                    trace_id = %trace_id,
                    run_id,
                    error = %fail_error,
                    "failed to mark reward strategy run as failed"
                );
            }
            Err(error)
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_reward_bot_live_tick_prepared(
    state: &AppState,
    connector: &LivePolymarketConnector,
    mut cycle: RewardLiveCycle,
    mut books: HashMap<String, RewardOrderBook>,
    books_fetched: usize,
    run_id: i64,
    trace_id: &str,
    book_history: &mut HashMap<String, VecDeque<BookSnapshot>>,
    orderbook_cache: Option<&RewardOrderbookLocalCache>,
) -> Result<RewardBotRunReport> {
    let pre_provider_decisions = RewardDecisionEngine::evaluate_pre_provider(RewardStrategyInput {
        cycle,
        books: &books,
        book_history,
        now: OffsetDateTime::now_utc(),
    });
    let funding_precheck_blocked = pre_provider_decisions.funding_precheck_blocked;
    cycle = pre_provider_decisions.cycle;
    record_reward_fair_value_estimates(
        state,
        &cycle.config,
        &pre_provider_decisions.fair_value_estimates,
        trace_id,
    )
    .await;
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
    let post_provider_decisions =
        RewardDecisionEngine::evaluate_post_provider(cycle, OffsetDateTime::now_utc());
    let first_quote_entry_changed = post_provider_decisions.first_quote_entry_changed;
    cycle = post_provider_decisions.cycle;
    if first_quote_entry_changed {
        debug!(
            trace_id = %trace_id,
            "applied first-quote entry gates to reward quote plans"
        );
    }
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
    let (merge_intents, merge_events) = plan_live_balanced_merge_intents_for_positions(
        state,
        &cycle.config,
        &cycle.positions,
        trace_id,
    )
    .await?;
    if !merge_intents.is_empty() || !merge_events.is_empty() {
        persist_live_reward_updates_with_merge_intents(
            state,
            &mut account,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            merge_intents,
            merge_events,
            &report,
            trace_id,
        )
        .await?;
    }
    let executed_merge_intents = execute_pending_balanced_merge_intents(
        state,
        &cycle.config,
        &mut account,
        &cycle.positions,
        &open_orders,
        &report,
        trace_id,
    )
    .await?;
    if executed_merge_intents > 0 {
        debug!(
            trace_id = %trace_id,
            executed_merge_intents,
            "submitted balanced merge transactions"
        );
    }

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

    cycle.account = account.clone();
    cycle.open_orders = open_orders.clone();
    let final_decisions = RewardDecisionEngine::refresh_snapshot(RewardStrategyInput {
        cycle,
        books: &books,
        book_history,
        now: OffsetDateTime::now_utc(),
    });
    let readiness_changed = final_decisions.readiness_changed;
    cycle = final_decisions.cycle;
    record_reward_fair_value_estimates(
        state,
        &cycle.config,
        &final_decisions.fair_value_estimates,
        trace_id,
    )
    .await;
    save_reward_quote_plans_for_run(state, run_id, &mut cycle.plans).await?;
    register_reward_eligible_orderbook_tokens_from_plans(state, &cycle.plans, trace_id).await;
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
            LiveRewardOrderUpdate::CancelReplace(_) => {
                unreachable!("cancel_one_live_reward_order never returns CancelReplace")
            }
        }
    }

    for update in reselect_adaptive_exit_orders(
        &cycle.config,
        &cycle.plans,
        &books,
        &cycle.positions,
        &mut open_orders,
        reward_ai_min_confidence(state.settings.rewards.ai_min_confidence_bps),
        trace_id,
        OffsetDateTime::now_utc(),
    ) {
        match update {
            LiveRewardOrderUpdate::Changed(updated, event) => {
                persist_live_reward_updates(
                    state,
                    &mut account,
                    Vec::new(), // positions unchanged during adaptive exit reselection
                    vec![updated],
                    Vec::new(),
                    vec![event],
                    &report,
                    trace_id,
                )
                .await?;
            }
            LiveRewardOrderUpdate::Unchanged(event) | LiveRewardOrderUpdate::Retryable(event) => {
                persist_live_reward_updates(
                    state,
                    &mut account,
                    Vec::new(), // positions unchanged during adaptive exit reselection
                    Vec::new(),
                    Vec::new(),
                    vec![event],
                    &report,
                    trace_id,
                )
                .await?;
            }
            LiveRewardOrderUpdate::CancelReplace(intent) => {
                // The reselect decision used a snapshot; execute against the freshest
                // in-memory row. The helper owns its re-entry guard, so a state change
                // since the decision emits a skipped event instead of touching the CLOB.
                let Some(index) = open_orders.iter().position(|o| o.id == intent.order.id) else {
                    continue;
                };
                let outcome = cancel_replace_live_exit_order(
                    connector,
                    &open_orders[index],
                    intent.floor_price,
                    intent.new_price,
                    intent.new_strategy,
                    intent.decision_meta,
                    intent.drift_cents,
                    &cycle.positions,
                    trace_id,
                )
                .await?;

                // outcome.orders[0] is the cancelled/awaiting original (same id as
                // intent.order); any later row is the fresh replacement (new id).
                if let Some(cancelled) = outcome.orders.first() {
                    open_orders[index] = cancelled.clone();
                }
                for extra in outcome.orders.iter().skip(1) {
                    open_orders.push(extra.clone());
                }

                persist_live_reward_updates(
                    state,
                    &mut account,
                    Vec::new(), // positions unchanged during cancel-replace
                    outcome.orders,
                    Vec::new(),
                    outcome.events,
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
        save_reward_quote_plans_for_run(state, run_id, &mut cycle.plans).await?;
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
            LiveRewardOrderUpdate::CancelReplace(_) => {
                unreachable!("cancel_one_live_reward_order never returns CancelReplace")
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
