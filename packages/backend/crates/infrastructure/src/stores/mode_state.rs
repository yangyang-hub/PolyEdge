pub struct InMemoryModeStateStore {
    snapshot: RwLock<ModeSnapshot>,
}

impl InMemoryModeStateStore {
    #[must_use]
    pub fn new(initial_mode: SystemMode, environment: impl Into<String>) -> Self {
        Self {
            snapshot: RwLock::new(ModeSnapshot {
                mode: initial_mode,
                environment: environment.into(),
                version: 1,
                updated_at: OffsetDateTime::now_utc(),
            }),
        }
    }
}

#[async_trait]
impl ModeStateStore for InMemoryModeStateStore {
    async fn current(&self) -> Result<ModeSnapshot> {
        Ok(self.snapshot.read().await.clone())
    }

    async fn transition(&self, command: &ModeTransitionCommand) -> Result<ModeSnapshot> {
        let mut snapshot = self.snapshot.write().await;
        snapshot.mode = command.to_mode;
        snapshot.version += 1;
        snapshot.updated_at = OffsetDateTime::now_utc();
        Ok(snapshot.clone())
    }
}

pub struct PostgresModeStateStore {
    pool: PgPool,
    initial_mode: SystemMode,
    environment: String,
}

impl PostgresModeStateStore {
    #[must_use]
    pub fn new(pool: PgPool, initial_mode: SystemMode, environment: impl Into<String>) -> Self {
        Self {
            pool,
            initial_mode,
            environment: environment.into(),
        }
    }

    pub async fn bootstrap(&self) -> Result<()> {
        let now = OffsetDateTime::now_utc();
        let trace_id = format!("trc_{}", Uuid::now_v7());

        sqlx::query(
            r#"
            INSERT INTO system_runtime_state (id, mode, environment, version, updated_at, trace_id)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (id) DO NOTHING
            "#,
        )
        .bind(SYSTEM_RUNTIME_STATE_ID)
        .bind(self.initial_mode.as_str())
        .bind(&self.environment)
        .bind(1_i64)
        .bind(now)
        .bind(trace_id)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_BOOTSTRAP_FAILED",
                format!("failed to bootstrap system runtime state: {error}"),
            )
        })?;

        Ok(())
    }

    fn snapshot_from_row(row: &sqlx::postgres::PgRow) -> Result<ModeSnapshot> {
        let mode_raw: String = row.try_get("mode").map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode runtime mode: {error}"),
            )
        })?;
        let environment: String = row.try_get("environment").map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode runtime environment: {error}"),
            )
        })?;
        let version: i64 = row.try_get("version").map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode runtime version: {error}"),
            )
        })?;
        let updated_at: OffsetDateTime = row.try_get("updated_at").map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode runtime updated_at: {error}"),
            )
        })?;

        Ok(ModeSnapshot {
            mode: SystemMode::from_str(&mode_raw)?,
            environment,
            version,
            updated_at,
        })
    }
}

#[async_trait]
impl ModeStateStore for PostgresModeStateStore {
    async fn current(&self) -> Result<ModeSnapshot> {
        let row = sqlx::query(
            r#"
            SELECT mode, environment, version, updated_at
            FROM system_runtime_state
            WHERE id = $1
            "#,
        )
        .bind(SYSTEM_RUNTIME_STATE_ID)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query system runtime state: {error}"),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(
                "SYSTEM_RUNTIME_STATE_NOT_FOUND",
                "system runtime state row was not found; run migrations or bootstrap first",
            )
        })?;

        Self::snapshot_from_row(&row)
    }

    async fn transition(&self, command: &ModeTransitionCommand) -> Result<ModeSnapshot> {
        let now = OffsetDateTime::now_utc();
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin runtime state transaction: {error}"),
            )
        })?;

        let current_row = sqlx::query(
            r#"
            SELECT mode, environment, version, updated_at
            FROM system_runtime_state
            WHERE id = $1
            FOR UPDATE
            "#,
        )
        .bind(SYSTEM_RUNTIME_STATE_ID)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to lock system runtime state row: {error}"),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(
                "SYSTEM_RUNTIME_STATE_NOT_FOUND",
                "system runtime state row was not found; run migrations or bootstrap first",
            )
        })?;

        let current_snapshot = Self::snapshot_from_row(&current_row)?;
        let next_version = current_snapshot.version + 1;

        sqlx::query(
            r#"
            UPDATE system_runtime_state
            SET mode = $1, version = $2, updated_at = $3, trace_id = $4
            WHERE id = $5
            "#,
        )
        .bind(command.to_mode.as_str())
        .bind(next_version)
        .bind(now)
        .bind(&command.trace_id)
        .bind(SYSTEM_RUNTIME_STATE_ID)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!("failed to update system runtime state: {error}"),
            )
        })?;

        sqlx::query(
            r#"
            INSERT INTO mode_transitions (
              id,
              from_mode,
              to_mode,
              reason,
              requested_by_user_id,
              requested_by_session_id,
              request_id,
              trace_id,
              created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(format!("mod_{}", Uuid::now_v7()))
        .bind(current_snapshot.mode.as_str())
        .bind(command.to_mode.as_str())
        .bind(&command.reason)
        .bind(&command.actor.user_id)
        .bind(&command.actor.session_id)
        .bind(&command.request_id)
        .bind(&command.trace_id)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_INSERT_FAILED",
                format!("failed to insert mode transition audit row: {error}"),
            )
        })?;

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit runtime state transaction: {error}"),
            )
        })?;

        Ok(ModeSnapshot {
            mode: command.to_mode,
            environment: current_snapshot.environment,
            version: next_version,
            updated_at: now,
        })
    }
}
