#[derive(Clone, Copy)]
struct LiveBuySubmitRiskContext<'a> {
    config: &'a RewardBotConfig,
    plans: &'a HashMap<&'a str, &'a RewardQuotePlan>,
    book_history: &'a HashMap<String, VecDeque<BookSnapshot>>,
    open_orders: &'a [ManagedRewardOrder],
    account: &'a RewardAccountState,
    kill_switch: bool,
}

const LIVE_BUY_SUBMISSION_LAST_LOOK_MAX_AGE_MS: i64 = 1_000;

async fn fetch_live_buy_submission_last_look_book(
    state: &AppState,
    token_id: &str,
) -> Result<Option<RewardOrderBook>> {
    let token_ids = vec![token_id.to_string()];
    let books = state
        .orderbook_cache
        .get_books_with_max_age(&token_ids, LIVE_BUY_SUBMISSION_LAST_LOOK_MAX_AGE_MS)
        .await?;
    Ok(books
        .into_iter()
        .find(|book| book.token_id == token_id)
        .map(|book| cached_order_book_to_reward(&book)))
}

fn live_submission_unknown_has_possible_position_fill(
    order: &ManagedRewardOrder,
    positions: &[RewardPosition],
) -> bool {
    if order.side != RewardOrderSide::Buy {
        return false;
    }
    positions.iter().any(|position| {
        position.token_id == order.token_id
            && position.size > Decimal::ZERO
            && position.updated_at >= order.created_at
    })
}

