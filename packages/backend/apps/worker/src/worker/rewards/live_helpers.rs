fn floor_reward_price_to_tick(price: Decimal) -> Decimal {
    price
        .max(REWARD_PRICE_TICK)
        .min(Decimal::ONE - REWARD_PRICE_TICK)
        .round_dp_with_strategy(2, RoundingStrategy::ToZero)
}

fn ceil_reward_price_to_tick(price: Decimal) -> Decimal {
    let bounded = price
        .max(REWARD_PRICE_TICK)
        .min(Decimal::ONE - REWARD_PRICE_TICK);
    ((bounded / REWARD_PRICE_TICK).ceil() * REWARD_PRICE_TICK)
        .min(Decimal::ONE - REWARD_PRICE_TICK)
}

fn reward_best_bid_tick(book: Option<&RewardOrderBook>) -> Option<Decimal> {
    book.and_then(|book| book.bids.first())
        .map(|level| floor_reward_price_to_tick(level.price))
        .filter(|price| *price > Decimal::ZERO)
}

fn reward_sell_exit_floor(
    order: &ManagedRewardOrder,
    positions: &[RewardPosition],
) -> Decimal {
    let position_floor = positions
        .iter()
        .find(|position| position.token_id == order.token_id)
        .map(|position| position.avg_price)
        .filter(|price| *price > Decimal::ZERO)
        .unwrap_or(Decimal::ZERO);
    ceil_reward_price_to_tick(Decimal::max(order.price, position_floor))
}

fn reward_non_loss_exit_bid(
    order: &ManagedRewardOrder,
    books: &HashMap<String, RewardOrderBook>,
    positions: &[RewardPosition],
) -> Option<Decimal> {
    if order.side != RewardOrderSide::Sell {
        return None;
    }
    let exit_floor = reward_sell_exit_floor(order, positions);
    reward_best_bid_tick(books.get(&order.token_id)).filter(|best_bid| *best_bid >= exit_floor)
}

