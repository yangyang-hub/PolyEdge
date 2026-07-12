const REWARD_ACTION_EXECUTOR_MIN_LEASE_SECS: i64 = 30;

#[derive(Debug, Clone, PartialEq)]
enum RewardDurableDispatch {
    ReconciliationRequired,
    ReplanRequired,
    CreateMergeIntent(RewardMergeIntent),
    ExecuteMerge(polyedge_application::RewardDurableMergeActionPayload),
    SubmitExitSell(polyedge_application::RewardDurableOrderActionPayload),
    CancelOrder(polyedge_application::RewardDurableOrderActionPayload),
    CancelReplaceExit(polyedge_application::RewardDurableOrderActionPayload),
    PlaceBuy(polyedge_application::RewardDurableOrderActionPayload),
}

#[derive(Debug)]
struct RewardDurableDispatchDecision {
    disposition: RewardDurableDispatch,
    parse_error: Option<String>,
}

fn classify_reward_durable_action(
    action: &RewardStrategyAction,
) -> RewardDurableDispatchDecision {
    match action.parse_durable_request() {
        Ok(validated) => RewardDurableDispatchDecision {
            disposition: match validated.envelope.request {
                // Exit SELL is safe to recover across an expired lease because the
                // dispatcher always performs strict venue matching first. It only
                // submits after a fresh position/book preflight proves that no
                // matching open order exists and inventory still remains.
                RewardDurableActionRequest::SubmitExitSell(payload) => {
                    RewardDurableDispatch::SubmitExitSell(payload)
                }
                // A persisted transaction hash changes execute-merge recovery
                // from "possibly broadcast" to read-only receipt
                // reconciliation. This path is safe across any lease attempt
                // because it never signs or broadcasts another transaction.
                RewardDurableActionRequest::ExecuteMerge(payload) => {
                    RewardDurableDispatch::ExecuteMerge(payload)
                }
                request => match validated.recovery {
                RewardDurableActionRecovery::Recoverable => match request {
                    RewardDurableActionRequest::CreateMergeIntent(payload) => {
                        RewardDurableDispatch::CreateMergeIntent(payload.merge_intent)
                    }
                    RewardDurableActionRequest::CancelOrder(payload) => {
                        RewardDurableDispatch::CancelOrder(payload)
                    }
                    RewardDurableActionRequest::CancelReplaceExit(payload) => {
                        RewardDurableDispatch::CancelReplaceExit(payload)
                    }
                    RewardDurableActionRequest::PlaceBuy(payload) => {
                        RewardDurableDispatch::PlaceBuy(payload)
                    }
                    _ => RewardDurableDispatch::ReplanRequired,
                },
                RewardDurableActionRecovery::ReconciliationRequired => {
                    RewardDurableDispatch::ReconciliationRequired
                },
                },
            },
            parse_error: None,
        },
        Err(error) => RewardDurableDispatchDecision {
            disposition: RewardDurableDispatch::ReconciliationRequired,
            parse_error: Some(error.to_string()),
        },
    }
}

async fn dispatch_reward_place_buy(
    state: &AppState,
    connector: &LivePolymarketConnector,
    payload: &polyedge_application::RewardDurableOrderActionPayload,
    trace_id: &str,
) -> Result<RewardCancelDispatchResult> {
    let mut cycle = state.reward_bot_service.current_live_cycle_state().await?;
    let Some(mut order) = cycle
        .open_orders
        .iter()
        .find(|candidate| candidate.id == payload.order.id)
        .cloned()
    else {
        return Ok(RewardCancelDispatchResult {
            status: RewardStrategyActionStatus::Skipped,
            reason_code: "executor_buy_local_order_missing",
            reason: "managed BUY no longer exists; a fresh tick must replan it",
            result_json: json!({"status": "skipped", "external_side_effect_executed": false}),
        });
    };
    if order.external_order_id.is_some() || order.status == ManagedRewardOrderStatus::Open {
        return Ok(RewardCancelDispatchResult {
            status: RewardStrategyActionStatus::Succeeded,
            reason_code: "executor_buy_local_order_already_bound",
            reason: "managed BUY is already bound to a venue order",
            result_json: json!({
                "status": "succeeded",
                "external_order_id": order.external_order_id,
                "external_side_effect_executed": false,
            }),
        });
    }
    if order != payload.order || order.side != RewardOrderSide::Buy
        || order.status != ManagedRewardOrderStatus::Planned
    {
        return Ok(RewardCancelDispatchResult {
            status: RewardStrategyActionStatus::Skipped,
            reason_code: "executor_buy_payload_stale",
            reason: "managed BUY changed after planning; a fresh tick must revalidate it",
            result_json: json!({"status": "skipped", "external_side_effect_executed": false}),
        });
    }

    let request = LivePolymarketTokenOrderRequest {
        client_order_id: order.id.clone(),
        connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
        token_id: order.token_id.clone(),
        side: reward_side_to_polymarket(order.side),
        limit_price: Probability::new(order.price)?,
        quantity: Quantity::new(order.size)?,
        post_only: true,
    };
    match connector.find_matching_open_token_order(&request).await {
        Ok(Some(acceptance)) => {
            order.external_order_id = Some(acceptance.order_id.clone());
            order.size = acceptance.submitted_quantity.value();
            order.status = ManagedRewardOrderStatus::Open;
            order.scoring = false;
            order.reason = "durable executor recovered matching live post-only rewards quote"
                .to_string();
            order.updated_at = acceptance.accepted_at;
            let event = reward_live_event(
                &order,
                "reward_live_order_submission_recovered",
                RewardRiskSeverity::Critical,
                order.reason.clone(),
                json!({"external_order_id": acceptance.order_id, "executor": true}),
            );
            persist_live_reward_updates(
                state,
                &mut cycle.account,
                Vec::new(),
                vec![order],
                Vec::new(),
                vec![event],
                &RewardBotRunReport::default(),
                trace_id,
            )
            .await?;
            return Ok(RewardCancelDispatchResult {
                status: RewardStrategyActionStatus::Succeeded,
                reason_code: "executor_buy_matching_order_recovered",
                reason: "matching venue BUY already exists and was bound idempotently",
                result_json: json!({"status": "succeeded", "external_order_id": acceptance.order_id, "submitted": false}),
            });
        }
        Ok(None) => {}
        Err(error) => {
            return Ok(RewardCancelDispatchResult {
                status: RewardStrategyActionStatus::Unknown,
                reason_code: "executor_buy_venue_match_unknown",
                reason: "venue matching query failed or was ambiguous; submission is forbidden",
                result_json: json!({"status": "unknown", "reconciliation_required": true, "code": error.code(), "submitted": false}),
            });
        }
    }

    // Venue matching above is read-only and remains available while risk state
    // is unavailable so an already-submitted order can still be adopted. A
    // confirmed venue miss, however, can lead to a new external BUY and must
    // therefore use the latest durable risk state rather than the stale
    // strategy-cycle snapshot.
    let latest_kill_switch = match state.risk_service.read_state().await {
        Ok(risk) => Some(risk.kill_switch),
        Err(error) => {
            warn!(
                error = %error,
                order_id = %order.id,
                "reward action executor could not read current risk state; BUY is blocked"
            );
            None
        }
    };
    let kill_switch = match authorize_executor_buy_risk(latest_kill_switch) {
        Ok(kill_switch) => kill_switch,
        Err(blocked) => return Ok(blocked),
    };

    let plan_index = reward_live_plan_index(&cycle.plans);
    let token_ids = live_buy_submission_last_look_token_ids(
        &order,
        reward_live_plan_for_order(&plan_index, &order),
    );
    let books = fetch_live_buy_submission_last_look_books(state, &token_ids).await?;
    let book_history = HashMap::new();
    let mut isolated_orders = vec![order.clone()];
    let risk_account = cycle.account.clone();
    let context = LiveBuySubmitRiskContext {
        config: &cycle.config,
        plans: &plan_index,
        book_history: &book_history,
        open_orders: &cycle.open_orders,
        positions: &cycle.positions,
        account: &risk_account,
        kill_switch,
    };
    submit_pending_live_reward_orders(
        connector,
        &mut isolated_orders,
        &books,
        Some(context),
        state,
        &mut cycle.account,
        &cycle.positions,
        &mut RewardBotRunReport::default(),
        trace_id,
        cycle.should_execute,
    )
    .await?;
    let updated = &isolated_orders[0];
    if let Some(external_order_id) = updated.external_order_id.as_deref() {
        Ok(RewardCancelDispatchResult {
            status: RewardStrategyActionStatus::Succeeded,
            reason_code: "executor_buy_submitted",
            reason: "fresh last-look and risk validation passed; venue BUY was submitted",
            result_json: json!({"status": "succeeded", "external_order_id": external_order_id, "submitted": true}),
        })
    } else if live_submission_result_is_unknown(updated) {
        Ok(RewardCancelDispatchResult {
            status: RewardStrategyActionStatus::Unknown,
            reason_code: "executor_buy_submit_unknown",
            reason: "BUY submission outcome is unknown; reconciliation is required",
            result_json: json!({"status": "unknown", "reconciliation_required": true, "submitted": true}),
        })
    } else {
        Ok(RewardCancelDispatchResult {
            status: RewardStrategyActionStatus::Skipped,
            reason_code: "executor_buy_last_look_blocked",
            reason: "fresh last-look or risk validation did not permit BUY submission",
            result_json: json!({"status": "skipped", "submitted": false, "managed_order_status": updated.status.as_str(), "reason": updated.reason}),
        })
    }
}

