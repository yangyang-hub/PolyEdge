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
            order.scoring = order.side == RewardOrderSide::Buy;
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
    if !order.status.is_open_like() || order.reason.contains(LIVE_EXTERNAL_ORDER_NOT_FOUND_MARKER) {
        return None;
    }

    order.scoring = false;
    order.reason = format!(
        "{LIVE_EXTERNAL_ORDER_NOT_FOUND_MARKER}; manual reconciliation required: {external_order_id}"
    );
    order.updated_at = OffsetDateTime::now_utc();
    let event = reward_live_event(
        &order,
        "reward_live_external_order_not_found",
        RewardRiskSeverity::Critical,
        order.reason.clone(),
        json!({ "external_order_id": external_order_id }),
    );
    Some((order, event))
}
