async fn postgres_enqueue_copytrade_control_command(
    pool: &PgPool,
    command: CopyControlCommand,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO copytrade_control_commands (
          id,
          action,
          account_id,
          reason,
          status,
          requested_at,
          started_at,
          completed_at,
          trace_id,
          error
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        "#,
    )
    .bind(&command.id)
    .bind(command.action.as_str())
    .bind(&command.account_id)
    .bind(&command.reason)
    .bind(command.status.as_str())
    .bind(command.requested_at)
    .bind(command.started_at)
    .bind(command.completed_at)
    .bind(&command.trace_id)
    .bind(&command.error)
    .execute(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_INSERT_FAILED",
            format!("failed to enqueue copytrade control command: {error}"),
        )
    })?;
    Ok(())
}

async fn postgres_claim_next_copytrade_control_command(
    pool: &PgPool,
    trace_id: &str,
    now: OffsetDateTime,
) -> Result<Option<CopyControlCommand>> {
    let mut transaction = pool.begin().await.map_err(|error| {
        db_error(
            "POSTGRES_TRANSACTION_BEGIN_FAILED",
            format!("failed to begin copytrade control command transaction: {error}"),
        )
    })?;

    let row = sqlx::query(
        r#"
        SELECT id,
               action,
               account_id,
               reason,
               status,
               requested_at,
               started_at,
               completed_at,
               trace_id,
               error
        FROM copytrade_control_commands
        WHERE status = 'pending'
        ORDER BY requested_at ASC
        LIMIT 1
        FOR UPDATE SKIP LOCKED
        "#,
    )
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to query pending copytrade control command: {error}"),
        )
    })?;

    let Some(row) = row else {
        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit copytrade control command transaction: {error}"),
            )
        })?;
        return Ok(None);
    };
    let command = copytrade_control_command_from_row(&row)?;

    sqlx::query(
        r#"
        UPDATE copytrade_control_commands
        SET status = 'running',
            started_at = $2,
            trace_id = $3,
            error = NULL
        WHERE id = $1
        "#,
    )
    .bind(&command.id)
    .bind(now)
    .bind(trace_id)
    .execute(&mut *transaction)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_UPDATE_FAILED",
            format!("failed to claim copytrade control command: {error}"),
        )
    })?;

    transaction.commit().await.map_err(|error| {
        db_error(
            "POSTGRES_TRANSACTION_COMMIT_FAILED",
            format!("failed to commit copytrade control command transaction: {error}"),
        )
    })?;

    Ok(Some(CopyControlCommand {
        status: CopyControlCommandStatus::Running,
        started_at: Some(now),
        trace_id: Some(trace_id.to_string()),
        error: None,
        ..command
    }))
}

async fn postgres_complete_copytrade_control_command(
    pool: &PgPool,
    command_id: &str,
    trace_id: &str,
    now: OffsetDateTime,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE copytrade_control_commands
        SET status = 'completed',
            completed_at = $2,
            trace_id = $3,
            error = NULL
        WHERE id = $1
        "#,
    )
    .bind(command_id)
    .bind(now)
    .bind(trace_id)
    .execute(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_UPDATE_FAILED",
            format!("failed to complete copytrade control command: {error}"),
        )
    })?;
    Ok(())
}

async fn postgres_fail_copytrade_control_command(
    pool: &PgPool,
    command_id: &str,
    trace_id: &str,
    error: &str,
    now: OffsetDateTime,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE copytrade_control_commands
        SET status = 'failed',
            completed_at = $2,
            trace_id = $3,
            error = $4
        WHERE id = $1
        "#,
    )
    .bind(command_id)
    .bind(now)
    .bind(trace_id)
    .bind(error)
    .execute(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_UPDATE_FAILED",
            format!("failed to fail copytrade control command: {error}"),
        )
    })?;
    Ok(())
}