fn authorize_executor_buy_risk(
    latest_kill_switch: Option<bool>,
) -> std::result::Result<bool, RewardCancelDispatchResult> {
    match latest_kill_switch {
        Some(false) => Ok(false),
        Some(true) => Err(RewardCancelDispatchResult {
            status: RewardStrategyActionStatus::Skipped,
            reason_code: "executor_buy_kill_switch_active",
            reason: "current global kill switch blocks durable BUY submission",
            result_json: json!({
                "status": "skipped",
                "submitted": false,
                "external_side_effect_executed": false,
                "kill_switch": true,
            }),
        }),
        None => Err(RewardCancelDispatchResult {
            status: RewardStrategyActionStatus::Skipped,
            reason_code: "executor_buy_risk_state_unavailable",
            reason: "current risk state is unavailable; durable BUY submission failed closed",
            result_json: json!({
                "status": "skipped",
                "submitted": false,
                "external_side_effect_executed": false,
                "risk_state_available": false,
            }),
        }),
    }
}

async fn dispatch_reward_exit_sell(
    state: &AppState,
    connector: &LivePolymarketConnector,
    payload: &polyedge_application::RewardDurableOrderActionPayload,
    trace_id: &str,
) -> Result<RewardCancelDispatchResult> {
    let mut cycle = state.reward_bot_service.current_live_cycle_state().await?;
    let Some(current_order) = cycle
        .open_orders
        .iter()
        .find(|candidate| candidate.id == payload.order.id)
        .cloned()
    else {
        return Ok(RewardCancelDispatchResult {
            status: RewardStrategyActionStatus::Skipped,
            reason_code: "executor_exit_local_order_missing",
            reason: "managed exit no longer exists; no durable SELL was submitted",
            result_json: json!({
                "status": "skipped",
                "external_side_effect_executed": false,
            }),
        });
    };
    if current_order.external_order_id.is_some() {
        return Ok(RewardCancelDispatchResult {
            status: RewardStrategyActionStatus::Succeeded,
            reason_code: "executor_exit_local_order_already_bound",
            reason: "managed exit is already bound to a venue order",
            result_json: json!({
                "status": "succeeded",
                "external_order_id": current_order.external_order_id,
                "external_side_effect_executed": false,
            }),
        });
    }
    if current_order.side != RewardOrderSide::Sell
        || !matches!(
            current_order.status,
            ManagedRewardOrderStatus::Planned | ManagedRewardOrderStatus::ExitPending
        )
    {
        return Ok(RewardCancelDispatchResult {
            status: RewardStrategyActionStatus::Skipped,
            reason_code: "executor_exit_local_order_not_pending",
            reason: "managed exit is no longer pending submission",
            result_json: json!({
                "status": "skipped",
                "managed_order_status": current_order.status.as_str(),
                "external_side_effect_executed": false,
            }),
        });
    }

    // Matching always precedes a new post. Use the immutable durable request,
    // not mutable local retry markers, so an interrupted first attempt can be
    // adopted without creating a duplicate venue order.
    let recovery_post_only = deferred_live_exit_is_post_only(&payload.order);
    let recovery_request = LivePolymarketTokenOrderRequest {
        client_order_id: payload.order.id.clone(),
        connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
        token_id: payload.order.token_id.clone(),
        side: reward_side_to_polymarket(payload.order.side),
        limit_price: Probability::new(payload.order.price)?,
        quantity: Quantity::new(payload.order.size)?,
        post_only: recovery_post_only,
    };
    match connector
        .find_matching_open_token_order(&recovery_request)
        .await
    {
        Ok(Some(acceptance)) => {
            let mut recovered = current_order;
            recovered.external_order_id = Some(acceptance.order_id.clone());
            recovered.size = acceptance.submitted_quantity.value();
            recovered.status = ManagedRewardOrderStatus::ExitPending;
            recovered.scoring = false;
            recovered.reason = if recovery_post_only {
                "durable executor recovered matching live post-only rewards exit".to_string()
            } else {
                "durable executor recovered matching live non-post-only rewards exit".to_string()
            };
            recovered.updated_at = acceptance.accepted_at;
            let event = reward_live_event(
                &recovered,
                "reward_live_order_submission_recovered",
                RewardRiskSeverity::Critical,
                recovered.reason.clone(),
                json!({
                    "external_order_id": acceptance.order_id,
                    "post_only": recovery_post_only,
                    "executor": true,
                }),
            );
            persist_live_reward_updates(
                state,
                &mut cycle.account,
                Vec::new(),
                vec![recovered],
                Vec::new(),
                vec![event],
                &RewardBotRunReport::default(),
                trace_id,
            )
            .await?;
            return Ok(RewardCancelDispatchResult {
                status: RewardStrategyActionStatus::Succeeded,
                reason_code: "executor_exit_matching_order_recovered",
                reason: "matching venue SELL already exists and was bound idempotently",
                result_json: json!({
                    "status": "succeeded",
                    "external_order_id": acceptance.order_id,
                    "submitted": false,
                    "post_only": recovery_post_only,
                }),
            });
        }
        Ok(None) => {}
        Err(error) => {
            return Ok(RewardCancelDispatchResult {
                status: RewardStrategyActionStatus::Unknown,
                reason_code: "executor_exit_venue_match_unknown",
                reason: "venue matching query failed or was ambiguous; SELL submission is forbidden",
                result_json: json!({
                    "status": "unknown",
                    "reconciliation_required": true,
                    "code": error.code(),
                    "submitted": false,
                }),
            });
        }
    }

    // A confirmed venue miss permits a fresh submission attempt, but only
    // through the existing pending-order path. That path re-reads the current
    // position, clips size, enforces the minimum notional, and derives either
    // post-only maker or controlled-flatten price semantics from a fresh book.
    let books = fetch_live_buy_submission_last_look_books(
        state,
        std::slice::from_ref(&current_order.token_id),
    )
    .await?;
    let mut isolated_orders = vec![current_order];
    // This venue lookup already resolved the previous attempt. Reset only the
    // retry markers by restoring the durable planning reason; strategy fields
    // and the current persisted order identity remain unchanged.
    if live_submission_was_attempted(&isolated_orders[0])
        || live_submission_result_is_unknown(&isolated_orders[0])
    {
        isolated_orders[0].reason = payload.reason.clone();
    }
    let mut report = RewardBotRunReport::default();
    submit_pending_live_reward_orders(
        connector,
        &mut isolated_orders,
        &books,
        None,
        state,
        &mut cycle.account,
        &cycle.positions,
        &mut report,
        trace_id,
        false,
    )
    .await?;
    let updated = &isolated_orders[0];
    if let Some(external_order_id) = updated.external_order_id.as_deref() {
        Ok(RewardCancelDispatchResult {
            status: RewardStrategyActionStatus::Succeeded,
            reason_code: "executor_exit_submitted",
            reason: "current inventory and exit preflight passed; venue SELL was submitted",
            result_json: json!({
                "status": "succeeded",
                "external_order_id": external_order_id,
                "submitted": true,
                "post_only": deferred_live_exit_is_post_only(updated),
                "size": updated.size,
            }),
        })
    } else if live_submission_result_is_unknown(updated) {
        Ok(RewardCancelDispatchResult {
            status: RewardStrategyActionStatus::Unknown,
            reason_code: "executor_exit_submit_unknown",
            reason: "SELL submission outcome is unknown; reconciliation is required",
            result_json: json!({
                "status": "unknown",
                "reconciliation_required": true,
                "submitted": true,
            }),
        })
    } else {
        Ok(RewardCancelDispatchResult {
            status: RewardStrategyActionStatus::Skipped,
            reason_code: "executor_exit_preflight_deferred",
            reason: "current position, minimum notional, or exit-price preflight deferred SELL submission",
            result_json: json!({
                "status": "skipped",
                "submitted": false,
                "managed_order_status": updated.status.as_str(),
                "reason": updated.reason,
            }),
        })
    }
}

