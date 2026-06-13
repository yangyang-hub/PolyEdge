const LIVE_SUBMISSION_ATTEMPTED_MARKER: &str = "live submission attempted";
const LIVE_SUBMISSION_UNKNOWN_MARKER: &str =
    "live submission result unknown; manual reconciliation required";
const MAX_EXIT_REJECTION_COUNT: usize = 10;

fn is_transient_order_rejection(rejection: &PolymarketOrderRejection) -> bool {
    let code = rejection.code.to_lowercase();
    let message = rejection.message.to_lowercase();
    code.contains("425")
        || code.contains("429")
        || message.contains("425")
        || message.contains("429")
        || message.contains("order manager not ready")
        || message.contains("please retry")
}

fn live_submission_was_attempted(order: &ManagedRewardOrder) -> bool {
    order.reason.contains(LIVE_SUBMISSION_ATTEMPTED_MARKER)
}

fn live_submission_result_is_unknown(order: &ManagedRewardOrder) -> bool {
    order.reason.contains(LIVE_SUBMISSION_UNKNOWN_MARKER)
}

fn has_unresolved_live_reconciliation(orders: &[ManagedRewardOrder]) -> bool {
    orders.iter().any(|order| {
        order.status.is_open_like()
            && ((order.external_order_id.is_none()
                && (live_submission_was_attempted(order)
                    || live_submission_result_is_unknown(order)))
                || order.reason.contains("awaiting final reconciliation")
                || order.reason.contains(LIVE_EXTERNAL_ORDER_NOT_FOUND_MARKER))
    })
}

fn live_order_has_post_only_violation(order: &ManagedRewardOrder) -> bool {
    order.reason.starts_with("Polymarket returned ")
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
            order.size = acceptance.submitted_quantity.value();
            if acceptance.status != PolymarketAcceptedOrderStatus::Live {
                return handle_non_live_reward_order_acceptance(
                    connector,
                    order,
                    acceptance.status,
                )
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
        LivePolymarketExecutionOutcome::Rejected(rejection) => {
            if is_transient_order_rejection(&rejection) {
                Ok(LiveRewardOrderUpdate::Retryable(reward_live_event(
                    order,
                    "reward_live_order_rejected_transient",
                    RewardRiskSeverity::Warning,
                    format!(
                        "live rewards order rejected (will retry): {}",
                        rejection.message
                    ),
                    json!({ "code": rejection.code }),
                )))
            } else {
                Ok(LiveRewardOrderUpdate::Unchanged(reward_live_event(
                    order,
                    "reward_live_order_rejected",
                    RewardRiskSeverity::Warning,
                    format!("live rewards order rejected: {}", rejection.message),
                    json!({ "code": rejection.code }),
                )))
            }
        }
    }
}

