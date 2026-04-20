use async_trait::async_trait;
use polyedge_application::{
    AuditLogEntry, AuditLogSink, IdempotencyBegin, IdempotencyRequest, IdempotencyStore,
    ModeSnapshot, ModeStateStore, ModeTransitionCommand, RiskStateSnapshot, RiskStateStore,
};
use polyedge_domain::{
    AppError, ExposureRatio, IdempotencyStatus, Result, SignedUsdAmount, SystemMode,
};
use rust_decimal::Decimal;
use serde_json::Value;
use sqlx::{PgPool, Row, types::Json};
use std::{collections::HashMap, str::FromStr, sync::Arc};
use time::{Duration, OffsetDateTime};
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

const SYSTEM_RUNTIME_STATE_ID: &str = "global";
const RISK_STATE_ID: &str = "global";

fn db_error(code: &'static str, context: impl Into<String>) -> AppError {
    AppError::dependency_unavailable(code, context.into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalEventBegin {
    New,
    Replay,
}

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

pub struct InMemoryRiskStateStore {
    snapshot: RwLock<RiskStateSnapshot>,
}

impl InMemoryRiskStateStore {
    #[must_use]
    pub fn new(
        kill_switch: bool,
        daily_pnl: SignedUsdAmount,
        gross_exposure: ExposureRatio,
        net_exposure: ExposureRatio,
        open_alerts: u32,
    ) -> Self {
        Self {
            snapshot: RwLock::new(RiskStateSnapshot {
                kill_switch,
                daily_pnl,
                gross_exposure,
                net_exposure,
                open_alerts,
                updated_at: OffsetDateTime::now_utc(),
                version: 1,
            }),
        }
    }
}

#[async_trait]
impl RiskStateStore for InMemoryRiskStateStore {
    async fn current(&self) -> Result<RiskStateSnapshot> {
        Ok(self.snapshot.read().await.clone())
    }

    async fn set_kill_switch(
        &self,
        kill_switch: bool,
        _trace_id: &str,
        expected_version: Option<i64>,
    ) -> Result<RiskStateSnapshot> {
        let mut snapshot = self.snapshot.write().await;

        if let Some(expected_version) = expected_version {
            if snapshot.version != expected_version {
                return Err(AppError::conflict(
                    "STATE_VERSION_MISMATCH",
                    "risk state version does not match the expected_version",
                ));
            }
        }

        snapshot.kill_switch = kill_switch;
        snapshot.version += 1;
        snapshot.updated_at = OffsetDateTime::now_utc();
        Ok(snapshot.clone())
    }

    async fn update_metrics(
        &self,
        daily_pnl: SignedUsdAmount,
        gross_exposure: ExposureRatio,
        net_exposure: ExposureRatio,
        _trace_id: &str,
    ) -> Result<RiskStateSnapshot> {
        let mut snapshot = self.snapshot.write().await;
        snapshot.daily_pnl = daily_pnl;
        snapshot.gross_exposure = gross_exposure;
        snapshot.net_exposure = net_exposure;
        snapshot.version += 1;
        snapshot.updated_at = OffsetDateTime::now_utc();
        Ok(snapshot.clone())
    }
}

pub struct PostgresRiskStateStore {
    pool: PgPool,
    initial_kill_switch: bool,
    initial_daily_pnl: SignedUsdAmount,
    initial_gross_exposure: ExposureRatio,
    initial_net_exposure: ExposureRatio,
    initial_open_alerts: u32,
}

impl PostgresRiskStateStore {
    #[must_use]
    pub fn new(
        pool: PgPool,
        initial_kill_switch: bool,
        initial_daily_pnl: SignedUsdAmount,
        initial_gross_exposure: ExposureRatio,
        initial_net_exposure: ExposureRatio,
        initial_open_alerts: u32,
    ) -> Self {
        Self {
            pool,
            initial_kill_switch,
            initial_daily_pnl,
            initial_gross_exposure,
            initial_net_exposure,
            initial_open_alerts,
        }
    }

    pub async fn bootstrap(&self) -> Result<()> {
        let now = OffsetDateTime::now_utc();
        let trace_id = format!("trc_{}", Uuid::now_v7());

        sqlx::query(
            r#"
            INSERT INTO risk_state (
              id,
              kill_switch,
              daily_pnl,
              gross_exposure,
              net_exposure,
              open_alerts,
              notes,
              updated_at,
              version,
              trace_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (id) DO NOTHING
            "#,
        )
        .bind(RISK_STATE_ID)
        .bind(self.initial_kill_switch)
        .bind(self.initial_daily_pnl.value())
        .bind(self.initial_gross_exposure.value())
        .bind(self.initial_net_exposure.value())
        .bind(i32::try_from(self.initial_open_alerts).unwrap_or(i32::MAX))
        .bind(Vec::<String>::new())
        .bind(now)
        .bind(1_i64)
        .bind(trace_id)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_BOOTSTRAP_FAILED",
                format!("failed to bootstrap risk state: {error}"),
            )
        })?;

        Ok(())
    }

    fn snapshot_from_row(row: &sqlx::postgres::PgRow) -> Result<RiskStateSnapshot> {
        let daily_pnl: Decimal = row.try_get("daily_pnl").map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode risk_state daily_pnl: {error}"),
            )
        })?;
        let gross_exposure: Decimal = row.try_get("gross_exposure").map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode risk_state gross_exposure: {error}"),
            )
        })?;
        let net_exposure: Decimal = row.try_get("net_exposure").map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode risk_state net_exposure: {error}"),
            )
        })?;
        let open_alerts: i32 = row.try_get("open_alerts").map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode risk_state open_alerts: {error}"),
            )
        })?;

        Ok(RiskStateSnapshot {
            kill_switch: row.try_get("kill_switch").map_err(|error| {
                db_error(
                    "POSTGRES_DECODE_FAILED",
                    format!("failed to decode risk_state kill_switch: {error}"),
                )
            })?,
            daily_pnl: SignedUsdAmount::new(daily_pnl).map_err(|error| {
                db_error(
                    "POSTGRES_DECODE_FAILED",
                    format!("failed to decode risk_state daily_pnl value: {error}"),
                )
            })?,
            gross_exposure: ExposureRatio::new(gross_exposure).map_err(|error| {
                db_error(
                    "POSTGRES_DECODE_FAILED",
                    format!("failed to decode risk_state gross_exposure value: {error}"),
                )
            })?,
            net_exposure: ExposureRatio::new(net_exposure).map_err(|error| {
                db_error(
                    "POSTGRES_DECODE_FAILED",
                    format!("failed to decode risk_state net_exposure value: {error}"),
                )
            })?,
            open_alerts: open_alerts.max(0) as u32,
            updated_at: row.try_get("updated_at").map_err(|error| {
                db_error(
                    "POSTGRES_DECODE_FAILED",
                    format!("failed to decode risk_state updated_at: {error}"),
                )
            })?,
            version: row.try_get("version").map_err(|error| {
                db_error(
                    "POSTGRES_DECODE_FAILED",
                    format!("failed to decode risk_state version: {error}"),
                )
            })?,
        })
    }
}