#[derive(Debug)]
struct RewardCancelDispatchResult {
    status: RewardStrategyActionStatus,
    reason_code: &'static str,
    reason: &'static str,
    result_json: Value,
}

fn reward_cancel_dispatch_result(
    update: &LiveRewardOrderUpdate,
    cancel_replace: bool,
) -> RewardCancelDispatchResult {
    let replacement_deferred = cancel_replace;
    match update {
        LiveRewardOrderUpdate::Changed(order, event) if live_cancel_result_is_unknown(order) => {
            RewardCancelDispatchResult {
                status: RewardStrategyActionStatus::Unknown,
                reason_code: "executor_cancel_outcome_unknown",
                reason: "cancel result is unknown; reconciliation is required and no replacement was created",
                result_json: json!({
                    "status": "unknown",
                    "external_order_id": order.external_order_id,
                    "event_type": event.event_type,
                    "reconciliation_required": true,
                    "replacement_created": false,
                }),
            }
        }
        LiveRewardOrderUpdate::Changed(order, event) => RewardCancelDispatchResult {
            status: RewardStrategyActionStatus::Succeeded,
            reason_code: if order.status == ManagedRewardOrderStatus::Cancelled {
                "executor_cancel_idempotently_closed"
            } else {
                "executor_cancel_accepted"
            },
            reason: if order.status == ManagedRewardOrderStatus::Cancelled {
                "venue reports the order is no longer open; cancel is idempotently complete"
            } else {
                "venue accepted the cancel; final order reconciliation remains pending"
            },
            result_json: json!({
                "status": "succeeded",
                "external_order_id": order.external_order_id,
                "event_type": event.event_type,
                "awaiting_reconciliation": order.status != ManagedRewardOrderStatus::Cancelled,
                "replacement_deferred": replacement_deferred,
                "replacement_created": false,
            }),
        },
        LiveRewardOrderUpdate::Unchanged(event) | LiveRewardOrderUpdate::Retryable(event) => {
            RewardCancelDispatchResult {
                status: if event.event_type == "reward_live_order_cancel_already_in_flight" {
                    RewardStrategyActionStatus::Skipped
                } else {
                    RewardStrategyActionStatus::Failed
                },
                reason_code: if event.event_type == "reward_live_order_cancel_already_in_flight" {
                    "executor_cancel_already_in_flight"
                } else {
                    "executor_cancel_rejected"
                },
                reason: if event.event_type == "reward_live_order_cancel_already_in_flight" {
                    "another cancel for the same external order is already in flight"
                } else {
                    "venue did not confirm cancellation; the managed order remains unchanged"
                },
                result_json: json!({
                    "status": if event.event_type == "reward_live_order_cancel_already_in_flight" { "skipped" } else { "failed" },
                    "event_type": event.event_type,
                    "replacement_created": false,
                }),
            }
        }
        LiveRewardOrderUpdate::CancelReplace(_) => unreachable!(
            "cancel_one_live_reward_order never returns a cancel-replace intent"
        ),
    }
}

