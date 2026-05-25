pub struct InMemoryAuditLogSink {
    entries: Mutex<Vec<AuditLogEntry>>,
}

impl InMemoryAuditLogSink {
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
        }
    }

    pub async fn entries(&self) -> Vec<AuditLogEntry> {
        self.entries.lock().await.clone()
    }
}

#[async_trait]
impl AuditLogSink for InMemoryAuditLogSink {
    async fn append(&self, entry: AuditLogEntry) -> Result<()> {
        self.entries.lock().await.push(entry);
        Ok(())
    }
}

pub struct PostgresAuditLogSink {
    pool: PgPool,
}

impl PostgresAuditLogSink {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AuditLogSink for PostgresAuditLogSink {
    async fn append(&self, entry: AuditLogEntry) -> Result<()> {
        let actor_roles_json = serde_json::to_value(&entry.actor.roles).map_err(|error| {
            AppError::internal(
                "AUDIT_LOG_ROLES_JSON_INVALID",
                format!("failed to serialize actor roles for audit log: {error}"),
            )
        })?;

        sqlx::query(
            r#"
            INSERT INTO audit_logs (
              id,
              occurred_at,
              request_id,
              trace_id,
              actor_user_id,
              actor_session_id,
              actor_roles_json,
              action,
              resource_type,
              resource_id,
              reason,
              result,
              error_code,
              ip,
              user_agent_summary,
              payload_json,
              version_snapshot_json
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
            "#,
        )
        .bind(format!("aud_{}", Uuid::now_v7()))
        .bind(entry.occurred_at)
        .bind(&entry.request_id)
        .bind(&entry.trace_id)
        .bind(&entry.actor.user_id)
        .bind(&entry.actor.session_id)
        .bind(Json(actor_roles_json))
        .bind(&entry.action)
        .bind(&entry.resource_type)
        .bind(&entry.resource_id)
        .bind(&entry.reason)
        .bind(entry.result.as_str())
        .bind(&entry.error_code)
        .bind(&entry.actor.ip)
        .bind(&entry.actor.user_agent)
        .bind(Option::<Json<Value>>::None)
        .bind(Option::<Json<Value>>::None)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_INSERT_FAILED",
                format!("failed to insert audit log row: {error}"),
            )
        })?;

        Ok(())
    }
}
