fn postgres_decode_error(error: sqlx::Error) -> AppError {
    db_error(
        "POSTGRES_DECODE_FAILED",
        format!("failed to decode postgres row: {error}"),
    )
}

fn apply_reward_config_value(config: &mut RewardBotConfig, key: &str, value: &str) -> Result<()> {
    match key {
        "enabled" => config.enabled = parse_bool_config(key, value)?,
        "mode" => config.mode = RewardBotMode::from_str(value)?,
        "account_id" => config.account_id = value.to_string(),
        "max_markets" => config.max_markets = parse_u16_config(key, value)?,
        "max_open_orders" => config.max_open_orders = parse_u16_config(key, value)?,
        "per_market_usd" => config.per_market_usd = parse_decimal_config(key, value)?,
        "quote_size_usd" => config.quote_size_usd = parse_decimal_config(key, value)?,
        "min_daily_reward" => config.min_daily_reward = parse_decimal_config(key, value)?,
        "min_market_score" => config.min_market_score = parse_decimal_config(key, value)?,
        "max_spread_cents" => config.max_spread_cents = parse_decimal_config(key, value)?,
        "quote_edge_cents" => config.quote_edge_cents = parse_decimal_config(key, value)?,
        "safety_margin_cents" => config.safety_margin_cents = parse_decimal_config(key, value)?,
        "min_midpoint" => config.min_midpoint = parse_decimal_config(key, value)?,
        "max_midpoint" => config.max_midpoint = parse_decimal_config(key, value)?,
        "stale_book_ms" => config.stale_book_ms = parse_u64_config(key, value)?,
        "min_scoring_check_sec" => config.min_scoring_check_sec = parse_u64_config(key, value)?,
        "max_position_usd" => config.max_position_usd = parse_decimal_config(key, value)?,
        "max_global_position_usd" => {
            config.max_global_position_usd = parse_decimal_config(key, value)?;
        }
        "exit_markup_cents" => config.exit_markup_cents = parse_decimal_config(key, value)?,
        "cancel_on_fill" => config.cancel_on_fill = parse_bool_config(key, value)?,
        _ => {}
    }
    Ok(())
}

fn reward_config_entries(config: &RewardBotConfig) -> Vec<(&'static str, String)> {
    vec![
        ("enabled", config.enabled.to_string()),
        ("mode", config.mode.as_str().to_string()),
        ("account_id", config.account_id.clone()),
        ("max_markets", config.max_markets.to_string()),
        ("max_open_orders", config.max_open_orders.to_string()),
        ("per_market_usd", config.per_market_usd.to_string()),
        ("quote_size_usd", config.quote_size_usd.to_string()),
        ("min_daily_reward", config.min_daily_reward.to_string()),
        ("min_market_score", config.min_market_score.to_string()),
        ("max_spread_cents", config.max_spread_cents.to_string()),
        ("quote_edge_cents", config.quote_edge_cents.to_string()),
        (
            "safety_margin_cents",
            config.safety_margin_cents.to_string(),
        ),
        ("min_midpoint", config.min_midpoint.to_string()),
        ("max_midpoint", config.max_midpoint.to_string()),
        ("stale_book_ms", config.stale_book_ms.to_string()),
        (
            "min_scoring_check_sec",
            config.min_scoring_check_sec.to_string(),
        ),
        ("max_position_usd", config.max_position_usd.to_string()),
        (
            "max_global_position_usd",
            config.max_global_position_usd.to_string(),
        ),
        ("exit_markup_cents", config.exit_markup_cents.to_string()),
        ("cancel_on_fill", config.cancel_on_fill.to_string()),
    ]
}

fn parse_bool_config(key: &str, value: &str) -> Result<bool> {
    match value {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(AppError::invalid_input(
            "REWARD_CONFIG_BOOL_INVALID",
            format!("reward config key {key} must be a boolean"),
        )),
    }
}

fn parse_u16_config(key: &str, value: &str) -> Result<u16> {
    value.parse::<u16>().map_err(|error| {
        AppError::invalid_input(
            "REWARD_CONFIG_U16_INVALID",
            format!("reward config key {key} must be a u16: {error}"),
        )
    })
}

fn parse_u64_config(key: &str, value: &str) -> Result<u64> {
    value.parse::<u64>().map_err(|error| {
        AppError::invalid_input(
            "REWARD_CONFIG_U64_INVALID",
            format!("reward config key {key} must be a u64: {error}"),
        )
    })
}

fn parse_decimal_config(key: &str, value: &str) -> Result<Decimal> {
    Decimal::from_str(value).map_err(|error| {
        AppError::invalid_input(
            "REWARD_CONFIG_DECIMAL_INVALID",
            format!("reward config key {key} must be a decimal: {error}"),
        )
    })
}