async fn dispatch_reward_cancel(
    state: &AppState,
    connector: &LivePolymarketConnector,
    payload: &polyedge_application::RewardDurableOrderActionPayload,
    cancel_replace: bool,
    trace_id: &str,
) -> Result<RewardCancelDispatchResult> {
    let external_order_id = payload.order.external_order_id.as_deref().ok_or_else(|| {
        AppError::invalid_input(
            "REWARD_ACTION_EXECUTOR_CANCEL_ID_REQUIRED",
            "durable cancel action is missing external_order_id",
        )
    })?;
    let order = state
        .reward_bot_service
        .get_managed_order_by_external_order_id(external_order_id)
        .await?
        .unwrap_or_else(|| payload.order.clone());

    if matches!(
        order.status,
        ManagedRewardOrderStatus::Cancelled
            | ManagedRewardOrderStatus::Filled
            | ManagedRewardOrderStatus::Error
    ) {
        return Ok(RewardCancelDispatchResult {
            status: RewardStrategyActionStatus::Succeeded,
            reason_code: "executor_cancel_local_terminal",
            reason: "managed order is already terminal; cancel is idempotently complete",
            result_json: json!({
                "status": "succeeded",
                "external_order_id": external_order_id,
                "managed_order_status": order.status.as_str(),
                "external_side_effect_executed": false,
                "replacement_created": false,
            }),
        });
    }

    let update = cancel_one_live_reward_order(connector, order, &payload.reason, trace_id).await?;
    let terminal = reward_cancel_dispatch_result(&update, cancel_replace);
    let mut cycle = state.reward_bot_service.current_live_cycle_state().await?;
    let (orders, events) = match update {
        LiveRewardOrderUpdate::Changed(order, event) => (vec![order], vec![event]),
        LiveRewardOrderUpdate::Unchanged(event) | LiveRewardOrderUpdate::Retryable(event) => {
            (Vec::new(), vec![event])
        }
        LiveRewardOrderUpdate::CancelReplace(_) => unreachable!(
            "cancel_one_live_reward_order never returns a cancel-replace intent"
        ),
    };
    persist_live_reward_updates(
        state,
        &mut cycle.account,
        Vec::new(),
        orders,
        Vec::new(),
        events,
        &RewardBotRunReport::default(),
        trace_id,
    )
    .await?;
    Ok(terminal)
}

async fn dispatch_create_merge_intent(
    service: &polyedge_application::RewardBotService,
    intent: &RewardMergeIntent,
) -> Result<bool> {
    service.create_reward_merge_intent_if_absent(intent).await
}

async fn dispatch_reward_merge_receipt(
    state: &AppState,
    payload: &polyedge_application::RewardDurableMergeActionPayload,
    execution_attempts: i32,
) -> Result<RewardCancelDispatchResult> {
    let durable_intent = &payload.merge_intent;
    let stored_intent = state
        .reward_bot_service
        .get_reward_merge_intent(&durable_intent.id)
        .await?;
    if stored_intent.as_ref().is_some_and(|stored| {
        stored.account_id != durable_intent.account_id
            || stored.condition_id != durable_intent.condition_id
            || stored.yes_token_id != durable_intent.yes_token_id
            || stored.no_token_id != durable_intent.no_token_id
            || stored.merge_size != durable_intent.merge_size
    }) {
        return Ok(RewardCancelDispatchResult {
            status: RewardStrategyActionStatus::Unknown,
            reason_code: "executor_merge_intent_identity_mismatch",
            reason: "stored merge intent does not match the immutable durable request",
            result_json: json!({
                "status": "unknown",
                "merge_intent_id": durable_intent.id,
                "reconciliation_required": true,
                "automatic_replay": false,
            }),
        });
    }
    let intent = stored_intent.as_ref().unwrap_or(durable_intent);
    let Some(tx_hash) = intent.tx_hash.as_deref() else {
        return Ok(reward_merge_missing_hash_result(
            &intent.id,
            execution_attempts,
        ));
    };
    let chain = PolymarketChainConnector::new(&state.settings.polymarket.polygon_rpc_url)?;
    let receipt = match chain.fetch_transaction_receipt(tx_hash).await {
        Ok(receipt) => receipt,
        Err(error) => {
            return Ok(RewardCancelDispatchResult {
                status: RewardStrategyActionStatus::Unknown,
                reason_code: "executor_merge_receipt_unknown",
                reason: "Polygon receipt lookup failed; merge outcome requires reconciliation",
                result_json: json!({
                    "status": "unknown",
                    "tx_hash": tx_hash,
                    "reconciliation_required": true,
                    "automatic_replay": false,
                    "code": error.code(),
                }),
            });
        }
    };
    match receipt.status {
        PolymarketTransactionReceiptStatus::Pending => Ok(RewardCancelDispatchResult {
            status: RewardStrategyActionStatus::Unknown,
            reason_code: "executor_merge_receipt_pending",
            reason: "merge transaction is not yet mined; automatic replay is forbidden",
            result_json: json!({
                "status": "unknown",
                "tx_hash": receipt.tx_hash,
                "reconciliation_required": true,
                "automatic_replay": false,
                "receipt_status": "pending",
            }),
        }),
        PolymarketTransactionReceiptStatus::Succeeded
        | PolymarketTransactionReceiptStatus::Reverted => {
            let succeeded = receipt.status == PolymarketTransactionReceiptStatus::Succeeded;
            let reason = if succeeded {
                "Polygon receipt confirmed the balanced merge transaction"
            } else {
                "Polygon receipt confirmed the balanced merge transaction reverted"
            };
            let resolved = state
                .reward_bot_service
                .resolve_reward_merge_intent_transaction(
                    &intent.id,
                    tx_hash,
                    succeeded,
                    reason,
                    OffsetDateTime::now_utc(),
                )
                .await?;
            Ok(RewardCancelDispatchResult {
                status: if succeeded {
                    RewardStrategyActionStatus::Succeeded
                } else {
                    RewardStrategyActionStatus::Failed
                },
                reason_code: if succeeded {
                    "executor_merge_receipt_confirmed"
                } else {
                    "executor_merge_receipt_reverted"
                },
                reason,
                result_json: json!({
                    "status": if succeeded { "succeeded" } else { "failed" },
                    "tx_hash": receipt.tx_hash,
                    "receipt_status": if succeeded { "succeeded" } else { "reverted" },
                    "block_number": receipt.block_number,
                    "merge_intent_resolved": resolved,
                    "external_side_effect_executed": false,
                    "automatic_replay": false,
                }),
            })
        }
    }
}

