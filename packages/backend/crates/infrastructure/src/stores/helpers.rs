fn postgres_decode_error(error: sqlx::Error) -> AppError {
    db_error(
        "POSTGRES_DECODE_FAILED",
        format!("failed to decode postgres row: {error}"),
    )
}

fn apply_reward_config_value(config: &mut RewardBotConfig, key: &str, value: &str) -> Result<()> {
    match key {
        "enabled" => config.enabled = parse_bool_config(key, value)?,
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
        "account_capital_usd" => config.account_capital_usd = parse_decimal_config(key, value)?,
        "reward_competition_factor" => {
            config.reward_competition_factor = parse_decimal_config(key, value)?;
        }
        "single_sided_divisor_c" => {
            config.single_sided_divisor_c = parse_decimal_config(key, value)?;
        }
        "fill_rate_per_tick" => config.fill_rate_per_tick = parse_decimal_config(key, value)?,
        "max_fill_ratio" => config.max_fill_ratio = parse_decimal_config(key, value)?,
        "requote_drift_cents" => config.requote_drift_cents = parse_decimal_config(key, value)?,
        "post_fill_strategy" => config.post_fill_strategy = PostFillStrategy::from_str(value)?,
        _ => {}
    }
    Ok(())
}

fn reward_config_entries(config: &RewardBotConfig) -> Vec<(&'static str, String)> {
    vec![
        ("enabled", config.enabled.to_string()),
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
        (
            "account_capital_usd",
            config.account_capital_usd.to_string(),
        ),
        (
            "reward_competition_factor",
            config.reward_competition_factor.to_string(),
        ),
        (
            "single_sided_divisor_c",
            config.single_sided_divisor_c.to_string(),
        ),
        ("fill_rate_per_tick", config.fill_rate_per_tick.to_string()),
        ("max_fill_ratio", config.max_fill_ratio.to_string()),
        (
            "requote_drift_cents",
            config.requote_drift_cents.to_string(),
        ),
        (
            "post_fill_strategy",
            config.post_fill_strategy.as_str().to_string(),
        ),
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
          filled_size,
          reward_earned,
          last_scored_at,
          created_at,
          updated_at,
          trace_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
        ON CONFLICT (id) DO UPDATE
        SET external_order_id = EXCLUDED.external_order_id,
            status = EXCLUDED.status,
            scoring = EXCLUDED.scoring,
            reason = EXCLUDED.reason,
            filled_size = EXCLUDED.filled_size,
            reward_earned = EXCLUDED.reward_earned,
            last_scored_at = EXCLUDED.last_scored_at,
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
    .bind(order.filled_size)
    .bind(order.reward_earned)
    .bind(order.last_scored_at)
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

async fn insert_reward_fill(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    fill: &RewardFill,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO reward_fills (
          id, order_id, account_id, condition_id, token_id, outcome, side,
          price, size, notional_usd, role, realized_pnl, reason, trace_id, created_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(&fill.id)
    .bind(&fill.order_id)
    .bind(&fill.account_id)
    .bind(&fill.condition_id)
    .bind(&fill.token_id)
    .bind(&fill.outcome)
    .bind(fill.side.as_str())
    .bind(fill.price)
    .bind(fill.size)
    .bind(fill.notional_usd)
    .bind(fill.role.as_str())
    .bind(fill.realized_pnl)
    .bind(&fill.reason)
    .bind(&fill.trace_id)
    .bind(fill.created_at)
    .execute(&mut **transaction)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_INSERT_FAILED",
            format!("failed to insert reward fill: {error}"),
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
        filled_size: row.try_get("filled_size").map_err(postgres_decode_error)?,
        reward_earned: row.try_get("reward_earned").map_err(postgres_decode_error)?,
        last_scored_at: row.try_get("last_scored_at").map_err(postgres_decode_error)?,
        created_at: row.try_get("created_at").map_err(postgres_decode_error)?,
        updated_at: row.try_get("updated_at").map_err(postgres_decode_error)?,
    })
}

fn reward_fill_from_row(row: &sqlx::postgres::PgRow) -> Result<RewardFill> {
    let side_raw: String = row.try_get("side").map_err(postgres_decode_error)?;
    let role_raw: String = row.try_get("role").map_err(postgres_decode_error)?;
    Ok(RewardFill {
        id: row.try_get("id").map_err(postgres_decode_error)?,
        order_id: row.try_get("order_id").map_err(postgres_decode_error)?,
        account_id: row.try_get("account_id").map_err(postgres_decode_error)?,
        condition_id: row.try_get("condition_id").map_err(postgres_decode_error)?,
        token_id: row.try_get("token_id").map_err(postgres_decode_error)?,
        outcome: row.try_get("outcome").map_err(postgres_decode_error)?,
        side: RewardOrderSide::from_str(&side_raw)?,
        price: row.try_get("price").map_err(postgres_decode_error)?,
        size: row.try_get("size").map_err(postgres_decode_error)?,
        notional_usd: row.try_get("notional_usd").map_err(postgres_decode_error)?,
        role: RewardFillRole::from_str(&role_raw)?,
        realized_pnl: row.try_get("realized_pnl").map_err(postgres_decode_error)?,
        reason: row.try_get("reason").map_err(postgres_decode_error)?,
        trace_id: row.try_get("trace_id").map_err(postgres_decode_error)?,
        created_at: row.try_get("created_at").map_err(postgres_decode_error)?,
    })
}

fn reward_account_state_from_row(row: &sqlx::postgres::PgRow) -> Result<RewardAccountState> {
    Ok(RewardAccountState {
        account_id: row.try_get("account_id").map_err(postgres_decode_error)?,
        capital_usd: row.try_get("capital_usd").map_err(postgres_decode_error)?,
        available_usd: row.try_get("available_usd").map_err(postgres_decode_error)?,
        reserved_usd: row.try_get("reserved_usd").map_err(postgres_decode_error)?,
        realized_pnl: row.try_get("realized_pnl").map_err(postgres_decode_error)?,
        reward_earned_usd: row
            .try_get("reward_earned_usd")
            .map_err(postgres_decode_error)?,
        fees_paid: row.try_get("fees_paid").map_err(postgres_decode_error)?,
        tick_index: row.try_get("tick_index").map_err(postgres_decode_error)?,
        updated_at: row.try_get("updated_at").map_err(postgres_decode_error)?,
    })
}

const REWARD_ACCOUNT_STATE_UPSERT: &str = r#"
    INSERT INTO reward_account_state (
      account_id, capital_usd, available_usd, reserved_usd, realized_pnl,
      reward_earned_usd, fees_paid, tick_index, updated_at
    )
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
    ON CONFLICT (account_id) DO UPDATE
    SET capital_usd = EXCLUDED.capital_usd,
        available_usd = EXCLUDED.available_usd,
        reserved_usd = EXCLUDED.reserved_usd,
        realized_pnl = EXCLUDED.realized_pnl,
        reward_earned_usd = EXCLUDED.reward_earned_usd,
        fees_paid = EXCLUDED.fees_paid,
        tick_index = EXCLUDED.tick_index,
        updated_at = EXCLUDED.updated_at
"#;

fn bind_reward_account_state<'q>(
    query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    state: &'q RewardAccountState,
) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
    query
        .bind(&state.account_id)
        .bind(state.capital_usd)
        .bind(state.available_usd)
        .bind(state.reserved_usd)
        .bind(state.realized_pnl)
        .bind(state.reward_earned_usd)
        .bind(state.fees_paid)
        .bind(state.tick_index)
        .bind(state.updated_at)
}

