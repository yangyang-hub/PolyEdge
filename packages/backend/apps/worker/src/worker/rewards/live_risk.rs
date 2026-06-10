fn live_cancel_candidates(
    config: &RewardBotConfig,
    plans: &[RewardQuotePlan],
    open_orders: &[ManagedRewardOrder],
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    kill_switch: bool,
) -> Vec<(String, String)> {
    let plan_index: HashMap<&str, &RewardQuotePlan> = plans
        .iter()
        .map(|plan| (plan.condition_id.as_str(), plan))
        .collect();
    let now = OffsetDateTime::now_utc();
    open_orders
        .iter()
        .filter(|order| order.status.is_open_like())
        .filter_map(|order| {
            live_cancel_reason(
                config,
                &plan_index,
                books,
                book_history,
                order,
                now,
                kill_switch,
            )
                .map(|reason| (order.id.clone(), reason))
        })
        .collect()
}

fn live_cancel_reason(
    config: &RewardBotConfig,
    plans: &HashMap<&str, &RewardQuotePlan>,
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    order: &ManagedRewardOrder,
    now: OffsetDateTime,
    kill_switch: bool,
) -> Option<String> {
    if order.reason.contains("awaiting final reconciliation")
        || live_submission_was_attempted(order)
    {
        return None;
    }
    if live_order_has_post_only_violation(order) {
        return Some("post-only violation requires cancellation".to_string());
    }
    if order.reason.contains("cancellation must be retried") {
        return Some("previous cancellation attempt left the order live".to_string());
    }
    if kill_switch && order.side == RewardOrderSide::Buy {
        return Some("global kill switch is active".to_string());
    }
    if order.side == RewardOrderSide::Sell
        && order.status == ManagedRewardOrderStatus::ExitPending
        && order.external_order_id.is_none()
    {
        return None;
    }
    if let Some(reason) = live_quote_book_unavailable_reason(config, books, &order.token_id, now) {
        return Some(reason);
    }
    if order.side != RewardOrderSide::Buy {
        return None;
    }
    let Some(plan) = plans.get(order.condition_id.as_str()) else {
        return Some("market no longer offers rewards".to_string());
    };
    if !plan.eligible {
        return Some("market dropped below eligibility threshold".to_string());
    }
    let Some(leg) = plan.legs.iter().find(|leg| leg.token_id == order.token_id) else {
        return Some("token no longer appears in live quote plan".to_string());
    };
    if config.requote_drift_cents > Decimal::ZERO {
        let drift_cents = ((order.price - leg.price).abs()) * Decimal::from(100_u64);
        if drift_cents > config.requote_drift_cents {
            return Some(format!(
                "midpoint drifted {drift_cents} cents beyond requote threshold"
            ));
        }
    }
    if let Some(reason) = live_min_depth_cancel_reason(config, books, order) {
        return Some(reason);
    }
    if let Some(reason) = live_bid_rank_cancel_reason(config, books, order) {
        return Some(reason);
    }
    if let Some(reason) = live_depth_drop_cancel_reason(config, books, book_history, order, now) {
        return Some(reason);
    }
    if let Some(reason) = live_fill_velocity_cancel_reason(config, books, book_history, order, now) {
        return Some(reason);
    }
    if let Some(reason) = live_mass_cancel_reason(config, books, book_history, order, now) {
        return Some(reason);
    }
    if let Some(reason) = live_requote_age_cancel_reason(config, order, now) {
        return Some(reason);
    }
    None
}

