async fn sync_live_reward_orders(
    state: &AppState,
    connector: &LivePolymarketConnector,
    open_orders: &[ManagedRewardOrder],
    books: &HashMap<String, RewardOrderBook>,
    trace_id: &str,
) -> Result<RewardBotRunReport> {
    let mut report = RewardBotRunReport::default();
    let cycle = state.reward_bot_service.current_live_cycle_state().await?;
    let mut account = cycle.account.clone();
    let mut positions: HashMap<String, RewardPosition> = cycle
        .positions
        .iter()
        .cloned()
        .map(|position| (position.token_id.clone(), position))
        .collect();
    let mut sibling_cancelled = HashSet::new();
    let mut working_orders: HashMap<String, ManagedRewardOrder> = cycle
        .open_orders
        .iter()
        .cloned()
        .map(|order| (order.id.clone(), order))
        .collect();
    let external_order_index: HashMap<String, String> = cycle
        .open_orders
        .iter()
        .filter_map(|order| {
            order
                .external_order_id
                .as_ref()
                .map(|external_order_id| (external_order_id.clone(), order.id.clone()))
        })
        .collect();

    for order in open_orders
        .iter()
        .filter(|order| {
            order
                .external_order_id
                .as_ref()
                .is_some_and(|id| !is_internal_reward_order_id(id))
        })
    {
        let Some(external_order_id) = order.external_order_id.as_deref() else {
            continue;
        };

        let trade_sync = match connector
            .collect_trade_updates(&LivePolymarketTradeSyncRequest {
                connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
                account_id: connector.account_id().to_string(),
                external_order_id: external_order_id.to_string(),
            })
            .await
        {
            Ok(outcome) => outcome,
            Err(error) if error.code() == "POLYMARKET_ORDER_NOT_FOUND" => {
                let Some(mut missing_order) = working_orders.get(&order.id).cloned() else {
                    continue;
                };
                if !missing_order
                    .reason
                    .contains("external order lookup returned not found")
                {
                    missing_order.scoring = false;
                    missing_order.reason = format!(
                        "external order lookup returned not found; manual reconciliation required: {external_order_id}"
                    );
                    missing_order.updated_at = OffsetDateTime::now_utc();
                    let event = reward_live_event(
                        &missing_order,
                        "reward_live_external_order_not_found",
                        RewardRiskSeverity::Critical,
                        missing_order.reason.clone(),
                        json!({ "external_order_id": external_order_id }),
                    );
                    working_orders.insert(missing_order.id.clone(), missing_order.clone());
                    persist_live_reward_updates(
                        state,
                        &mut account,
                        positions.values().cloned().collect(),
                        vec![missing_order],
                        Vec::new(),
                        vec![event],
                        &report,
                        trace_id,
                    )
                    .await?;
                }
                continue;
            }
            Err(error) => return Err(error),
        };

        for update in trade_sync.updates {
            let fill_id = reward_live_fill_id(&update);
            let legacy_fill_id = reward_live_legacy_fill_id(&update);
            if state
                .reward_bot_service
                .reward_fill_exists(&fill_id)
                .await?
                || state
                    .reward_bot_service
                    .reward_fill_exists(&legacy_fill_id)
                    .await?
            {
                continue;
            }

            let Some(current_order) = external_order_index
                .get(&update.external_order_id)
                .and_then(|order_id| working_orders.get(order_id))
                .cloned()
            else {
                continue;
            };
            if !current_order.status.is_open_like() {
                continue;
            }

            let Some(fill_update) = apply_live_reward_fill_update(
                current_order,
                &mut account,
                &mut positions,
                &update,
                &fill_id,
                trace_id,
            ) else {
                continue;
            };
            report.filled_orders += 1;
            let LiveRewardFillUpdate {
                order: filled_order,
                fill,
                event,
                fill_size,
            } = fill_update;
            working_orders.insert(filled_order.id.clone(), filled_order.clone());
            let mut changed_orders = vec![filled_order.clone()];
            let mut events = vec![event];
            if filled_order.side == RewardOrderSide::Buy {
                for update in plan_live_post_fill_orders(
                    &cycle.config,
                    &filled_order,
                    fill_size,
                    &positions,
                    books,
                    trace_id,
                ) {
                    match update {
                        LiveRewardOrderUpdate::Changed(order, event) => {
                            working_orders.insert(order.id.clone(), order.clone());
                            changed_orders.push(order);
                            events.push(event);
                        }
                        LiveRewardOrderUpdate::Unchanged(event) => events.push(event),
                    }
                }
            }
            persist_live_reward_updates(
                state,
                &mut account,
                positions.values().cloned().collect(),
                changed_orders,
                vec![fill],
                events,
                &report,
                trace_id,
            )
            .await?;

            if filled_order.side == RewardOrderSide::Buy {
                if cycle.config.cancel_on_fill {
                    cancel_sibling_live_reward_orders(
                        connector,
                        &mut working_orders,
                        &filled_order,
                        &mut sibling_cancelled,
                        state,
                        &mut account,
                        &positions,
                        &mut report,
                        trace_id,
                    )
                    .await?;
                }
            }
        }

        if let Some(status_update) = trade_sync.order_status {
            let Some(current_order) = external_order_index
                .get(&status_update.external_order_id)
                .and_then(|order_id| working_orders.get(order_id))
                .cloned()
            else {
                continue;
            };
            if let Some((order, event)) = apply_live_reward_status_update_to_order(
                current_order.clone(),
                status_update,
                trace_id,
            )
            {
                working_orders.insert(order.id.clone(), order.clone());
                let should_retry_exit = order.status == ManagedRewardOrderStatus::Cancelled;
                let mut changed_orders = vec![order];
                let mut events = vec![event];
                if should_retry_exit
                    && let Some(retry) = deferred_live_exit_after_cancellation(
                        &current_order,
                        positions.get(&current_order.token_id),
                        trace_id,
                    )
                {
                    events.push(reward_live_event(
                        &retry,
                        "reward_live_exit_retry_deferred",
                        RewardRiskSeverity::Warning,
                        "deferred a replacement rewards exit after external cancellation",
                        json!({
                            "cancelled_order_id": current_order.id,
                            "cancelled_external_order_id": current_order.external_order_id,
                            "retry_order_id": retry.id,
                            "retry_size": retry.size,
                        }),
                    ));
                    working_orders.insert(retry.id.clone(), retry.clone());
                    changed_orders.push(retry);
                }
                persist_live_reward_updates(
                    state,
                    &mut account,
                    positions.values().cloned().collect(),
                    changed_orders,
                    Vec::new(),
                    events,
                    &report,
                    trace_id,
                )
                .await?;
            }
        }
    }
    Ok(report)
}

