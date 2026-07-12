#[async_trait]
pub trait ExternalEventStore: Send + Sync {
    async fn begin(
        &self,
        source_system: &str,
        external_event_id: &str,
        payload_hash: &str,
        trace_id: &str,
    ) -> Result<ExternalEventBegin>;

    async fn mark_processed(
        &self,
        source_system: &str,
        external_event_id: &str,
        trace_id: &str,
    ) -> Result<()>;

    async fn abandon(
        &self,
        source_system: &str,
        external_event_id: &str,
        trace_id: &str,
    ) -> Result<()>;
}

const EXTERNAL_EVENT_LEASE: Duration = Duration::minutes(5);

#[derive(Debug, Clone)]
struct ExternalEventRecord {
    payload_hash: String,
    processed_at: Option<OffsetDateTime>,
    lease_expires_at: OffsetDateTime,
    trace_id: String,
}

pub struct InMemoryExternalEventStore {
    records: Mutex<HashMap<(String, String), ExternalEventRecord>>,
}

impl InMemoryExternalEventStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            records: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl ExternalEventStore for InMemoryExternalEventStore {
    async fn begin(
        &self,
        source_system: &str,
        external_event_id: &str,
        payload_hash: &str,
        trace_id: &str,
    ) -> Result<ExternalEventBegin> {
        let mut records = self.records.lock().await;
        let key = (source_system.to_string(), external_event_id.to_string());

        if let Some(record) = records.get_mut(&key) {
            if record.payload_hash != payload_hash {
                return Err(AppError::conflict(
                    "EXTERNAL_EVENT_PAYLOAD_MISMATCH",
                    "external event id was already used with a different payload",
                ));
            }

            if record.processed_at.is_some() {
                return Ok(ExternalEventBegin::Replay);
            }

            if record.lease_expires_at <= OffsetDateTime::now_utc() {
                record.lease_expires_at = OffsetDateTime::now_utc() + EXTERNAL_EVENT_LEASE;
                record.trace_id = trace_id.to_string();
                return Ok(ExternalEventBegin::New);
            }

            return Err(AppError::conflict(
                "EXTERNAL_EVENT_IN_PROGRESS",
                "external event is already being processed",
            ));
        }

        records.insert(
            key,
            ExternalEventRecord {
                payload_hash: payload_hash.to_string(),
                processed_at: None,
                lease_expires_at: OffsetDateTime::now_utc() + EXTERNAL_EVENT_LEASE,
                trace_id: trace_id.to_string(),
            },
        );

        Ok(ExternalEventBegin::New)
    }

    async fn mark_processed(
        &self,
        source_system: &str,
        external_event_id: &str,
        trace_id: &str,
    ) -> Result<()> {
        let mut records = self.records.lock().await;
        let Some(record) =
            records.get_mut(&(source_system.to_string(), external_event_id.to_string()))
        else {
            return Err(AppError::internal(
                "EXTERNAL_EVENT_RECORD_NOT_FOUND",
                "external event record was not found for completion handling",
            ));
        };

        if record.trace_id != trace_id {
            return Err(AppError::conflict(
                "EXTERNAL_EVENT_LEASE_LOST",
                "external event lease is owned by another request",
            ));
        }

        record.processed_at = Some(OffsetDateTime::now_utc());
        record.trace_id = trace_id.to_string();
        Ok(())
    }

    async fn abandon(
        &self,
        source_system: &str,
        external_event_id: &str,
        trace_id: &str,
    ) -> Result<()> {
        let mut records = self.records.lock().await;
        let key = (source_system.to_string(), external_event_id.to_string());
        if records
            .get(&key)
            .is_some_and(|record| record.processed_at.is_none() && record.trace_id == trace_id)
        {
            records.remove(&key);
        } else {
            return Err(AppError::conflict(
                "EXTERNAL_EVENT_LEASE_LOST",
                "external event could not be abandoned because its lease was lost",
            ));
        }
        Ok(())
    }
}

pub struct PostgresExternalEventStore {
    pool: PgPool,
}

