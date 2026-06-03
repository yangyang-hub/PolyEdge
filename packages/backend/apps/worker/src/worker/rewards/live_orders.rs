enum LiveRewardOrderUpdate {
    Changed(ManagedRewardOrder, RewardRiskEvent),
    Unchanged(RewardRiskEvent),
}

struct LiveRewardFillUpdate {
    order: ManagedRewardOrder,
    fill: RewardFill,
    event: RewardRiskEvent,
    fill_size: Decimal,
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
            Ok(LiveRewardOrderUpdate::Unchanged(reward_live_event(
                order,
                "reward_live_exit_order_rejected",
                RewardRiskSeverity::Warning,
                format!("live rewards exit order rejected: {}", rejection.message),
                json!({ "code": rejection.code, "post_only": post_only }),
            )))
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
        LivePolymarketCancelOutcome::Rejected(rejection) => {
            Ok(LiveRewardOrderUpdate::Unchanged(reward_live_event(
                &order,
                "reward_live_order_cancel_rejected",
                RewardRiskSeverity::Warning,
                format!("live rewards cancel rejected: {}", rejection.message),
                json!({ "code": rejection.code, "external_order_id": external_order_id }),
            )))
        }
    }
}

fn apply_live_reward_fill_update(
    mut order: ManagedRewardOrder,
    account: &mut polyedge_application::RewardAccountState,
    positions: &mut HashMap<String, RewardPosition>,
    update: &ConnectorTradeFillUpdate,
    fill_id: &str,
    trace_id: &str,
) -> Option<LiveRewardFillUpdate> {
    let remaining = (order.size - order.filled_size).max(Decimal::ZERO);
    let fill_size = Decimal::min(update.filled_quantity.value(), remaining)
        .round_dp_with_strategy(2, RoundingStrategy::ToZero);
    if fill_size <= Decimal::ZERO {
        return None;
    }

    let price = update.fill_price.value();
    let fee = update.fee.value();
    let notional = (price * fill_size).round_dp(4);
    let now = OffsetDateTime::now_utc();
    let mut realized_pnl = Decimal::ZERO;
    let fill_role = reward_fill_role_for_live_order(&order);

    match order.side {
        RewardOrderSide::Buy => {
            account.available_usd = (account.available_usd - notional - fee).max(Decimal::ZERO);
            let position =
                positions
                    .entry(order.token_id.clone())
                    .or_insert_with(|| RewardPosition {
                        account_id: order.account_id.clone(),
                        condition_id: order.condition_id.clone(),
                        token_id: order.token_id.clone(),
                        outcome: order.outcome.clone(),
                        size: Decimal::ZERO,
                        avg_price: Decimal::ZERO,
                        realized_pnl: Decimal::ZERO,
                        updated_at: now,
                    });
            let next_size = position.size + fill_size;
            if next_size > Decimal::ZERO {
                position.avg_price =
                    ((position.size * position.avg_price) + (fill_size * price)) / next_size;
            }
            position.size = next_size;
            position.updated_at = now;
        }
        RewardOrderSide::Sell => {
            let position =
                positions
                    .entry(order.token_id.clone())
                    .or_insert_with(|| RewardPosition {
                        account_id: order.account_id.clone(),
                        condition_id: order.condition_id.clone(),
                        token_id: order.token_id.clone(),
                        outcome: order.outcome.clone(),
                        size: Decimal::ZERO,
                        avg_price: order.price,
                        realized_pnl: Decimal::ZERO,
                        updated_at: now,
                    });
            let avg_price = if position.avg_price > Decimal::ZERO {
                position.avg_price
            } else {
                order.price
            };
            realized_pnl = ((price - avg_price) * fill_size - fee).round_dp(4);
            account.available_usd += (notional - fee).max(Decimal::ZERO);
            account.realized_pnl += realized_pnl;
            position.size = (position.size - fill_size).max(Decimal::ZERO);
            position.realized_pnl += realized_pnl;
            position.updated_at = now;
        }
    }
    account.fees_paid += fee;

    order.filled_size = (order.filled_size + fill_size).min(order.size);
    order.updated_at = now;
    if order.filled_size >= order.size {
        order.status = ManagedRewardOrderStatus::Filled;
        order.scoring = false;
        order.reason = "live rewards order fully filled on Polymarket".to_string();
    } else {
        order.reason = "live rewards order partially filled on Polymarket".to_string();
    }

    let fill = RewardFill {
        id: fill_id.to_string(),
        order_id: order.id.clone(),
        account_id: order.account_id.clone(),
        condition_id: order.condition_id.clone(),
        token_id: order.token_id.clone(),
        outcome: order.outcome.clone(),
        side: order.side,
        price,
        size: fill_size,
        notional_usd: notional,
        role: fill_role,
        realized_pnl,
        reason: "live Polymarket trade reconciled".to_string(),
        trace_id: trace_id.to_string(),
        created_at: now,
    };
    let event = reward_live_event(
        &order,
        "reward_live_order_filled",
        RewardRiskSeverity::Info,
        format!(
            "{} live {} fill reconciled: {} @ {}",
            order.outcome,
            order.side.as_str(),
            fill_size,
            price
        ),
        json!({
            "fill_id": fill.id,
            "external_trade_id": update.external_trade_id,
            "external_order_id": update.external_order_id,
            "fill_size": fill_size,
            "price": price,
            "fee": fee,
            "realized_pnl": realized_pnl,
        }),
    );

    Some(LiveRewardFillUpdate {
        order,
        fill,
        event,
        fill_size,
    })
}