fn reward_merge_missing_hash_result(
    intent_id: &str,
    execution_attempts: i32,
) -> RewardCancelDispatchResult {
    RewardCancelDispatchResult {
        status: if execution_attempts <= 1 {
            RewardStrategyActionStatus::Skipped
        } else {
            RewardStrategyActionStatus::Unknown
        },
        reason_code: if execution_attempts <= 1 {
            "executor_merge_fresh_tick_replan_required"
        } else {
            "executor_merge_broadcast_outcome_unknown"
        },
        reason: if execution_attempts <= 1 {
            "execute-merge has no persisted transaction hash; a fresh tick must revalidate and submit it"
        } else {
            "recovered execute-merge has no persisted transaction hash; automatic rebroadcast is forbidden"
        },
        result_json: json!({
            "status": if execution_attempts <= 1 { "skipped" } else { "unknown" },
            "merge_intent_id": intent_id,
            "reconciliation_required": execution_attempts > 1,
            "fresh_tick_replan_required": execution_attempts <= 1,
            "automatic_replay": false,
            "broadcast": false,
            "execution_attempts": execution_attempts,
        }),
    }
}

async fn poll_reward_action_executor(
    state: &AppState,
    max_cycles: Option<usize>,
) -> Result<RewardActionExecutorReport> {
    poll_reward_action_executor_loop(state, max_cycles, None).await
}

async fn poll_reward_action_executor_until_shutdown(
    state: &AppState,
    shutdown_rx: watch::Receiver<bool>,
) -> Result<RewardActionExecutorReport> {
    poll_reward_action_executor_loop(state, None, Some(shutdown_rx)).await
}

async fn poll_reward_action_executor_loop(
    state: &AppState,
    max_cycles: Option<usize>,
    mut shutdown_rx: Option<watch::Receiver<bool>>,
) -> Result<RewardActionExecutorReport> {
    let account_id = state.settings.polymarket.account_id.trim();
    if account_id.is_empty() {
        return Err(AppError::invalid_input(
            "REWARD_ACTION_EXECUTOR_ACCOUNT_REQUIRED",
            "Polymarket account_id is required for the reward durable action executor",
        ));
    }
    if state.dependencies.postgres.is_none() {
        return Err(AppError::dependency_unavailable(
            "REWARD_ACTION_EXECUTOR_POSTGRES_REQUIRED",
            "Postgres is required for durable reward action execution and fencing",
        ));
    }

    let lease_owner = format!("reward-action-executor:{}", Uuid::new_v4());
    let poll_interval_secs = state.settings.rewards.poll_interval_secs.max(1);
    let lease_secs = i64::try_from(poll_interval_secs.saturating_mul(2))
        .unwrap_or(i64::MAX)
        .max(REWARD_ACTION_EXECUTOR_MIN_LEASE_SECS);
    let lease_for = TimeDuration::seconds(lease_secs);
    let claim_limit = task_limit(state).unwrap_or(100).clamp(1, 100);
    let connector = build_live_polymarket_connector(state).await?;
    let mut cycles = 0usize;
    let mut total = RewardActionExecutorReport::default();

    loop {
        if shutdown_rx
            .as_ref()
            .is_some_and(|receiver| *receiver.borrow())
        {
            break;
        }
        if max_cycles.is_some_and(|limit| cycles >= limit) {
            break;
        }
        cycles += 1;

        let report = run_reward_action_executor_once(
            state,
            &connector,
            account_id,
            lease_owner.as_str(),
            lease_for,
            claim_limit,
        )
        .await?;
        total.claimed += report.claimed;
        total.finalized += report.finalized;
        total.reconciliation_required += report.reconciliation_required;
        total.replanned += report.replanned;
        total.lost_leases += report.lost_leases;

        if let Some(receiver) = shutdown_rx.as_mut() {
            if wait_for_worker_interval(receiver, poll_interval_secs).await {
                break;
            }
        } else {
            tokio::time::sleep(Duration::from_secs(poll_interval_secs)).await;
        }
    }

    Ok(total)
}