#[async_trait]
impl RiskStateStore for PostgresRiskStateStore {
    async fn current(&self) -> Result<RiskStateSnapshot> {
        let row = sqlx::query(
            r#"
            SELECT kill_switch, daily_pnl, gross_exposure, net_exposure, open_alerts, updated_at, version
            FROM risk_state
            WHERE id = $1
            "#,
        )
        .bind(RISK_STATE_ID)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query risk state: {error}"),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(
                "RISK_STATE_NOT_FOUND",
                "risk state row was not found; run migrations or bootstrap first",
            )
        })?;

        Self::snapshot_from_row(&row)
    }

    async fn set_kill_switch(
        &self,
        kill_switch: bool,
        trace_id: &str,
        expected_version: Option<i64>,
    ) -> Result<RiskStateSnapshot> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin risk state transaction: {error}"),
            )
        })?;

        let row = sqlx::query(
            r#"
            SELECT kill_switch, daily_pnl, gross_exposure, net_exposure, open_alerts, updated_at, version
            FROM risk_state
            WHERE id = $1
            FOR UPDATE
            "#,
        )
        .bind(RISK_STATE_ID)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to lock risk state row: {error}"),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(
                "RISK_STATE_NOT_FOUND",
                "risk state row was not found; run migrations or bootstrap first",
            )
        })?;
        let current = Self::snapshot_from_row(&row)?;

        if let Some(expected_version) = expected_version {
            if current.version != expected_version {
                return Err(AppError::conflict(
                    "STATE_VERSION_MISMATCH",
                    "risk state version does not match the expected_version",
                ));
            }
        }

        let next_version = current.version + 1;
        let updated_at = OffsetDateTime::now_utc();

        sqlx::query(
            r#"
            UPDATE risk_state
            SET kill_switch = $1, updated_at = $2, version = $3, trace_id = $4
            WHERE id = $5
            "#,
        )
        .bind(kill_switch)
        .bind(updated_at)
        .bind(next_version)
        .bind(trace_id)
        .bind(RISK_STATE_ID)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!("failed to update risk state: {error}"),
            )
        })?;

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit risk state transaction: {error}"),
            )
        })?;

        Ok(RiskStateSnapshot {
            kill_switch,
            daily_pnl: current.daily_pnl,
            gross_exposure: current.gross_exposure,
            net_exposure: current.net_exposure,
            open_alerts: current.open_alerts,
            updated_at,
            version: next_version,
        })
    }

    async fn update_metrics(
        &self,
        daily_pnl: SignedUsdAmount,
        gross_exposure: ExposureRatio,
        net_exposure: ExposureRatio,
        trace_id: &str,
    ) -> Result<RiskStateSnapshot> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin risk metrics transaction: {error}"),
            )
        })?;

        let row = sqlx::query(
            r#"
            SELECT kill_switch, daily_pnl, gross_exposure, net_exposure, open_alerts, updated_at, version
            FROM risk_state
            WHERE id = $1
            FOR UPDATE
            "#,
        )
        .bind(RISK_STATE_ID)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to lock risk state row for metrics update: {error}"),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(
                "RISK_STATE_NOT_FOUND",
                "risk state row was not found; run migrations or bootstrap first",
            )
        })?;
        let current = Self::snapshot_from_row(&row)?;
        let next_version = current.version + 1;
        let updated_at = OffsetDateTime::now_utc();

        sqlx::query(
            r#"
            UPDATE risk_state
            SET
              daily_pnl = $1,
              gross_exposure = $2,
              net_exposure = $3,
              updated_at = $4,
              version = $5,
              trace_id = $6
            WHERE id = $7
            "#,
        )
        .bind(daily_pnl.value())
        .bind(gross_exposure.value())
        .bind(net_exposure.value())
        .bind(updated_at)
        .bind(next_version)
        .bind(trace_id)
        .bind(RISK_STATE_ID)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!("failed to update risk metrics: {error}"),
            )
        })?;

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit risk metrics transaction: {error}"),
            )
        })?;

        Ok(RiskStateSnapshot {
            kill_switch: current.kill_switch,
            daily_pnl,
            gross_exposure,
            net_exposure,
            open_alerts: current.open_alerts,
            updated_at,
            version: next_version,
        })
    }
}

