const COPYTRADE_ACCOUNT_STATE_UPSERT: &str = r#"
    INSERT INTO copytrade_account_state (
      account_id, capital_usd, available_usd, reserved_usd, realized_pnl,
      daily_realized_pnl, fees_paid, tick_index, updated_at
    )
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
    ON CONFLICT (account_id) DO UPDATE
    SET capital_usd = EXCLUDED.capital_usd,
        available_usd = EXCLUDED.available_usd,
        reserved_usd = EXCLUDED.reserved_usd,
        realized_pnl = EXCLUDED.realized_pnl,
        daily_realized_pnl = EXCLUDED.daily_realized_pnl,
        fees_paid = EXCLUDED.fees_paid,
        tick_index = EXCLUDED.tick_index,
        updated_at = EXCLUDED.updated_at
"#;

fn bind_copytrade_account_state<'q>(
    query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    state: &'q CopyAccountState,
) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
    query
        .bind(&state.account_id)
        .bind(state.capital_usd)
        .bind(state.available_usd)
        .bind(state.reserved_usd)
        .bind(state.realized_pnl)
        .bind(state.daily_realized_pnl)
        .bind(state.fees_paid)
        .bind(state.tick_index)
        .bind(state.updated_at)
}

async fn upsert_copytrade_account_state(pool: &PgPool, state: &CopyAccountState) -> Result<()> {
    bind_copytrade_account_state(sqlx::query(COPYTRADE_ACCOUNT_STATE_UPSERT), state)
        .execute(pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!("failed to upsert copytrade account state: {error}"),
            )
        })?;
    Ok(())
}

async fn upsert_copytrade_account_state_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    state: &CopyAccountState,
) -> Result<()> {
    bind_copytrade_account_state(sqlx::query(COPYTRADE_ACCOUNT_STATE_UPSERT), state)
        .execute(&mut **transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!("failed to upsert copytrade account state: {error}"),
            )
        })?;
    Ok(())
}

async fn upsert_copytrade_position_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    position: &CopyPosition,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO copytrade_positions (
          account_id, wallet_address, condition_id, token_id, outcome,
          size, avg_price, realized_pnl, updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        ON CONFLICT (account_id, token_id) DO UPDATE
        SET wallet_address = EXCLUDED.wallet_address,
            condition_id = EXCLUDED.condition_id,
            outcome = EXCLUDED.outcome,
            size = EXCLUDED.size,
            avg_price = EXCLUDED.avg_price,
            realized_pnl = EXCLUDED.realized_pnl,
            updated_at = EXCLUDED.updated_at
        "#,
    )
    .bind(&position.account_id)
    .bind(&position.wallet_address)
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
            format!("failed to upsert copytrade position: {error}"),
        )
    })?;
    Ok(())
}

async fn insert_copytrade_order(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    order: &CopyOrder,
    trace_id: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO copytrade_orders (
          id, account_id, wallet_address, source_trade_id, condition_id,
          token_id, outcome, side, price, size, notional_usd,
          external_order_id, status, reason, filled_size, realized_pnl,
          created_at, updated_at, trace_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)
        ON CONFLICT (id) DO UPDATE
        SET external_order_id = EXCLUDED.external_order_id,
            status = EXCLUDED.status,
            reason = EXCLUDED.reason,
            filled_size = EXCLUDED.filled_size,
            realized_pnl = EXCLUDED.realized_pnl,
            updated_at = EXCLUDED.updated_at,
            trace_id = EXCLUDED.trace_id
        "#,
    )
    .bind(&order.id)
    .bind(&order.account_id)
    .bind(&order.wallet_address)
    .bind(&order.source_trade_id)
    .bind(&order.condition_id)
    .bind(&order.token_id)
    .bind(&order.outcome)
    .bind(order.side.as_str())
    .bind(order.price)
    .bind(order.size)
    .bind(order.notional_usd)
    .bind(&order.external_order_id)
    .bind(order.status.as_str())
    .bind(&order.reason)
    .bind(order.filled_size)
    .bind(order.realized_pnl)
    .bind(order.created_at)
    .bind(order.updated_at)
    .bind(trace_id)
    .execute(&mut **transaction)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_INSERT_FAILED",
            format!("failed to insert copytrade order: {error}"),
        )
    })?;
    Ok(())
}

async fn insert_copytrade_fill(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    fill: &CopyFill,
) -> Result<()> {
    // Fills are persisted as events (not a separate table in this schema).
    insert_copytrade_event_tx(
        transaction,
        &CopyEvent {
            id: fill.id.clone(),
            wallet_address: Some(fill.wallet_address.clone()),
            condition_id: Some(fill.condition_id.clone()),
            event_type: "copytrade_fill".to_string(),
            severity: CopyEventSeverity::Info,
            message: format!(
                "Filled {} {} @ {} ({})",
                fill.size, fill.token_id, fill.price, fill.reason
            ),
            metadata: serde_json::json!({
                "order_id": fill.order_id,
                "token_id": fill.token_id,
                "side": fill.side.as_str(),
                "price": fill.price,
                "size": fill.size,
                "notional_usd": fill.notional_usd,
                "realized_pnl": fill.realized_pnl,
                "reason": fill.reason,
            }),
            created_at: fill.created_at,
        },
    )
    .await
}

async fn insert_copytrade_event_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event: &CopyEvent,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO copytrade_events (
          id, wallet_address, condition_id, event_type,
          severity, message, metadata_json, created_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(&event.id)
    .bind(&event.wallet_address)
    .bind(&event.condition_id)
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
            format!("failed to insert copytrade event: {error}"),
        )
    })?;
    Ok(())
}
