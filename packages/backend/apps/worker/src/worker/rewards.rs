const REWARD_WORKER_ADVISORY_LOCK_KEY: i64 = 0x504f_4c59_5245_5744;
const LIVE_EXTERNAL_ORDER_NOT_FOUND_MARKER: &str = "external order lookup returned not found";
const LIVE_EXTERNAL_ORDER_NOT_FOUND_CLOSE_AFTER_SECS: i64 = 300;

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
    info!(
        trace_id = %trace_id,
        markets = cycle.markets.len(),
        books = books_fetched,
        plans = cycle.plans.len(),
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
    refresh_reward_ai_advisories(state, &mut cycle, &books, trace_id).await?;
    apply_cached_reward_info_risks_to_cycle(state, &mut cycle, trace_id).await?;
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
    submit_pending_live_reward_orders(
        connector,
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
        &account,
        &cycle.plans,
        &books,
        &open_orders,
        &cycle.positions,
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
            connector,
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

async fn refresh_reward_ai_advisories(
    state: &AppState,
    cycle: &mut RewardLiveCycle,
    books: &HashMap<String, RewardOrderBook>,
    trace_id: &str,
) -> Result<()> {
    if !cycle.config.ai_advisory_enabled {
        info!(
            trace_id = %trace_id,
            plans = cycle.plans.len(),
            "skipping reward AI advisory refresh because it is disabled in rewards config",
        );
        return Ok(());
    }
    if cycle.plans.is_empty() {
        info!(
            trace_id = %trace_id,
            "skipping reward AI advisory refresh because no quote plans were built",
        );
        return Ok(());
    }
    info!(
        trace_id = %trace_id,
        provider = cycle.config.ai_provider.as_str(),
        request_format = cycle.config.ai_request_format.as_str(),
        plans = cycle.plans.len(),
        open_orders = cycle.open_orders.len(),
        positions = cycle.positions.len(),
        "starting reward AI advisory refresh",
    );
    let Some(connector) = build_reward_ai_advisory_connector(state, &cycle.config)? else {
        warn!(
            trace_id = %trace_id,
            provider = cycle.config.ai_provider.as_str(),
            "reward AI advisory is enabled but provider configuration is incomplete",
        );
        return Ok(());
    };

    let model = state.settings.rewards.ai_model.trim();
    if model.is_empty() {
        warn!(trace_id = %trace_id, "reward AI advisory model is empty");
        return Ok(());
    }

    let min_confidence = reward_ai_min_confidence(state.settings.rewards.ai_min_confidence_bps);
    let markets_by_condition = cycle
        .markets
        .iter()
        .map(|market| (market.condition_id.as_str(), market))
        .collect::<HashMap<_, _>>();
    let mut advisories = HashMap::<String, RewardMarketAdvisory>::new();
    let candidate_plans =
        reward_ai_advisory_candidate_plans(&cycle.plans, &cycle.open_orders, &cycle.positions);
    let candidates = candidate_plans.len();
    let mut cache_hits = 0usize;
    let mut requested = 0usize;
    let mut saved = 0usize;
    let mut failures = 0usize;
    let mut skipped_missing_market = 0usize;

    for plan in candidate_plans {
        let Some(market) = markets_by_condition.get(plan.condition_id.as_str()) else {
            skipped_missing_market += 1;
            continue;
        };
        let request = build_reward_ai_advisory_request(
            market,
            plan,
            &cycle.account,
            &cycle.positions,
            &cycle.open_orders,
            books,
            &cycle.config,
            cycle.config.ai_provider,
            cycle.config.ai_request_format,
            model,
        )?;
        if let Some(cached) = state
            .reward_bot_service
            .latest_market_advisory(&request)
            .await?
        {
            cache_hits += 1;
            advisories.insert(plan.condition_id.clone(), cached);
            continue;
        }

        requested += 1;
        match connector.advise(&request).await {
            Ok(decision) => {
                let advisory = decision.into_advisory(
                    &request,
                    cycle.config.ai_advisory_ttl_sec,
                    OffsetDateTime::now_utc(),
                );
                state
                    .reward_bot_service
                    .save_market_advisory(&advisory)
                    .await?;
                saved += 1;
                advisories.insert(plan.condition_id.clone(), advisory);
            }
            Err(error) => {
                failures += 1;
                warn!(
                    trace_id = %trace_id,
                    condition_id = %plan.condition_id,
                    error = %error,
                    "reward AI advisory request failed; keeping deterministic plan",
                );
            }
        }
    }

    info!(
        trace_id = %trace_id,
        candidates,
        cache_hits,
        requested,
        saved,
        failures,
        skipped_missing_market,
        applied = advisories.len(),
        "completed reward AI advisory refresh",
    );

    if advisories.is_empty() {
        return Ok(());
    }
    apply_reward_ai_advisories(
        &mut cycle.plans,
        &advisories,
        &cycle.config,
        min_confidence,
    );
    state.reward_bot_service.save_quote_plans(&cycle.plans).await?;
    Ok(())
}

fn reward_ai_advisory_candidate_plans<'a>(
    plans: &'a [RewardQuotePlan],
    open_orders: &[ManagedRewardOrder],
    positions: &[RewardPosition],
) -> Vec<&'a RewardQuotePlan> {
    let plans_by_condition = plans
        .iter()
        .map(|plan| (plan.condition_id.as_str(), plan))
        .collect::<HashMap<_, _>>();
    let mut seen = HashSet::new();
    let mut ordered = Vec::with_capacity(plans.len());

    for order in open_orders {
        push_reward_ai_advisory_plan(
            &mut ordered,
            &mut seen,
            &plans_by_condition,
            &order.condition_id,
        );
    }
    for position in positions {
        push_reward_ai_advisory_plan(
            &mut ordered,
            &mut seen,
            &plans_by_condition,
            &position.condition_id,
        );
    }
    for plan in plans.iter().filter(|plan| plan.eligible) {
        push_reward_ai_advisory_plan(
            &mut ordered,
            &mut seen,
            &plans_by_condition,
            &plan.condition_id,
        );
    }
    for plan in plans {
        push_reward_ai_advisory_plan(
            &mut ordered,
            &mut seen,
            &plans_by_condition,
            &plan.condition_id,
        );
    }

    ordered
}