fn reward_fill_role_for_live_order(order: &ManagedRewardOrder) -> RewardFillRole {
    if order.side == RewardOrderSide::Sell && order.reason.contains("flatten") {
        RewardFillRole::Taker
    } else {
        RewardFillRole::Maker
    }
}

fn apply_live_reward_status_update_to_order(
    mut order: ManagedRewardOrder,
    update: ConnectorOrderStatusUpdate,
    trace_id: &str,
) -> Option<(ManagedRewardOrder, RewardRiskEvent)> {
    if order.external_order_id.as_deref() != Some(update.external_order_id.as_str())
        || !order.status.is_open_like()
    {
        return None;
    }

    match update.status {
        OrderStatus::Canceled => {
            order.status = ManagedRewardOrderStatus::Cancelled;
            order.scoring = false;
            order.reason = "live rewards order was cancelled on Polymarket".to_string();
            order.updated_at = OffsetDateTime::now_utc();
            let event = reward_live_event(
                &order,
                "reward_live_order_status_cancelled",
                RewardRiskSeverity::Info,
                "live rewards order cancellation observed on Polymarket",
                json!({
                    "event_id": update.event_id,
                    "trace_id": trace_id,
                    "external_order_id": update.external_order_id,
                }),
            );
            Some((order, event))
        }
        OrderStatus::Open | OrderStatus::Submitted | OrderStatus::PartiallyFilled => None,
        _ => None,
    }
}

async fn submit_live_post_fill_orders(
    connector: &LivePolymarketConnector,
    config: &RewardBotConfig,
    entry: &ManagedRewardOrder,
    fill_size: Decimal,
    positions: &HashMap<String, RewardPosition>,
    books: &HashMap<String, RewardOrderBook>,
    trace_id: &str,
) -> Result<Vec<LiveRewardOrderUpdate>> {
    if fill_size <= Decimal::ZERO {
        return Ok(Vec::new());
    }

    match config.post_fill_strategy {
        PostFillStrategy::HoldAndRequote => Ok(Vec::new()),
        PostFillStrategy::ExitAtMarkup => {
            let avg_price = positions
                .get(&entry.token_id)
                .map(|position| position.avg_price)
                .filter(|price| *price > Decimal::ZERO)
                .unwrap_or(entry.price);
            let exit_price = floor_reward_price_to_tick(Decimal::min(
                Decimal::from_parts(99, 0, 0, false, 2),
                avg_price + config.exit_markup_cents / Decimal::from(100_u64),
            ));
            let mut exit = live_exit_order(
                entry,
                fill_size,
                exit_price,
                "post-fill exit at markup",
                trace_id,
            );
            Ok(vec![
                submit_one_live_exit_order(connector, &mut exit, true).await?,
            ])
        }
        PostFillStrategy::FlattenImmediately => {
            let Some(best_bid) = books
                .get(&entry.token_id)
                .and_then(|book| book.bids.first())
                .map(|level| level.price)
                .filter(|price| *price > Decimal::ZERO)
            else {
                let avg_price = positions
                    .get(&entry.token_id)
                    .map(|position| position.avg_price)
                    .filter(|price| *price > Decimal::ZERO)
                    .unwrap_or(entry.price);
                let mut exit = live_exit_order(
                    entry,
                    fill_size,
                    floor_reward_price_to_tick(avg_price),
                    "flatten deferred until bid liquidity is observed",
                    trace_id,
                );
                exit.status = ManagedRewardOrderStatus::ExitPending;
                let event = reward_live_event(
                    &exit,
                    "reward_live_flatten_deferred",
                    RewardRiskSeverity::Warning,
                    "deferred rewards flatten because no bid liquidity is available",
                    json!({
                        "token_id": entry.token_id,
                        "size": fill_size,
                        "deferred_price": exit.price,
                        "trace_id": trace_id,
                    }),
                );
                return Ok(vec![LiveRewardOrderUpdate::Changed(exit, event)]);
            };
            let mut exit = live_exit_order(
                entry,
                fill_size,
                floor_reward_price_to_tick(best_bid),
                "post-fill flatten immediately",
                trace_id,
            );
            Ok(vec![
                submit_one_live_exit_order(connector, &mut exit, false).await?,
            ])
        }
    }
}