fn live_placement_orders(
    config: &RewardBotConfig,
    account_id: &str,
    plans: &[RewardQuotePlan],
    books: &HashMap<String, RewardOrderBook>,
    open_orders: &[ManagedRewardOrder],
    positions: &[RewardPosition],
    trace_id: &str,
) -> Vec<ManagedRewardOrder> {
    let max_markets = usize::from(config.max_markets);
    let max_open_orders = usize::from(config.max_open_orders);
    if max_markets == 0 || max_open_orders == 0 {
        return Vec::new();
    }

    let mut active_markets: HashSet<String> = open_orders
        .iter()
        .filter(|order| order.status.is_open_like())
        .map(|order| order.condition_id.clone())
        .collect();
    let mut orders = open_orders.to_vec();
    let mut placements = Vec::new();
    let mut seq = 0usize;

    for plan in plans.iter().filter(|plan| plan.eligible) {
        if !live_plan_has_fresh_quote_books(plan, books, config) {
            continue;
        }
        if active_markets.len() >= max_markets && !active_markets.contains(&plan.condition_id) {
            continue;
        }
        for leg in &plan.legs {
            if orders.iter().filter(|order| order.status.is_open_like()).count()
                >= max_open_orders
            {
                return placements;
            }
            if orders.iter().any(|order| {
                order.condition_id == plan.condition_id
                    && order.token_id == leg.token_id
                    && order.side == RewardOrderSide::Buy
                    && order.status.is_open_like()
            }) {
                continue;
            }
            let notional = (leg.price * leg.size).round_dp(4);
            if notional <= Decimal::ZERO {
                continue;
            }
            if live_position_over_cap(config, positions, &leg.token_id, leg.price, notional) {
                continue;
            }
            // Live maker buys intentionally do not reserve global cash until a
            // fill is observed. This cap applies to actual inventory only, so
            // the same funds can be quoted across markets while orders rest.
            if config.max_global_position_usd > Decimal::ZERO
                && live_global_inventory_notional(positions) + notional
                    > config.max_global_position_usd
            {
                continue;
            }

            active_markets.insert(plan.condition_id.clone());
            seq += 1;
            let now = OffsetDateTime::now_utc();
            let order = ManagedRewardOrder {
                id: format!(
                    "rewlive_{}_{}_{}",
                    now.unix_timestamp_nanos(),
                    seq,
                    trace_id.trim_start_matches("trc_")
                ),
                account_id: account_id.to_string(),
                condition_id: plan.condition_id.clone(),
                token_id: leg.token_id.clone(),
                outcome: leg.outcome.clone(),
                side: RewardOrderSide::Buy,
                price: leg.price,
                size: leg.size,
                external_order_id: None,
                status: ManagedRewardOrderStatus::Planned,
                scoring: true,
                reason: "pending live post-only rewards quote".to_string(),
                filled_size: Decimal::ZERO,
                reward_earned: Decimal::ZERO,
                last_scored_at: None,
                created_at: now,
                updated_at: now,
            };
            orders.push(order.clone());
            placements.push(order);
        }
    }

    placements
}

fn live_plan_has_fresh_quote_books(
    plan: &RewardQuotePlan,
    books: &HashMap<String, RewardOrderBook>,
    config: &RewardBotConfig,
) -> bool {
    let now = OffsetDateTime::now_utc();
    plan.legs.iter().all(|leg| {
        live_quote_book_unavailable_reason(config, books, &leg.token_id, now).is_none()
    })
}

fn live_quote_book_unavailable_reason(
    config: &RewardBotConfig,
    books: &HashMap<String, RewardOrderBook>,
    token_id: &str,
    now: OffsetDateTime,
) -> Option<String> {
    let Some(book) = books.get(token_id) else {
        return Some("orderbook unavailable for live order".to_string());
    };
    if book.bids.is_empty() || book.asks.is_empty() {
        return Some("orderbook is empty for live order".to_string());
    }
    if config.stale_book_ms == 0 {
        return None;
    }

    let age_ms = (now - book.observed_at).whole_milliseconds();
    if age_ms < 0 || age_ms > i128::from(config.stale_book_ms) {
        return Some(format!(
            "orderbook stale for live order: age_ms={age_ms}, max_age_ms={}",
            config.stale_book_ms
        ));
    }
    None
}

fn live_min_depth_cancel_reason(
    config: &RewardBotConfig,
    books: &HashMap<String, RewardOrderBook>,
    order: &ManagedRewardOrder,
) -> Option<String> {
    if config.min_depth_usd <= Decimal::ZERO || order.side != RewardOrderSide::Buy {
        return None;
    }
    let book = books.get(&order.token_id)?;
    let depth_usd: Decimal = book
        .bids
        .iter()
        .filter(|level| level.price > order.price)
        .map(|level| (level.price * level.size).round_dp(4))
        .sum();
    if depth_usd < config.min_depth_usd {
        Some(format!(
            "bid depth {depth_usd} above order price {} is below minimum {}",
            order.price, config.min_depth_usd
        ))
    } else {
        None
    }
}

