struct RewardLiveOrderSyncReport {
    report: RewardBotRunReport,
    reconciliation_reliable: bool,
}

async fn sync_live_reward_orders(
    state: &AppState,
    connector: &LivePolymarketConnector,
    open_orders: &[ManagedRewardOrder],
    books: &HashMap<String, RewardOrderBook>,
    trace_id: &str,
) -> Result<RewardLiveOrderSyncReport> {
    let mut report = RewardBotRunReport::default();
    let mut reconciliation_reliable = true;
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

    for order in open_orders.iter().filter(|order| {
        order
            .external_order_id
            .as_ref()
            .is_some_and(|id| !is_internal_reward_order_id(id))
    }) {
        let Some(external_order_id) = order.external_order_id.as_deref() else {
            continue;
        };

        let (trade_sync, external_snapshot_includes_fill) = match connector
            .collect_trade_updates(&LivePolymarketTradeSyncRequest {
                connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
                account_id: connector.account_id().to_string(),
                external_order_id: external_order_id.to_string(),
                fallback_token_id: Some(order.token_id.clone()),
                fallback_after: Some(order.created_at.unix_timestamp().saturating_sub(300)),
            })
            .await
        {
            Ok(outcome) if should_try_data_api_fallback_for_clob_outcome(&outcome) => {
                match collect_data_api_reward_trade_fallback(
                    state,
                    connector,
                    order,
                    open_orders,
                    &cycle.account,
                    &cycle.positions,
                    true,
                )
                .await
                {
                    Ok(Some(fallback)) => (
                        fallback.outcome,
                        fallback.external_snapshot_includes_fill,
                    ),
                    Ok(None) => (outcome, false),
                    Err(error) => {
                        warn!(
                            external_order_id,
                            error = %error,
                            "Data API fallback failed after an empty missing-order trade scan"
                        );
                        (outcome, false)
                    }
                }
            }
            Ok(outcome) => (outcome, false),
            Err(error) if is_missing_external_order_reconciliation_error(&error) => {
                if error.code() == "POLYMARKET_MISSING_ORDER_TRADE_QUERY_FAILED" {
                    warn!(
                        external_order_id,
                        error = %error,
                        "fallback trade query failed for missing rewards order; keeping the reconciliation lock and continuing the cycle",
                    );
                }
                match collect_data_api_reward_trade_fallback(
                    state,
                    connector,
                    order,
                    open_orders,
                    &cycle.account,
                    &cycle.positions,
                    true,
                )
                .await
                {
                    Ok(Some(fallback)) => (
                        fallback.outcome,
                        fallback.external_snapshot_includes_fill,
                    ),
                    Ok(None) | Err(_) => {
                        let Some(missing_order) = working_orders.get(&order.id).cloned() else {
                            continue;
                        };
                        if let Some((missing_order, event)) =
                            mark_live_external_order_not_found(missing_order, external_order_id)
                        {
                            if missing_order.status == ManagedRewardOrderStatus::Cancelled {
                                report.cancelled_orders += 1;
                            }
                            working_orders.insert(missing_order.id.clone(), missing_order.clone());
                            persist_live_reward_updates(
                                state,
                                &mut account,
                                Vec::new(), // positions unchanged
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
                }
            }
            Err(error) => {
                warn!(
                    external_order_id,
                    error_code = error.code(),
                    error = %error,
                    "failed to reconcile managed rewards order; continuing with remaining orders"
                );
                match collect_data_api_reward_trade_fallback(
                    state,
                    connector,
                    order,
                    open_orders,
                    &cycle.account,
                    &cycle.positions,
                    false,
                )
                .await
                {
                    Ok(Some(fallback)) => (
                        fallback.outcome,
                        fallback.external_snapshot_includes_fill,
                    ),
                    Ok(None) => {
                        reconciliation_reliable = false;
                        continue;
                    }
                    Err(fallback_error) => {
                        warn!(
                            external_order_id,
                            error = %fallback_error,
                            "Data API trade fallback failed; continuing with remaining orders"
                        );
                        reconciliation_reliable = false;
                        continue;
                    }
                }
            }
        };

        let order_not_found = trade_sync.order_not_found;
        let order_status = trade_sync.order_status;

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
                external_snapshot_includes_fill,
            ) else {
                continue;
            };
            report.filled_orders += 1;
            let LiveRewardFillUpdate {
                order: filled_order,
                fill,
                event,
                fill_size,
                overdraft_warning,
            } = fill_update;
            working_orders.insert(filled_order.id.clone(), filled_order.clone());
            let mut changed_orders = vec![filled_order.clone()];
            let mut merge_intents = Vec::new();
            let mut events = vec![event];
            if let Some(warning) = overdraft_warning {
                events.push(warning);
            }
            if filled_order.side == RewardOrderSide::Buy {
                for update in plan_live_post_fill_orders(
                    &cycle.config,
                    &cycle.plans,
                    &filled_order,
                    fill_size,
                    &positions,
                    books,
                    reward_ai_min_confidence(state.settings.rewards.ai_min_confidence_bps),
                    trace_id,
                ) {
                    match update {
                        LiveRewardOrderUpdate::Changed(order, event) => {
                            working_orders.insert(order.id.clone(), order.clone());
                            changed_orders.push(order);
                            events.push(event);
                        }
                        LiveRewardOrderUpdate::Unchanged(event)
                        | LiveRewardOrderUpdate::Retryable(event) => events.push(event),
                    }
                }
                let (planned_merge_intents, merge_events) = plan_live_balanced_merge_intent(
                    state,
                    &cycle.config,
                    &filled_order,
                    &fill,
                    &positions,
                    trace_id,
                )
                .await?;
                merge_intents.extend(planned_merge_intents);
                events.extend(merge_events);
            }
            persist_live_reward_updates_with_merge_intents(
                state,
                &mut account,
                positions.values().cloned().collect(),
                changed_orders,
                vec![fill],
                merge_intents,
                events,
                &report,
                trace_id,
            )
            .await?;

            if filled_order.side == RewardOrderSide::Buy
                && cycle.config.cancel_on_fill
                && filled_order.strategy_profile != RewardStrategyProfile::BalancedMerge
            {
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

        if let Some(status_update) = order_status {
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
            ) {
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
                    Vec::new(), // positions unchanged during status update
                    changed_orders,
                    Vec::new(),
                    events,
                    &report,
                    trace_id,
                )
                .await?;
            }
        }

        if order_not_found {
            let Some(current_order) = working_orders.get(&order.id).cloned() else {
                continue;
            };
            if let Some((missing_order, event)) =
                mark_live_external_order_not_found(current_order, external_order_id)
            {
                if missing_order.status == ManagedRewardOrderStatus::Cancelled {
                    report.cancelled_orders += 1;
                }
                working_orders.insert(missing_order.id.clone(), missing_order.clone());
                persist_live_reward_updates(
                    state,
                    &mut account,
                    Vec::new(),
                    vec![missing_order],
                    Vec::new(),
                    vec![event],
                    &report,
                    trace_id,
                )
                .await?;
            }
        }
    }
    Ok(RewardLiveOrderSyncReport {
        report,
        reconciliation_reliable,
    })
}

struct DataApiRewardTradeFallback {
    outcome: LivePolymarketTradeSyncOutcome,
    external_snapshot_includes_fill: bool,
}

async fn collect_data_api_reward_trade_fallback(
    state: &AppState,
    connector: &LivePolymarketConnector,
    order: &ManagedRewardOrder,
    open_orders: &[ManagedRewardOrder],
    account: &RewardAccountState,
    positions: &[RewardPosition],
    allow_missing_order: bool,
) -> Result<Option<DataApiRewardTradeFallback>> {
    if order.side != RewardOrderSide::Buy {
        return Ok(None);
    }
    let Some(external_order_id) = order.external_order_id.as_deref() else {
        return Ok(None);
    };
    let matched_size = match connector
        .matched_order_hint(external_order_id)
        .await
    {
        Ok(Some(hint)) if hint.token_id == order.token_id && hint.price == order.price => {
            hint.size_matched
        }
        Ok(Some(_)) | Ok(None) if !allow_missing_order => return Ok(None),
        Err(error) if !allow_missing_order || error.code() != "POLYMARKET_ORDER_NOT_FOUND" => {
            return Err(error);
        }
        Ok(Some(_)) | Ok(None) | Err(_) => order.size,
    };
    let wallet = polymarket_funding_wallet_address(
        &state.settings.polymarket.account_id,
        state.settings.polymarket.funder.as_deref(),
    )
    .ok_or_else(|| {
        AppError::invalid_input(
            "POLYMARKET_FUNDING_WALLET_REQUIRED",
            "funding wallet is required for Data API trade fallback",
        )
    })?;
    let data_connector = PolymarketDataApiConnector::new(&state.settings.polymarket.data_api_host)?;
    let activities = data_connector.fetch_wallet_activity(&wallet, 500).await?;
    let mut matches = activities
        .into_iter()
        .filter(|activity| data_api_activity_matches_reward_order(activity, order, open_orders))
        .collect::<Vec<_>>();
    matches.sort_by_key(|activity| activity.timestamp);
    matches.dedup_by(|left, right| left.transaction_hash == right.transaction_hash);

    let remaining = (matched_size.min(order.size) - order.filled_size).max(Decimal::ZERO);
    let matched_size: Decimal = matches.iter().map(|activity| activity.size).sum();
    if remaining <= Decimal::ZERO || matched_size != remaining {
        return Ok(None);
    }
    let latest_trade_at = matches
        .last()
        .map(|activity| activity.timestamp)
        .unwrap_or(order.created_at);
    let updates = matches
        .into_iter()
        .map(|activity| data_api_activity_to_fill_update(activity, order))
        .collect::<Result<Vec<_>>>()?;
    let external_snapshot_includes_fill = external_snapshot_covers_buy_fill(
        account,
        positions,
        order,
        remaining,
        latest_trade_at,
    );
    if allow_missing_order && !external_snapshot_includes_fill {
        return Ok(None);
    }
    Ok(Some(DataApiRewardTradeFallback {
        outcome: LivePolymarketTradeSyncOutcome {
            updates,
            order_status: None,
            order_not_found: false,
        },
        external_snapshot_includes_fill,
    }))
}

fn data_api_activity_matches_reward_order(
    activity: &PolymarketWalletActivity,
    order: &ManagedRewardOrder,
    open_orders: &[ManagedRewardOrder],
) -> bool {
    if activity.kind != "TRADE"
        || activity.side != "BUY"
        || activity.asset != order.token_id
        || activity.price != order.price
        || activity.size <= Decimal::ZERO
        || activity.transaction_hash.is_empty()
        || activity.timestamp < order.created_at - TimeDuration::seconds(5)
    {
        return false;
    }
    open_orders
        .iter()
        .filter(|candidate| {
            candidate.status.is_open_like()
                && candidate.side == order.side
                && candidate.token_id == order.token_id
                && candidate.price == order.price
                && candidate.created_at <= activity.timestamp + TimeDuration::seconds(5)
        })
        .count()
        == 1
}

fn data_api_activity_to_fill_update(
    activity: PolymarketWalletActivity,
    order: &ManagedRewardOrder,
) -> Result<ConnectorTradeFillUpdate> {
    let external_order_id = order.external_order_id.clone().ok_or_else(|| {
        AppError::invalid_input(
            "REWARD_EXTERNAL_ORDER_REQUIRED",
            "Data API fill fallback requires an external order id",
        )
    })?;
    Ok(ConnectorTradeFillUpdate {
        event_id: format!(
            "evt_pm_data_trade:{}:{}",
            external_order_id,
            activity.transaction_hash
        ),
        connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
        external_order_id,
        account_id: order.account_id.clone(),
        external_trade_id: format!("data_api:{}", activity.transaction_hash),
        fill_price: Probability::new(activity.price)?,
        filled_quantity: Quantity::new(activity.size)?,
        fee: UsdAmount::new(Decimal::ZERO)?,
    })
}

fn external_snapshot_covers_buy_fill(
    account: &RewardAccountState,
    positions: &[RewardPosition],
    order: &ManagedRewardOrder,
    fill_size: Decimal,
    latest_trade_at: OffsetDateTime,
) -> bool {
    account.updated_at >= latest_trade_at
        && positions.iter().any(|position| {
            position.token_id == order.token_id
                && position.updated_at >= latest_trade_at
                && position.size >= order.filled_size + fill_size
        })
}

fn is_missing_external_order_reconciliation_error(error: &AppError) -> bool {
    matches!(
        error.code(),
        "POLYMARKET_ORDER_NOT_FOUND" | "POLYMARKET_MISSING_ORDER_TRADE_QUERY_FAILED"
    )
}

fn should_try_data_api_fallback_for_clob_outcome(
    outcome: &LivePolymarketTradeSyncOutcome,
) -> bool {
    outcome.order_not_found && outcome.updates.is_empty()
}

async fn run_reward_bot_live_reconcile_unlocked(
    state: &AppState,
    connector: &LivePolymarketConnector,
    trace_id: &str,
    book_history: &mut HashMap<String, VecDeque<BookSnapshot>>,
    orderbook_cache: Option<&RewardOrderbookLocalCache>,
    sync_policy: RewardFastReconcileSyncPolicy,
) -> Result<RewardBotRunReport> {
    let mut cycle = state.reward_bot_service.current_live_cycle_state().await?;
    let books = fetch_reward_bot_active_books(state, orderbook_cache).await?;
    record_reward_book_history(book_history, &books);
    let mut report = RewardBotRunReport {
        books_fetched: books.len(),
        ..RewardBotRunReport::default()
    };

    let mut live_order_sync_reliable = true;
    if sync_policy.order_statuses && !cycle.open_orders.is_empty() {
        let sync_report =
            sync_live_reward_orders(state, connector, &cycle.open_orders, &books, trace_id)
                .await?;
        live_order_sync_reliable = sync_report.reconciliation_reliable;
        accumulate_report(&mut report, &sync_report.report);
        cycle = state.reward_bot_service.current_live_cycle_state().await?;
    }

    if sync_policy.reward_earnings {
        sync_reward_earnings(state, connector, &mut cycle.account, trace_id).await;
    }

    let account_sync_policy = RewardAccountSyncPolicy {
        managed_scoring: sync_policy.managed_scoring,
        open_orders: sync_policy.open_orders,
        account_snapshot: sync_policy.account_snapshot
            && (sync_policy.order_statuses || cycle.open_orders.is_empty()),
        close_absent_buy_orders: live_order_sync_reliable,
    };
    if account_sync_policy.any() {
        sync_external_account_state_with_policy(
            state,
            connector,
            &mut cycle.account,
            &mut cycle.positions,
            &mut cycle.open_orders,
            trace_id,
            can_refresh_external_account_after_order_sync(&report),
            account_sync_policy,
        )
        .await;
    }

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
            "submitted balanced merge transactions during fast reconcile"
        );
    }

    let kill_switch = state.risk_service.read_state().await?.kill_switch;

    let cancel_candidates = live_cancel_candidates_with_account(
        &cycle.config,
        &cycle.plans,
        &open_orders,
        &books,
        book_history,
        &account,
        kill_switch,
    );

    if !cancel_candidates.is_empty() {
        for (order_id, reason) in cancel_candidates {
            let Some(index) = open_orders.iter().position(|order| order.id == order_id) else {
                continue;
            };
            let order = open_orders[index].clone();
            match cancel_one_live_reward_order(connector, order, &reason, trace_id).await? {
                LiveRewardOrderUpdate::Changed(updated, event) => {
                    open_orders[index] = updated.clone();
                    if !live_cancel_result_is_unknown(&updated) {
                        report.cancelled_orders += 1;
                        report.risk_cancelled_orders += 1;
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
    }

    if open_orders.iter().any(|order| {
        order.external_order_id.is_none()
            && (order.status == ManagedRewardOrderStatus::Planned
                || order.status == ManagedRewardOrderStatus::ExitPending)
    }) {
        submit_pending_live_reward_orders(
            connector,
            &mut open_orders,
            &books,
            None,
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
