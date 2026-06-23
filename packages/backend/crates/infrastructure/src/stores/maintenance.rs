const DATABASE_MAINTENANCE_DELETE_BATCH_SIZE: i64 = 10_000;
const DATABASE_MAINTENANCE_MAX_DELETE_BATCHES: usize = 20;

pub struct NoopDatabaseMaintenanceStore;

impl NoopDatabaseMaintenanceStore {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl DatabaseMaintenanceStore for NoopDatabaseMaintenanceStore {
    async fn prune_database_history(
        &self,
        _cutoffs: DatabaseMaintenanceCutoffs,
    ) -> Result<DatabaseMaintenanceReport> {
        Ok(DatabaseMaintenanceReport::default())
    }
}

pub struct PostgresDatabaseMaintenanceStore {
    pool: PgPool,
}

impl PostgresDatabaseMaintenanceStore {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DatabaseMaintenanceStore for PostgresDatabaseMaintenanceStore {
    async fn prune_database_history(
        &self,
        cutoffs: DatabaseMaintenanceCutoffs,
    ) -> Result<DatabaseMaintenanceReport> {
        let idempotency_keys_deleted = execute_maintenance_delete(
            &self.pool,
            "idempotency_keys",
            r#"
            WITH doomed AS (
                SELECT ctid
                FROM idempotency_keys
                WHERE expires_at < $1
                LIMIT $2
            )
            DELETE FROM idempotency_keys rows
            USING doomed
            WHERE rows.ctid = doomed.ctid
            "#,
            cutoffs.now,
        )
        .await?;

        let outbox_events_deleted = execute_maintenance_delete(
            &self.pool,
            "outbox_events",
            r#"
            WITH doomed AS (
                SELECT ctid
                FROM outbox_events
                WHERE (
                    status = 'published'
                    AND COALESCE(published_at, created_at) < $1
                )
                OR (
                    status IN ('failed', 'dead_letter')
                    AND created_at < $2
                )
                LIMIT $3
            )
            DELETE FROM outbox_events rows
            USING doomed
            WHERE rows.ctid = doomed.ctid
            "#,
            (cutoffs.outbox_published_before, cutoffs.outbox_failed_before),
        )
        .await?;

        let external_event_dedup_deleted = execute_maintenance_delete(
            &self.pool,
            "external_event_dedup",
            r#"
            WITH doomed AS (
                SELECT ctid
                FROM external_event_dedup
                WHERE (
                    processed_at IS NOT NULL
                    AND processed_at < $1
                )
                OR (
                    processed_at IS NULL
                    AND first_seen_at < $2
                )
                LIMIT $3
            )
            DELETE FROM external_event_dedup rows
            USING doomed
            WHERE rows.ctid = doomed.ctid
            "#,
            (
                cutoffs.external_event_processed_before,
                cutoffs.external_event_stale_unprocessed_before,
            ),
        )
        .await?;

        let llm_calls_deleted = execute_maintenance_delete(
            &self.pool,
            "llm_calls",
            r#"
            WITH doomed AS (
                SELECT ctid
                FROM llm_calls
                WHERE created_at < $1
                LIMIT $2
            )
            DELETE FROM llm_calls rows
            USING doomed
            WHERE rows.ctid = doomed.ctid
            "#,
            cutoffs.llm_calls_before,
        )
        .await?;

        let raw_events_deleted = prune_raw_events(&self.pool, cutoffs).await?;

        let reward_market_advisories_deleted = execute_maintenance_delete(
            &self.pool,
            "reward_market_advisories",
            r#"
            WITH doomed AS (
                SELECT ctid
                FROM reward_market_advisories
                WHERE expires_at < $1
                LIMIT $2
            )
            DELETE FROM reward_market_advisories rows
            USING doomed
            WHERE rows.ctid = doomed.ctid
            "#,
            cutoffs.expired_cache_before,
        )
        .await?;

        let reward_market_info_risks_deleted = execute_maintenance_delete(
            &self.pool,
            "reward_market_info_risks",
            r#"
            WITH doomed AS (
                SELECT ctid
                FROM reward_market_info_risks
                WHERE expires_at < $1
                LIMIT $2
            )
            DELETE FROM reward_market_info_risks rows
            USING doomed
            WHERE rows.ctid = doomed.ctid
            "#,
            cutoffs.expired_cache_before,
        )
        .await?;

        let reward_market_candles_deleted = execute_maintenance_delete(
            &self.pool,
            "reward_market_candles",
            r#"
            WITH doomed AS (
                SELECT ctid
                FROM reward_market_candles
                WHERE bucket_start < $1
                LIMIT $2
            )
            DELETE FROM reward_market_candles rows
            USING doomed
            WHERE rows.ctid = doomed.ctid
            "#,
            cutoffs.reward_candles_before,
        )
        .await?;

        let reward_control_commands_deleted = prune_control_commands(
            &self.pool,
            "reward_control_commands",
            cutoffs.control_commands_completed_before,
            cutoffs.control_commands_failed_before,
        )
        .await?;

        let copytrade_control_commands_deleted = prune_control_commands(
            &self.pool,
            "copytrade_control_commands",
            cutoffs.control_commands_completed_before,
            cutoffs.control_commands_failed_before,
        )
        .await?;

        let copytrade_events_deleted = execute_maintenance_delete(
            &self.pool,
            "copytrade_events",
            r#"
            WITH doomed AS (
                SELECT ctid
                FROM copytrade_events
                WHERE created_at < $1
                LIMIT $2
            )
            DELETE FROM copytrade_events rows
            USING doomed
            WHERE rows.ctid = doomed.ctid
            "#,
            cutoffs.copytrade_events_before,
        )
        .await?;

        let copytrade_source_trades_deleted = execute_maintenance_delete(
            &self.pool,
            "copytrade_source_trades",
            r#"
            WITH doomed AS (
                SELECT ctid
                FROM copytrade_source_trades
                WHERE observed_at < $1
                LIMIT $2
            )
            DELETE FROM copytrade_source_trades rows
            USING doomed
            WHERE rows.ctid = doomed.ctid
            "#,
            cutoffs.copytrade_source_trades_before,
        )
        .await?;

        let audit_logs_deleted = execute_maintenance_delete(
            &self.pool,
            "audit_logs",
            r#"
            WITH doomed AS (
                SELECT ctid
                FROM audit_logs
                WHERE occurred_at < $1
                LIMIT $2
            )
            DELETE FROM audit_logs rows
            USING doomed
            WHERE rows.ctid = doomed.ctid
            "#,
            cutoffs.audit_before,
        )
        .await?;

        let mode_transitions_deleted = execute_maintenance_delete(
            &self.pool,
            "mode_transitions",
            r#"
            WITH doomed AS (
                SELECT ctid
                FROM mode_transitions
                WHERE created_at < $1
                LIMIT $2
            )
            DELETE FROM mode_transitions rows
            USING doomed
            WHERE rows.ctid = doomed.ctid
            "#,
            cutoffs.audit_before,
        )
        .await?;

        Ok(DatabaseMaintenanceReport {
            idempotency_keys_deleted,
            outbox_events_deleted,
            external_event_dedup_deleted,
            llm_calls_deleted,
            raw_events_deleted,
            reward_market_advisories_deleted,
            reward_market_info_risks_deleted,
            reward_market_candles_deleted,
            reward_control_commands_deleted,
            copytrade_control_commands_deleted,
            copytrade_events_deleted,
            copytrade_source_trades_deleted,
            audit_logs_deleted,
            mode_transitions_deleted,
        })
    }
}

async fn prune_raw_events(
    pool: &PgPool,
    cutoffs: DatabaseMaintenanceCutoffs,
) -> Result<u64> {
    let unlinked_deleted = execute_maintenance_delete(
        pool,
        "raw_events",
        r#"
        WITH doomed AS (
            SELECT raw.ctid
            FROM raw_events raw
            WHERE raw.ingested_at < $1
              AND NOT EXISTS (
                  SELECT 1
                  FROM events event
                  WHERE event.raw_event_id = raw.id
              )
            LIMIT $2
        )
        DELETE FROM raw_events rows
        USING doomed
        WHERE rows.ctid = doomed.ctid
        "#,
        cutoffs.raw_events_unlinked_before,
    )
    .await?;

    let linked_deleted = execute_maintenance_delete(
        pool,
        "raw_events",
        r#"
        WITH doomed AS (
            SELECT raw.ctid
            FROM raw_events raw
            WHERE raw.ingested_at < $1
              AND EXISTS (
                  SELECT 1
                  FROM events event
                  WHERE event.raw_event_id = raw.id
              )
            LIMIT $2
        )
        DELETE FROM raw_events rows
        USING doomed
        WHERE rows.ctid = doomed.ctid
        "#,
        cutoffs.raw_events_linked_before,
    )
    .await?;

    Ok(unlinked_deleted.saturating_add(linked_deleted))
}

async fn prune_control_commands(
    pool: &PgPool,
    table_name: &'static str,
    completed_before: OffsetDateTime,
    failed_before: OffsetDateTime,
) -> Result<u64> {
    let sql = match table_name {
        "reward_control_commands" => {
            r#"
            WITH doomed AS (
                SELECT ctid
                FROM reward_control_commands
                WHERE (
                    status = 'completed'
                    AND COALESCE(completed_at, requested_at) < $1
                )
                OR (
                    status = 'failed'
                    AND COALESCE(completed_at, requested_at) < $2
                )
                LIMIT $3
            )
            DELETE FROM reward_control_commands rows
            USING doomed
            WHERE rows.ctid = doomed.ctid
            "#
        }
        "copytrade_control_commands" => {
            r#"
            WITH doomed AS (
                SELECT ctid
                FROM copytrade_control_commands
                WHERE (
                    status = 'completed'
                    AND COALESCE(completed_at, requested_at) < $1
                )
                OR (
                    status = 'failed'
                    AND COALESCE(completed_at, requested_at) < $2
                )
                LIMIT $3
            )
            DELETE FROM copytrade_control_commands rows
            USING doomed
            WHERE rows.ctid = doomed.ctid
            "#
        }
        _ => {
            return Err(AppError::internal(
                "DATABASE_MAINTENANCE_INVALID_TABLE",
                format!("unsupported control command table for maintenance: {table_name}"),
            ));
        }
    };

    execute_maintenance_delete(pool, table_name, sql, (completed_before, failed_before)).await
}

async fn execute_maintenance_delete<B>(
    pool: &PgPool,
    label: &'static str,
    sql: &str,
    bind: B,
) -> Result<u64>
where
    B: BindMaintenanceDelete + Copy,
{
    let mut deleted = 0_u64;
    for _ in 0..DATABASE_MAINTENANCE_MAX_DELETE_BATCHES {
        let result = bind
            .bind(sqlx::query(sql))
            .bind(DATABASE_MAINTENANCE_DELETE_BATCH_SIZE)
            .execute(pool)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_DELETE_FAILED",
                    format!("failed to prune {label}: {error}"),
                )
            })?;
        let rows = result.rows_affected();
        deleted = deleted.saturating_add(rows);
        if rows < DATABASE_MAINTENANCE_DELETE_BATCH_SIZE as u64 {
            break;
        }
    }

    Ok(deleted)
}

trait BindMaintenanceDelete {
    fn bind<'q>(
        self,
        query: sqlx::query::Query<'q, Postgres, sqlx::postgres::PgArguments>,
    ) -> sqlx::query::Query<'q, Postgres, sqlx::postgres::PgArguments>;
}

impl BindMaintenanceDelete for OffsetDateTime {
    fn bind<'q>(
        self,
        query: sqlx::query::Query<'q, Postgres, sqlx::postgres::PgArguments>,
    ) -> sqlx::query::Query<'q, Postgres, sqlx::postgres::PgArguments> {
        query.bind(self)
    }
}

impl BindMaintenanceDelete for (OffsetDateTime, OffsetDateTime) {
    fn bind<'q>(
        self,
        query: sqlx::query::Query<'q, Postgres, sqlx::postgres::PgArguments>,
    ) -> sqlx::query::Query<'q, Postgres, sqlx::postgres::PgArguments> {
        query.bind(self.0).bind(self.1)
    }
}
