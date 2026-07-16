use super::*;

impl PostgresStore {
    pub async fn create_execution_batch(
        &self,
        request: &CreateExecutionBatchRequest,
        actor_id: &str,
        request_id: &str,
    ) -> Result<ExecutionBatchData> {
        if request.strategy_id <= 0 {
            return Err(ServerError::InvalidInput(
                "strategy_id must be positive".to_string(),
            ));
        }
        let operator_note = optional_note(request.operator_note.as_deref())?;
        let mut tx = self.pool.begin().await?;
        let strategy_version_id: i64 = sqlx::query_scalar(
            r#"
            SELECT v.strategy_version_id
            FROM strategy_versions v
            JOIN market_strategies s ON s.strategy_id = v.strategy_id
            WHERE v.strategy_id = $1 AND v.status = 'published' AND s.status = 'active'
            "#,
        )
        .bind(request.strategy_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| {
            ServerError::Conflict("strategy has no active published version".to_string())
        })?;
        let wallet_ids = if request.wallet_ids.is_empty() {
            sqlx::query_scalar::<_, i64>(
                r#"
                SELECT t.wallet_id
                FROM strategy_wallet_targets t
                JOIN wallet_accounts w ON w.wallet_id = t.wallet_id
                WHERE t.strategy_id = $1 AND t.enabled
                  AND w.status = 'active' AND w.trading_enabled
                ORDER BY t.wallet_id
                "#,
            )
            .bind(request.strategy_id)
            .fetch_all(&mut *tx)
            .await?
        } else {
            let enabled = sqlx::query_scalar::<_, i64>(
                r#"
                SELECT t.wallet_id
                FROM strategy_wallet_targets t
                JOIN wallet_accounts w ON w.wallet_id = t.wallet_id
                WHERE t.strategy_id = $1 AND t.wallet_id = ANY($2)
                  AND t.enabled AND w.status = 'active' AND w.trading_enabled
                ORDER BY t.wallet_id
                "#,
            )
            .bind(request.strategy_id)
            .bind(&request.wallet_ids)
            .fetch_all(&mut *tx)
            .await?;
            if enabled.len() != request.wallet_ids.len() {
                return Err(ServerError::InvalidInput(
                    "all requested wallets must be active enabled targets of the strategy"
                        .to_string(),
                ));
            }
            enabled
        };
        if wallet_ids.is_empty() {
            return Err(ServerError::InvalidInput(
                "execution batch requires at least one enabled wallet".to_string(),
            ));
        }
        let batch_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO execution_batches (
              strategy_version_id, status, requested_by, operator_note
            ) VALUES ($1, 'pending', $2, $3)
            RETURNING batch_id
            "#,
        )
        .bind(strategy_version_id)
        .bind(actor_id)
        .bind(operator_note.as_deref())
        .fetch_one(&mut *tx)
        .await?;
        for wallet_id in wallet_ids {
            sqlx::query(
                "INSERT INTO wallet_execution_jobs (batch_id, wallet_id, status) VALUES ($1, $2, 'pending')",
            )
            .bind(batch_id)
            .bind(wallet_id)
            .execute(&mut *tx)
            .await?;
        }
        insert_audit(
            &mut tx,
            request_id,
            actor_id,
            "execution_batch.create",
            "execution_batch",
            &batch_id.to_string(),
            operator_note.as_deref(),
        )
        .await?;
        tx.commit().await?;
        self.get_execution_batch(batch_id).await
    }

    pub async fn list_execution_batches(
        &self,
        query: &ManualTradingListQuery,
    ) -> Result<Vec<ExecutionBatchData>> {
        let (limit, offset) = page_values(query);
        let batch_ids = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT b.batch_id
            FROM execution_batches b
            JOIN strategy_versions v ON v.strategy_version_id = b.strategy_version_id
            WHERE ($1::text IS NULL OR b.status = $1)
              AND ($2::bigint IS NULL OR v.strategy_id = $2)
            ORDER BY b.created_at DESC, b.batch_id DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(query.status.as_deref())
        .bind(query.strategy_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        self.load_execution_batches(&batch_ids).await
    }

    pub async fn get_execution_batch(&self, batch_id: i64) -> Result<ExecutionBatchData> {
        self.load_execution_batches(&[batch_id])
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| ServerError::NotFound(format!("execution batch {batch_id}")))
    }

    pub async fn cancel_execution_batch(
        &self,
        batch_id: i64,
        operator_note: Option<&str>,
        actor_id: &str,
        request_id: &str,
    ) -> Result<ExecutionBatchData> {
        let operator_note = optional_note(operator_note)?;
        let mut tx = self.pool.begin().await?;
        let updated = sqlx::query(
            r#"
            UPDATE execution_batches
            SET status = 'cancelled', completed_at = now()
            WHERE batch_id = $1 AND status IN ('pending', 'running')
            "#,
        )
        .bind(batch_id)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() == 0 {
            return Err(ServerError::Conflict(
                "execution batch is not cancellable".to_string(),
            ));
        }
        sqlx::query(
            r#"
            UPDATE wallet_execution_jobs
            SET status = 'cancelled', completed_at = now(), updated_at = now(),
                lease_owner = NULL, lease_expires_at = NULL
            WHERE batch_id = $1 AND status = 'pending'
            "#,
        )
        .bind(batch_id)
        .execute(&mut *tx)
        .await?;
        insert_audit(
            &mut tx,
            request_id,
            actor_id,
            "execution_batch.cancel",
            "execution_batch",
            &batch_id.to_string(),
            operator_note.as_deref(),
        )
        .await?;
        tx.commit().await?;
        self.get_execution_batch(batch_id).await
    }

    async fn load_execution_batches(&self, batch_ids: &[i64]) -> Result<Vec<ExecutionBatchData>> {
        if batch_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query(
            r#"
            SELECT batch_id, strategy_version_id, status, requested_by,
                   operator_note, created_at, started_at, completed_at
            FROM execution_batches
            WHERE batch_id = ANY($1)
            "#,
        )
        .bind(batch_ids)
        .fetch_all(&self.pool)
        .await?;
        let job_rows = sqlx::query(
            r#"
            SELECT job_id, batch_id, wallet_id, status, attempt_count,
                   error_code, error_message, lease_epoch, lease_owner,
                   lease_expires_at, created_at, updated_at
            FROM wallet_execution_jobs
            WHERE batch_id = ANY($1)
            ORDER BY batch_id, wallet_id
            "#,
        )
        .bind(batch_ids)
        .fetch_all(&self.pool)
        .await?;
        let mut jobs: HashMap<i64, Vec<WalletExecutionJob>> = HashMap::new();
        for row in job_rows {
            let batch_id: i64 = row.try_get("batch_id")?;
            jobs.entry(batch_id).or_default().push(job_from_row(&row)?);
        }
        let mut result = rows
            .into_iter()
            .map(|row| {
                let batch = batch_from_row(&row)?;
                Ok(ExecutionBatchData {
                    jobs: jobs.remove(&batch.id).unwrap_or_default(),
                    batch,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        result.sort_by_key(|item| {
            batch_ids
                .iter()
                .position(|id| *id == item.batch.id)
                .unwrap_or(usize::MAX)
        });
        Ok(result)
    }
}

fn batch_from_row(row: &sqlx::postgres::PgRow) -> Result<ExecutionBatch> {
    Ok(ExecutionBatch {
        id: row.try_get("batch_id")?,
        strategy_version_id: row.try_get("strategy_version_id")?,
        status: enum_value(row.try_get("status")?, "batch status")?,
        requested_by: row.try_get("requested_by")?,
        operator_note: row.try_get("operator_note")?,
        created_at: row.try_get("created_at")?,
        started_at: row.try_get("started_at")?,
        completed_at: row.try_get("completed_at")?,
    })
}

pub(super) fn job_from_row(row: &sqlx::postgres::PgRow) -> Result<WalletExecutionJob> {
    Ok(WalletExecutionJob {
        id: row.try_get("job_id")?,
        batch_id: row.try_get("batch_id")?,
        wallet_id: row.try_get("wallet_id")?,
        status: enum_value(row.try_get("status")?, "wallet job status")?,
        attempt_count: row.try_get("attempt_count")?,
        error_code: row.try_get("error_code")?,
        error_message: row.try_get("error_message")?,
        lease_epoch: row.try_get("lease_epoch")?,
        lease_owner: row.try_get("lease_owner")?,
        lease_expires_at: row.try_get("lease_expires_at")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}