async fn run_reward_action_executor_once(
    state: &AppState,
    connector: &LivePolymarketConnector,
    account_id: &str,
    lease_owner: &str,
    lease_for: TimeDuration,
    claim_limit: u16,
) -> Result<RewardActionExecutorReport> {
    let Some(advisory_lease) = state
        .try_acquire_postgres_advisory_lease(REWARD_WORKER_ADVISORY_LOCK_KEY)
        .await?
    else {
        debug!("skipping reward action executor cycle because the live tick holds the account lease");
        return Ok(RewardActionExecutorReport::default());
    };

    let result = async {
        let actions = state
            .reward_bot_service
            .claim_strategy_actions(account_id, lease_owner, lease_for, claim_limit)
            .await?;
        let mut report = RewardActionExecutorReport {
            claimed: actions.len(),
            ..RewardActionExecutorReport::default()
        };

        for action in actions {
            let renewed = state
                .reward_bot_service
                .renew_strategy_action_lease(action.action_id, lease_owner, lease_for)
                .await?;
            if !renewed {
                report.lost_leases += 1;
                warn!(
                    action_id = action.action_id,
                    idempotency_key = %action.idempotency_key,
                    "reward action executor lost lease before dispatch"
                );
                continue;
            }

            let dispatch = classify_reward_durable_action(&action);
            let (status, reason_code, reason, result_json) = match dispatch.disposition {
                    RewardDurableDispatch::ReconciliationRequired => {
                        report.reconciliation_required += 1;
                        (
                            RewardStrategyActionStatus::Unknown,
                            "executor_reconciliation_required",
                            "expired executing lease has an unknown external outcome; automatic replay is forbidden",
                            json!({
                                "status": "unknown",
                                "classification": "reconciliation_required",
                                "automatic_replay": false,
                                "execution_attempts": action.execution_attempts,
                                "request_validation_error": dispatch.parse_error,
                            }),
                        )
                    }
                    RewardDurableDispatch::ReplanRequired => {
                        report.replanned += 1;
                        (
                            RewardStrategyActionStatus::Skipped,
                            "executor_fresh_tick_replan_required",
                            "live side effect remains inline; a fresh strategy tick must revalidate and replan it",
                            json!({
                                "status": "skipped",
                                "classification": "fresh_tick_replan_required",
                                "external_side_effect_executed": false,
                            }),
                        )
                    }
                    RewardDurableDispatch::CreateMergeIntent(intent) => {
                        let inserted = dispatch_create_merge_intent(
                            &state.reward_bot_service,
                            &intent,
                        )
                        .await?;
                        (
                            RewardStrategyActionStatus::Succeeded,
                            if inserted {
                                "executor_merge_intent_created"
                            } else {
                                "executor_merge_intent_already_exists"
                            },
                            if inserted {
                                "durable executor created the merge intent"
                            } else {
                                "merge intent already exists; durable create is idempotently complete"
                            },
                            json!({
                                "status": "succeeded",
                                "merge_intent_id": intent.id,
                                "created": inserted,
                                "idempotent_replay": !inserted,
                                "external_side_effect_executed": false,
                            }),
                        )
                    }
                    RewardDurableDispatch::ExecuteMerge(payload) => {
                        let terminal = dispatch_reward_merge_receipt(
                            state,
                            &payload,
                            action.execution_attempts,
                        )
                        .await?;
                        (
                            terminal.status,
                            terminal.reason_code,
                            terminal.reason,
                            terminal.result_json,
                        )
                    }
                    RewardDurableDispatch::CancelOrder(payload)
                    | RewardDurableDispatch::CancelReplaceExit(payload) => {
                        let cancel_replace = matches!(
                            action.action_type,
                            RewardStrategyActionType::CancelReplaceExit
                        );
                        let terminal = dispatch_reward_cancel(
                            state,
                            connector,
                            &payload,
                            cancel_replace,
                            &format!("reward-action-executor:{}", action.action_id),
                        )
                        .await?;
                        (
                            terminal.status,
                            terminal.reason_code,
                            terminal.reason,
                            terminal.result_json,
                        )
                    }
                    RewardDurableDispatch::SubmitExitSell(payload) => {
                        let terminal = dispatch_reward_exit_sell(
                            state,
                            connector,
                            &payload,
                            &format!("reward-action-executor:{}", action.action_id),
                        )
                        .await?;
                        (
                            terminal.status,
                            terminal.reason_code,
                            terminal.reason,
                            terminal.result_json,
                        )
                    }
                    RewardDurableDispatch::PlaceBuy(payload) => {
                        let terminal = dispatch_reward_place_buy(
                            state,
                            connector,
                            &payload,
                            &format!("reward-action-executor:{}", action.action_id),
                        )
                        .await?;
                        (
                            terminal.status,
                            terminal.reason_code,
                            terminal.reason,
                            terminal.result_json,
                        )
                    }
                };
            let now = OffsetDateTime::now_utc();
            let mut terminal = RewardActionPlanner::transition_action(
                &action,
                status,
                now,
                reason,
                result_json,
            );
            terminal.reason_code = reason_code.to_string();
            let finalized = state
                .reward_bot_service
                .finalize_strategy_action_lease(&terminal, lease_owner)
                .await?;
            if finalized {
                report.finalized += 1;
            } else {
                report.lost_leases += 1;
                warn!(
                    action_id = action.action_id,
                    idempotency_key = %action.idempotency_key,
                    "reward action executor terminal update was fenced by lease ownership"
                );
            }
        }
        Ok(report)
    }
    .await;

    finish_reward_worker_lease(advisory_lease, result).await
}

#[cfg(test)]
mod reward_action_executor_tests {
    use super::*;
    use polyedge_infrastructure::stores::InMemoryRewardBotStore;

    fn claimed_place_buy(execution_attempts: i32) -> RewardStrategyAction {
        let now = OffsetDateTime::UNIX_EPOCH;
        let order = ManagedRewardOrder {
            id: "order-1".to_string(),
            account_id: "acct".to_string(),
            condition_id: "condition".to_string(),
            token_id: "token".to_string(),
            outcome: "YES".to_string(),
            side: RewardOrderSide::Buy,
            price: Decimal::new(42, 2),
            size: Decimal::new(10, 0),
            strategy_bucket: RewardStrategyBucket::Standard,
            strategy_profile: RewardStrategyProfile::Standard,
            exit_strategy_source: RewardExitStrategySource::Configured,
            exit_strategy_selected: None,
            exit_floor_price: None,
            exit_reselect_count: 0,
            exit_last_reselected_at: None,
            external_order_id: None,
            status: ManagedRewardOrderStatus::Planned,
            scoring: true,
            reason: "test".to_string(),
            filled_size: Decimal::ZERO,
            reward_earned: Decimal::ZERO,
            last_scored_at: None,
            created_at: now,
            updated_at: now,
        };
        let mut action = RewardActionPlanner::plan_order_action(
            RewardActionPlannerContext {
                run_id: 1,
                trace_id: "trace",
                now,
            },
            RewardOrderActionProposal {
                order: &order,
                intent: RewardOrderActionIntent::PlaceBuy,
                reason: "test",
                metadata: json!({}),
            },
        );
        action.action_id = 1;
        action.status = RewardStrategyActionStatus::Executing;
        action.lease_owner = Some("owner".to_string());
        action.lease_expires_at = Some(now + TimeDuration::minutes(1));
        action.execution_attempts = execution_attempts;
        action
    }

    fn claimed_create_merge_intent(execution_attempts: i32) -> RewardStrategyAction {
        let now = OffsetDateTime::UNIX_EPOCH;
        let intent = RewardMergeIntent {
            id: "merge-1".to_string(),
            account_id: "acct".to_string(),
            condition_id: "condition".to_string(),
            yes_token_id: "yes-token".to_string(),
            no_token_id: "no-token".to_string(),
            merge_size: Decimal::new(10, 0),
            yes_position_size: Decimal::new(10, 0),
            no_position_size: Decimal::new(10, 0),
            yes_avg_price: Decimal::new(45, 2),
            no_avg_price: Decimal::new(47, 2),
            status: RewardMergeIntentStatus::Pending,
            reason: "paired inventory".to_string(),
            source_fill_id: "fill-1".to_string(),
            tx_hash: None,
            submitted_at: None,
            confirmed_at: None,
            failed_reason: None,
            retry_count: 0,
            trace_id: "trace".to_string(),
            created_at: now,
            updated_at: now,
        };
        let mut action = RewardActionPlanner::plan_merge_action(
            RewardActionPlannerContext {
                run_id: 1,
                trace_id: "trace",
                now,
            },
            RewardMergeActionProposal {
                intent: &intent,
                action_type: RewardStrategyActionType::CreateMergeIntent,
                reason: "paired inventory",
                metadata: json!({}),
                idempotency_suffix: "",
            },
        );
        action.action_id = 2;
        action.status = RewardStrategyActionStatus::Executing;
        action.lease_owner = Some("owner".to_string());
        action.lease_expires_at = Some(now + TimeDuration::minutes(1));
        action.execution_attempts = execution_attempts;
        action
    }

