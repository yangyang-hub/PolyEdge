fn apply_smart_money_config_value(
    config: &mut SmartMoneyConfig,
    key: &str,
    value: &str,
) -> Result<()> {
    match key {
        "enabled" => config.enabled = parse_smart_bool_config(key, value)?,
        "mode" => config.mode = SmartMoneyMode::from_str(value)?,
        "discovery_enabled" => config.discovery_enabled = parse_smart_bool_config(key, value)?,
        "wallet_advisory_enabled" => {
            config.wallet_advisory_enabled = parse_smart_bool_config(key, value)?;
        }
        "signal_advisory_enabled" => {
            config.signal_advisory_enabled = parse_smart_bool_config(key, value)?;
        }
        "signal_advisory_provider" => {
            config.signal_advisory_provider = RewardAiProvider::from_str(value)?;
        }
        "signal_advisory_request_format" => {
            config.signal_advisory_request_format = RewardAiRequestFormat::from_str(value)?;
        }
        "signal_advisory_model" => {
            config.signal_advisory_model = value.trim().to_string();
        }
        "signal_advisory_concurrency_enabled" => {
            config.signal_advisory_concurrency_enabled = parse_smart_bool_config(key, value)?;
        }
        "signal_advisory_max_concurrency" => {
            config.signal_advisory_max_concurrency = parse_smart_u16_config(key, value)?;
        }
        "min_trade_count" => config.min_trade_count = parse_smart_i64_config(key, value)?,
        "min_settled_trade_count" => {
            config.min_settled_trade_count = parse_smart_i64_config(key, value)?;
        }
        "min_total_volume_usd" => {
            config.min_total_volume_usd = parse_smart_decimal_config(key, value)?;
        }
        "min_copyability_score" => {
            config.min_copyability_score = parse_smart_decimal_config(key, value)?;
        }
        "max_signal_age_ms" => config.max_signal_age_ms = parse_smart_i64_config(key, value)?,
        "max_price_slippage_cents" => {
            config.max_price_slippage_cents = parse_smart_decimal_config(key, value)?;
        }
        "min_orderbook_depth_usd" => {
            config.min_orderbook_depth_usd = parse_smart_decimal_config(key, value)?;
        }
        "max_wallet_exposure_usd" => {
            config.max_wallet_exposure_usd = parse_smart_decimal_config(key, value)?;
        }
        "max_market_exposure_usd" => {
            config.max_market_exposure_usd = parse_smart_decimal_config(key, value)?;
        }
        "max_daily_notional_usd" => {
            config.max_daily_notional_usd = parse_smart_decimal_config(key, value)?;
        }
        _ => {}
    }
    Ok(())
}

fn smart_money_config_entries(config: &SmartMoneyConfig) -> Vec<(String, String)> {
    vec![
        ("enabled".to_string(), config.enabled.to_string()),
        ("mode".to_string(), config.mode.as_str().to_string()),
        (
            "discovery_enabled".to_string(),
            config.discovery_enabled.to_string(),
        ),
        (
            "wallet_advisory_enabled".to_string(),
            config.wallet_advisory_enabled.to_string(),
        ),
        (
            "signal_advisory_enabled".to_string(),
            config.signal_advisory_enabled.to_string(),
        ),
        (
            "signal_advisory_provider".to_string(),
            config.signal_advisory_provider.as_str().to_string(),
        ),
        (
            "signal_advisory_request_format".to_string(),
            config.signal_advisory_request_format.as_str().to_string(),
        ),
        (
            "signal_advisory_model".to_string(),
            config.signal_advisory_model.clone(),
        ),
        (
            "signal_advisory_concurrency_enabled".to_string(),
            config.signal_advisory_concurrency_enabled.to_string(),
        ),
        (
            "signal_advisory_max_concurrency".to_string(),
            config.signal_advisory_max_concurrency.to_string(),
        ),
        (
            "min_trade_count".to_string(),
            config.min_trade_count.to_string(),
        ),
        (
            "min_settled_trade_count".to_string(),
            config.min_settled_trade_count.to_string(),
        ),
        (
            "min_total_volume_usd".to_string(),
            config.min_total_volume_usd.to_string(),
        ),
        (
            "min_copyability_score".to_string(),
            config.min_copyability_score.to_string(),
        ),
        (
            "max_signal_age_ms".to_string(),
            config.max_signal_age_ms.to_string(),
        ),
        (
            "max_price_slippage_cents".to_string(),
            config.max_price_slippage_cents.to_string(),
        ),
        (
            "min_orderbook_depth_usd".to_string(),
            config.min_orderbook_depth_usd.to_string(),
        ),
        (
            "max_wallet_exposure_usd".to_string(),
            config.max_wallet_exposure_usd.to_string(),
        ),
        (
            "max_market_exposure_usd".to_string(),
            config.max_market_exposure_usd.to_string(),
        ),
        (
            "max_daily_notional_usd".to_string(),
            config.max_daily_notional_usd.to_string(),
        ),
    ]
}

fn parse_smart_bool_config(key: &str, value: &str) -> Result<bool> {
    value.parse::<bool>().map_err(|error| {
        AppError::invalid_input(
            "SMART_MONEY_CONFIG_INVALID",
            format!("invalid boolean smart money config {key}: {error}"),
        )
    })
}

fn parse_smart_i64_config(key: &str, value: &str) -> Result<i64> {
    value.parse::<i64>().map_err(|error| {
        AppError::invalid_input(
            "SMART_MONEY_CONFIG_INVALID",
            format!("invalid integer smart money config {key}: {error}"),
        )
    })
}

fn parse_smart_u16_config(key: &str, value: &str) -> Result<u16> {
    value.parse::<u16>().map_err(|error| {
        AppError::invalid_input(
            "SMART_MONEY_CONFIG_INVALID",
            format!("invalid unsigned integer smart money config {key}: {error}"),
        )
    })
}

fn parse_smart_decimal_config(key: &str, value: &str) -> Result<Decimal> {
    Decimal::from_str_exact(value).map_err(|error| {
        AppError::invalid_input(
            "SMART_MONEY_CONFIG_INVALID",
            format!("invalid decimal smart money config {key}: {error}"),
        )
    })
}
