// Risk-control checks executed during order reconciliation.
//
// Each function returns `Some(reason)` when the order should be cancelled,
// or `None` to let it continue. All checks are gated by their config field
// being non-zero (0 = disabled).

/// Feature 1 — Minimum depth threshold.
///
/// Sums the resting bid notional at and above the order's price on the same
/// token book. Cancels when the book above us is thinner than `min_depth_usd`.
fn check_min_depth(
    order: &ManagedRewardOrder,
    books: &HashMap<String, RewardOrderBook>,
    config: &RewardBotConfig,
) -> Option<String> {
    if config.min_depth_usd <= Decimal::ZERO || order.side != RewardOrderSide::Buy {
        return None;
    }
    let book = books.get(&order.token_id)?;
    let depth_usd: Decimal = book
        .bids
        .iter()
        .filter(|level| level.price >= order.price)
        .map(|level| (level.price * level.size).round_dp(4))
        .sum();
    if depth_usd < config.min_depth_usd {
        Some(format!(
            "bid depth {} above order price {} is below minimum {}",
            depth_usd, order.price, config.min_depth_usd
        ))
    } else {
        None
    }
}

/// Compute the 1-based bid rank of `price` among the book's distinct price
/// levels (1 = best/highest bid). Returns `None` when the book is empty.
fn compute_bid_rank(price: Decimal, book: &RewardOrderBook) -> Option<usize> {
    // Bids are expected sorted descending by price.
    let mut seen_prices: Vec<Decimal> = Vec::new();
    for level in &book.bids {
        if seen_prices.last() != Some(&level.price) {
            seen_prices.push(level.price);
        }
    }
    if seen_prices.is_empty() {
        return None;
    }

    let better_levels = seen_prices.iter().filter(|level_price| **level_price > price).count();
    Some(better_levels + 1)
}

/// Feature 2 — Bid-rank promotion cancel.
///
/// Cancels when the order's price rank rises to `cancel_bid_rank` or better
/// (i.e. closer to the top). Rank 1 = best bid, rank 2 = second best, etc.
fn check_bid_rank(
    order: &ManagedRewardOrder,
    books: &HashMap<String, RewardOrderBook>,
    config: &RewardBotConfig,
) -> Option<String> {
    if config.cancel_bid_rank == 0 || order.side != RewardOrderSide::Buy {
        return None;
    }
    let book = books.get(&order.token_id)?;
    let rank = compute_bid_rank(order.price, book)?;
    if rank <= config.cancel_bid_rank as usize {
        Some(format!(
            "order promoted to bid-{} (cancel threshold: bid-{})",
            rank, config.cancel_bid_rank
        ))
    } else {
        None
    }
}

/// Find the book snapshot closest to `window_sec` ago from the history deque.
fn find_snapshot_ago(
    history: &VecDeque<BookSnapshot>,
    window_sec: u64,
    now: OffsetDateTime,
) -> Option<BookSnapshot> {
    if window_sec == 0 || history.is_empty() {
        return None;
    }
    let target = now - time::Duration::seconds(window_sec as i64);
    // Walk backwards to find the snapshot closest to (but not after) target.
    history
        .iter()
        .rev()
        .find(|snap| snap.observed_at <= target)
        .cloned()
}

/// Sum the notional of the top-N bid levels.
fn top_bid_notional(book: &RewardOrderBook, n: usize) -> Decimal {
    book.bids
        .iter()
        .take(n)
        .map(|level| (level.price * level.size).round_dp(4))
        .sum()
}

/// Sum the notional of the top-N ask levels.
fn top_ask_notional(book: &RewardOrderBook, n: usize) -> Decimal {
    book.asks
        .iter()
        .take(n)
        .map(|level| (level.price * level.size).round_dp(4))
        .sum()
}