impl PostgresExternalEventStore {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ExternalEventStore for PostgresExternalEventStore {
    async fn begin(
        &self,
        source_system: &str,
        external_event_id: &str,
        payload_hash: &str,
        trace_id: &str,
    ) -> Result<ExternalEventBegin> {
        let now = OffsetDateTime::now_utc();
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin external event transaction: {error}"),
            )
        })?;

        let insert_result = sqlx::query(
            r#"
            INSERT INTO external_event_dedup (
              source_system,
              external_event_id,
              payload_hash,
              first_seen_at,
              processed_at,
              lease_expires_at,
              trace_id
            )
            VALUES ($1, $2, $3, $4, NULL, $5, $6)
            ON CONFLICT (source_system, external_event_id) DO NOTHING
            "#,
        )
        .bind(source_system)
        .bind(external_event_id)
        .bind(payload_hash)
        .bind(now)
        .bind(now + EXTERNAL_EVENT_LEASE)
        .bind(trace_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_INSERT_FAILED",
                format!("failed to begin external event dedup row: {error}"),
            )
        })?;

        if insert_result.rows_affected() == 1 {
            transaction.commit().await.map_err(|error| {
                db_error(
                    "POSTGRES_TRANSACTION_COMMIT_FAILED",
                    format!("failed to commit new external event lease: {error}"),
                )
            })?;
            return Ok(ExternalEventBegin::New);
        }

        let row = sqlx::query(
            r#"
            SELECT payload_hash, processed_at, lease_expires_at
            FROM external_event_dedup
            WHERE source_system = $1
              AND external_event_id = $2
            "#,
        )
        .bind(source_system)
        .bind(external_event_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to load external event dedup row: {error}"),
            )
        })?
        .ok_or_else(|| {
            AppError::internal(
                "EXTERNAL_EVENT_RECORD_NOT_FOUND",
                "external event dedup row disappeared during begin",
            )
        })?;

        let stored_payload_hash: String = row.try_get("payload_hash").map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode external event payload_hash: {error}"),
            )
        })?;
        let processed_at: Option<OffsetDateTime> =
            row.try_get("processed_at").map_err(|error| {
                db_error(
                    "POSTGRES_DECODE_FAILED",
                    format!("failed to decode external event processed_at: {error}"),
                )
            })?;
        let lease_expires_at: OffsetDateTime =
            row.try_get("lease_expires_at").map_err(|error| {
                db_error(
                    "POSTGRES_DECODE_FAILED",
                    format!("failed to decode external event lease_expires_at: {error}"),
                )
            })?;

        if stored_payload_hash != payload_hash {
            return Err(AppError::conflict(
                "EXTERNAL_EVENT_PAYLOAD_MISMATCH",
                "external event id was already used with a different payload",
            ));
        }

        if processed_at.is_some() {
            transaction.commit().await.map_err(|error| {
                db_error(
                    "POSTGRES_TRANSACTION_COMMIT_FAILED",
                    format!("failed to commit external event replay lookup: {error}"),
                )
            })?;
            return Ok(ExternalEventBegin::Replay);
        }

        if lease_expires_at <= now {
            let result = sqlx::query(
                r#"
                UPDATE external_event_dedup
                SET lease_expires_at = $3,
                    trace_id = $4
                WHERE source_system = $1
                  AND external_event_id = $2
                  AND processed_at IS NULL
                "#,
            )
            .bind(source_system)
            .bind(external_event_id)
            .bind(now + EXTERNAL_EVENT_LEASE)
            .bind(trace_id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_UPDATE_FAILED",
                    format!("failed to reclaim expired external event lease: {error}"),
                )
            })?;
            if result.rows_affected() != 1 {
                return Err(AppError::conflict(
                    "EXTERNAL_EVENT_LEASE_LOST",
                    "external event lease changed while it was being reclaimed",
                ));
            }
            transaction.commit().await.map_err(|error| {
                db_error(
                    "POSTGRES_TRANSACTION_COMMIT_FAILED",
                    format!("failed to commit external event lease reclaim: {error}"),
                )
            })?;
            return Ok(ExternalEventBegin::New);
        }

        Err(AppError::conflict(
            "EXTERNAL_EVENT_IN_PROGRESS",
            "external event is already being processed",
        ))
    }

    async fn mark_processed(
        &self,
        source_system: &str,
        external_event_id: &str,
        trace_id: &str,
    ) -> Result<()> {
        let result = sqlx::query(
            r#"
            UPDATE external_event_dedup
            SET processed_at = $3, trace_id = $4
            WHERE source_system = $1
              AND external_event_id = $2
              AND trace_id = $4
              AND processed_at IS NULL
            "#,
        )
        .bind(source_system)
        .bind(external_event_id)
        .bind(OffsetDateTime::now_utc())
        .bind(trace_id)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!("failed to mark external event as processed: {error}"),
            )
        })?;
        if result.rows_affected() != 1 {
            return Err(AppError::conflict(
                "EXTERNAL_EVENT_LEASE_LOST",
                "external event could not be completed because its lease was lost",
            ));
        }

        Ok(())
    }

    async fn abandon(
        &self,
        source_system: &str,
        external_event_id: &str,
        trace_id: &str,
    ) -> Result<()> {
        let result = sqlx::query(
            r#"
            DELETE FROM external_event_dedup
            WHERE source_system = $1
              AND external_event_id = $2
              AND processed_at IS NULL
              AND trace_id = $3
            "#,
        )
        .bind(source_system)
        .bind(external_event_id)
        .bind(trace_id)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_DELETE_FAILED",
                format!("failed to abandon external event dedup row: {error}"),
            )
        })?;
        if result.rows_affected() != 1 {
            return Err(AppError::conflict(
                "EXTERNAL_EVENT_LEASE_LOST",
                "external event could not be abandoned because its lease was lost",
            ));
        }

        Ok(())
    }
}
