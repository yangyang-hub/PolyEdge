#[must_use]
pub fn validate_reward_list_limit(limit: Option<u16>) -> u16 {
    limit.unwrap_or(DEFAULT_LIST_LIMIT).clamp(1, MAX_LIST_LIMIT)
}

#[must_use]
pub fn new_risk_event(
    account_id: Option<String>,
    condition_id: Option<String>,
    external_order_id: Option<String>,
    event_type: impl Into<String>,
    severity: RewardRiskSeverity,
    message: impl Into<String>,
    metadata: Value,
) -> RewardRiskEvent {
    let now = OffsetDateTime::now_utc();
    RewardRiskEvent {
        id: format!("rew_evt_{}", now.unix_timestamp_nanos()),
        account_id,
        condition_id,
        external_order_id,
        event_type: event_type.into(),
        severity,
        message: message.into(),
        metadata,
        created_at: now,
    }
}

const REWARD_INTERNAL_ORDER_ID_PREFIXES: &[&str] = &[
    "rew_",
    "rewx_",
    "rewfill_",
    "rewevt_",
    "rewlive_",
    "rewexit_",
    "sim_rew_",
];

const REWARD_EXTERNAL_OPEN_COUNT_BLOCKER_MARKERS: &[&str] = &[
    "manual reconciliation required",
    "live submission result unknown",
    "cancel result unknown",
    "awaiting final reconciliation",
];

#[must_use]
pub fn reward_order_counts_as_external_open(order: &ManagedRewardOrder) -> bool {
    order.status.is_open_like()
        && order.size > order.filled_size
        && order
            .external_order_id
            .as_deref()
            .is_some_and(reward_external_order_id_counts_as_external)
        && !reward_order_has_external_open_count_blocker(order)
}

#[must_use]
pub fn reward_external_order_id_counts_as_external(external_order_id: &str) -> bool {
    let external_order_id = external_order_id.trim();
    !external_order_id.is_empty()
        && !REWARD_INTERNAL_ORDER_ID_PREFIXES
            .iter()
            .any(|prefix| external_order_id.starts_with(prefix))
}

fn reward_order_has_external_open_count_blocker(order: &ManagedRewardOrder) -> bool {
    REWARD_EXTERNAL_OPEN_COUNT_BLOCKER_MARKERS
        .iter()
        .any(|marker| order.reason.contains(marker))
}

fn normalize_reward_spread_cents(raw: Decimal) -> Decimal {
    raw.max(Decimal::ZERO)
}

fn floor_to_tick(value: Decimal, tick: Decimal) -> Decimal {
    (value / tick).floor() * tick
}

fn floor_reward_size_for_cost_precision(price: Decimal, size: Decimal) -> Decimal {
    reward_size_for_cost_precision(price, size, false)
}

fn ceil_reward_size_for_cost_precision(price: Decimal, size: Decimal) -> Decimal {
    reward_size_for_cost_precision(price, size, true)
}

fn reward_size_for_cost_precision(price: Decimal, size: Decimal, round_up: bool) -> Decimal {
    let price_cents = (price * decimal("100")).normalize().mantissa().unsigned_abs() as u64;
    if price_cents == 0 {
        return Decimal::ZERO;
    }
    let step_hundredths = 100 / reward_greatest_common_divisor(price_cents, 100);
    let raw_hundredths = if round_up {
        (size * decimal("100")).ceil()
    } else {
        (size * decimal("100")).floor()
    }
    .normalize()
    .mantissa()
    .max(0) as u64;
    let adjusted_hundredths = if round_up {
        raw_hundredths.div_ceil(step_hundredths) * step_hundredths
    } else {
        (raw_hundredths / step_hundredths) * step_hundredths
    };
    Decimal::new(adjusted_hundredths as i64, 2)
}

fn reward_greatest_common_divisor(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left.max(1)
}

fn normalize_account_id(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "reward_bot".to_string()
    } else {
        trimmed.to_string()
    }
}

fn clamp_u16(value: u16, min: u16, max: u16) -> u16 {
    value.clamp(min, max)
}

fn clamp_decimal(value: Decimal, min: Decimal, max: Decimal) -> Decimal {
    Decimal::min(max, Decimal::max(min, value))
}

fn decimal(value: &str) -> Decimal {
    Decimal::from_str_exact(value).expect("static reward configuration default must be valid")
}

fn reward_order_has_active_reconciliation_error(order: &ManagedRewardOrder) -> bool {
    order.status.is_open_like()
        && [
            "manual reconciliation required",
            "live submission result unknown",
            "cancel result unknown",
            "cancellation must be retried",
        ]
        .iter()
        .any(|marker| order.reason.contains(marker))
}

fn decimal_from_f64(value: f64) -> Decimal {
    if !value.is_finite() {
        return Decimal::ZERO;
    }

    Decimal::from_str(&format!("{value:.6}")).unwrap_or(Decimal::ZERO)
}

fn decimal_to_f64(value: Decimal) -> f64 {
    value.to_string().parse::<f64>().unwrap_or(0.0)
}

const REWARD_PROVIDER_CACHE_TTL_JITTER_DIVISOR: u64 = 5;
const REWARD_PROVIDER_CACHE_MAX_JITTER_SEC: u64 = 15 * 60;