fn live_exit_order(
    entry: &ManagedRewardOrder,
    size: Decimal,
    price: Decimal,
    reason: &str,
    trace_id: &str,
) -> ManagedRewardOrder {
    let now = OffsetDateTime::now_utc();
    ManagedRewardOrder {
        id: format!(
            "rewexit_{}_{}",
            now.unix_timestamp_nanos(),
            trace_id.trim_start_matches("trc_")
        ),
        account_id: entry.account_id.clone(),
        condition_id: entry.condition_id.clone(),
        token_id: entry.token_id.clone(),
        outcome: entry.outcome.clone(),
        side: RewardOrderSide::Sell,
        price,
        size,
        external_order_id: None,
        status: ManagedRewardOrderStatus::Planned,
        scoring: false,
        reason: reason.to_string(),
        filled_size: Decimal::ZERO,
        reward_earned: Decimal::ZERO,
        last_scored_at: None,
        created_at: now,
        updated_at: now,
    }
}

async fn cancel_sibling_live_reward_orders(
    connector: &LivePolymarketConnector,
    working_orders: &mut HashMap<String, ManagedRewardOrder>,
    filled_order: &ManagedRewardOrder,
    sibling_cancelled: &mut HashSet<String>,
    changed_orders: &mut Vec<ManagedRewardOrder>,
    events: &mut Vec<RewardRiskEvent>,
    report: &mut RewardBotRunReport,
    trace_id: &str,
) -> Result<()> {
    let sibling_ids = working_orders
        .values()
        .filter(|order| {
            order.id != filled_order.id
                && order.condition_id == filled_order.condition_id
                && order.token_id != filled_order.token_id
                && order.status.is_open_like()
                && sibling_cancelled.insert(order.id.clone())
        })
        .map(|order| order.id.clone())
        .collect::<Vec<_>>();

    for sibling_id in sibling_ids {
        let Some(sibling) = working_orders.get(&sibling_id).cloned() else {
            continue;
        };
        match cancel_one_live_reward_order(
            connector,
            sibling,
            "sibling rewards quote cancelled after live fill",
            trace_id,
        )
        .await?
        {
            LiveRewardOrderUpdate::Changed(order, event) => {
                working_orders.insert(order.id.clone(), order.clone());
                changed_orders.push(order);
                events.push(event);
                report.cancelled_orders += 1;
                report.risk_cancelled_orders += 1;
            }
            LiveRewardOrderUpdate::Unchanged(event) => events.push(event),
        }
    }
    Ok(())
}

async fn submit_deferred_live_exit_orders(
    connector: &LivePolymarketConnector,
    open_orders: &mut [ManagedRewardOrder],
    books: &HashMap<String, RewardOrderBook>,
    changed_orders: &mut Vec<ManagedRewardOrder>,
    events: &mut Vec<RewardRiskEvent>,
    report: &mut RewardBotRunReport,
) -> Result<()> {
    for order in open_orders.iter_mut().filter(|order| {
        order.side == RewardOrderSide::Sell
            && order.status == ManagedRewardOrderStatus::ExitPending
            && order.external_order_id.is_none()
            && order
                .reason
                .contains("flatten deferred until bid liquidity is observed")
    }) {
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
        order.updated_at = OffsetDateTime::now_utc();

        match submit_one_live_exit_order(connector, order, false).await? {
            LiveRewardOrderUpdate::Changed(updated, event) => {
                *order = updated.clone();
                changed_orders.push(updated);
                events.push(event);
                report.simulated_orders += 1;
            }
            LiveRewardOrderUpdate::Unchanged(event) => events.push(event),
        }
    }
    Ok(())
}

fn floor_reward_price_to_tick(price: Decimal) -> Decimal {
    price
        .max(REWARD_PRICE_TICK)
        .min(Decimal::ONE - REWARD_PRICE_TICK)
        .round_dp_with_strategy(2, RoundingStrategy::ToZero)
}

fn reward_live_fill_id(update: &ConnectorTradeFillUpdate) -> String {
    format!(
        "rewfill_{}_{}",
        sanitize_reward_id_fragment(reward_live_fill_source_id(update)),
        sanitize_reward_id_fragment(&update.external_order_id)
    )
}

fn reward_live_legacy_fill_id(update: &ConnectorTradeFillUpdate) -> String {
    format!(
        "rewfill_{}",
        sanitize_reward_id_fragment(reward_live_fill_source_id(update))
    )
}

fn reward_live_fill_source_id(update: &ConnectorTradeFillUpdate) -> &str {
    if update.external_trade_id.trim().is_empty() {
        update.event_id.as_str()
    } else {
        update.external_trade_id.as_str()
    }
}

fn sanitize_reward_id_fragment(raw: &str) -> String {
    raw.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}
