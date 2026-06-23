const LIVE_ORDERBOOK_VALIDATION_SKIP_TTL: TimeDuration = TimeDuration::hours(12);
const LIVE_ORDERBOOK_WAITING_REASON_PREFIX: &str =
    "waiting for fresh orderbook data from subscription";
const LIVE_ORDERBOOK_PLACEMENT_STALE_HEADROOM_MS: i128 = 10_000;
const LIVE_ORDERBOOK_STALE_CANCEL_GRACE_MIN_MS: i64 = 60_000;
const LIVE_ORDERBOOK_STALE_CANCEL_GRACE_MAX_MS: i64 = 180_000;

fn live_orderbook_wait_reason(
    config: &RewardBotConfig,
    plan: &RewardQuotePlan,
    books: &HashMap<String, RewardOrderBook>,
    now: OffsetDateTime,
) -> Option<String> {
    live_orderbook_wait_reason_for_legs(config, &plan.legs, books, now, false)
}

fn live_orderbook_placement_wait_reason(
    config: &RewardBotConfig,
    legs: &[RewardQuoteLeg],
    books: &HashMap<String, RewardOrderBook>,
    now: OffsetDateTime,
) -> Option<String> {
    live_orderbook_wait_reason_for_legs(config, legs, books, now, true)
}

fn live_orderbook_wait_reason_for_legs(
    config: &RewardBotConfig,
    legs: &[RewardQuoteLeg],
    books: &HashMap<String, RewardOrderBook>,
    now: OffsetDateTime,
    require_placement_headroom: bool,
) -> Option<String> {
    let mut reasons = Vec::new();
    let mut seen = HashSet::new();

    for leg in legs {
        if !seen.insert(leg.token_id.clone()) {
            continue;
        }
        let label = if leg.outcome.trim().is_empty() {
            leg.token_id.as_str()
        } else {
            leg.outcome.as_str()
        };
        let Some(book) = books.get(&leg.token_id) else {
            reasons.push(format!("{label} orderbook unavailable"));
            continue;
        };
        if book.bids.is_empty() || book.asks.is_empty() {
            reasons.push(format!("{label} orderbook is empty"));
            continue;
        }
        if config.stale_book_ms == 0 {
            continue;
        }
        let age_ms = live_orderbook_age_ms(book, now);
        if live_orderbook_age_is_stale(age_ms, config.stale_book_ms) {
            reasons.push(format!(
                "{label} {}",
                live_orderbook_stale_reason(age_ms, config.stale_book_ms)
            ));
            continue;
        }
        if require_placement_headroom {
            let max_placement_age_ms = live_orderbook_max_placement_age_ms(config);
            if age_ms > max_placement_age_ms {
                reasons.push(format!(
                    "{label} orderbook too close to stale: age_ms={age_ms}, max_placement_age_ms={max_placement_age_ms}, max_age_ms={}",
                    config.stale_book_ms
                ));
            }
        }
    }

    if reasons.is_empty() {
        None
    } else {
        Some(format!(
            "{LIVE_ORDERBOOK_WAITING_REASON_PREFIX}: {}",
            reasons.join("; ")
        ))
    }
}

fn live_quote_book_missing_or_empty_reason(
    books: &HashMap<String, RewardOrderBook>,
    token_id: &str,
) -> Option<String> {
    let Some(book) = books.get(token_id) else {
        return Some("orderbook unavailable for live order".to_string());
    };
    if book.bids.is_empty() || book.asks.is_empty() {
        return Some("orderbook is empty for live order".to_string());
    }
    None
}

fn live_quote_book_stale_age_ms(
    config: &RewardBotConfig,
    books: &HashMap<String, RewardOrderBook>,
    token_id: &str,
    now: OffsetDateTime,
) -> Option<i128> {
    if config.stale_book_ms == 0 {
        return None;
    }
    let book = books.get(token_id)?;
    if book.bids.is_empty() || book.asks.is_empty() {
        return None;
    }
    let age_ms = live_orderbook_age_ms(book, now);
    live_orderbook_age_is_stale(age_ms, config.stale_book_ms).then_some(age_ms)
}

fn live_orderbook_age_ms(book: &RewardOrderBook, now: OffsetDateTime) -> i128 {
    (now - book.confirmed_at).whole_milliseconds()
}

fn live_orderbook_age_is_stale(age_ms: i128, stale_book_ms: u64) -> bool {
    age_ms < 0 || age_ms > i128::from(stale_book_ms)
}

fn live_orderbook_stale_reason(age_ms: i128, stale_book_ms: u64) -> String {
    format!("orderbook stale for live order: age_ms={age_ms}, max_age_ms={stale_book_ms}")
}

fn live_orderbook_max_placement_age_ms(config: &RewardBotConfig) -> i128 {
    if config.stale_book_ms == 0 {
        return i128::MAX;
    }
    let stale_ms = i128::from(config.stale_book_ms);
    let headroom = LIVE_ORDERBOOK_PLACEMENT_STALE_HEADROOM_MS.min(stale_ms / 2);
    stale_ms.saturating_sub(headroom)
}

fn live_stale_orderbook_cancel_grace_active(
    config: &RewardBotConfig,
    order: &ManagedRewardOrder,
    now: OffsetDateTime,
) -> bool {
    if order.side != RewardOrderSide::Buy
        || order.external_order_id.is_none()
        || config.stale_book_ms == 0
    {
        return false;
    }
    now < order.created_at + live_stale_orderbook_cancel_grace(config)
}

fn live_stale_orderbook_cancel_grace(config: &RewardBotConfig) -> TimeDuration {
    let stale_ms = i64::try_from(config.stale_book_ms).unwrap_or(i64::MAX / 2);
    let grace_ms = stale_ms.saturating_mul(2).clamp(
        LIVE_ORDERBOOK_STALE_CANCEL_GRACE_MIN_MS,
        LIVE_ORDERBOOK_STALE_CANCEL_GRACE_MAX_MS,
    );
    TimeDuration::milliseconds(grace_ms)
}