    fn claimed_execute_merge(
        execution_attempts: i32,
        tx_hash: Option<&str>,
    ) -> RewardStrategyAction {
        let mut intent = match claimed_create_merge_intent(1)
            .parse_durable_request()
            .unwrap()
            .envelope
            .request
        {
            RewardDurableActionRequest::CreateMergeIntent(payload) => payload.merge_intent,
            _ => unreachable!(),
        };
        intent.status = if tx_hash.is_some() {
            RewardMergeIntentStatus::Submitted
        } else {
            RewardMergeIntentStatus::Pending
        };
        intent.tx_hash = tx_hash.map(str::to_string);
        let now = OffsetDateTime::UNIX_EPOCH;
        let mut action = RewardActionPlanner::plan_merge_action(
            RewardActionPlannerContext {
                run_id: 1,
                trace_id: "trace",
                now,
            },
            RewardMergeActionProposal {
                intent: &intent,
                action_type: RewardStrategyActionType::ExecuteMerge,
                reason: "execute paired inventory",
                metadata: json!({}),
                idempotency_suffix: "execute",
            },
        );
        action.action_id = 5;
        action.status = RewardStrategyActionStatus::Executing;
        action.lease_owner = Some("owner".to_string());
        action.lease_expires_at = Some(now + TimeDuration::minutes(1));
        action.execution_attempts = execution_attempts;
        action
    }

    fn claimed_cancel_order(
        execution_attempts: i32,
        intent: RewardOrderActionIntent,
    ) -> RewardStrategyAction {
        let now = OffsetDateTime::UNIX_EPOCH;
        let mut order = match claimed_place_buy(1)
            .parse_durable_request()
            .unwrap()
            .envelope
            .request
        {
            RewardDurableActionRequest::PlaceBuy(payload) => payload.order,
            _ => unreachable!(),
        };
        order.side = if matches!(intent, RewardOrderActionIntent::CancelReplaceExit) {
            RewardOrderSide::Sell
        } else {
            RewardOrderSide::Buy
        };
        order.status = ManagedRewardOrderStatus::Open;
        order.external_order_id = Some("external-1".to_string());
        let mut action = RewardActionPlanner::plan_order_action(
            RewardActionPlannerContext {
                run_id: 1,
                trace_id: "trace",
                now,
            },
            RewardOrderActionProposal {
                order: &order,
                intent,
                reason: "test cancel",
                metadata: json!({}),
            },
        );
        action.action_id = 3;
        action.status = RewardStrategyActionStatus::Executing;
        action.lease_owner = Some("owner".to_string());
        action.lease_expires_at = Some(now + TimeDuration::minutes(1));
        action.execution_attempts = execution_attempts;
        action
    }

    fn claimed_exit_sell(execution_attempts: i32) -> RewardStrategyAction {
        let now = OffsetDateTime::UNIX_EPOCH;
        let mut order = match claimed_place_buy(1)
            .parse_durable_request()
            .unwrap()
            .envelope
            .request
        {
            RewardDurableActionRequest::PlaceBuy(payload) => payload.order,
            _ => unreachable!(),
        };
        order.side = RewardOrderSide::Sell;
        order.status = ManagedRewardOrderStatus::ExitPending;
        order.scoring = false;
        order.reason = "exit at markup; post_only=true".to_string();
        let mut action = RewardActionPlanner::plan_order_action(
            RewardActionPlannerContext {
                run_id: 1,
                trace_id: "trace",
                now,
            },
            RewardOrderActionProposal {
                order: &order,
                intent: RewardOrderActionIntent::SubmitExitSell,
                reason: order.reason.as_str(),
                metadata: json!({}),
            },
        );
        action.action_id = 4;
        action.status = RewardStrategyActionStatus::Executing;
        action.lease_owner = Some("owner".to_string());
        action.lease_expires_at = Some(now + TimeDuration::minutes(1));
        action.execution_attempts = execution_attempts;
        action
    }

    #[test]
    fn recovered_execution_is_never_replayed() {
        let action = claimed_place_buy(2);
        assert_eq!(
            classify_reward_durable_action(&action).disposition,
            RewardDurableDispatch::ReconciliationRequired
        );
    }

    #[test]
    fn first_valid_buy_is_dispatched_from_typed_payload() {
        let action = claimed_place_buy(1);
        assert!(matches!(
            classify_reward_durable_action(&action).disposition,
            RewardDurableDispatch::PlaceBuy(_)
        ));
    }

    #[test]
    fn durable_buy_execution_uses_fail_closed_current_risk_gate() {
        assert!(matches!(authorize_executor_buy_risk(Some(false)), Ok(false)));

        let kill_switch = authorize_executor_buy_risk(Some(true))
            .expect_err("active kill switch must block BUY");
        assert_eq!(
            kill_switch.reason_code,
            "executor_buy_kill_switch_active"
        );
        assert_eq!(kill_switch.result_json["submitted"], false);

        let unavailable = authorize_executor_buy_risk(None)
            .expect_err("unavailable risk state must fail closed");
        assert_eq!(
            unavailable.reason_code,
            "executor_buy_risk_state_unavailable"
        );
        assert_eq!(unavailable.result_json["external_side_effect_executed"], false);
    }

    #[test]
    fn exit_sell_is_dispatched_after_lease_recovery_for_match_first_reconciliation() {
        for execution_attempts in [1, 2] {
            let action = claimed_exit_sell(execution_attempts);
            assert!(matches!(
                classify_reward_durable_action(&action).disposition,
                RewardDurableDispatch::SubmitExitSell(ref payload)
                    if payload.order.side == RewardOrderSide::Sell
                        && payload.order.status == ManagedRewardOrderStatus::ExitPending
            ));
        }
    }

    #[test]
    fn invalid_or_legacy_payload_requires_reconciliation() {
        let mut action = claimed_place_buy(1);
        action.request_json = json!({ "order": "legacy-unversioned" });
        let decision = classify_reward_durable_action(&action);
        assert_eq!(
            decision.disposition,
            RewardDurableDispatch::ReconciliationRequired
        );
        assert!(decision.parse_error.is_some());
    }

