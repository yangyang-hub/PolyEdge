async fn run_reward_bot_once(state: &AppState, trace_id: &str) -> Result<RewardBotRunReport> {
    let command_report = process_pending_reward_control_commands(state).await?;
    if command_report.processed > 0 {
        return Ok(command_report.report);
    }

    run_reward_bot_tick(state, trace_id, false).await
}

async fn run_reward_bot_tick(
    state: &AppState,
    trace_id: &str,
    force_orders: bool,
) -> Result<RewardBotRunReport> {
    let (markets, books) = fetch_reward_bot_inputs(state).await?;
    let config = state.reward_bot_service.read_config().await?;
    match config.execution_mode {
        RewardExecutionMode::Validation => {
            if force_orders {
                state
                    .reward_bot_service
                    .run_simulation_forced(markets, books, trace_id)
                    .await
            } else {
                state
                    .reward_bot_service
                    .run_simulation(markets, books, trace_id)
                    .await
            }
        }
        RewardExecutionMode::Live => {
            run_reward_bot_live_tick(state, markets, books, trace_id, force_orders).await
        }
    }
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

        let result = execute_reward_control_command(state, &command, &trace_id).await;
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
) -> Result<RewardBotRunReport> {
    match command.action {
        RewardControlAction::RunOnce => run_reward_bot_tick(state, trace_id, true).await,
        RewardControlAction::CancelAll => {
            let config = state.reward_bot_service.read_config().await?;
            if config.execution_mode.is_live() {
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
            } else {
                state
                    .reward_bot_service
                    .cancel_all_orders(
                        command.account_id.as_deref(),
                        "worker processed queued rewards validation cancel-all command",
                        trace_id,
                    )
                    .await?;
                Ok(RewardBotRunReport::default())
            }
        }
        RewardControlAction::Reset => {
            let config = state.reward_bot_service.read_config().await?;
            if config.execution_mode.is_live() {
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
                Ok(cancel_report.as_run_report())
            } else {
                state.reward_bot_service.reset_simulation(trace_id).await?;
                Ok(RewardBotRunReport::default())
            }
        }
    }
}