fn live_bid_rank_cancel_reason(
    config: &RewardBotConfig,
    books: &HashMap<String, RewardOrderBook>,
    order: &ManagedRewardOrder,
) -> Option<String> {
    if config.cancel_bid_rank == 0 || order.side != RewardOrderSide::Buy {
        return None;
    }
    let book = books.get(&order.token_id)?;
    let rank = live_bid_rank(order.price, book)?;
    if rank <= usize::from(config.cancel_bid_rank) {
        Some(format!(
            "order promoted to bid-{rank} (cancel threshold: bid-{})",
            config.cancel_bid_rank
        ))
    } else {
        None
    }
}

fn live_bid_rank(price: Decimal, book: &RewardOrderBook) -> Option<usize> {
    let mut seen_prices = Vec::new();
    for level in &book.bids {
        if seen_prices.last() != Some(&level.price) {
            seen_prices.push(level.price);
        }
    }
    if seen_prices.is_empty() {
        return None;
    }
    Some(seen_prices.iter().filter(|level_price| **level_price > price).count() + 1)
}

fn live_depth_drop_cancel_reason(
    config: &RewardBotConfig,
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    order: &ManagedRewardOrder,
    now: OffsetDateTime,
) -> Option<String> {
    if config.depth_drop_pct <= Decimal::ZERO || order.side != RewardOrderSide::Buy {
        return None;
    }
    let current = books.get(&order.token_id)?;
    let old = live_snapshot_ago(book_history.get(&order.token_id)?, config.depth_drop_window_sec, now)?;
    let old_notional = live_top_bid_notional_snapshot(old, 2);
    if old_notional <= Decimal::ZERO {
        return None;
    }
    let current_notional = live_top_bid_notional(current, 2);
    let drop_pct = ((old_notional - current_notional) / old_notional * Decimal::from(100_u64))
        .round_dp(2);
    if drop_pct >= config.depth_drop_pct {
        Some(format!(
            "top-2 bid depth dropped {drop_pct}% in {}s (threshold {}%)",
            config.depth_drop_window_sec, config.depth_drop_pct
        ))
    } else {
        None
    }
}

fn live_fill_velocity_cancel_reason(
    config: &RewardBotConfig,
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    order: &ManagedRewardOrder,
    now: OffsetDateTime,
) -> Option<String> {
    if config.fill_velocity_usd <= Decimal::ZERO || order.side != RewardOrderSide::Buy {
        return None;
    }
    let current = books.get(&order.token_id)?;
    let old =
        live_snapshot_ago(book_history.get(&order.token_id)?, config.fill_velocity_window_sec, now)?;
    let old_ask = live_top_ask_notional_snapshot(old, 2);
    let current_ask = live_top_ask_notional(current, 2);
    if old_ask <= current_ask {
        return None;
    }
    let decrease = old_ask - current_ask;
    if decrease >= config.fill_velocity_usd {
        Some(format!(
            "ask depth decreased ${decrease} in {}s (threshold ${})",
            config.fill_velocity_window_sec, config.fill_velocity_usd
        ))
    } else {
        None
    }
}

fn live_mass_cancel_reason(
    config: &RewardBotConfig,
    books: &HashMap<String, RewardOrderBook>,
    book_history: &HashMap<String, VecDeque<BookSnapshot>>,
    order: &ManagedRewardOrder,
    now: OffsetDateTime,
) -> Option<String> {
    if config.mass_cancel_pct <= Decimal::ZERO || order.side != RewardOrderSide::Buy {
        return None;
    }
    let current = books.get(&order.token_id)?;
    let old = live_snapshot_ago(book_history.get(&order.token_id)?, config.mass_cancel_window_sec, now)?;
    let old_total: Decimal = old.bids.iter().map(|level| (level.price * level.size).round_dp(4)).sum();
    if old_total <= Decimal::ZERO {
        return None;
    }
    let current_total: Decimal = current
        .bids
        .iter()
        .map(|level| (level.price * level.size).round_dp(4))
        .sum();
    let drop_pct =
        ((old_total - current_total) / old_total * Decimal::from(100_u64)).round_dp(2);
    if drop_pct >= config.mass_cancel_pct {
        Some(format!(
            "total bid depth dropped {drop_pct}% in {}s (threshold {}%)",
            config.mass_cancel_window_sec, config.mass_cancel_pct
        ))
    } else {
        None
    }
}

