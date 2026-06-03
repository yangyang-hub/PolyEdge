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
    let mut changed_orders = Vec::new();
    let mut fills = Vec::new();
    let mut events = Vec::new();
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
        .filter(|order| order.external_order_id.is_some())
    {
        let Some(external_order_id) = order.external_order_id.as_deref() else {
            continue;
        };

        let trade_updates = match connector
            .collect_trade_updates(&LivePolymarketTradeSyncRequest {
                connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
                account_id: connector.account_id().to_string(),
                external_order_id: external_order_id.to_string(),
            })
            .await
        {
            Ok(updates) => updates,
            Err(error) if error.code() == "POLYMARKET_ORDER_NOT_FOUND" => Vec::new(),
            Err(error) => return Err(error),
        };

        for update in trade_updates {
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
            changed_orders.push(filled_order.clone());
            fills.push(fill);
            events.push(event);

            if filled_order.side == RewardOrderSide::Buy {
                let exit_updates = submit_live_post_fill_orders(
                    connector,
                    &cycle.config,
                    &filled_order,
                    fill_size,
                    &positions,
                    books,
                    trace_id,
                )
                .await?;
                for update in exit_updates {
                    match update {
                        LiveRewardOrderUpdate::Changed(order, event) => {
                            let submitted = order.external_order_id.is_some();
                            working_orders.insert(order.id.clone(), order.clone());
                            changed_orders.push(order);
                            events.push(event);
                            if submitted {
                                report.simulated_orders += 1;
                            }
                        }
                        LiveRewardOrderUpdate::Unchanged(event) => events.push(event),
                    }
                }

                if cycle.config.cancel_on_fill {
                    cancel_sibling_live_reward_orders(
                        connector,
                        &mut working_orders,
                        &filled_order,
                        &mut sibling_cancelled,
                        &mut changed_orders,
                        &mut events,
                        &mut report,
                        trace_id,
                    )
                    .await?;
                }
            }
        }

        let status_update = match connector
            .poll_order_status(&LivePolymarketOrderStatusRequest {
                connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
                external_order_id: external_order_id.to_string(),
            })
            .await
        {
            Ok(update) => update,
            Err(error) if error.code() == "POLYMARKET_ORDER_NOT_FOUND" => None,
            Err(error) => return Err(error),
        };
        if let Some(status_update) = status_update {
            let Some(current_order) = external_order_index
                .get(&status_update.external_order_id)
                .and_then(|order_id| working_orders.get(order_id))
                .cloned()
            else {
                continue;
            };
            if let Some((order, event)) =
                apply_live_reward_status_update_to_order(current_order, status_update, trace_id)
            {
                working_orders.insert(order.id.clone(), order.clone());
                changed_orders.push(order);
                events.push(event);
            }
        }
    }

    if changed_orders.is_empty() && fills.is_empty() && events.is_empty() {
        return Ok(report);
    }

    account.tick_index += 1;
    account.updated_at = OffsetDateTime::now_utc();
    let outcome = RewardSimulationOutcome {
        account,
        markets: cycle.markets,
        plans: cycle.plans,
        orders: changed_orders,
        positions: positions.into_values().collect(),
        fills,
        events,
        report: report.clone(),
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
    book_history: &mut HashMap<String, VecDeque<BookSnapshot>>,
) -> Result<RewardBotRunReport> {
    let mut cycle = state.reward_bot_service.current_live_cycle_state().await?;
    let books = fetch_reward_bot_active_books(state).await?;
    record_reward_book_history(book_history, &books);
    let mut changed_orders = Vec::new();
    let mut events = Vec::new();
    let mut report = RewardBotRunReport {
        books_fetched: books.len(),
        ..RewardBotRunReport::default()
    };

    let mut connector = None;
    if !cycle.open_orders.is_empty() {
        let live_connector = build_live_polymarket_connector(state).await?;
        let sync_report =
            sync_live_reward_orders(state, &live_connector, &cycle.open_orders, &books, trace_id)
                .await?;
        accumulate_report(&mut report, &sync_report);
        connector = Some(live_connector);
        cycle = state.reward_bot_service.current_live_cycle_state().await?;
    }

    let mut open_orders = cycle.open_orders.clone();

    if open_orders.iter().any(|order| {
        order.side == RewardOrderSide::Sell
            && order.status == ManagedRewardOrderStatus::ExitPending
            && order.external_order_id.is_none()
    }) {
        let live_connector = match connector.take() {
            Some(connector) => connector,
            None => build_live_polymarket_connector(state).await?,
        };
        submit_deferred_live_exit_orders(
            &live_connector,
            &mut open_orders,
            &books,
            &mut changed_orders,
            &mut events,
            &mut report,
        )
        .await?;
        connector = Some(live_connector);
    }

    let cancel_candidates = live_cancel_candidates(
        &cycle.config,
        &cycle.plans,
        &open_orders,
        &books,
        book_history,
    );

    if cancel_candidates.is_empty() && changed_orders.is_empty() && events.is_empty() {
        return Ok(report);
    }

    if !cancel_candidates.is_empty() {
        let connector = match connector {
            Some(connector) => connector,
            None => build_live_polymarket_connector(state).await?,
        };
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