async fn insert_reward_order(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    order: &ManagedRewardOrder,
    trace_id: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO reward_managed_orders (
          id,
          account_id,
          condition_id,
          token_id,
          outcome,
          side,
          price,
          size,
          external_order_id,
          status,
          scoring,
          reason,
          created_at,
          updated_at,
          trace_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        ON CONFLICT (id) DO UPDATE
        SET external_order_id = EXCLUDED.external_order_id,
            status = EXCLUDED.status,
            scoring = EXCLUDED.scoring,
            reason = EXCLUDED.reason,
            updated_at = EXCLUDED.updated_at,
            trace_id = EXCLUDED.trace_id
        "#,
    )
    .bind(&order.id)
    .bind(&order.account_id)
    .bind(&order.condition_id)
    .bind(&order.token_id)
    .bind(&order.outcome)
    .bind(order.side.as_str())
    .bind(order.price)
    .bind(order.size)
    .bind(&order.external_order_id)
    .bind(order.status.as_str())
    .bind(order.scoring)
    .bind(&order.reason)
    .bind(order.created_at)
    .bind(order.updated_at)
    .bind(trace_id)
    .execute(&mut **transaction)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_INSERT_FAILED",
            format!("failed to insert reward managed order: {error}"),
        )
    })?;
    Ok(())
}

fn reward_market_from_row(row: &sqlx::postgres::PgRow) -> Result<RewardMarket> {
    let tokens: Json<Vec<RewardToken>> =
        row.try_get("tokens_json").map_err(postgres_decode_error)?;
    Ok(RewardMarket {
        condition_id: row.try_get("condition_id").map_err(postgres_decode_error)?,
        question: row.try_get("question").map_err(postgres_decode_error)?,
        market_slug: row.try_get("market_slug").map_err(postgres_decode_error)?,
        event_slug: row.try_get("event_slug").map_err(postgres_decode_error)?,
        image: row.try_get("image").map_err(postgres_decode_error)?,
        rewards_max_spread: row
            .try_get("rewards_max_spread")
            .map_err(postgres_decode_error)?,
        rewards_min_size: row
            .try_get("rewards_min_size")
            .map_err(postgres_decode_error)?,
        total_daily_rate: row
            .try_get("total_daily_rate")
            .map_err(postgres_decode_error)?,
        tokens: tokens.0,
        active: row.try_get("active").map_err(postgres_decode_error)?,
        updated_at: row.try_get("updated_at").map_err(postgres_decode_error)?,
    })
}

fn reward_order_from_row(row: &sqlx::postgres::PgRow) -> Result<ManagedRewardOrder> {
    let side_raw: String = row.try_get("side").map_err(postgres_decode_error)?;
    let status_raw: String = row.try_get("status").map_err(postgres_decode_error)?;
    Ok(ManagedRewardOrder {
        id: row.try_get("id").map_err(postgres_decode_error)?,
        account_id: row.try_get("account_id").map_err(postgres_decode_error)?,
        condition_id: row.try_get("condition_id").map_err(postgres_decode_error)?,
        token_id: row.try_get("token_id").map_err(postgres_decode_error)?,
        outcome: row.try_get("outcome").map_err(postgres_decode_error)?,
        side: RewardOrderSide::from_str(&side_raw)?,
        price: row.try_get("price").map_err(postgres_decode_error)?,
        size: row.try_get("size").map_err(postgres_decode_error)?,
        external_order_id: row
            .try_get("external_order_id")
            .map_err(postgres_decode_error)?,
        status: ManagedRewardOrderStatus::from_str(&status_raw)?,
        scoring: row.try_get("scoring").map_err(postgres_decode_error)?,
        reason: row.try_get("reason").map_err(postgres_decode_error)?,
        created_at: row.try_get("created_at").map_err(postgres_decode_error)?,
        updated_at: row.try_get("updated_at").map_err(postgres_decode_error)?,
    })
}

fn reward_position_from_row(row: &sqlx::postgres::PgRow) -> Result<RewardPosition> {
    Ok(RewardPosition {
        account_id: row.try_get("account_id").map_err(postgres_decode_error)?,
        condition_id: row.try_get("condition_id").map_err(postgres_decode_error)?,
        token_id: row.try_get("token_id").map_err(postgres_decode_error)?,
        outcome: row.try_get("outcome").map_err(postgres_decode_error)?,
        size: row.try_get("size").map_err(postgres_decode_error)?,
        avg_price: row.try_get("avg_price").map_err(postgres_decode_error)?,
        realized_pnl: row.try_get("realized_pnl").map_err(postgres_decode_error)?,
        updated_at: row.try_get("updated_at").map_err(postgres_decode_error)?,
    })
}

fn reward_event_from_row(row: &sqlx::postgres::PgRow) -> Result<RewardRiskEvent> {
    let severity_raw: String = row.try_get("severity").map_err(postgres_decode_error)?;
    let metadata: Json<Value> = row
        .try_get("metadata_json")
        .map_err(postgres_decode_error)?;
    Ok(RewardRiskEvent {
        id: row.try_get("id").map_err(postgres_decode_error)?,
        account_id: row.try_get("account_id").map_err(postgres_decode_error)?,
        condition_id: row.try_get("condition_id").map_err(postgres_decode_error)?,
        external_order_id: row
            .try_get("external_order_id")
            .map_err(postgres_decode_error)?,
        event_type: row.try_get("event_type").map_err(postgres_decode_error)?,
        severity: RewardRiskSeverity::from_str(&severity_raw)?,
        message: row.try_get("message").map_err(postgres_decode_error)?,
        metadata: metadata.0,
        created_at: row.try_get("created_at").map_err(postgres_decode_error)?,
    })
}