#[derive(Debug, Clone)]
struct IdempotencyRecord {
    request_hash: String,
    response_json: Option<String>,
    status: IdempotencyStatus,
    expires_at: OffsetDateTime,
    error_code: Option<String>,
}

pub struct InMemoryIdempotencyStore {
    records: Mutex<HashMap<(String, String), IdempotencyRecord>>,
}

impl InMemoryIdempotencyStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            records: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl IdempotencyStore for InMemoryIdempotencyStore {
    async fn begin(&self, request: &IdempotencyRequest) -> Result<IdempotencyBegin> {
        let mut records = self.records.lock().await;
        let now = OffsetDateTime::now_utc();
        let compound_key = (request.scope.clone(), request.idempotency_key.clone());

        if let Some(existing) = records.get(&compound_key) {
            if existing.expires_at <= now {
                records.remove(&compound_key);
            } else if existing.request_hash != request.request_hash {
                return Err(AppError::conflict(
                    "IDEMPOTENCY_PAYLOAD_MISMATCH",
                    "idempotency key was already used with a different payload",
                ));
            } else {
                match existing.status {
                    IdempotencyStatus::Completed => {
                        if let Some(response_json) = &existing.response_json {
                            return Ok(IdempotencyBegin::Replay(response_json.clone()));
                        }

                        return Err(AppError::conflict(
                            "IDEMPOTENCY_RESPONSE_MISSING",
                            "completed idempotent request is missing a stored response",
                        ));
                    }
                    IdempotencyStatus::Failed => {
                        let existing = records
                            .get_mut(&compound_key)
                            .expect("idempotency key exists");
                        existing.status = IdempotencyStatus::Started;
                        existing.error_code = None;
                        existing.response_json = None;
                        existing.expires_at = now + Duration::hours(24);
                        return Ok(IdempotencyBegin::Started);
                    }
                    IdempotencyStatus::Started => {
                        return Err(AppError::conflict(
                            "IDEMPOTENCY_REQUEST_IN_PROGRESS",
                            "idempotent request is already in progress",
                        ));
                    }
                }
            }
        }

        records.insert(
            compound_key,
            IdempotencyRecord {
                request_hash: request.request_hash.clone(),
                response_json: None,
                status: IdempotencyStatus::Started,
                expires_at: now + Duration::hours(24),
                error_code: None,
            },
        );

        Ok(IdempotencyBegin::Started)
    }