fn live_exit_retry_due_or_crossable(
    order: &ManagedRewardOrder,
    now: OffsetDateTime,
    books: &HashMap<String, RewardOrderBook>,
    positions: &[RewardPosition],
) -> bool {
    live_exit_retry_due(order, now) || reward_non_loss_exit_bid(order, books, positions).is_some()
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

fn live_available_usd_after_unmanaged_external_buys(
    account: &RewardAccountState,
    open_orders: &[ManagedRewardOrder],
) -> Decimal {
    let managed_external_buy_notional: Decimal = open_orders
        .iter()
        .filter(|order| {
            order.side == RewardOrderSide::Buy
                && order.status.is_open_like()
                && order
                    .external_order_id
                    .as_deref()
                    .is_some_and(|id| !is_internal_reward_order_id(id))
        })
        .map(|order| {
            (order.price * (order.size - order.filled_size).max(Decimal::ZERO)).round_dp(4)
        })
        .sum();
    let unmanaged_external_buy_notional =
        (account.external_buy_notional - managed_external_buy_notional).max(Decimal::ZERO);
    (account.available_usd - unmanaged_external_buy_notional).max(Decimal::ZERO)
}

fn deferred_live_exit_after_cancellation(
    order: &ManagedRewardOrder,
    position: Option<&RewardPosition>,
    trace_id: &str,
) -> Option<ManagedRewardOrder> {
    if order.side != RewardOrderSide::Sell || order.reason.contains("cancel-all command") {
        return None;
    }
    let position_size = position
        .map(|position| position.size.max(Decimal::ZERO))
        .unwrap_or_default();
    let remaining = (order.size - order.filled_size)
        .max(Decimal::ZERO)
        .min(position_size)
        .round_dp_with_strategy(2, RoundingStrategy::ToZero);
    if remaining <= Decimal::ZERO {
        return None;
    }

    let reason = if deferred_live_exit_is_post_only(order) {
        "retry post-fill exit at markup after external cancellation"
    } else {
        "retry post-fill flatten after external cancellation"
    };
    let mut retry = live_exit_order(order, remaining, order.price, reason, trace_id);
    retry.status = ManagedRewardOrderStatus::ExitPending;
    Some(retry)
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
        OrderStatus::Canceled | OrderStatus::Filled => {
            order.status = ManagedRewardOrderStatus::Cancelled;
            order.scoring = false;
            order.reason = if update.status == OrderStatus::Filled {
                "live rewards order reached a terminal match after settled trades; unfilled remainder closed"
                    .to_string()
            } else {
                "live rewards order was cancelled on Polymarket".to_string()
            };
            order.updated_at = OffsetDateTime::now_utc();
            let event = reward_live_event(
                &order,
                if update.status == OrderStatus::Filled {
                    "reward_live_order_status_terminal_match"
                } else {
                    "reward_live_order_status_cancelled"
                },
                RewardRiskSeverity::Info,
                order.reason.clone(),
                json!({
                    "event_id": update.event_id,
                    "trace_id": trace_id,
                    "external_order_id": update.external_order_id,
                }),
            );
            Some((order, event))
        }
        OrderStatus::Open | OrderStatus::Submitted | OrderStatus::PartiallyFilled
            if order.reason.contains(LIVE_EXTERNAL_ORDER_NOT_FOUND_MARKER) =>
        {
            order.scoring = false;
            order.reason = "external order lookup recovered; live order confirmed".to_string();
            order.updated_at = OffsetDateTime::now_utc();
            let event = reward_live_event(
                &order,
                "reward_live_external_order_recovered",
                RewardRiskSeverity::Info,
                order.reason.clone(),
                json!({
                    "event_id": update.event_id,
                    "trace_id": trace_id,
                    "external_order_id": update.external_order_id,
                }),
            );
            Some((order, event))
        }
        OrderStatus::Open if order.reason.contains("awaiting final reconciliation") => {
            order.scoring = false;
            order.reason =
                "order confirmed live after cancellation attempt; cancellation must be retried"
                    .to_string();
            order.updated_at = OffsetDateTime::now_utc();
            let event = reward_live_event(
                &order,
                "reward_live_order_cancel_retry_required",
                RewardRiskSeverity::Critical,
                order.reason.clone(),
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

fn mark_live_external_order_not_found(
    mut order: ManagedRewardOrder,
    external_order_id: &str,
) -> Option<(ManagedRewardOrder, RewardRiskEvent)> {
    if !order.status.is_open_like() {
        return None;
    }

    let now = OffsetDateTime::now_utc();
    if order.reason.contains(LIVE_EXTERNAL_ORDER_NOT_FOUND_MARKER) {
        if now
            < order.updated_at
                + TimeDuration::seconds(LIVE_EXTERNAL_ORDER_NOT_FOUND_CLOSE_AFTER_SECS)
        {
            return None;
        }

        order.status = ManagedRewardOrderStatus::Cancelled;
        order.scoring = false;
        order.reason = format!(
            "external order lookup remained not found for 5 minutes; local order closed with no confirmed fill: {external_order_id}"
        );
        order.updated_at = now;
        let event = reward_live_event(
            &order,
            "reward_live_external_order_not_found_closed",
            RewardRiskSeverity::Warning,
            order.reason.clone(),
            json!({
                "external_order_id": external_order_id,
                "close_after_seconds": LIVE_EXTERNAL_ORDER_NOT_FOUND_CLOSE_AFTER_SECS,
            }),
        );
        return Some((order, event));
    }

    order.scoring = false;
    order.reason = format!(
        "{LIVE_EXTERNAL_ORDER_NOT_FOUND_MARKER}; manual reconciliation required: {external_order_id}"
    );
    order.updated_at = now;
    let event = reward_live_event(
        &order,
        "reward_live_external_order_not_found",
        RewardRiskSeverity::Critical,
        order.reason.clone(),
        json!({ "external_order_id": external_order_id }),
    );
    Some((order, event))
}