fn live_requote_age_cancel_reason(
    config: &RewardBotConfig,
    order: &ManagedRewardOrder,
    now: OffsetDateTime,
) -> Option<String> {
    if config.requote_interval_sec == 0 || order.side != RewardOrderSide::Buy {
        return None;
    }
    let age_sec = (now - order.created_at).whole_seconds().max(0) as u64;
    let threshold =
        config.requote_interval_sec + deterministic_reward_jitter(&order.id, config.requote_jitter_sec);
    if age_sec >= threshold {
        Some(format!(
            "order age {age_sec}s exceeds requote interval {threshold}s"
        ))
    } else {
        None
    }
}

fn live_snapshot_ago(
    history: &VecDeque<BookSnapshot>,
    window_sec: u64,
    now: OffsetDateTime,
) -> Option<&BookSnapshot> {
    if window_sec == 0 {
        return None;
    }
    let target = now - TimeDuration::seconds(window_sec as i64);
    history.iter().rev().find(|snapshot| snapshot.observed_at <= target)
}

fn live_top_bid_notional(book: &RewardOrderBook, depth: usize) -> Decimal {
    book.bids
        .iter()
        .take(depth)
        .map(|level| (level.price * level.size).round_dp(4))
        .sum()
}

fn live_top_ask_notional(book: &RewardOrderBook, depth: usize) -> Decimal {
    book.asks
        .iter()
        .take(depth)
        .map(|level| (level.price * level.size).round_dp(4))
        .sum()
}

fn live_top_bid_notional_snapshot(snapshot: &BookSnapshot, depth: usize) -> Decimal {
    snapshot
        .bids
        .iter()
        .take(depth)
        .map(|level| (level.price * level.size).round_dp(4))
        .sum()
}

fn live_top_ask_notional_snapshot(snapshot: &BookSnapshot, depth: usize) -> Decimal {
    snapshot
        .asks
        .iter()
        .take(depth)
        .map(|level| (level.price * level.size).round_dp(4))
        .sum()
}

fn deterministic_reward_jitter(input: &str, max_jitter: u64) -> u64 {
    if max_jitter == 0 {
        return 0;
    }
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash % (max_jitter + 1)
}

fn live_position_over_cap(
    config: &RewardBotConfig,
    positions: &[RewardPosition],
    token_id: &str,
    price: Decimal,
    candidate_notional: Decimal,
) -> bool {
    if config.max_position_usd <= Decimal::ZERO {
        return false;
    }
    let current = positions
        .iter()
        .find(|position| position.token_id == token_id && position.size > Decimal::ZERO)
        .map(|position| position.size * price)
        .unwrap_or_default();
    current + candidate_notional > config.max_position_usd
}

fn live_global_inventory_notional(positions: &[RewardPosition]) -> Decimal {
    positions
        .iter()
        .filter(|position| position.size > Decimal::ZERO)
        .map(|position| position.size * position.avg_price)
        .sum()
}

fn reward_side_to_polymarket(side: RewardOrderSide) -> PolymarketTokenOrderSide {
    match side {
        RewardOrderSide::Buy => PolymarketTokenOrderSide::Buy,
        RewardOrderSide::Sell => PolymarketTokenOrderSide::Sell,
    }
}

fn reward_live_event(
    order: &ManagedRewardOrder,
    event_type: &str,
    severity: RewardRiskSeverity,
    message: impl Into<String>,
    metadata: serde_json::Value,
) -> RewardRiskEvent {
    new_risk_event(
        Some(order.account_id.clone()),
        Some(order.condition_id.clone()),
        order.external_order_id.clone(),
        event_type,
        severity,
        message,
        metadata,
    )
}