    async fn complete(&self, request: &IdempotencyRequest, response_json: &str) -> Result<()> {
        let mut records = self.records.lock().await;
        let Some(record) =
            records.get_mut(&(request.scope.clone(), request.idempotency_key.clone()))
        else {
            return Err(AppError::internal(
                "IDEMPOTENCY_RECORD_NOT_FOUND",
                "idempotency record was not found for completion",
            ));
        };

        record.status = IdempotencyStatus::Completed;
        record.response_json = Some(response_json.to_string());
        record.error_code = None;
        Ok(())
    }

    async fn fail(&self, request: &IdempotencyRequest, error_code: &str) -> Result<()> {
        let mut records = self.records.lock().await;
        let Some(record) =
            records.get_mut(&(request.scope.clone(), request.idempotency_key.clone()))
        else {
            return Err(AppError::internal(
                "IDEMPOTENCY_RECORD_NOT_FOUND",
                "idempotency record was not found for failure handling",
            ));
        };

        record.status = IdempotencyStatus::Failed;
        record.error_code = Some(error_code.to_string());
        Ok(())
    }
}

pub struct PostgresIdempotencyStore {
    pool: PgPool,
}

impl PostgresIdempotencyStore {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl IdempotencyStore for PostgresIdempotencyStore {
    async fn begin(&self, request: &IdempotencyRequest) -> Result<IdempotencyBegin> {
        let now = OffsetDateTime::now_utc();
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin idempotency transaction: {error}"),
            )
        })?;

        let existing = sqlx::query(
            r#"
            SELECT request_hash, status, response_json, expires_at
            FROM idempotency_keys
            WHERE scope = $1 AND idempotency_key = $2
            FOR UPDATE
            "#,
        )
        .bind(&request.scope)
        .bind(&request.idempotency_key)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query idempotency key row: {error}"),
            )
        })?;

        if let Some(row) = existing {
            let request_hash: String = row.try_get("request_hash").map_err(|error| {
                db_error(
                    "POSTGRES_DECODE_FAILED",
                    format!("failed to decode idempotency request_hash: {error}"),
                )
            })?;
            let status_raw: String = row.try_get("status").map_err(|error| {
                db_error(
                    "POSTGRES_DECODE_FAILED",
                    format!("failed to decode idempotency status: {error}"),
                )
            })?;
            let response_json: Option<Value> = row.try_get("response_json").map_err(|error| {
                db_error(
                    "POSTGRES_DECODE_FAILED",
                    format!("failed to decode idempotency response_json: {error}"),
                )
            })?;
            let expires_at: OffsetDateTime = row.try_get("expires_at").map_err(|error| {
                db_error(
                    "POSTGRES_DECODE_FAILED",
                    format!("failed to decode idempotency expires_at: {error}"),
                )
            })?;

            if expires_at <= now {
                sqlx::query(
                    "DELETE FROM idempotency_keys WHERE scope = $1 AND idempotency_key = $2",
                )
                .bind(&request.scope)
                .bind(&request.idempotency_key)
                .execute(&mut *transaction)
                .await
                .map_err(|error| {
                    db_error(
                        "POSTGRES_DELETE_FAILED",
                        format!("failed to delete expired idempotency row: {error}"),
                    )
                })?;
            } else if request_hash != request.request_hash {
                return Err(AppError::conflict(
                    "IDEMPOTENCY_PAYLOAD_MISMATCH",
                    "idempotency key was already used with a different payload",
                ));
            } else {
                match IdempotencyStatus::from_str(&status_raw)? {
                    IdempotencyStatus::Completed => {
                        let Some(response_json) = response_json else {
                            return Err(AppError::conflict(
                                "IDEMPOTENCY_RESPONSE_MISSING",
                                "completed idempotent request is missing a stored response",
                            ));
                        };

                        transaction.commit().await.map_err(|error| {
                            db_error(
                                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                                format!("failed to commit idempotency replay transaction: {error}"),
                            )
                        })?;

                        return Ok(IdempotencyBegin::Replay(
                            serde_json::to_string(&response_json).map_err(|error| {
                                AppError::internal(
                                    "IDEMPOTENCY_RESPONSE_SERIALIZE_FAILED",
                                    format!(
                                        "failed to serialize stored idempotency response: {error}"
                                    ),
                                )
                            })?,
                        ));
                    }
                    IdempotencyStatus::Failed => {
                        sqlx::query(
                            r#"
                            UPDATE idempotency_keys
                            SET status = $3, request_id = $4, actor_user_id = $5,
                                actor_session_id = $6, resource_type = $7, resource_id = $8,
                                response_json = NULL, last_seen_at = $9
                            WHERE scope = $1 AND idempotency_key = $2
                            "#,
                        )
                        .bind(&request.scope)
                        .bind(&request.idempotency_key)
                        .bind(IdempotencyStatus::Started.as_str())
                        .bind(&request.request_id)
                        .bind(&request.actor_user_id)
                        .bind(&request.actor_session_id)
                        .bind(&request.resource_type)
                        .bind(&request.resource_id)
                        .bind(now)
                        .execute(&mut *transaction)
                        .await
                        .map_err(|error| {
                            db_error(
                                "POSTGRES_UPDATE_FAILED",
                                format!("failed to restart failed idempotency row: {error}"),
                            )
                        })?;

                        transaction.commit().await.map_err(|error| {
                            db_error(
                                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                                format!(
                                    "failed to commit idempotency restart transaction: {error}"
                                ),
                            )
                        })?;

                        return Ok(IdempotencyBegin::Started);
                    }
                    IdempotencyStatus::Started => {
                        return Err(AppError::conflict(
                            "IDEMPOTENCY_REQUEST_IN_PROGRESS",
                            "idempotent request is already in progress",
                        ));
                    }
                }
            }
        }

        sqlx::query(
            r#"
            INSERT INTO idempotency_keys (
              scope,
              idempotency_key,
              request_hash,
              request_id,
              actor_user_id,
              actor_session_id,
              status,
              resource_type,
              resource_id,
              response_json,
              first_seen_at,
              last_seen_at,
              expires_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#,
        )
        .bind(&request.scope)
        .bind(&request.idempotency_key)
        .bind(&request.request_hash)
        .bind(&request.request_id)
        .bind(&request.actor_user_id)
        .bind(&request.actor_session_id)
        .bind(IdempotencyStatus::Started.as_str())
        .bind(&request.resource_type)
        .bind(&request.resource_id)
        .bind(Option::<Json<Value>>::None)
        .bind(now)
        .bind(now)
        .bind(now + Duration::hours(24))
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_INSERT_FAILED",
                format!("failed to insert idempotency row: {error}"),
            )
        })?;

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit idempotency transaction: {error}"),
            )
        })?;

        Ok(IdempotencyBegin::Started)
    }

    async fn complete(&self, request: &IdempotencyRequest, response_json: &str) -> Result<()> {
        let response_json: Value = serde_json::from_str(response_json).map_err(|error| {
            AppError::internal(
                "IDEMPOTENCY_RESPONSE_JSON_INVALID",
                format!("failed to parse idempotency response json: {error}"),
            )
        })?;

        sqlx::query(
            r#"
            UPDATE idempotency_keys
            SET status = $3, response_json = $4, last_seen_at = $5
            WHERE scope = $1 AND idempotency_key = $2
            "#,
        )
        .bind(&request.scope)
        .bind(&request.idempotency_key)
        .bind(IdempotencyStatus::Completed.as_str())
        .bind(Json(response_json))
        .bind(OffsetDateTime::now_utc())
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!("failed to complete idempotency row: {error}"),
            )
        })?;

        Ok(())
    }

    async fn fail(&self, request: &IdempotencyRequest, error_code: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE idempotency_keys
            SET status = $3, last_seen_at = $4, error_code = $5
            WHERE scope = $1 AND idempotency_key = $2
            "#,
        )
        .bind(&request.scope)
        .bind(&request.idempotency_key)
        .bind(IdempotencyStatus::Failed.as_str())
        .bind(OffsetDateTime::now_utc())
        .bind(error_code)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!("failed to fail idempotency row: {error}"),
            )
        })?;

        Ok(())
    }
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

pub type SharedModeStateStore = Arc<InMemoryModeStateStore>;
pub type SharedRiskStateStore = Arc<InMemoryRiskStateStore>;
pub type SharedIdempotencyStore = Arc<InMemoryIdempotencyStore>;
pub type SharedAuditLogSink = Arc<InMemoryAuditLogSink>;
