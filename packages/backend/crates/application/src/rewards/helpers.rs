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
    if raw <= Decimal::ZERO {
        return Decimal::ZERO;
    }

    if raw > decimal("10") {
        raw / decimal("100")
    } else {
        raw
    }
}

fn floor_to_tick(value: Decimal, tick: Decimal) -> Decimal {
    (value / tick).floor() * tick
}

fn normalize_account_id(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "reward_simulator".to_string()
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

fn decimal_from_f64(value: f64) -> Decimal {
    if !value.is_finite() {
        return Decimal::ZERO;
    }

    Decimal::from_str(&format!("{value:.6}")).unwrap_or(Decimal::ZERO)
}

fn decimal_to_f64(value: Decimal) -> f64 {
    value.to_string().parse::<f64>().unwrap_or(0.0)
}