async fn run_reward_bot_live_reconcile(
    state: &AppState,
    trace_id: &str,
    book_history: &mut HashMap<String, VecDeque<BookSnapshot>>,
) -> Result<RewardBotRunReport> {
    let Some(lease) = state
        .try_acquire_postgres_advisory_lease(REWARD_WORKER_ADVISORY_LOCK_KEY)
        .await?
    else {
        debug!("skipping rewards reconcile because another worker holds the live lease");
        return Ok(RewardBotRunReport::default());
    };
    let result = run_reward_bot_live_reconcile_unlocked(state, trace_id, book_history).await;
    finish_reward_worker_lease(lease, result).await
}

async fn run_reward_bot_live_reconcile_unlocked(
    state: &AppState,
    trace_id: &str,
    book_history: &mut HashMap<String, VecDeque<BookSnapshot>>,
) -> Result<RewardBotRunReport> {
    let mut cycle = state.reward_bot_service.current_live_cycle_state().await?;
    let books = fetch_reward_bot_active_books(state).await?;
    record_reward_book_history(book_history, &books);
    let mut report = RewardBotRunReport {
        books_fetched: books.len(),
        ..RewardBotRunReport::default()
    };

    let live_connector = build_live_polymarket_connector(state).await?;
    if !cycle.open_orders.is_empty() {
        let sync_report =
            sync_live_reward_orders(state, &live_connector, &cycle.open_orders, &books, trace_id)
                .await?;
        accumulate_report(&mut report, &sync_report);
        cycle = state.reward_bot_service.current_live_cycle_state().await?;
    }

    if can_refresh_external_account_after_order_sync(&report) {
        sync_external_account_state(
            state,
            &live_connector,
            &mut cycle.account,
            &mut cycle.positions,
            trace_id,
        )
        .await;
    }

    let mut connector = Some(live_connector);
    let mut account = cycle.account.clone();
    let mut open_orders = cycle.open_orders.clone();
    let kill_switch = state.risk_service.read_state().await?.kill_switch;

    let cancel_candidates = live_cancel_candidates(
        &cycle.config,
        &cycle.plans,
        &open_orders,
        &books,
        book_history,
        kill_switch,
    );

    if !cancel_candidates.is_empty() {
        let live_connector = match connector.take() {
            Some(connector) => connector,
            None => build_live_polymarket_connector(state).await?,
        };
        for (order_id, reason) in cancel_candidates {
            let Some(index) = open_orders.iter().position(|order| order.id == order_id) else {
                continue;
            };
            let order = open_orders[index].clone();
            match cancel_one_live_reward_order(&live_connector, order, &reason, trace_id).await? {
                LiveRewardOrderUpdate::Changed(updated, event) => {
                    open_orders[index] = updated.clone();
                    if !live_cancel_result_is_unknown(&updated) {
                        report.cancelled_orders += 1;
                        report.risk_cancelled_orders += 1;
                    }
                    persist_live_reward_updates(
                        state,
                        &mut account,
                        cycle.positions.clone(),
                        vec![updated],
                        Vec::new(),
                        vec![event],
                        &report,
                        trace_id,
                    )
                    .await?;
                }
                LiveRewardOrderUpdate::Unchanged(event) => {
                    persist_live_reward_updates(
                        state,
                        &mut account,
                        cycle.positions.clone(),
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
        connector = Some(live_connector);
    }

    if open_orders.iter().any(|order| {
        order.external_order_id.is_none()
            && (order.status == ManagedRewardOrderStatus::Planned
                || order.status == ManagedRewardOrderStatus::ExitPending)
    }) {
        let live_connector = match connector {
            Some(connector) => connector,
            None => build_live_polymarket_connector(state).await?,
        };
        submit_pending_live_reward_orders(
            &live_connector,
            &mut open_orders,
            &books,
            state,
            &mut account,
            &cycle.positions,
            &mut report,
            trace_id,
            false,
        )
        .await?;
    }

    Ok(report)
}