/// Feature 3 — Depth-drop detection.
///
/// Compares the current top-2 bid notional against the snapshot from
/// `depth_drop_window_sec` ago. Cancels when the drop exceeds `depth_drop_pct`.
fn check_depth_drop(
    order: &ManagedRewardOrder,
    books: &HashMap<String, RewardOrderBook>,
    history: &VecDeque<BookSnapshot>,
    config: &RewardBotConfig,
    now: OffsetDateTime,
) -> Option<String> {
    if config.depth_drop_pct <= Decimal::ZERO || order.side != RewardOrderSide::Buy {
        return None;
    }
    let current = books.get(&order.token_id)?;
    let old_snap = find_snapshot_ago(history, config.depth_drop_window_sec, now)?;
    let old_book = RewardOrderBook {
        token_id: order.token_id.clone(),
        bids: old_snap.bids,
        asks: old_snap.asks,
        observed_at: old_snap.observed_at,
    };

    let old_notional = top_bid_notional(&old_book, 2);
    if old_notional <= Decimal::ZERO {
        return None;
    }
    let cur_notional = top_bid_notional(current, 2);
    let drop_pct = ((old_notional - cur_notional) / old_notional * decimal("100"))
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

/// Feature 4 — Fill-velocity detection (ask-side depth decrease).
///
/// A rapid decrease in ask depth is inferred as aggressive taker buying.
/// Compares the current top-2 ask notional against the historical snapshot.
fn check_fill_velocity(
    order: &ManagedRewardOrder,
    books: &HashMap<String, RewardOrderBook>,
    history: &VecDeque<BookSnapshot>,
    config: &RewardBotConfig,
    now: OffsetDateTime,
) -> Option<String> {
    if config.fill_velocity_usd <= Decimal::ZERO || order.side != RewardOrderSide::Buy {
        return None;
    }
    let current = books.get(&order.token_id)?;
    let old_snap = find_snapshot_ago(history, config.fill_velocity_window_sec, now)?;
    let old_book = RewardOrderBook {
        token_id: order.token_id.clone(),
        bids: old_snap.bids,
        asks: old_snap.asks,
        observed_at: old_snap.observed_at,
    };

    let old_ask = top_ask_notional(&old_book, 2);
    let cur_ask = top_ask_notional(current, 2);
    if old_ask <= cur_ask {
        return None; // ask depth didn't decrease
    }
    let decrease = old_ask - cur_ask;
    if decrease >= config.fill_velocity_usd {
        Some(format!(
            "ask depth decreased ${decrease} in {}s (threshold ${})",
            config.fill_velocity_window_sec, config.fill_velocity_usd
        ))
    } else {
        None
    }
}

/// Feature 5 — Mass-cancel following.
///
/// A large decrease in total bid depth (across all levels) is inferred as
/// other makers pulling their orders en masse.
fn check_mass_cancel(
    order: &ManagedRewardOrder,
    books: &HashMap<String, RewardOrderBook>,
    history: &VecDeque<BookSnapshot>,
    config: &RewardBotConfig,
    now: OffsetDateTime,
) -> Option<String> {
    if config.mass_cancel_pct <= Decimal::ZERO || order.side != RewardOrderSide::Buy {
        return None;
    }
    let current = books.get(&order.token_id)?;
    let old_snap = find_snapshot_ago(history, config.mass_cancel_window_sec, now)?;
    let old_book = RewardOrderBook {
        token_id: order.token_id.clone(),
        bids: old_snap.bids,
        asks: old_snap.asks,
        observed_at: old_snap.observed_at,
    };

    let old_total: Decimal = old_book
        .bids
        .iter()
        .map(|l| (l.price * l.size).round_dp(4))
        .sum();
    if old_total <= Decimal::ZERO {
        return None;
    }
    let cur_total: Decimal = current
        .bids
        .iter()
        .map(|l| (l.price * l.size).round_dp(4))
        .sum();
    let drop_pct = ((old_total - cur_total) / old_total * decimal("100")).round_dp(2);
    if drop_pct >= config.mass_cancel_pct {
        Some(format!(
            "total bid depth dropped {drop_pct}% in {}s (threshold {}%)",
            config.mass_cancel_window_sec, config.mass_cancel_pct
        ))
    } else {
        None
    }
}

/// Feature 6 — Periodic requote (queue-position reset).
///
/// Cancels resting orders older than `requote_interval_sec + jitter` so they
/// can be re-placed at the back of the queue. The jitter is deterministic per
/// order id so the interval is stable across ticks.
fn check_requote_age(
    order: &ManagedRewardOrder,
    config: &RewardBotConfig,
    now: OffsetDateTime,
) -> Option<String> {
    if config.requote_interval_sec == 0 || order.side != RewardOrderSide::Buy {
        return None;
    }
    let age_sec = (now - order.created_at).whole_seconds().max(0) as u64;
    let jitter = deterministic_jitter(order.id.as_bytes(), config.requote_jitter_sec);
    let threshold = config.requote_interval_sec + jitter;
    if age_sec >= threshold {
        Some(format!(
            "order age {age_sec}s exceeds requote interval {threshold}s"
        ))
    } else {
        None
    }
}

/// Simple deterministic jitter in [0, max_jitter] derived from a byte slice.
fn deterministic_jitter(bytes: &[u8], max_jitter: u64) -> u64 {
    if max_jitter == 0 {
        return 0;
    }
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash % (max_jitter + 1)
}
