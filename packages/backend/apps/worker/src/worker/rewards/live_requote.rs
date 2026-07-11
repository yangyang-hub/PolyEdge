fn live_cancel_reason_is_requote_drift(reason: &str) -> bool {
    reason.starts_with("competitive quote target moved up ")
}

fn live_requote_drift_cancel_reason(
    config: &RewardBotConfig,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    order: &ManagedRewardOrder,
    target_price: Decimal,
    now: OffsetDateTime,
) -> Option<String> {
    if order.side != RewardOrderSide::Buy {
        return None;
    }
    if target_price < order.price {
        let drift_cents = (order.price - target_price) * Decimal::from(100_u64);
        if config.adverse_requote_drift_cents <= Decimal::ZERO
            || drift_cents < config.adverse_requote_drift_cents
            || !live_requote_direction_confirmed(
                config,
                book_history,
                order,
                target_price,
                now,
                config.adverse_requote_confirm_sec,
            )
        {
            return None;
        }
        return Some(format!(
            "adverse quote target moved down {drift_cents} cents beyond threshold after {}s confirmation",
            config.adverse_requote_confirm_sec
        ));
    }
    if target_price <= order.price
        || config.requote_drift_cents <= Decimal::ZERO
        || config.requote_drift_max_cancels_per_cycle == 0
    {
        return None;
    }
    let drift_cents = (target_price - order.price) * Decimal::from(100_u64);
    if drift_cents <= config.requote_drift_cents {
        return None;
    }
    if config.requote_drift_cooldown_sec > 0 {
        let age_sec = (now - order.created_at).whole_seconds().max(0) as u64;
        if age_sec < config.requote_drift_cooldown_sec {
            return None;
        }
    }
    if !live_requote_direction_confirmed(
        config,
        book_history,
        order,
        target_price,
        now,
        config.requote_drift_confirm_sec,
    ) {
        return None;
    }
    Some(format!(
        "competitive quote target moved up {drift_cents} cents beyond requote threshold after {}s confirmation and {}s cooldown",
        config.requote_drift_confirm_sec, config.requote_drift_cooldown_sec
    ))
}

fn live_requote_direction_confirmed(
    config: &RewardBotConfig,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    order: &ManagedRewardOrder,
    target_price: Decimal,
    now: OffsetDateTime,
    confirm_sec: u64,
) -> bool {
    if confirm_sec == 0 {
        return true;
    }
    let Some(history) = book_history.get(&order.token_id) else {
        return false;
    };
    let Some(old) = live_snapshot_ago(history, confirm_sec, now) else {
        return false;
    };
    let Some(current) = history
        .iter()
        .rev()
        .find(|snapshot| snapshot.observed_at <= now)
    else {
        return false;
    };
    let Some(current_best_bid) = live_distinct_bid_prices(&current.bids).first().copied() else {
        return false;
    };
    // Preserve the materializer's currently selected distance from buy-one.
    // Standard V2 may dynamically choose rank 1..N, so recomputing history
    // with the configured preferred rank would confirm the wrong price band.
    let selected_offset = (current_best_bid - target_price).max(Decimal::ZERO);
    let Some(old_target_price) = live_quote_bid_price_from_snapshot(old, selected_offset) else {
        return false;
    };
    let current_delta = target_price - order.price;
    let old_delta = old_target_price - order.price;
    if (current_delta > Decimal::ZERO && old_delta <= Decimal::ZERO)
        || (current_delta < Decimal::ZERO && old_delta >= Decimal::ZERO)
    {
        return false;
    }
    let old_drift_cents = old_delta.abs() * Decimal::from(100_u64);
    let threshold = if current_delta < Decimal::ZERO {
        config.adverse_requote_drift_cents
    } else {
        config.requote_drift_cents
    };
    old_drift_cents >= threshold
}

fn live_quote_bid_price_from_snapshot(
    snapshot: &BookSnapshot,
    selected_offset: Decimal,
) -> Option<Decimal> {
    let bid_prices = live_distinct_bid_prices(&snapshot.bids);
    let best_bid = bid_prices.first().copied()?;
    let venue_tick = live_inferred_bid_price_tick(&bid_prices);
    let price = best_bid - selected_offset.max(Decimal::ZERO);
    (price > Decimal::ZERO).then(|| live_floor_to_tick(price, venue_tick))
}

fn live_distinct_bid_prices(levels: &[RewardBookLevel]) -> Vec<Decimal> {
    let mut prices = Vec::new();
    for level in levels {
        if level.price <= Decimal::ZERO || prices.last() == Some(&level.price) {
            continue;
        }
        prices.push(level.price);
    }
    prices
}

fn live_inferred_bid_price_tick(bid_prices: &[Decimal]) -> Decimal {
    let default_tick = Decimal::new(1, 2);
    bid_prices
        .windows(2)
        .filter_map(|window| {
            let diff = (window[0] - window[1]).abs();
            (diff > Decimal::ZERO).then_some(diff)
        })
        .min()
        .unwrap_or(default_tick)
        .min(default_tick)
}

fn live_floor_to_tick(value: Decimal, tick: Decimal) -> Decimal {
    (value / tick).floor() * tick
}