fn push_reward_ai_advisory_plan<'a>(
    ordered: &mut Vec<&'a RewardQuotePlan>,
    seen: &mut HashSet<String>,
    plans_by_condition: &HashMap<&str, &'a RewardQuotePlan>,
    condition_id: &str,
) {
    let condition_id = condition_id.trim();
    if condition_id.is_empty() {
        return;
    }
    let Some(plan) = plans_by_condition.get(condition_id) else {
        return;
    };
    if seen.insert(condition_id.to_string()) {
        ordered.push(*plan);
    }
}

fn build_reward_ai_advisory_connector(
    state: &AppState,
    config: &RewardBotConfig,
) -> Result<Option<RewardAiAdvisoryConnector>> {
    let rewards = &state.settings.rewards;
    let (api_key, base_url) = match config.ai_provider {
        polyedge_application::RewardAiProvider::OpenAi => (
            rewards.ai_openai_api_key.as_deref(),
            rewards.ai_openai_base_url.as_str(),
        ),
        polyedge_application::RewardAiProvider::Anthropic => (
            rewards.ai_anthropic_api_key.as_deref(),
            rewards.ai_anthropic_base_url.as_str(),
        ),
    };
    let Some(api_key) = api_key.filter(|value| !value.trim().is_empty()) else {
        return Ok(None);
    };
    RewardAiAdvisoryConnector::new(
        base_url,
        api_key,
        rewards.ai_request_timeout_secs.max(1),
    )
    .map(Some)
}

fn reward_ai_min_confidence(bps: u16) -> Decimal {
    Decimal::from(bps.min(10_000)) / Decimal::from(10_000_u64)
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
include!("rewards/info_risk.rs");