    #[test]
    fn create_merge_intent_is_dispatched_even_after_lease_recovery() {
        let action = claimed_create_merge_intent(2);
        let decision = classify_reward_durable_action(&action);
        assert!(matches!(
            decision.disposition,
            RewardDurableDispatch::CreateMergeIntent(ref intent) if intent.id == "merge-1"
        ));
    }

    #[test]
    fn execute_merge_with_tx_hash_is_receipt_only_across_lease_recovery() {
        let tx_hash =
            "0x1111111111111111111111111111111111111111111111111111111111111111";
        for execution_attempts in [1, 2] {
            let action = claimed_execute_merge(execution_attempts, Some(tx_hash));
            assert!(matches!(
                classify_reward_durable_action(&action).disposition,
                RewardDurableDispatch::ExecuteMerge(ref payload)
                    if payload.merge_intent.tx_hash.as_deref() == Some(tx_hash)
            ));
        }
    }

    #[test]
    fn execute_merge_without_tx_hash_is_never_broadcast_by_executor() {
        for execution_attempts in [1, 2] {
            assert!(matches!(
                classify_reward_durable_action(&claimed_execute_merge(execution_attempts, None))
                    .disposition,
                RewardDurableDispatch::ExecuteMerge(ref payload)
                    if payload.merge_intent.tx_hash.is_none()
            ));
            let terminal = reward_merge_missing_hash_result("merge-1", execution_attempts);
            assert_eq!(terminal.result_json["broadcast"], false);
            assert_eq!(terminal.result_json["automatic_replay"], false);
            assert_eq!(
                terminal.status,
                if execution_attempts == 1 {
                    RewardStrategyActionStatus::Skipped
                } else {
                    RewardStrategyActionStatus::Unknown
                }
            );
        }
    }

    #[tokio::test]
    async fn submitted_merge_intent_resolution_is_hash_fenced() {
        let store = Arc::new(InMemoryRewardBotStore::new());
        let service = polyedge_application::RewardBotService::new(store);
        let tx_hash =
            "0x2222222222222222222222222222222222222222222222222222222222222222";
        let action = claimed_execute_merge(1, Some(tx_hash));
        let RewardDurableDispatch::ExecuteMerge(payload) =
            classify_reward_durable_action(&action).disposition
        else {
            panic!("expected merge receipt reconciliation");
        };
        assert!(service
            .create_reward_merge_intent_if_absent(&payload.merge_intent)
            .await
            .expect("create submitted intent"));

        assert!(!service
            .resolve_reward_merge_intent_transaction(
                &payload.merge_intent.id,
                "0x3333333333333333333333333333333333333333333333333333333333333333",
                true,
                "wrong hash",
                OffsetDateTime::UNIX_EPOCH,
            )
            .await
            .expect("wrong hash is fenced"));
        assert!(service
            .resolve_reward_merge_intent_transaction(
                &payload.merge_intent.id,
                tx_hash,
                true,
                "confirmed",
                OffsetDateTime::UNIX_EPOCH,
            )
            .await
            .expect("matching hash resolves"));
    }

    #[tokio::test]
    async fn create_merge_intent_dispatch_is_idempotent() {
        let store = Arc::new(InMemoryRewardBotStore::new());
        let service = polyedge_application::RewardBotService::new(store);
        let action = claimed_create_merge_intent(1);
        let RewardDurableDispatch::CreateMergeIntent(intent) =
            classify_reward_durable_action(&action).disposition
        else {
            panic!("expected create-merge dispatch");
        };

        assert!(dispatch_create_merge_intent(&service, &intent)
            .await
            .expect("first create"));
        assert!(!dispatch_create_merge_intent(&service, &intent)
            .await
            .expect("idempotent replay"));
        let stored = service
            .list_executable_reward_merge_intents("acct", 10)
            .await
            .expect("list merge intents");
        assert_eq!(stored, vec![intent]);
    }

    #[test]
    fn recoverable_cancel_actions_are_dispatched_from_typed_payload() {
        assert!(matches!(
            classify_reward_durable_action(&claimed_cancel_order(
                2,
                RewardOrderActionIntent::CancelOrder,
            ))
            .disposition,
            RewardDurableDispatch::CancelOrder(_)
        ));
        assert!(matches!(
            classify_reward_durable_action(&claimed_cancel_order(
                2,
                RewardOrderActionIntent::CancelReplaceExit,
            ))
            .disposition,
            RewardDurableDispatch::CancelReplaceExit(_)
        ));
    }

    #[test]
    fn cancel_unknown_is_terminal_unknown_and_never_creates_replacement() {
        let mut order = match claimed_cancel_order(1, RewardOrderActionIntent::CancelReplaceExit)
            .parse_durable_request()
            .unwrap()
            .envelope
            .request
        {
            RewardDurableActionRequest::CancelReplaceExit(payload) => payload.order,
            _ => unreachable!(),
        };
        order.reason = "cancel result unknown and awaiting final reconciliation".to_string();
        let event = reward_live_event(
            &order,
            "reward_live_order_cancel_unknown",
            RewardRiskSeverity::Critical,
            order.reason.clone(),
            json!({}),
        );
        let terminal = reward_cancel_dispatch_result(
            &LiveRewardOrderUpdate::Changed(order, event),
            true,
        );
        assert_eq!(terminal.status, RewardStrategyActionStatus::Unknown);
        assert_eq!(terminal.result_json["replacement_created"], false);
        assert_eq!(terminal.result_json["reconciliation_required"], true);
    }

    #[test]
    fn cancel_acceptance_and_remote_not_open_are_idempotent_successes() {
        for status in [
            ManagedRewardOrderStatus::Open,
            ManagedRewardOrderStatus::Cancelled,
        ] {
            let mut order = match claimed_cancel_order(1, RewardOrderActionIntent::CancelOrder)
                .parse_durable_request()
                .unwrap()
                .envelope
                .request
            {
                RewardDurableActionRequest::CancelOrder(payload) => payload.order,
                _ => unreachable!(),
            };
            order.status = status;
            let event = reward_live_event(
                &order,
                if status == ManagedRewardOrderStatus::Cancelled {
                    "reward_live_order_remote_not_open_closed"
                } else {
                    "reward_live_order_cancel_pending"
                },
                RewardRiskSeverity::Info,
                "cancel result",
                json!({}),
            );
            let terminal = reward_cancel_dispatch_result(
                &LiveRewardOrderUpdate::Changed(order, event),
                false,
            );
            assert_eq!(terminal.status, RewardStrategyActionStatus::Succeeded);
            assert_eq!(terminal.result_json["replacement_created"], false);
        }
    }
}
