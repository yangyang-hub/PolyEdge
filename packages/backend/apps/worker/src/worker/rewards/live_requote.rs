fn live_cancel_reason_is_requote_drift(reason: &str) -> bool {
    reason.starts_with("quote target moved ")
}

fn live_requote_drift_cancel_reason(
    config: &RewardBotConfig,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    order: &ManagedRewardOrder,
    target_price: Decimal,
    now: OffsetDateTime,
) -> Option<String> {
    if config.requote_drift_cents <= Decimal::ZERO
        || config.requote_drift_max_cancels_per_cycle == 0
        || order.side != RewardOrderSide::Buy
    {
        return None;
    }
    let drift_cents = ((order.price - target_price).abs()) * Decimal::from(100_u64);
    if drift_cents <= config.requote_drift_cents {
        return None;
    }
    if config.requote_drift_cooldown_sec > 0 {
        let age_sec = (now - order.created_at).whole_seconds().max(0) as u64;
        if age_sec < config.requote_drift_cooldown_sec {
            return None;
        }
    }
    if !live_requote_drift_confirmed(config, book_history, order, target_price, now) {
        return None;
    }
    Some(format!(
        "quote target moved {drift_cents} cents beyond requote threshold after {}s confirmation and {}s cooldown",
        config.requote_drift_confirm_sec, config.requote_drift_cooldown_sec
    ))
}

fn live_requote_drift_confirmed(
    config: &RewardBotConfig,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    order: &ManagedRewardOrder,
    target_price: Decimal,
    now: OffsetDateTime,
) -> bool {
    if config.requote_drift_confirm_sec == 0 {
        return true;
    }
    let Some(history) = book_history.get(&order.token_id) else {
        return false;
    };
    let Some(old) = live_snapshot_ago(history, config.requote_drift_confirm_sec, now) else {
        return false;
    };
    let Some(old_target_price) = live_quote_bid_price_from_snapshot(old, config.quote_bid_rank)
    else {
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
    old_drift_cents > config.requote_drift_cents
}

fn live_quote_bid_price_from_snapshot(
    snapshot: &BookSnapshot,
    quote_bid_rank: u16,
) -> Option<Decimal> {
    let bid_prices = live_distinct_bid_prices(&snapshot.bids);
    let price = if live_bid_prices_use_fine_tick(&bid_prices) {
        live_quote_fine_tick_bid_price(&bid_prices, quote_bid_rank)?
    } else {
        bid_prices
            .get(usize::from(quote_bid_rank.saturating_sub(1)))
            .copied()?
    };
    Some(live_floor_to_tick(
        price,
        live_inferred_bid_price_tick(&bid_prices),
    ))
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

fn live_quote_fine_tick_bid_price(bid_prices: &[Decimal], rank: u16) -> Option<Decimal> {
    let best_bid = bid_prices.first().copied()?;
    let default_tick = Decimal::new(1, 2);
    let target = best_bid - default_tick * Decimal::from(rank.saturating_sub(1));
    bid_prices.iter().copied().find(|price| *price <= target)
}

fn live_bid_prices_use_fine_tick(bid_prices: &[Decimal]) -> bool {
    live_inferred_bid_price_tick(bid_prices) < Decimal::new(1, 2)
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