async fn run_reward_bot_live_tick(
    state: &AppState,
    markets: Vec<RewardMarket>,
    books: HashMap<String, RewardOrderBook>,
    trace_id: &str,
    force_orders: bool,
) -> Result<RewardBotRunReport> {
    let books_fetched = books.len();
    let cycle = state
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

    if !cycle.should_execute {
        return Ok(report);
    }

    let connector = build_live_polymarket_connector(state).await?;
    let mut open_orders = cycle.open_orders.clone();
    let mut changed_orders = Vec::new();
    let mut events = Vec::new();
    let mut cancel_rejected = false;

    for (order_id, reason) in
        live_cancel_candidates(&cycle.config, &cycle.plans, &open_orders, &books)
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
        .filter(|order| account_id.map_or(true, |id| id == order.account_id.as_str()))
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

async fn run_reward_bot_live_reconcile(
    state: &AppState,
    trace_id: &str,
) -> Result<RewardBotRunReport> {
    let cycle = state.reward_bot_service.current_live_cycle_state().await?;
    let books = fetch_reward_bot_active_books(state).await?;
    let cancel_candidates =
        live_cancel_candidates(&cycle.config, &cycle.plans, &cycle.open_orders, &books);
    let mut open_orders = cycle.open_orders.clone();
    let mut changed_orders = Vec::new();
    let mut events = Vec::new();
    let mut report = RewardBotRunReport {
        books_fetched: books.len(),
        ..RewardBotRunReport::default()
    };

    if cancel_candidates.is_empty() {
        return Ok(report);
    }

    let connector = build_live_polymarket_connector(state).await?;
    for (order_id, reason) in cancel_candidates {
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
                report.risk_cancelled_orders += 1;
            }
            LiveRewardOrderUpdate::Unchanged(event) => events.push(event),
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

enum LiveRewardOrderUpdate {
    Changed(ManagedRewardOrder, RewardRiskEvent),
    Unchanged(RewardRiskEvent),
}

async fn submit_one_live_reward_order(
    connector: &LivePolymarketConnector,
    order: &mut ManagedRewardOrder,
) -> Result<LiveRewardOrderUpdate> {
    let request = LivePolymarketTokenOrderRequest {
        client_order_id: order.id.clone(),
        connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
        token_id: order.token_id.clone(),
        side: reward_side_to_polymarket(order.side),
        limit_price: Probability::new(order.price)?,
        quantity: Quantity::new(order.size)?,
        post_only: true,
    };
    match connector.submit_token_order(&request).await? {
        LivePolymarketExecutionOutcome::Accepted(acceptance) => {
            order.external_order_id = Some(acceptance.order_id.clone());
            if acceptance.status != PolymarketAcceptedOrderStatus::Live {
                return handle_non_live_reward_order_acceptance(connector, order, acceptance.status)
                    .await;
            }
            order.status = ManagedRewardOrderStatus::Open;
            order.reason = "live post-only rewards quote accepted".to_string();
            order.updated_at = acceptance.accepted_at;
            Ok(LiveRewardOrderUpdate::Changed(
                order.clone(),
                reward_live_event(
                    order,
                    "reward_live_order_placed",
                    RewardRiskSeverity::Info,
                    format!(
                        "{} live quote placed: {} @ {}",
                        order.outcome, order.size, order.price
                    ),
                    json!({
                        "token_id": order.token_id,
                        "side": order.side.as_str(),
                        "size": order.size,
                        "price": order.price,
                        "polymarket_status": acceptance.status.as_str(),
                    }),
                ),
            ))
        }
        LivePolymarketExecutionOutcome::Rejected(rejection) => Ok(LiveRewardOrderUpdate::Unchanged(
            reward_live_event(
                order,
                "reward_live_order_rejected",
                RewardRiskSeverity::Warning,
                format!("live rewards order rejected: {}", rejection.message),
                json!({ "code": rejection.code }),
            ),
        )),
    }
}

async fn handle_non_live_reward_order_acceptance(
    connector: &LivePolymarketConnector,
    order: &mut ManagedRewardOrder,
    accepted_status: PolymarketAcceptedOrderStatus,
) -> Result<LiveRewardOrderUpdate> {
    let Some(external_order_id) = order.external_order_id.clone() else {
        order.status = ManagedRewardOrderStatus::Error;
        order.scoring = false;
        order.reason = format!(
            "Polymarket returned {} without an order id",
            accepted_status.as_str()
        );
        order.updated_at = OffsetDateTime::now_utc();
        return Ok(LiveRewardOrderUpdate::Changed(
            order.clone(),
            reward_live_event(
                order,
                "reward_live_order_post_only_violation",
                RewardRiskSeverity::Critical,
                order.reason.clone(),
                json!({ "polymarket_status": accepted_status.as_str() }),
            ),
        ));
    };

    let cancel_request = LivePolymarketCancelOrderRequest {
        connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
        external_order_id: external_order_id.clone(),
    };
    match connector.cancel_order(&cancel_request).await? {
        LivePolymarketCancelOutcome::Accepted(acceptance) => {
            order.status = ManagedRewardOrderStatus::Cancelled;
            order.scoring = false;
            order.reason = format!(
                "Polymarket returned {} for a post-only rewards quote; order cancelled immediately",
                accepted_status.as_str()
            );
            order.updated_at = acceptance.cancelled_at;
            Ok(LiveRewardOrderUpdate::Changed(
                order.clone(),
                reward_live_event(
                    order,
                    "reward_live_order_post_only_violation_cancelled",
                    RewardRiskSeverity::Critical,
                    order.reason.clone(),
                    json!({
                        "external_order_id": acceptance.external_order_id,
                        "polymarket_status": accepted_status.as_str(),
                    }),
                ),
            ))
        }
        LivePolymarketCancelOutcome::Rejected(rejection) => {
            order.status = ManagedRewardOrderStatus::Error;
            order.scoring = false;
            order.reason = format!(
                "Polymarket returned {} for a post-only rewards quote and cancel was rejected: {}",
                accepted_status.as_str(),
                rejection.message
            );
            order.updated_at = OffsetDateTime::now_utc();
            Ok(LiveRewardOrderUpdate::Changed(
                order.clone(),
                reward_live_event(
                    order,
                    "reward_live_order_post_only_violation_cancel_rejected",
                    RewardRiskSeverity::Critical,
                    order.reason.clone(),
                    json!({
                        "code": rejection.code,
                        "external_order_id": external_order_id,
                        "polymarket_status": accepted_status.as_str(),
                    }),
                ),
            ))
        }
    }
}

async fn cancel_one_live_reward_order(
    connector: &LivePolymarketConnector,
    mut order: ManagedRewardOrder,
    reason: &str,
    _trace_id: &str,
) -> Result<LiveRewardOrderUpdate> {
    let Some(external_order_id) = order.external_order_id.clone() else {
        order.status = ManagedRewardOrderStatus::Cancelled;
        order.scoring = false;
        order.reason = format!("local-only order cancelled: {reason}");
        order.updated_at = OffsetDateTime::now_utc();
        return Ok(LiveRewardOrderUpdate::Changed(
            order.clone(),
            reward_live_event(
                &order,
                "reward_live_order_cancelled",
                RewardRiskSeverity::Info,
                order.reason.clone(),
                json!({ "local_only": true }),
            ),
        ));
    };

    if external_order_id.starts_with("sim_") {
        order.status = ManagedRewardOrderStatus::Cancelled;
        order.scoring = false;
        order.reason = format!("validation order cancelled before live execution: {reason}");
        order.updated_at = OffsetDateTime::now_utc();
        return Ok(LiveRewardOrderUpdate::Changed(
            order.clone(),
            reward_live_event(
                &order,
                "reward_live_order_cancelled",
                RewardRiskSeverity::Info,
                order.reason.clone(),
                json!({ "local_validation_order": true }),
            ),
        ));
    }

    let request = LivePolymarketCancelOrderRequest {
        connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
        external_order_id: external_order_id.clone(),
    };
    match connector.cancel_order(&request).await? {
        LivePolymarketCancelOutcome::Accepted(acceptance) => {
            order.status = ManagedRewardOrderStatus::Cancelled;
            order.scoring = false;
            order.reason = reason.to_string();
            order.updated_at = acceptance.cancelled_at;
            Ok(LiveRewardOrderUpdate::Changed(
                order.clone(),
                reward_live_event(
                    &order,
                    "reward_live_order_cancelled",
                    RewardRiskSeverity::Info,
                    format!("{} live order cancelled: {reason}", order.outcome),
                    json!({ "external_order_id": acceptance.external_order_id }),
                ),
            ))
        }
        LivePolymarketCancelOutcome::Rejected(rejection) => Ok(LiveRewardOrderUpdate::Unchanged(
            reward_live_event(
                &order,
                "reward_live_order_cancel_rejected",
                RewardRiskSeverity::Warning,
                format!("live rewards cancel rejected: {}", rejection.message),
                json!({ "code": rejection.code, "external_order_id": external_order_id }),
            ),
        )),
    }
}

fn live_cancel_candidates(
    config: &RewardBotConfig,
    plans: &[RewardQuotePlan],
    open_orders: &[ManagedRewardOrder],
    books: &HashMap<String, RewardOrderBook>,
) -> Vec<(String, String)> {
    let plan_index: HashMap<&str, &RewardQuotePlan> = plans
        .iter()
        .map(|plan| (plan.condition_id.as_str(), plan))
        .collect();
    let now = OffsetDateTime::now_utc();
    open_orders
        .iter()
        .filter(|order| order.status.is_open_like())
        .filter_map(|order| {
            live_cancel_reason(config, &plan_index, books, order, now)
                .map(|reason| (order.id.clone(), reason))
        })
        .collect()
}

fn live_cancel_reason(
    config: &RewardBotConfig,
    plans: &HashMap<&str, &RewardQuotePlan>,
    books: &HashMap<String, RewardOrderBook>,
    order: &ManagedRewardOrder,
    now: OffsetDateTime,
) -> Option<String> {
    if let Some(reason) = live_quote_book_unavailable_reason(config, books, &order.token_id, now) {
        return Some(reason);
    }
    if order.side != RewardOrderSide::Buy {
        return None;
    }
    let Some(plan) = plans.get(order.condition_id.as_str()) else {
        return Some("market no longer offers rewards".to_string());
    };
    if !plan.eligible {
        return Some("market dropped below eligibility threshold".to_string());
    }
    let Some(leg) = plan.legs.iter().find(|leg| leg.token_id == order.token_id) else {
        return Some("token no longer appears in live quote plan".to_string());
    };
    if config.requote_drift_cents > Decimal::ZERO {
        let drift_cents = ((order.price - leg.price).abs()) * Decimal::from(100_u64);
        if drift_cents > config.requote_drift_cents {
            return Some(format!(
                "midpoint drifted {drift_cents} cents beyond requote threshold"
            ));
        }
    }
    None
}

fn live_placement_orders(
    config: &RewardBotConfig,
    account_id: &str,
    plans: &[RewardQuotePlan],
    books: &HashMap<String, RewardOrderBook>,
    open_orders: &[ManagedRewardOrder],
    positions: &[RewardPosition],
    trace_id: &str,
) -> Vec<ManagedRewardOrder> {
    let max_markets = usize::from(config.max_markets);
    let max_open_orders = usize::from(config.max_open_orders);
    if max_markets == 0 || max_open_orders == 0 {
        return Vec::new();
    }

    let mut active_markets: HashSet<String> = open_orders
        .iter()
        .filter(|order| order.status.is_open_like())
        .map(|order| order.condition_id.clone())
        .collect();
    let mut orders = open_orders.to_vec();
    let mut placements = Vec::new();
    let mut seq = 0usize;

    for plan in plans.iter().filter(|plan| plan.eligible) {
        if !live_plan_has_fresh_quote_books(plan, books, config) {
            continue;
        }
        if active_markets.len() >= max_markets && !active_markets.contains(&plan.condition_id) {
            continue;
        }
        for leg in &plan.legs {
            if orders.iter().filter(|order| order.status.is_open_like()).count()
                >= max_open_orders
            {
                return placements;
            }
            if orders.iter().any(|order| {
                order.condition_id == plan.condition_id
                    && order.token_id == leg.token_id
                    && order.side == RewardOrderSide::Buy
                    && order.status.is_open_like()
            }) {
                continue;
            }
            if live_position_over_cap(config, positions, &leg.token_id, leg.price) {
                continue;
            }
            let notional = (leg.price * leg.size).round_dp(4);
            if notional <= Decimal::ZERO {
                continue;
            }
            // Live maker buys intentionally do not reserve global cash until a
            // fill is observed. This cap applies to actual inventory only, so
            // the same funds can be quoted across markets while orders rest.
            if config.max_global_position_usd > Decimal::ZERO
                && live_global_inventory_notional(positions) + notional
                    > config.max_global_position_usd
            {
                continue;
            }

            active_markets.insert(plan.condition_id.clone());
            seq += 1;
            let now = OffsetDateTime::now_utc();
            let order = ManagedRewardOrder {
                id: format!(
                    "rewlive_{}_{}_{}",
                    now.unix_timestamp_nanos(),
                    seq,
                    trace_id.trim_start_matches("trc_")
                ),
                account_id: account_id.to_string(),
                condition_id: plan.condition_id.clone(),
                token_id: leg.token_id.clone(),
                outcome: leg.outcome.clone(),
                side: RewardOrderSide::Buy,
                price: leg.price,
                size: leg.size,
                external_order_id: None,
                status: ManagedRewardOrderStatus::Planned,
                scoring: true,
                reason: "pending live post-only rewards quote".to_string(),
                filled_size: Decimal::ZERO,
                reward_earned: Decimal::ZERO,
                last_scored_at: None,
                created_at: now,
                updated_at: now,
            };
            orders.push(order.clone());
            placements.push(order);
        }
    }

    placements
}

fn live_plan_has_fresh_quote_books(
    plan: &RewardQuotePlan,
    books: &HashMap<String, RewardOrderBook>,
    config: &RewardBotConfig,
) -> bool {
    let now = OffsetDateTime::now_utc();
    plan.legs.iter().all(|leg| {
        live_quote_book_unavailable_reason(config, books, &leg.token_id, now).is_none()
    })
}

fn live_quote_book_unavailable_reason(
    config: &RewardBotConfig,
    books: &HashMap<String, RewardOrderBook>,
    token_id: &str,
    now: OffsetDateTime,
) -> Option<String> {
    let Some(book) = books.get(token_id) else {
        return Some("orderbook unavailable for live order".to_string());
    };
    if book.bids.is_empty() || book.asks.is_empty() {
        return Some("orderbook is empty for live order".to_string());
    }
    if config.stale_book_ms == 0 {
        return None;
    }

    let age_ms = (now - book.observed_at).whole_milliseconds();
    if age_ms < 0 || age_ms > i128::from(config.stale_book_ms) {
        return Some(format!(
            "orderbook stale for live order: age_ms={age_ms}, max_age_ms={}",
            config.stale_book_ms
        ));
    }
    None
}

fn live_position_over_cap(
    config: &RewardBotConfig,
    positions: &[RewardPosition],
    token_id: &str,
    price: Decimal,
) -> bool {
    config.max_position_usd > Decimal::ZERO
        && positions.iter().any(|position| {
            position.token_id == token_id
                && position.size > Decimal::ZERO
                && position.size * price >= config.max_position_usd
        })
}

fn live_global_inventory_notional(positions: &[RewardPosition]) -> Decimal {
    positions
        .iter()
        .filter(|position| position.size > Decimal::ZERO)
        .map(|position| position.size * position.avg_price)
        .sum()
}

fn reward_side_to_polymarket(side: RewardOrderSide) -> PolymarketTokenOrderSide {
    match side {
        RewardOrderSide::Buy => PolymarketTokenOrderSide::Buy,
        RewardOrderSide::Sell => PolymarketTokenOrderSide::Sell,
    }
}

fn reward_live_event(
    order: &ManagedRewardOrder,
    event_type: &str,
    severity: RewardRiskSeverity,
    message: impl Into<String>,
    metadata: serde_json::Value,
) -> RewardRiskEvent {
    new_risk_event(
        Some(order.account_id.clone()),
        Some(order.condition_id.clone()),
        order.external_order_id.clone(),
        event_type,
        severity,
        message,
        metadata,
    )
}

async fn poll_reward_bot(
    state: &AppState,
    max_cycles: Option<usize>,
) -> Result<RewardBotRunReport> {
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    poll_reward_bot_loop(state, max_cycles, shutdown_rx, true).await
}

async fn poll_reward_bot_until_shutdown(
    state: &AppState,
    shutdown_rx: watch::Receiver<bool>,
) -> Result<RewardBotRunReport> {
    poll_reward_bot_loop(state, None, shutdown_rx, false).await
}

async fn poll_reward_bot_loop(
    state: &AppState,
    max_cycles: Option<usize>,
    mut shutdown_rx: watch::Receiver<bool>,
    listen_for_ctrl_c: bool,
) -> Result<RewardBotRunReport> {
    let mut total = RewardBotRunReport {
        markets_scanned: 0,
        books_fetched: 0,
        plans_built: 0,
        eligible_plans: 0,
        simulated_orders: 0,
        cancelled_orders: 0,
        filled_orders: 0,
        risk_cancelled_orders: 0,
        reward_accrued: rust_decimal::Decimal::ZERO,
    };
    let mut full_cycles = 0usize;
    let mut reconcile_cycles = 0usize;
    let full_interval = Duration::from_secs(state.settings.rewards.poll_interval_secs.max(1));
    // Start with a full cycle immediately.
    let mut last_full_at = Instant::now() - full_interval;

    loop {
        // Read the live config to get the reconcile interval.
        let config = state.reward_bot_service.read_config().await.unwrap_or_default();
        let reconcile_interval = Duration::from_secs(config.reconcile_interval_sec.max(1));
        let now = Instant::now();
        let since_full = now.duration_since(last_full_at);

        if since_full >= full_interval {
            // --- Full simulation cycle (rebuilds plans) ---
            let trace_id = new_trace_id();
            let report = run_reward_bot_once(state, &trace_id).await?;
            accumulate_report(&mut total, &report);
            full_cycles += 1;
            last_full_at = Instant::now();

            info!(
                trace_id = %trace_id,
                full_cycle = full_cycles,
                markets_scanned = report.markets_scanned,
                eligible_plans = report.eligible_plans,
                cancelled = report.cancelled_orders,
                risk_cancelled = report.risk_cancelled_orders,
                "completed full reward bot cycle",
            );

            if max_cycles.is_some_and(|limit| full_cycles >= limit) {
                break;
            }
        } else {
            // --- Fast reconcile-only cycle (risk checks + fills + quotes) ---
            let trace_id = new_trace_id();
            let report = if config.execution_mode.is_live() {
                run_reward_bot_live_reconcile(state, &trace_id).await?
            } else {
                let books = fetch_reward_bot_active_books(state).await?;
                state
                    .reward_bot_service
                    .run_reconcile_only(books, &trace_id)
                    .await?
            };
            accumulate_report(&mut total, &report);
            reconcile_cycles += 1;

            if report.risk_cancelled_orders > 0 || report.filled_orders > 0 {
                info!(
                    trace_id = %trace_id,
                    reconcile_cycle = reconcile_cycles,
                    risk_cancelled = report.risk_cancelled_orders,
                    filled = report.filled_orders,
                    "fast reconcile cycle",
                );
            }
        }

        // Sleep until the next reconcile tick or the next full cycle, whichever
        // comes first. Also check for shutdown.
        let elapsed_since_full = Instant::now().duration_since(last_full_at);
        let next_full_in = full_interval.checked_sub(elapsed_since_full).unwrap_or(reconcile_interval);
        let sleep_dur = reconcile_interval.min(next_full_in);

        tokio::select! {
            () = tokio::time::sleep(sleep_dur) => {}
            changed = shutdown_rx.changed() => {
                if changed.is_err() || *shutdown_rx.borrow() {
                    break;
                }
            }
            shutdown = tokio::signal::ctrl_c(), if listen_for_ctrl_c => {
                if let Err(error) = shutdown {
                    warn!(error = %error, "failed to listen for ctrl-c during reward bot polling");
                }
                break;
            }
        }
    }

    Ok(total)
}

fn accumulate_report(total: &mut RewardBotRunReport, report: &RewardBotRunReport) {
    total.markets_scanned += report.markets_scanned;
    total.books_fetched += report.books_fetched;
    total.plans_built += report.plans_built;
    total.eligible_plans += report.eligible_plans;
    total.simulated_orders += report.simulated_orders;
    total.cancelled_orders += report.cancelled_orders;
    total.filled_orders += report.filled_orders;
    total.risk_cancelled_orders += report.risk_cancelled_orders;
    total.reward_accrued += report.reward_accrued;
}

async fn fetch_reward_bot_inputs(
    state: &AppState,
) -> Result<(Vec<RewardMarket>, HashMap<String, RewardOrderBook>)> {
    // Read a bounded candidate pool from database (synced by the sync-markets worker).
    let markets = state
        .reward_bot_service
        .list_reward_run_candidate_markets()
        .await?;

    // Read order books from the worker-local cache maintained by orderbook-stream.
    let token_ids = select_reward_book_token_ids(&markets);
    let mut books = HashMap::new();
    let cache = state.orderbook_cache.clone();
    let cached_books = stream::iter(token_ids)
        .map(|token_id| {
            let cache = cache.clone();
            async move { cache.get_book(&token_id).await }
        })
        .buffer_unordered(32)
        .collect::<Vec<_>>()
        .await;

    for cached in cached_books {
        if let Some(cached) = cached? {
            books.insert(cached.token_id.clone(), cached_order_book_to_reward(&cached));
        }
    }

    Ok((markets, books))
}

/// Lightweight book fetch for the fast reconcile loop: only reads books for
/// tokens where the bot currently has open orders or positions (not the full
/// candidate market set).
async fn fetch_reward_bot_active_books(
    state: &AppState,
) -> Result<HashMap<String, RewardOrderBook>> {
    let token_ids = state
        .reward_bot_service
        .list_active_reward_book_token_ids()
        .await?;

    let mut books = HashMap::new();
    let cache = state.orderbook_cache.clone();
    let cached_books = stream::iter(token_ids)
        .map(|token_id| {
            let cache = cache.clone();
            async move { cache.get_book(&token_id).await }
        })
        .buffer_unordered(32)
        .collect::<Vec<_>>()
        .await;

    for cached in cached_books {
        if let Some(cached) = cached? {
            books.insert(cached.token_id.clone(), cached_order_book_to_reward(&cached));
        }
    }

    Ok(books)
}

fn cached_order_book_to_reward(book: &CachedOrderBook) -> RewardOrderBook {
    RewardOrderBook {
        token_id: book.token_id.clone(),
        bids: book
            .bids
            .iter()
            .map(|level| RewardBookLevel {
                price: level.price,
                size: level.size,
            })
            .collect(),
        asks: book
            .asks
            .iter()
            .map(|level| RewardBookLevel {
                price: level.price,
                size: level.size,
            })
            .collect(),
        observed_at: {
            let secs = book.observed_at / 1000;
            let nsecs = ((book.observed_at % 1000) * 1_000_000) as u32;
            OffsetDateTime::from_unix_timestamp(secs)
                .map(|dt| dt + TimeDuration::nanoseconds(i64::from(nsecs)))
                .unwrap_or_else(|_| OffsetDateTime::now_utc())
        },
    }
}

fn reward_market_from_connector(market: PolymarketRewardMarket) -> RewardMarket {
    RewardMarket {
        condition_id: market.condition_id,
        question: market.question,
        market_slug: market.market_slug,
        event_slug: market.event_slug,
        image: market.image,
        rewards_max_spread: market.rewards_max_spread,
        rewards_min_size: market.rewards_min_size,
        total_daily_rate: market.total_daily_rate,
        tokens: market
            .tokens
            .into_iter()
            .map(|token| RewardToken {
                token_id: token.token_id,
                outcome: token.outcome,
                price: token.price,
            })
            .collect(),
        active: market.active,
        updated_at: market.updated_at,
    }
}
