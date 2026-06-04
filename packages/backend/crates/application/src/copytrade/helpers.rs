#[must_use]
pub fn validate_copytrade_list_limit(limit: Option<u16>) -> u16 {
    limit.unwrap_or(DEFAULT_LIST_LIMIT).clamp(1, MAX_LIST_LIMIT)
}

#[must_use]
pub fn new_copy_event(
    wallet_address: Option<String>,
    condition_id: Option<String>,
    event_type: impl Into<String>,
    severity: CopyEventSeverity,
    message: impl Into<String>,
    metadata: Value,
) -> CopyEvent {
    let now = OffsetDateTime::now_utc();
    CopyEvent {
        id: format!("ct_evt_{}", now.unix_timestamp_nanos()),
        wallet_address,
        condition_id,
        event_type: event_type.into(),
        severity,
        message: message.into(),
        metadata,
        created_at: now,
    }
}

/// Normalize a 0x-prefixed 40-hex-char Ethereum address to lowercase.
/// Returns `None` if the address is structurally invalid.
#[must_use]
pub fn normalize_address(address: &str) -> Option<String> {
    let trimmed = address.trim().to_lowercase();
    let is_valid = trimmed.len() == 42
        && trimmed.starts_with("0x")
        && trimmed[2..].chars().all(|character| character.is_ascii_hexdigit());
    if is_valid {
        Some(trimmed)
    } else {
        None
    }
}

fn normalize_account_id(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "copytrade_simulator".to_string()
    } else {
        trimmed.to_string()
    }
}

fn clamp_decimal(value: Decimal, min: Decimal, max: Decimal) -> Decimal {
    Decimal::min(max, Decimal::max(min, value))
}

fn decimal(value: &str) -> Decimal {
    Decimal::from_str_exact(value).expect("static copytrade configuration default must be valid")
}

/// Generate a deterministic source trade ID from the wallet/tx/token/side/price/size/timestamp
/// so that re-scans of the same Data API page produce the same ID and dedup correctly,
/// while still distinguishing multiple distinct fills that share a tx hash and second
/// (price/size are immutable per execution, so re-scans stay idempotent).
fn source_trade_id(
    wallet_address: &str,
    tx_hash: &str,
    token_id: &str,
    side: &str,
    price: Decimal,
    size: Decimal,
    timestamp_secs: i64,
) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    wallet_address.hash(&mut hasher);
    tx_hash.hash(&mut hasher);
    token_id.hash(&mut hasher);
    side.hash(&mut hasher);
    // Normalize scale so "0.50" and "0.5" hash identically across re-scans.
    price.normalize().to_string().hash(&mut hasher);
    size.normalize().to_string().hash(&mut hasher);
    timestamp_secs.hash(&mut hasher);
    let hash = hasher.finish();
    format!("ct_st_{hash:016x}")
}

fn new_copy_order_id() -> String {
    let now = OffsetDateTime::now_utc();
    format!("ct_ord_{}", now.unix_timestamp_nanos())
}

fn new_copy_fill_id() -> String {
    let now = OffsetDateTime::now_utc();
    format!("ct_fill_{}", now.unix_timestamp_nanos())
}
