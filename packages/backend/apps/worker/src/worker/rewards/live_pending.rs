async fn submit_pending_live_reward_orders(
    connector: &LivePolymarketConnector,
    open_orders: &mut [ManagedRewardOrder],
    books: &HashMap<String, RewardOrderBook>,
    state: &AppState,
    account: &mut RewardAccountState,
    _positions: &[RewardPosition],
    report: &mut RewardBotRunReport,
    trace_id: &str,
    allow_buy_submit: bool,
) -> Result<()> {
    let mut allow_buy_submit = allow_buy_submit;
    let now = OffsetDateTime::now_utc();
    open_orders.sort_by_key(|order| {
        if live_submission_was_attempted(order) {
            0
        } else if order.side == RewardOrderSide::Sell {
            1
        } else {
            2
        }
    });
    for order in open_orders.iter_mut().filter(|order| {
        order.external_order_id.is_none()
            && ((order.side == RewardOrderSide::Buy
                && order.status == ManagedRewardOrderStatus::Planned)
                || (order.side == RewardOrderSide::Sell
                    && matches!(
                        order.status,
                        ManagedRewardOrderStatus::Planned | ManagedRewardOrderStatus::ExitPending
                    )
                    && live_exit_retry_due(order, now)))
    }) {
        let post_only =
            order.side == RewardOrderSide::Buy || deferred_live_exit_is_post_only(order);
        if live_submission_was_attempted(order) {
            let request = LivePolymarketTokenOrderRequest {
                client_order_id: order.id.clone(),
                connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
                token_id: order.token_id.clone(),
                side: reward_side_to_polymarket(order.side),
                limit_price: Probability::new(order.price)?,
                quantity: Quantity::new(order.size)?,
                post_only,
            };
            match connector.find_matching_open_token_order(&request).await {
                Ok(Some(acceptance)) => {
                    order.external_order_id = Some(acceptance.order_id.clone());
                    order.size = acceptance.submitted_quantity.value();
                    order.status = if order.side == RewardOrderSide::Buy {
                        ManagedRewardOrderStatus::Open
                    } else {
                        ManagedRewardOrderStatus::ExitPending
                    };
                    order.scoring = false;
                    order.reason = match (order.side, post_only) {
                        (RewardOrderSide::Buy, _) => {
                            "recovered live post-only rewards quote after interrupted submission"
                                .to_string()
                        }
                        (RewardOrderSide::Sell, true) => {
                            "recovered live post-only rewards exit after interrupted submission"
                                .to_string()
                        }
                        (RewardOrderSide::Sell, false) => {
                            "recovered live rewards flatten after interrupted submission"
                                .to_string()
                        }
                    };
                    order.updated_at = acceptance.accepted_at;
                    report.placed_orders += 1;
                    let event = reward_live_event(
                        order,
                        "reward_live_order_submission_recovered",
                        RewardRiskSeverity::Critical,
                        order.reason.clone(),
                        json!({
                            "external_order_id": acceptance.order_id,
                            "post_only": post_only,
                        }),
                    );
                    persist_live_reward_updates(
                        state,
                        account,
                        Vec::new(), // positions unchanged during submission
                        vec![order.clone()],
                        Vec::new(),
                        vec![event],
                        report,
                        trace_id,
                    )
                    .await?;
                }
                Ok(None) => {
                    if !live_submission_result_is_unknown(order) {
                        order.scoring = false;
                        order.reason = format!(
                            "{}; {LIVE_SUBMISSION_UNKNOWN_MARKER}: no matching open order found",
                            order.reason
                        );
                        order.updated_at = OffsetDateTime::now_utc();
                        let event = reward_live_event(
                            order,
                            "reward_live_order_submission_recovery_unresolved",
                            RewardRiskSeverity::Critical,
                            order.reason.clone(),
                            json!({ "post_only": post_only }),
                        );
                        persist_live_reward_updates(
                            state,
                            account,
                            Vec::new(), // positions unchanged during submission
                            vec![order.clone()],
                            Vec::new(),
                            vec![event],
                            report,
                            trace_id,
                        )
                        .await?;
                    }
                }
                Err(error) => {
                    if !live_submission_result_is_unknown(order) {
                        order.scoring = false;
                        order.reason = format!(
                            "{}; {LIVE_SUBMISSION_UNKNOWN_MARKER}: {error}",
                            order.reason
                        );
                        order.updated_at = OffsetDateTime::now_utc();
                        let event = reward_live_event(
                            order,
                            "reward_live_order_submission_recovery_failed",
                            RewardRiskSeverity::Critical,
                            order.reason.clone(),
                            json!({ "post_only": post_only, "code": error.code() }),
                        );
                        persist_live_reward_updates(
                            state,
                            account,
                            Vec::new(), // positions unchanged during submission
                            vec![order.clone()],
                            Vec::new(),
                            vec![event],
                            report,
                            trace_id,
                        )
                        .await?;
                    }
                }
            }
            continue;
        }

        if order.side == RewardOrderSide::Buy && !allow_buy_submit {
            continue;
        }
        if order.side == RewardOrderSide::Sell && !post_only {
            let Some(best_bid) = books
                .get(&order.token_id)
                .and_then(|book| book.bids.first())
                .map(|level| level.price)
                .filter(|price| *price > Decimal::ZERO)
            else {
                continue;
            };
            order.price = floor_reward_price_to_tick(best_bid);
            order.reason = "post-fill flatten immediately".to_string();
        }

        let pre_submit_reason = order.reason.clone();
        order.reason = format!("{pre_submit_reason}; {LIVE_SUBMISSION_ATTEMPTED_MARKER}");
        order.updated_at = OffsetDateTime::now_utc();
        persist_live_reward_updates(
            state,
            account,
            Vec::new(), // positions unchanged during submission
            vec![order.clone()],
            Vec::new(),
            Vec::new(),
            report,
            trace_id,
        )
        .await?;

        let submission = if order.side == RewardOrderSide::Buy {
            submit_one_live_reward_order(connector, order).await
        } else {
            submit_one_live_exit_order(connector, order, post_only).await
        };
        match submission {
            Err(error) => {
                if error.code() == "POLYMARKET_ORDER_POST_FAILED" {
                    allow_buy_submit = false;
                }
                order.scoring = false;
                let exit_pre_submit_failure =
                    live_exit_pre_submit_failure(order, &error, post_only, &pre_submit_reason);
                order.reason = if let Some((reason, _)) = &exit_pre_submit_failure {
                    reason.clone()
                } else if error.code() == "POLYMARKET_ORDER_POST_FAILED" {
                    format!(
                        "{pre_submit_reason}; {LIVE_SUBMISSION_ATTEMPTED_MARKER}; {LIVE_SUBMISSION_UNKNOWN_MARKER}: {error}"
                    )
                } else {
                    format!(
                        "retryable live submission failed before post: {error}; {pre_submit_reason}"
                    )
                };
                order.updated_at = OffsetDateTime::now_utc();
                let event = reward_live_event(
                    order,
                    if error.code() == "POLYMARKET_ORDER_POST_FAILED" {
                        "reward_live_order_submission_unknown"
                    } else if exit_pre_submit_failure.is_some() {
                        "reward_live_exit_order_rejected"
                    } else {
                        "reward_live_order_submission_failed_before_post"
                    },
                    if let Some((_, severity)) = exit_pre_submit_failure {
                        severity
                    } else if error.code() == "POLYMARKET_ORDER_POST_FAILED" {
                        RewardRiskSeverity::Critical
                    } else {
                        RewardRiskSeverity::Warning
                    },
                    order.reason.clone(),
                    json!({ "post_only": post_only, "code": error.code() }),
                );
                persist_live_reward_updates(
                    state,
                    account,
                    Vec::new(), // positions unchanged during submission
                    vec![order.clone()],
                    Vec::new(),
                    vec![event],
                    report,
                    trace_id,
                )
                .await?;
            }
            Ok(LiveRewardOrderUpdate::Changed(updated, event)) => {
                let stop_placements = order.side == RewardOrderSide::Buy
                    && live_order_has_post_only_violation(&updated);
                *order = updated.clone();
                if updated.external_order_id.is_some() {
                    report.placed_orders += 1;
                }
                persist_live_reward_updates(
                    state,
                    account,
                    Vec::new(), // positions unchanged during submission
                    vec![updated],
                    Vec::new(),
                    vec![event],
                    report,
                    trace_id,
                )
                .await?;
                if stop_placements {
                    break;
                }
            }
            Ok(LiveRewardOrderUpdate::Unchanged(event)) => {
                if order.side == RewardOrderSide::Buy {
                    order.status = ManagedRewardOrderStatus::Error;
                    order.scoring = false;
                    order.reason = event.message.clone();
                    order.updated_at = OffsetDateTime::now_utc();
                }
                persist_live_reward_updates(
                    state,
                    account,
                    Vec::new(), // positions unchanged during submission
                    (order.side == RewardOrderSide::Buy)
                        .then(|| order.clone())
                        .into_iter()
                        .collect(),
                    Vec::new(),
                    vec![event],
                    report,
                    trace_id,
                )
                .await?;
            }
            Ok(LiveRewardOrderUpdate::Retryable(event)) => {
                // Transient rejection (e.g. HTTP 425 "order manager not ready").
                // Keep the order as Planned so it is retried on the next cycle.
                if order.side == RewardOrderSide::Buy {
                    allow_buy_submit = false;
                }
                order.reason = format!(
                    "{}; transient rejection, will retry: {}",
                    pre_submit_reason, event.message
                );
                order.updated_at = OffsetDateTime::now_utc();
                persist_live_reward_updates(
                    state,
                    account,
                    Vec::new(), // positions unchanged during submission
                    vec![order.clone()],
                    Vec::new(),
                    vec![event],
                    report,
                    trace_id,
                )
                .await?;
            }
        }
    }
    Ok(())
}
