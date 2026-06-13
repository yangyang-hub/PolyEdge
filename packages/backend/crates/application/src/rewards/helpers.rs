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
            "awaiting final reconciliation",
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
