async fn postgres_enqueue_reward_control_command(
    pool: &PgPool,
    command: RewardControlCommand,
) -> Result<bool> {
    let result = sqlx::query(
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
          lease_owner,
          lease_version,
          lease_expires_at,
          error
        )
        SELECT $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13
        WHERE NOT EXISTS (
          SELECT 1
          FROM reward_control_commands
          WHERE action = $2
            AND account_id IS NOT DISTINCT FROM $3
            AND status IN ('pending', 'running')
        )
        ON CONFLICT DO NOTHING
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
    .bind(&command.lease_owner)
    .bind(command.lease_version)
    .bind(command.lease_expires_at)
    .bind(&command.error)
    .execute(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_INSERT_FAILED",
            format!("failed to enqueue reward control command: {error}"),
        )
    })?;
    Ok(result.rows_affected() > 0)
}

async fn postgres_claim_next_reward_control_command(
    pool: &PgPool,
    trace_id: &str,
    now: OffsetDateTime,
) -> Result<Option<RewardControlCommand>> {
    let row = sqlx::query(
        r#"
        UPDATE reward_control_commands
        SET status = 'running',
            started_at = $1,
            completed_at = NULL,
            trace_id = $2,
            lease_owner = $2,
            lease_version = lease_version + 1,
            lease_expires_at = $3,
            error = NULL
        WHERE id = (
            SELECT id
            FROM reward_control_commands
            WHERE status = 'pending'
               OR (
                   status = 'running'
                   AND (lease_expires_at IS NULL OR lease_expires_at <= $1)
               )
            ORDER BY requested_at ASC
            LIMIT 1
            FOR UPDATE SKIP LOCKED
        )
        RETURNING id,
                  action,
                  account_id,
                  reason,
                  status,
                  requested_at,
                  started_at,
                  completed_at,
                  trace_id,
                  lease_owner,
                  lease_version,
                  lease_expires_at,
                  error
        "#,
    )
    .bind(now)
    .bind(trace_id)
    .bind(now + REWARD_CONTROL_COMMAND_LEASE)
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to query pending reward control command: {error}"),
        )
    })?;

    let Some(row) = row else {
        return Ok(None);
    };
    Ok(Some(reward_control_command_from_row(&row)?))
}

async fn postgres_complete_reward_control_command(
    pool: &PgPool,
    command_id: &str,
    lease_owner: &str,
    lease_version: i64,
    now: OffsetDateTime,
) -> Result<()> {
    let result = sqlx::query(
        r#"
        UPDATE reward_control_commands
        SET status = 'completed',
            completed_at = $2,
            lease_expires_at = NULL,
            error = NULL
        WHERE id = $1
          AND status = 'running'
          AND lease_owner = $3
          AND lease_version = $4
        "#,
    )
    .bind(command_id)
    .bind(now)
    .bind(lease_owner)
    .bind(lease_version)
    .execute(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_UPDATE_FAILED",
            format!("failed to complete reward control command: {error}"),
        )
    })?;
    if result.rows_affected() != 1 {
        return Err(AppError::conflict(
            "REWARD_CONTROL_LEASE_LOST",
            "reward control command completion was rejected because its lease was lost",
        ));
    }
    Ok(())
}

async fn postgres_fail_reward_control_command(
    pool: &PgPool,
    command_id: &str,
    lease_owner: &str,
    lease_version: i64,
    error: &str,
    now: OffsetDateTime,
) -> Result<()> {
    let result = sqlx::query(
        r#"
        UPDATE reward_control_commands
        SET status = 'failed',
            completed_at = $2,
            lease_expires_at = NULL,
            error = $5
        WHERE id = $1
          AND status = 'running'
          AND lease_owner = $3
          AND lease_version = $4
        "#,
    )
    .bind(command_id)
    .bind(now)
    .bind(lease_owner)
    .bind(lease_version)
    .bind(error)
    .execute(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_UPDATE_FAILED",
            format!("failed to fail reward control command: {error}"),
        )
    })?;
    if result.rows_affected() != 1 {
        return Err(AppError::conflict(
            "REWARD_CONTROL_LEASE_LOST",
            "reward control command failure was rejected because its lease was lost",
        ));
    }
    Ok(())
}
