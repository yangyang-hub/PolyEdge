async fn postgres_record_reward_worker_heartbeat(
    pool: &PgPool,
    account_id: &str,
    observed_at: OffsetDateTime,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO reward_worker_heartbeats (account_id, observed_at)
        VALUES ($1, $2)
        ON CONFLICT (account_id) DO UPDATE
        SET observed_at = EXCLUDED.observed_at
        "#,
    )
    .bind(account_id)
    .bind(observed_at)
    .execute(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_REWARD_HEARTBEAT_UPSERT_FAILED",
            format!("failed to record rewards worker heartbeat: {error}"),
        )
    })?;
    Ok(())
}

async fn postgres_latest_reward_worker_heartbeat(
    pool: &PgPool,
    account_id: &str,
) -> Result<Option<OffsetDateTime>> {
    sqlx::query_scalar(
        r#"
        SELECT observed_at
        FROM reward_worker_heartbeats
        WHERE account_id = $1
        "#,
    )
    .bind(account_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_REWARD_HEARTBEAT_QUERY_FAILED",
            format!("failed to query rewards worker heartbeat: {error}"),
        )
    })
}