#[allow(clippy::too_many_arguments)]
async fn submit_pending_live_reward_orders(
    connector: &LivePolymarketConnector,
    open_orders: &mut [ManagedRewardOrder],
    books: &HashMap<String, RewardOrderBook>,
    risk_context: Option<LiveBuySubmitRiskContext<'_>>,
    state: &AppState,
    account: &mut RewardAccountState,
    positions: &[RewardPosition],
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
        let mut post_only =
            order.side == RewardOrderSide::Buy || deferred_live_exit_is_post_only(order);
        let mut submission_price = order.price;
        let mut pre_submit_events = Vec::new();
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
                    order.reason = match order.side {
                        RewardOrderSide::Buy => {
                            "recovered live post-only rewards quote after interrupted submission"
                                .to_string()
                        }
                        RewardOrderSide::Sell if post_only => {
                            "recovered live post-only rewards exit after interrupted submission"
                                .to_string()
                        }
                        RewardOrderSide::Sell => {
                            "recovered live non-post-only rewards exit after interrupted submission"
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
                    } else if let Some((updated, event)) =
                        close_stale_submission_unknown_order(order.clone(), now, positions)
                    {
                        // Already unknown and recovery confirmed no live Polymarket order:
                        // after the grace period, close locally so the global reconciliation
                        // lock self-clears instead of blocking new buys indefinitely.
                        persist_live_reward_updates(
                            state,
                            account,
                            Vec::new(), // positions unchanged during submission
                            vec![updated.clone()],
                            Vec::new(),
                            vec![event],
                            report,
                            trace_id,
                        )
                        .await?;
                        *order = updated;
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
        if order.side == RewardOrderSide::Buy
            && let Some(context) = risk_context
            && let Some(reason) = live_cancel_reason(
                context.config,
                context.plans,
                books,
                context.book_history,
                context.open_orders,
                context.account,
                order,
                OffsetDateTime::now_utc(),
                context.kill_switch,
            )
        {
            order.status = ManagedRewardOrderStatus::Cancelled;
            order.scoring = false;
            order.reason = format!("local-only order cancelled before live submission: {reason}");
            order.updated_at = OffsetDateTime::now_utc();
            let event = reward_live_event(
                order,
                "reward_live_order_pre_submit_cancelled",
                RewardRiskSeverity::Info,
                order.reason.clone(),
                json!({ "reason": reason }),
            );
            persist_live_reward_updates(
                state,
                account,
                Vec::new(),
                vec![order.clone()],
                Vec::new(),
                vec![event],
                report,
                trace_id,
            )
            .await?;
            continue;
        }
        if order.side == RewardOrderSide::Buy
            && let Some(context) = risk_context
        {
            let last_look_book =
                fetch_live_buy_submission_last_look_book(state, &order.token_id).await;
            let mut last_look_books = books.clone();
            match last_look_book {
                Ok(Some(book)) => {
                    report.books_fetched += 1;
                    last_look_books.insert(order.token_id.clone(), book);
                    if let Some(reason) = live_cancel_reason(
                        context.config,
                        context.plans,
                        &last_look_books,
                        context.book_history,
                        context.open_orders,
                        context.account,
                        order,
                        OffsetDateTime::now_utc(),
                        context.kill_switch,
                    ) {
                        order.status = ManagedRewardOrderStatus::Cancelled;
                        order.scoring = false;
                        order.reason = format!(
                            "local-only order cancelled by live submission last-look: {reason}"
                        );
                        order.updated_at = OffsetDateTime::now_utc();
                        let event = reward_live_event(
                            order,
                            "reward_live_order_pre_submit_last_look_cancelled",
                            RewardRiskSeverity::Warning,
                            order.reason.clone(),
                            json!({ "reason": reason }),
                        );
                        report.cancelled_orders += 1;
                        report.risk_cancelled_orders += 1;
                        persist_live_reward_updates(
                            state,
                            account,
                            Vec::new(),
                            vec![order.clone()],
                            Vec::new(),
                            vec![event],
                            report,
                            trace_id,
                        )
                        .await?;
                        continue;
                    }
                }
                Ok(None) => {
                    allow_buy_submit = false;
                    order.scoring = false;
                    order.reason = "live buy submission deferred by last-look: orderbook unavailable".to_string();
                    order.updated_at = OffsetDateTime::now_utc();
                    let event = reward_live_event(
                        order,
                        "reward_live_order_pre_submit_last_look_deferred",
                        RewardRiskSeverity::Warning,
                        order.reason.clone(),
                        json!({ "token_id": order.token_id }),
                    );
                    persist_live_reward_updates(
                        state,
                        account,
                        Vec::new(),
                        vec![order.clone()],
                        Vec::new(),
                        vec![event],
                        report,
                        trace_id,
                    )
                    .await?;
                    continue;
                }
                Err(error) => {
                    allow_buy_submit = false;
                    order.scoring = false;
                    order.reason = format!(
                        "live buy submission deferred by last-look orderbook refresh failure: {error}"
                    );
                    order.updated_at = OffsetDateTime::now_utc();
                    let event = reward_live_event(
                        order,
                        "reward_live_order_pre_submit_last_look_failed",
                        RewardRiskSeverity::Warning,
                        order.reason.clone(),
                        json!({
                            "token_id": order.token_id,
                            "code": error.code(),
                        }),
                    );
                    persist_live_reward_updates(
                        state,
                        account,
                        Vec::new(),
                        vec![order.clone()],
                        Vec::new(),
                        vec![event],
                        report,
                        trace_id,
                    )
                    .await?;
                    continue;
                }
            }
        }
        if order.side == RewardOrderSide::Sell {
            post_only = deferred_live_exit_is_post_only(order);
            let post_only_marker = if post_only {
                "post_only=true"
            } else {
                "post_only=false"
            };
            let exit_floor = reward_sell_exit_floor(order, positions);
            if order.price != exit_floor {
                order.price = exit_floor;
                order.reason = if post_only {
                    format!(
                        "post-only sell exit floor raised to non-loss price {exit_floor}; {post_only_marker}"
                    )
                } else {
                    format!(
                        "flatten sell exit floor raised to non-loss price {exit_floor}; {post_only_marker}"
                    )
                };
                order.updated_at = OffsetDateTime::now_utc();
            }

            let position_size = reward_sell_position_size(order, positions)
                .round_dp_with_strategy(2, RoundingStrategy::ToZero);
            let target_size = (order.size - order.filled_size)
                .max(Decimal::ZERO)
                .min(position_size)
                .round_dp_with_strategy(2, RoundingStrategy::ToZero);
            if target_size <= Decimal::ZERO {
                order.status = ManagedRewardOrderStatus::Cancelled;
                order.scoring = false;
                order.reason = if position_size <= Decimal::ZERO {
                    "sell exit closed because no matching token position remains".to_string()
                } else {
                    "sell exit closed because no remaining size is available".to_string()
                };
                order.updated_at = OffsetDateTime::now_utc();
                report.cancelled_orders += 1;
                report.risk_cancelled_orders += 1;
                let event = reward_live_event(
                    order,
                    "reward_live_exit_no_position_closed",
                    RewardRiskSeverity::Warning,
                    order.reason.clone(),
                    json!({
                        "token_id": order.token_id,
                        "position_size": position_size,
                        "order_size": order.size,
                        "filled_size": order.filled_size,
                    }),
                );
                persist_live_reward_updates(
                    state,
                    account,
                    Vec::new(),
                    vec![order.clone()],
                    Vec::new(),
                    vec![event],
                    report,
                    trace_id,
                )
                .await?;
                continue;
            }

            if order.size != target_size || order.filled_size != Decimal::ZERO {
                let previous_size = order.size;
                let previous_filled_size = order.filled_size;
                order.size = target_size;
                order.filled_size = Decimal::ZERO;
                order.reason = format!(
                    "sell exit size adjusted to current token position {target_size}; {post_only_marker}"
                );
                order.updated_at = OffsetDateTime::now_utc();
                pre_submit_events.push(reward_live_event(
                    order,
                    "reward_live_exit_size_adjusted_to_position",
                    RewardRiskSeverity::Info,
                    order.reason.clone(),
                    json!({
                        "token_id": order.token_id,
                        "previous_size": previous_size,
                        "previous_filled_size": previous_filled_size,
                        "position_size": position_size,
                        "target_size": target_size,
                    }),
                ));
            }

            submission_price = order.price;
            if post_only {
                if let Some(best_bid) = reward_post_only_exit_crossing_bid(order, books) {
                    order.reason = format!(
                        "{LIVE_EXIT_POST_ONLY_CROSSING_DEFERRED_MARKER}: best bid {best_bid} >= maker price {}; waiting for original-price sell to rest as maker",
                        order.price
                    );
                    order.updated_at = OffsetDateTime::now_utc();
                    pre_submit_events.push(reward_live_event(
                        order,
                        "reward_live_exit_post_only_crossing_deferred",
                        RewardRiskSeverity::Info,
                        order.reason.clone(),
                        json!({
                            "token_id": order.token_id,
                            "price": order.price,
                            "best_bid": best_bid,
                            "post_only": true,
                        }),
                    ));
                    persist_live_reward_updates(
                        state,
                        account,
                        Vec::new(),
                        vec![order.clone()],
                        Vec::new(),
                        pre_submit_events,
                        report,
                        trace_id,
                    )
                    .await?;
                    continue;
                }
            } else {
                match reward_flatten_submission_price(order, books) {
                    Ok(price) => {
                        submission_price = price;
                    }
                    Err(reason) => {
                        order.reason = reason;
                        order.updated_at = OffsetDateTime::now_utc();
                        pre_submit_events.push(reward_live_event(
                            order,
                            "reward_live_flatten_deferred",
                            RewardRiskSeverity::Info,
                            order.reason.clone(),
                            json!({
                                "token_id": order.token_id,
                                "floor_price": order.price,
                                "post_only": false,
                            }),
                        ));
                        persist_live_reward_updates(
                            state,
                            account,
                            Vec::new(),
                            vec![order.clone()],
                            Vec::new(),
                            pre_submit_events,
                            report,
                            trace_id,
                        )
                        .await?;
                        continue;
                    }
                }
            }

            if let Some((reason, event)) =
                live_exit_dust_deferred_at_price(order, submission_price)
            {
                order.reason = reason;
                order.updated_at = OffsetDateTime::now_utc();
                pre_submit_events.push(event);
                persist_live_reward_updates(
                    state,
                    account,
                    Vec::new(),
                    vec![order.clone()],
                    Vec::new(),
                    pre_submit_events,
                    report,
                    trace_id,
                )
                .await?;
                continue;
            }
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
            pre_submit_events,
            report,
            trace_id,
        )
        .await?;

        let submission = if order.side == RewardOrderSide::Buy {
            submit_one_live_reward_order(connector, order).await
        } else {
            submit_one_live_exit_order(connector, order, post_only, submission_price).await
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
                        "retryable live submission failed before post (post_only={post_only}): {error}; {pre_submit_reason}"
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
                    json!({
                        "post_only": post_only,
                        "submission_price": submission_price,
                        "code": error.code(),
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
