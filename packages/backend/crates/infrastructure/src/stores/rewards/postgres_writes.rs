async fn release_reward_reserve_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    account_id: &str,
    requested_release: Decimal,
    now: OffsetDateTime,
) -> Result<()> {
    if requested_release <= Decimal::ZERO {
        return Ok(());
    }

    let row = sqlx::query(
        r#"
        SELECT reserved_usd
        FROM reward_account_state
        WHERE account_id = $1
        FOR UPDATE
        "#,
    )
    .bind(account_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to lock reward account state: {error}"),
        )
    })?;

    let Some(row) = row else {
        return Ok(());
    };
    let reserved_usd: Decimal = row.try_get("reserved_usd").map_err(postgres_decode_error)?;
    let release_usd = Decimal::min(reserved_usd, requested_release);
    if release_usd <= Decimal::ZERO {
        return Ok(());
    }

    sqlx::query(
        r#"
        UPDATE reward_account_state
        SET available_usd = available_usd + $2,
            reserved_usd = reserved_usd - $2,
            updated_at = $3
        WHERE account_id = $1
        "#,
    )
    .bind(account_id)
    .bind(release_usd)
    .bind(now)
    .execute(&mut **transaction)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_UPDATE_FAILED",
            format!("failed to release reward account reserve: {error}"),
        )
    })?;

    Ok(())
}