async fn upsert_reward_account_state(pool: &PgPool, state: &RewardAccountState) -> Result<()> {
    bind_reward_account_state(sqlx::query(REWARD_ACCOUNT_STATE_UPSERT), state)
        .execute(pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!("failed to upsert reward account state: {error}"),
            )
        })?;
    Ok(())
}

async fn upsert_reward_account_state_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    state: &RewardAccountState,
) -> Result<()> {
    bind_reward_account_state(sqlx::query(REWARD_ACCOUNT_STATE_UPSERT), state)
        .execute(&mut **transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!("failed to upsert reward account state: {error}"),
            )
        })?;
    Ok(())
}

async fn upsert_reward_position_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    position: &RewardPosition,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO reward_positions (
          account_id, condition_id, token_id, outcome, size, avg_price, realized_pnl, updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (account_id, token_id) DO UPDATE
        SET condition_id = EXCLUDED.condition_id,
            outcome = EXCLUDED.outcome,
            size = EXCLUDED.size,
            avg_price = EXCLUDED.avg_price,
            realized_pnl = EXCLUDED.realized_pnl,
            updated_at = EXCLUDED.updated_at
        "#,
    )
    .bind(&position.account_id)
    .bind(&position.condition_id)
    .bind(&position.token_id)
    .bind(&position.outcome)
    .bind(position.size)
    .bind(position.avg_price)
    .bind(position.realized_pnl)
    .bind(position.updated_at)
    .execute(&mut **transaction)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_UPSERT_FAILED",
            format!("failed to upsert reward position: {error}"),
        )
    })?;
    Ok(())
}

async fn insert_reward_event_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event: &RewardRiskEvent,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO reward_risk_events (
          id, account_id, condition_id, external_order_id, event_type,
          severity, message, metadata_json, created_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(&event.id)
    .bind(&event.account_id)
    .bind(&event.condition_id)
    .bind(&event.external_order_id)
    .bind(&event.event_type)
    .bind(event.severity.as_str())
    .bind(&event.message)
    .bind(Json(event.metadata.clone()))
    .bind(event.created_at)
    .execute(&mut **transaction)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_INSERT_FAILED",
            format!("failed to insert reward risk event: {error}"),
        )
    })?;
    Ok(())
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

fn reward_control_command_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<RewardControlCommand> {
    let action_raw: String = row.try_get("action").map_err(postgres_decode_error)?;
    let status_raw: String = row.try_get("status").map_err(postgres_decode_error)?;
    Ok(RewardControlCommand {
        id: row.try_get("id").map_err(postgres_decode_error)?,
        action: RewardControlAction::from_str(&action_raw)?,
        account_id: row.try_get("account_id").map_err(postgres_decode_error)?,
        reason: row.try_get("reason").map_err(postgres_decode_error)?,
        status: RewardControlCommandStatus::from_str(&status_raw)?,
        requested_at: row.try_get("requested_at").map_err(postgres_decode_error)?,
        started_at: row.try_get("started_at").map_err(postgres_decode_error)?,
        completed_at: row.try_get("completed_at").map_err(postgres_decode_error)?,
        trace_id: row.try_get("trace_id").map_err(postgres_decode_error)?,
        error: row.try_get("error").map_err(postgres_decode_error)?,
    })
}
