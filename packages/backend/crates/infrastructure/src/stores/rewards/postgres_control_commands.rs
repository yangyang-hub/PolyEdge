async fn postgres_enqueue_reward_control_command(
    pool: &PgPool,
    command: RewardControlCommand,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO reward_control_commands (
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
            format!("failed to enqueue reward control command: {error}"),
        )
    })?;
    Ok(())
}

async fn postgres_claim_next_reward_control_command(
    pool: &PgPool,
    trace_id: &str,
    now: OffsetDateTime,
) -> Result<Option<RewardControlCommand>> {
    let mut transaction = pool.begin().await.map_err(|error| {
        db_error(
            "POSTGRES_TRANSACTION_BEGIN_FAILED",
            format!("failed to begin reward control command transaction: {error}"),
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
        FROM reward_control_commands
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
            format!("failed to query pending reward control command: {error}"),
        )
    })?;

    let Some(row) = row else {
        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit reward control command transaction: {error}"),
            )
        })?;
        return Ok(None);
    };
    let command = reward_control_command_from_row(&row)?;

    sqlx::query(
        r#"
        UPDATE reward_control_commands
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
            format!("failed to claim reward control command: {error}"),
        )
    })?;

    transaction.commit().await.map_err(|error| {
        db_error(
            "POSTGRES_TRANSACTION_COMMIT_FAILED",
            format!("failed to commit reward control command transaction: {error}"),
        )
    })?;

    Ok(Some(RewardControlCommand {
        status: RewardControlCommandStatus::Running,
        started_at: Some(now),
        trace_id: Some(trace_id.to_string()),
        error: None,
        ..command
    }))
}

async fn postgres_complete_reward_control_command(
    pool: &PgPool,
    command_id: &str,
    trace_id: &str,
    now: OffsetDateTime,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE reward_control_commands
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
            format!("failed to complete reward control command: {error}"),
        )
    })?;
    Ok(())
}

async fn postgres_fail_reward_control_command(
    pool: &PgPool,
    command_id: &str,
    trace_id: &str,
    error: &str,
    now: OffsetDateTime,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE reward_control_commands
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
            format!("failed to fail reward control command: {error}"),
        )
    })?;
    Ok(())
}
