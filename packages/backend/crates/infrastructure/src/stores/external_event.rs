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

    async fn abandon(&self, source_system: &str, external_event_id: &str) -> Result<()>;
}

#[derive(Debug, Clone)]
struct ExternalEventRecord {
    payload_hash: String,
    processed_at: Option<OffsetDateTime>,
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

        if let Some(record) = records.get(&key) {
            if record.payload_hash != payload_hash {
                return Err(AppError::conflict(
                    "EXTERNAL_EVENT_PAYLOAD_MISMATCH",
                    "external event id was already used with a different payload",
                ));
            }

            if record.processed_at.is_some() {
                return Ok(ExternalEventBegin::Replay);
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

        record.processed_at = Some(OffsetDateTime::now_utc());
        record.trace_id = trace_id.to_string();
        Ok(())
    }

    async fn abandon(&self, source_system: &str, external_event_id: &str) -> Result<()> {
        let mut records = self.records.lock().await;
        records.remove(&(source_system.to_string(), external_event_id.to_string()));
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

        let insert_result = sqlx::query(
            r#"
            INSERT INTO external_event_dedup (
              source_system,
              external_event_id,
              payload_hash,
              first_seen_at,
              processed_at,
              trace_id
            )
            VALUES ($1, $2, $3, $4, NULL, $5)
            ON CONFLICT (source_system, external_event_id) DO NOTHING
            "#,
        )
        .bind(source_system)
        .bind(external_event_id)
        .bind(payload_hash)
        .bind(now)
        .bind(trace_id)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_INSERT_FAILED",
                format!("failed to begin external event dedup row: {error}"),
            )
        })?;

        if insert_result.rows_affected() == 1 {
            return Ok(ExternalEventBegin::New);
        }

        let row = sqlx::query(
            r#"
            SELECT payload_hash, processed_at
            FROM external_event_dedup
            WHERE source_system = $1
              AND external_event_id = $2
            "#,
        )
        .bind(source_system)
        .bind(external_event_id)
        .fetch_optional(&self.pool)
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

        if stored_payload_hash != payload_hash {
            return Err(AppError::conflict(
                "EXTERNAL_EVENT_PAYLOAD_MISMATCH",
                "external event id was already used with a different payload",
            ));
        }

        if processed_at.is_some() {
            return Ok(ExternalEventBegin::Replay);
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
        sqlx::query(
            r#"
            UPDATE external_event_dedup
            SET processed_at = $3, trace_id = $4
            WHERE source_system = $1
              AND external_event_id = $2
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

        Ok(())
    }

    async fn abandon(&self, source_system: &str, external_event_id: &str) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM external_event_dedup
            WHERE source_system = $1
              AND external_event_id = $2
              AND processed_at IS NULL
            "#,
        )
        .bind(source_system)
        .bind(external_event_id)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_DELETE_FAILED",
                format!("failed to abandon external event dedup row: {error}"),
            )
        })?;

        Ok(())
    }
}