#[must_use]
pub fn reward_provider_cache_jitter_window_sec(ttl_sec: u64) -> u64 {
    (ttl_sec / REWARD_PROVIDER_CACHE_TTL_JITTER_DIVISOR)
        .min(REWARD_PROVIDER_CACHE_MAX_JITTER_SEC)
}

#[must_use]
pub fn reward_provider_cache_refresh_due(
    expires_at: OffsetDateTime,
    ttl_sec: u64,
    now: OffsetDateTime,
) -> bool {
    let refresh_window_sec = reward_provider_cache_jitter_window_sec(ttl_sec);
    expires_at <= now + TimeDuration::seconds(refresh_window_sec.min(i64::MAX as u64) as i64)
}

fn reward_provider_cache_policy_payload(ttl_sec: u64, now: OffsetDateTime) -> Value {
    let refresh_window_sec = reward_provider_cache_jitter_window_sec(ttl_sec);
    let max_valid_for_sec = ttl_sec.saturating_add(refresh_window_sec);
    json!({
        "ttl_sec": ttl_sec,
        "positive_jitter_window_sec": refresh_window_sec,
        "refresh_due_window_sec": refresh_window_sec,
        "requested_at_utc": now,
        "base_expires_at_utc": now + TimeDuration::seconds(ttl_sec.min(i64::MAX as u64) as i64),
        "max_expires_at_utc": now + TimeDuration::seconds(max_valid_for_sec.min(i64::MAX as u64) as i64),
        "decision_reuse_policy": "Provider decisions are cached until expires_at; make allow_quote conservative enough for this full TTL horizon.",
    })
}

fn reward_provider_cache_expires_at(
    now: OffsetDateTime,
    ttl_sec: u64,
    cache_scope: &str,
    stable_parts: &[&str],
) -> OffsetDateTime {
    let jitter_sec = reward_provider_cache_jitter_sec(ttl_sec, cache_scope, stable_parts);
    let total_sec = ttl_sec.saturating_add(jitter_sec).min(i64::MAX as u64);
    now + TimeDuration::seconds(total_sec as i64)
}

fn reward_provider_cache_jitter_sec(
    ttl_sec: u64,
    cache_scope: &str,
    stable_parts: &[&str],
) -> u64 {
    let window_sec = reward_provider_cache_jitter_window_sec(ttl_sec);
    if window_sec == 0 {
        return 0;
    }

    let mut hasher = Sha256::new();
    hasher.update(cache_scope.as_bytes());
    hasher.update([0]);
    for part in stable_parts {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    u64::from_be_bytes(bytes) % (window_sec + 1)
}

fn reward_market_candle_sample_from_cached_book(
    book: &CachedOrderBook,
    interval_sec: i32,
) -> Result<Option<RewardMarketCandleSample>> {
    if interval_sec <= 0 {
        return Err(AppError::invalid_input(
            "REWARD_CANDLE_INTERVAL_INVALID",
            "reward market candle interval must be positive",
        ));
    }
    let Some(best_bid) = book
        .bids
        .iter()
        .filter(|level| level.price > Decimal::ZERO && level.size > Decimal::ZERO)
        .map(|level| level.price)
        .max()
    else {
        return Ok(None);
    };
    let Some(best_ask) = book
        .asks
        .iter()
        .filter(|level| level.price > Decimal::ZERO && level.size > Decimal::ZERO)
        .map(|level| level.price)
        .min()
    else {
        return Ok(None);
    };
    if best_ask < best_bid {
        return Ok(None);
    }
    let observed_at = offset_datetime_from_unix_millis(book.observed_at)?;
    let midpoint = ((best_bid + best_ask) / Decimal::from(2)).round_dp(8);
    let bucket_start = reward_candle_bucket_start(observed_at, interval_sec)?;
    Ok(Some(RewardMarketCandleSample {
        token_id: book.token_id.clone(),
        interval_sec,
        bucket_start,
        midpoint,
        best_bid,
        best_ask,
        spread_cents: ((best_ask - best_bid) * decimal("100")).round_dp(8),
        observed_at,
    }))
}

fn reward_candle_bucket_start(
    observed_at: OffsetDateTime,
    interval_sec: i32,
) -> Result<OffsetDateTime> {
    if interval_sec <= 0 {
        return Err(AppError::invalid_input(
            "REWARD_CANDLE_INTERVAL_INVALID",
            "reward market candle interval must be positive",
        ));
    }
    let interval = i64::from(interval_sec);
    let bucket = observed_at.unix_timestamp().div_euclid(interval) * interval;
    OffsetDateTime::from_unix_timestamp(bucket).map_err(|error| {
        AppError::invalid_input(
            "REWARD_CANDLE_BUCKET_INVALID",
            format!("failed to build reward candle bucket timestamp: {error}"),
        )
    })
}

fn offset_datetime_from_unix_millis(timestamp_ms: i64) -> Result<OffsetDateTime> {
    let seconds = timestamp_ms.div_euclid(1_000);
    let millis = timestamp_ms.rem_euclid(1_000);
    OffsetDateTime::from_unix_timestamp(seconds)
        .map(|time| time + TimeDuration::milliseconds(millis))
        .map_err(|error| {
            AppError::invalid_input(
                "REWARD_CANDLE_OBSERVED_AT_INVALID",
                format!("failed to decode orderbook observed_at milliseconds: {error}"),
            )
        })
}
