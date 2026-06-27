pub fn validate_smart_money_list_limit(limit: Option<u16>) -> u16 {
    limit
        .unwrap_or(DEFAULT_SMART_MONEY_LIST_LIMIT)
        .clamp(1, MAX_SMART_MONEY_LIST_LIMIT)
}

pub fn normalize_smart_wallet_address(address: &str) -> Result<String> {
    let trimmed = address.trim();
    let Some(rest) = trimmed.strip_prefix("0x") else {
        return Err(AppError::invalid_input(
            "SMART_WALLET_ADDRESS_INVALID",
            "wallet address must be a 0x-prefixed 40-hex string",
        ));
    };
    if rest.len() != 40 || !rest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(AppError::invalid_input(
            "SMART_WALLET_ADDRESS_INVALID",
            "wallet address must be a 0x-prefixed 40-hex string",
        ));
    }
    Ok(format!("0x{}", rest.to_ascii_lowercase()))
}

fn clamp_unit_decimal(value: Decimal) -> Decimal {
    value.clamp(Decimal::ZERO, Decimal::ONE)
}