async fn submit_one_live_exit_order(
    connector: &LivePolymarketConnector,
    order: &mut ManagedRewardOrder,
    post_only: bool,
) -> Result<LiveRewardOrderUpdate> {
    let request = LivePolymarketTokenOrderRequest {
        client_order_id: order.id.clone(),
        connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
        token_id: order.token_id.clone(),
        side: reward_side_to_polymarket(order.side),
        limit_price: Probability::new(order.price)?,
        quantity: Quantity::new(order.size)?,
        post_only,
    };
    match connector.submit_token_order(&request).await? {
        LivePolymarketExecutionOutcome::Accepted(acceptance) => {
            order.external_order_id = Some(acceptance.order_id.clone());
            order.size = acceptance.submitted_quantity.value();
            if post_only && acceptance.status != PolymarketAcceptedOrderStatus::Live {
                return handle_non_live_reward_order_acceptance(
                    connector,
                    order,
                    acceptance.status,
                )
                .await;
            }
            order.status = ManagedRewardOrderStatus::ExitPending;
            order.reason = if post_only {
                "live post-only rewards exit accepted".to_string()
            } else {
                format!(
                    "live rewards flatten order accepted with Polymarket status {}",
                    acceptance.status.as_str()
                )
            };
            order.updated_at = acceptance.accepted_at;
            Ok(LiveRewardOrderUpdate::Changed(
                order.clone(),
                reward_live_event(
                    order,
                    "reward_live_exit_order_placed",
                    RewardRiskSeverity::Info,
                    format!(
                        "{} live exit placed: {} @ {}",
                        order.outcome, order.size, order.price
                    ),
                    json!({
                        "token_id": order.token_id,
                        "side": order.side.as_str(),
                        "size": order.size,
                        "price": order.price,
                        "post_only": post_only,
                        "polymarket_status": acceptance.status.as_str(),
                    }),
                ),
            ))
        }
        LivePolymarketExecutionOutcome::Rejected(rejection) => {
            let current_rejections = parse_exit_rejection_count(&order.reason);
            let next_rejections = (current_rejections + 1).min(MAX_EXIT_REJECTION_COUNT);
            order.status = ManagedRewardOrderStatus::ExitPending;
            order.scoring = false;
            order.reason = format!(
                "retryable live exit rejected [{next_rejections}/{MAX_EXIT_REJECTION_COUNT}] (post_only={post_only}): {}",
                rejection.message
            );
            order.updated_at = OffsetDateTime::now_utc();
            Ok(LiveRewardOrderUpdate::Changed(
                order.clone(),
                reward_live_event(
                    order,
                    "reward_live_exit_order_rejected",
                    if next_rejections >= MAX_EXIT_REJECTION_COUNT {
                        RewardRiskSeverity::Critical
                    } else {
                        RewardRiskSeverity::Warning
                    },
                    order.reason.clone(),
                    json!({
                        "code": rejection.code,
                        "post_only": post_only,
                        "rejections": next_rejections,
                        "max_rejections": MAX_EXIT_REJECTION_COUNT,
                    }),
                ),
            ))
        }
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
    match connector.cancel_order(&cancel_request).await {
        Ok(LivePolymarketCancelOutcome::Accepted(acceptance)) => {
            order.status = ManagedRewardOrderStatus::Open;
            order.scoring = false;
            order.reason = format!(
                "Polymarket returned {} for a post-only rewards quote; cancel accepted; awaiting final reconciliation",
                accepted_status.as_str()
            );
            order.updated_at = acceptance.cancelled_at;
            Ok(LiveRewardOrderUpdate::Changed(
                order.clone(),
                reward_live_event(
                    order,
                    "reward_live_order_post_only_violation_cancel_pending",
                    RewardRiskSeverity::Critical,
                    order.reason.clone(),
                    json!({
                        "external_order_id": acceptance.external_order_id,
                        "polymarket_status": accepted_status.as_str(),
                    }),
                ),
            ))
        }
        Ok(LivePolymarketCancelOutcome::Rejected(rejection)) => {
            order.status = ManagedRewardOrderStatus::Open;
            order.scoring = false;
            order.reason = format!(
                "Polymarket returned {} for a post-only rewards quote and cancel was rejected; cancellation must be retried: {}",
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
        Err(error) => {
            order.status = ManagedRewardOrderStatus::Open;
            order.scoring = false;
            order.reason = format!(
                "Polymarket returned {} for a post-only rewards quote; cancel result unknown and awaiting final reconciliation: {error}",
                accepted_status.as_str()
            );
            order.updated_at = OffsetDateTime::now_utc();
            Ok(LiveRewardOrderUpdate::Changed(
                order.clone(),
                reward_live_event(
                    order,
                    "reward_live_order_post_only_violation_cancel_unknown",
                    RewardRiskSeverity::Critical,
                    order.reason.clone(),
                    json!({
                        "external_order_id": external_order_id,
                        "polymarket_status": accepted_status.as_str(),
                    }),
                ),
            ))
        }
    }
}

/// Parse the exit rejection counter from a reason string formatted as
/// `"retryable live exit rejected [N/M] ..."`. Returns 0 if the pattern
/// is not found (first rejection or non-rejection reason).
fn parse_exit_rejection_count(reason: &str) -> usize {
    let marker = "rejected [";
    let Some(marker_start) = reason.find(marker) else {
        return 0;
    };
    let count_start = marker_start + marker.len();
    let Some(slash_pos) = reason[count_start..].find('/') else {
        return 0;
    };
    reason[count_start..count_start + slash_pos]
        .parse()
        .unwrap_or(0)
}

fn live_exit_retry_due(order: &ManagedRewardOrder, now: OffsetDateTime) -> bool {
    let rejection_count = parse_exit_rejection_count(&order.reason);
    if rejection_count == 0 {
        return true;
    }
    let exponent = u32::try_from(rejection_count.saturating_sub(1).min(6)).unwrap_or(6);
    let delay_seconds = (5_i64 * 2_i64.pow(exponent)).min(300);
    now >= order.updated_at + TimeDuration::seconds(delay_seconds)
}

/// Returns true if the order is in a known stuck-reconciliation state.
/// These are orders that block new placements via `has_unresolved_live_reconciliation`
/// or are otherwise stuck awaiting external resolution that may never come.
fn is_stuck_reconciliation_order(order: &ManagedRewardOrder) -> bool {
    order.reason.contains(LIVE_EXTERNAL_ORDER_NOT_FOUND_MARKER)
        || live_submission_result_is_unknown(order)
        || order.reason.contains("awaiting final reconciliation")
        || order.reason.contains("cancellation must be retried")
        || order.reason.contains("cancel result unknown")
        || live_order_has_post_only_violation(order)
}
