use super::*;

pub(super) fn page_values(query: &ManualTradingListQuery) -> (i64, i64) {
    let page = query.page.unwrap_or(1).clamp(1, 1_000_000);
    let page_size = query.page_size.unwrap_or(50).clamp(1, 200);
    let offset = (page.saturating_sub(1)).saturating_mul(page_size);
    (page_size as i64, offset as i64)
}

pub(super) fn enum_value<T>(raw: String, column: &'static str) -> Result<T>
where
    T: FromStr<Err = polyedge_domain::AppError>,
{
    T::from_str(&raw).map_err(|error| {
        ServerError::Internal(format!("invalid {column} stored in database: {error}"))
    })
}

pub(super) fn required_text(value: &str, field: &'static str, max_len: usize) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > max_len {
        return Err(ServerError::InvalidInput(format!(
            "{field} must contain 1..={max_len} characters"
        )));
    }
    Ok(value.to_string())
}

pub(super) fn optional_note(value: Option<&str>) -> Result<Option<String>> {
    let value = value.map(str::trim).filter(|value| !value.is_empty());
    if value.is_some_and(|value| value.contains(['\r', '\n']) || value.len() > 500) {
        return Err(ServerError::InvalidInput(
            "operator_note must be a single line with at most 500 characters".to_string(),
        ));
    }
    Ok(value.map(ToOwned::to_owned))
}

pub(super) fn validate_price(value: Decimal, field: &'static str) -> Result<()> {
    if value <= Decimal::ZERO || value >= Decimal::ONE {
        return Err(ServerError::InvalidInput(format!(
            "{field} must be between 0 and 1"
        )));
    }
    Ok(())
}

impl PostgresStore {
    pub async fn begin_idempotency(
        &self,
        scope: &str,
        key: &str,
        request_hash: &str,
    ) -> Result<IdempotencyBegin> {
        let scope = required_text(scope, "idempotency scope", 120)?;
        let key = required_text(key, "idempotency key", 200)?;
        let owner_token = format!("idem_{}", uuid::Uuid::now_v7());
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            r#"
            INSERT INTO idempotency_keys (
              scope, idempotency_key, request_hash, owner_token, status,
              lease_expires_at, expires_at
            ) VALUES ($1, $2, $3, $4, 'started', now() + interval '30 seconds', now() + interval '24 hours')
            ON CONFLICT (scope, idempotency_key) DO NOTHING
            "#,
        )
        .bind(&scope)
        .bind(&key)
        .bind(request_hash)
        .bind(&owner_token)
        .execute(&mut *tx)
        .await?;
        let row = sqlx::query(
            r#"
            SELECT request_hash, owner_token, status, response_json, lease_expires_at
            FROM idempotency_keys
            WHERE scope = $1 AND idempotency_key = $2
            FOR UPDATE
            "#,
        )
        .bind(&scope)
        .bind(&key)
        .fetch_one(&mut *tx)
        .await?;
        let stored_hash: String = row.try_get("request_hash")?;
        if stored_hash != request_hash {
            return Err(ServerError::Conflict(
                "idempotency key was already used with another payload".to_string(),
            ));
        }
        let status: String = row.try_get("status")?;
        let stored_owner: String = row.try_get("owner_token")?;
        if status == "completed" {
            let response = row
                .try_get::<Option<serde_json::Value>, _>("response_json")?
                .ok_or_else(|| {
                    ServerError::Internal("completed idempotency row has no response".to_string())
                })?;
            tx.commit().await?;
            return Ok(IdempotencyBegin::Replay(response));
        }
        if stored_owner != owner_token {
            let lease_expires_at: Option<OffsetDateTime> = row.try_get("lease_expires_at")?;
            if lease_expires_at.is_some_and(|expiry| expiry > OffsetDateTime::now_utc()) {
                return Err(ServerError::Conflict(
                    "an identical request is already executing".to_string(),
                ));
            }
            sqlx::query(
                r#"
                UPDATE idempotency_keys
                SET owner_token = $3, status = 'started', response_json = NULL,
                    error_code = NULL, lease_epoch = lease_epoch + 1,
                    lease_expires_at = now() + interval '30 seconds', updated_at = now()
                WHERE scope = $1 AND idempotency_key = $2
                "#,
            )
            .bind(&scope)
            .bind(&key)
            .bind(&owner_token)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(IdempotencyBegin::Started { owner_token })
    }

    pub async fn complete_idempotency(
        &self,
        scope: &str,
        key: &str,
        owner_token: &str,
        response: &serde_json::Value,
    ) -> Result<()> {
        let result = sqlx::query(
            r#"
            UPDATE idempotency_keys
            SET status = 'completed', response_json = $4, completed_at = now(),
                updated_at = now(), lease_expires_at = NULL
            WHERE scope = $1 AND idempotency_key = $2 AND owner_token = $3 AND status = 'started'
            "#,
        )
        .bind(scope)
        .bind(key)
        .bind(owner_token)
        .bind(response)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() != 1 {
            return Err(ServerError::Conflict(
                "idempotency lease was lost before completion".to_string(),
            ));
        }
        Ok(())
    }
}
