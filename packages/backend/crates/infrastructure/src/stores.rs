use async_trait::async_trait;
use polyedge_application::{
    AuditLogEntry, AuditLogSink, IdempotencyBegin, IdempotencyRequest, IdempotencyStore,
    ManagedRewardOrder, ManagedRewardOrderStatus, ModeSnapshot, ModeStateStore,
    ModeTransitionCommand, RewardBotConfig, RewardBotMode, RewardBotStore, RewardMarket,
    RewardOrderSide, RewardPosition, RewardQuotePlan, RewardRiskEvent, RewardRiskSeverity,
    RewardToken, RiskStateSnapshot, RiskStateStore,
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

pub struct InMemoryRewardBotStore {
    config: RwLock<RewardBotConfig>,
    markets: RwLock<HashMap<String, RewardMarket>>,
    quote_plans: RwLock<HashMap<String, RewardQuotePlan>>,
    orders: RwLock<Vec<ManagedRewardOrder>>,
    positions: RwLock<HashMap<(String, String), RewardPosition>>,
    events: RwLock<Vec<RewardRiskEvent>>,
}

impl InMemoryRewardBotStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: RwLock::new(RewardBotConfig::default()),
            markets: RwLock::new(HashMap::new()),
            quote_plans: RwLock::new(HashMap::new()),
            orders: RwLock::new(Vec::new()),
            positions: RwLock::new(HashMap::new()),
            events: RwLock::new(Vec::new()),
        }
    }
}

#[async_trait]
impl RewardBotStore for InMemoryRewardBotStore {
    async fn load_config(&self) -> Result<RewardBotConfig> {
        Ok(self.config.read().await.clone().normalized())
    }

    async fn save_config(&self, config: &RewardBotConfig) -> Result<()> {
        *self.config.write().await = config.clone().normalized();
        Ok(())
    }

    async fn upsert_markets(&self, markets: &[RewardMarket]) -> Result<()> {
        let mut store = self.markets.write().await;
        for market in markets {
            store.insert(market.condition_id.clone(), market.clone());
        }
        Ok(())
    }

    async fn save_quote_plans(&self, plans: &[RewardQuotePlan]) -> Result<()> {
        let mut store = self.quote_plans.write().await;
        for plan in plans {
            store.insert(plan.condition_id.clone(), plan.clone());
        }
        Ok(())
    }

    async fn replace_simulated_orders(
        &self,
        account_id: &str,
        orders: &[ManagedRewardOrder],
        _trace_id: &str,
    ) -> Result<usize> {
        let now = OffsetDateTime::now_utc();
        let mut store = self.orders.write().await;
        let mut cancelled = 0;
        for order in store.iter_mut() {
            if order.account_id == account_id && order.status.is_open_like() {
                order.status = ManagedRewardOrderStatus::Cancelled;
                order.reason = "replaced by latest rewards simulation".to_string();
                order.updated_at = now;
                cancelled += 1;
            }
        }
        store.extend(orders.iter().cloned());
        store.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(cancelled)
    }

    async fn cancel_open_orders(
        &self,
        account_id: Option<&str>,
        reason: &str,
        _trace_id: &str,
    ) -> Result<usize> {
        let now = OffsetDateTime::now_utc();
        let mut cancelled = 0;
        let mut store = self.orders.write().await;
        for order in store.iter_mut() {
            let account_matches =
                account_id.is_none_or(|account_id| account_id == order.account_id);
            if account_matches && order.status.is_open_like() {
                order.status = ManagedRewardOrderStatus::Cancelled;
                order.reason = reason.to_string();
                order.updated_at = now;
                cancelled += 1;
            }
        }
        Ok(cancelled)
    }

    async fn list_markets(&self, limit: u16) -> Result<Vec<RewardMarket>> {
        let mut markets = self
            .markets
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        markets.sort_by(|left, right| {
            right
                .total_daily_rate
                .cmp(&left.total_daily_rate)
                .then_with(|| right.updated_at.cmp(&left.updated_at))
        });
        markets.truncate(usize::from(limit));
        Ok(markets)
    }

    async fn list_quote_plans(&self, limit: u16) -> Result<Vec<RewardQuotePlan>> {
        let mut plans = self
            .quote_plans
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        plans.sort_by(|left, right| {
            right
                .eligible
                .cmp(&left.eligible)
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| right.updated_at.cmp(&left.updated_at))
        });
        plans.truncate(usize::from(limit));
        Ok(plans)
    }

    async fn list_orders(&self, limit: u16) -> Result<Vec<ManagedRewardOrder>> {
        let mut orders = self.orders.read().await.clone();
        orders.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        orders.truncate(usize::from(limit));
        Ok(orders)
    }

    async fn list_positions(&self, limit: u16) -> Result<Vec<RewardPosition>> {
        let mut positions = self
            .positions
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        positions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        positions.truncate(usize::from(limit));
        Ok(positions)
    }

    async fn list_events(&self, limit: u16) -> Result<Vec<RewardRiskEvent>> {
        let mut events = self.events.read().await.clone();
        events.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        events.truncate(usize::from(limit));
        Ok(events)
    }

    async fn log_event(&self, event: RewardRiskEvent) -> Result<()> {
        let mut events = self.events.write().await;
        events.push(event);
        events.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        events.truncate(1_000);
        Ok(())
    }
}

pub struct PostgresRewardBotStore {
    pool: PgPool,
}

impl PostgresRewardBotStore {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RewardBotStore for PostgresRewardBotStore {
    async fn load_config(&self) -> Result<RewardBotConfig> {
        let rows = sqlx::query(
            r#"
            SELECT key, value
            FROM reward_bot_config
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query reward bot config: {error}"),
            )
        })?;

        let mut config = RewardBotConfig::default();
        for row in rows {
            let key: String = row.try_get("key").map_err(postgres_decode_error)?;
            let value: String = row.try_get("value").map_err(postgres_decode_error)?;
            apply_reward_config_value(&mut config, &key, &value)?;
        }
        Ok(config.normalized())
    }

    async fn save_config(&self, config: &RewardBotConfig) -> Result<()> {
        let config = config.clone().normalized();
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin reward config transaction: {error}"),
            )
        })?;

        for (key, value) in reward_config_entries(&config) {
            sqlx::query(
                r#"
                INSERT INTO reward_bot_config (key, value, updated_at)
                VALUES ($1, $2, now())
                ON CONFLICT (key) DO UPDATE
                SET value = EXCLUDED.value,
                    updated_at = now()
                "#,
            )
            .bind(key)
            .bind(value)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_UPSERT_FAILED",
                    format!("failed to upsert reward bot config: {error}"),
                )
            })?;
        }

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit reward config transaction: {error}"),
            )
        })?;
        Ok(())
    }

    async fn upsert_markets(&self, markets: &[RewardMarket]) -> Result<()> {
        for market in markets {
            sqlx::query(
                r#"
                INSERT INTO reward_markets (
                  condition_id,
                  question,
                  market_slug,
                  event_slug,
                  image,
                  rewards_max_spread,
                  rewards_min_size,
                  total_daily_rate,
                  tokens_json,
                  active,
                  updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                ON CONFLICT (condition_id) DO UPDATE
                SET question = EXCLUDED.question,
                    market_slug = EXCLUDED.market_slug,
                    event_slug = EXCLUDED.event_slug,
                    image = EXCLUDED.image,
                    rewards_max_spread = EXCLUDED.rewards_max_spread,
                    rewards_min_size = EXCLUDED.rewards_min_size,
                    total_daily_rate = EXCLUDED.total_daily_rate,
                    tokens_json = EXCLUDED.tokens_json,
                    active = EXCLUDED.active,
                    updated_at = EXCLUDED.updated_at
                "#,
            )
            .bind(&market.condition_id)
            .bind(&market.question)
            .bind(&market.market_slug)
            .bind(&market.event_slug)
            .bind(&market.image)
            .bind(market.rewards_max_spread)
            .bind(market.rewards_min_size)
            .bind(market.total_daily_rate)
            .bind(Json(market.tokens.clone()))
            .bind(market.active)
            .bind(market.updated_at)
            .execute(&self.pool)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_UPSERT_FAILED",
                    format!("failed to upsert reward market: {error}"),
                )
            })?;
        }
        Ok(())
    }

    async fn save_quote_plans(&self, plans: &[RewardQuotePlan]) -> Result<()> {
        for plan in plans {
            sqlx::query(
                r#"
                INSERT INTO reward_quote_plans (
                  condition_id,
                  score,
                  eligible,
                  reason,
                  quote_plan_json,
                  updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6)
                ON CONFLICT (condition_id) DO UPDATE
                SET score = EXCLUDED.score,
                    eligible = EXCLUDED.eligible,
                    reason = EXCLUDED.reason,
                    quote_plan_json = EXCLUDED.quote_plan_json,
                    updated_at = EXCLUDED.updated_at
                "#,
            )
            .bind(&plan.condition_id)
            .bind(plan.score)
            .bind(plan.eligible)
            .bind(&plan.reason)
            .bind(Json(plan.clone()))
            .bind(plan.updated_at)
            .execute(&self.pool)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_UPSERT_FAILED",
                    format!("failed to upsert reward quote plan: {error}"),
                )
            })?;
        }
        Ok(())
    }

    async fn replace_simulated_orders(
        &self,
        account_id: &str,
        orders: &[ManagedRewardOrder],
        trace_id: &str,
    ) -> Result<usize> {
        let now = OffsetDateTime::now_utc();
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin reward order transaction: {error}"),
            )
        })?;

        let cancelled = sqlx::query(
            r#"
            UPDATE reward_managed_orders
            SET status = 'cancelled',
                reason = 'replaced by latest rewards simulation',
                updated_at = $1,
                trace_id = $2
            WHERE account_id = $3
              AND status IN ('planned', 'open', 'exit_pending')
            "#,
        )
        .bind(now)
        .bind(trace_id)
        .bind(account_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!("failed to cancel stale reward orders: {error}"),
            )
        })?
        .rows_affected() as usize;

        for order in orders {
            insert_reward_order(&mut transaction, order, trace_id).await?;
        }

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit reward order transaction: {error}"),
            )
        })?;
        Ok(cancelled)
    }

    async fn cancel_open_orders(
        &self,
        account_id: Option<&str>,
        reason: &str,
        trace_id: &str,
    ) -> Result<usize> {
        let now = OffsetDateTime::now_utc();
        let result = sqlx::query(
            r#"
            UPDATE reward_managed_orders
            SET status = 'cancelled',
                reason = $1,
                updated_at = $2,
                trace_id = $3
            WHERE status IN ('planned', 'open', 'exit_pending')
              AND ($4::text IS NULL OR account_id = $4)
            "#,
        )
        .bind(reason)
        .bind(now)
        .bind(trace_id)
        .bind(account_id)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!("failed to cancel reward orders: {error}"),
            )
        })?;
        Ok(result.rows_affected() as usize)
    }

    async fn list_markets(&self, limit: u16) -> Result<Vec<RewardMarket>> {
        let rows = sqlx::query(
            r#"
            SELECT condition_id,
                   question,
                   market_slug,
                   event_slug,
                   image,
                   rewards_max_spread,
                   rewards_min_size,
                   total_daily_rate,
                   tokens_json,
                   active,
                   updated_at
            FROM reward_markets
            WHERE active = true
            ORDER BY total_daily_rate DESC, updated_at DESC
            LIMIT $1
            "#,
        )
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query reward markets: {error}"),
            )
        })?;

        rows.iter().map(reward_market_from_row).collect()
    }

    async fn list_quote_plans(&self, limit: u16) -> Result<Vec<RewardQuotePlan>> {
        let rows = sqlx::query(
            r#"
            SELECT quote_plan_json
            FROM reward_quote_plans
            ORDER BY eligible DESC, score DESC, updated_at DESC
            LIMIT $1
            "#,
        )
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query reward quote plans: {error}"),
            )
        })?;

        rows.iter()
            .map(|row| {
                let plan: Json<RewardQuotePlan> = row
                    .try_get("quote_plan_json")
                    .map_err(postgres_decode_error)?;
                Ok(plan.0)
            })
            .collect()
    }

    async fn list_orders(&self, limit: u16) -> Result<Vec<ManagedRewardOrder>> {
        let rows = sqlx::query(
            r#"
            SELECT id,
                   account_id,
                   condition_id,
                   token_id,
                   outcome,
                   side,
                   price,
                   size,
                   external_order_id,
                   status,
                   scoring,
                   reason,
                   created_at,
                   updated_at
            FROM reward_managed_orders
            ORDER BY updated_at DESC
            LIMIT $1
            "#,
        )
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query reward managed orders: {error}"),
            )
        })?;

        rows.iter().map(reward_order_from_row).collect()
    }

    async fn list_positions(&self, limit: u16) -> Result<Vec<RewardPosition>> {
        let rows = sqlx::query(
            r#"
            SELECT account_id,
                   condition_id,
                   token_id,
                   outcome,
                   size,
                   avg_price,
                   realized_pnl,
                   updated_at
            FROM reward_positions
            WHERE size <> 0
            ORDER BY updated_at DESC
            LIMIT $1
            "#,
        )
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query reward positions: {error}"),
            )
        })?;

        rows.iter().map(reward_position_from_row).collect()
    }

    async fn list_events(&self, limit: u16) -> Result<Vec<RewardRiskEvent>> {
        let rows = sqlx::query(
            r#"
            SELECT id,
                   account_id,
                   condition_id,
                   external_order_id,
                   event_type,
                   severity,
                   message,
                   metadata_json,
                   created_at
            FROM reward_risk_events
            ORDER BY created_at DESC
            LIMIT $1
            "#,
        )
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query reward risk events: {error}"),
            )
        })?;

        rows.iter().map(reward_event_from_row).collect()
    }

    async fn log_event(&self, event: RewardRiskEvent) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO reward_risk_events (
              id,
              account_id,
              condition_id,
              external_order_id,
              event_type,
              severity,
              message,
              metadata_json,
              created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(&event.id)
        .bind(&event.account_id)
        .bind(&event.condition_id)
        .bind(&event.external_order_id)
        .bind(&event.event_type)
        .bind(event.severity.as_str())
        .bind(&event.message)
        .bind(Json(event.metadata.clone()))
        .bind(event.created_at)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_INSERT_FAILED",
                format!("failed to insert reward risk event: {error}"),
            )
        })?;
        Ok(())
    }
}

fn postgres_decode_error(error: sqlx::Error) -> AppError {
    db_error(
        "POSTGRES_DECODE_FAILED",
        format!("failed to decode postgres row: {error}"),
    )
}

fn apply_reward_config_value(config: &mut RewardBotConfig, key: &str, value: &str) -> Result<()> {
    match key {
        "enabled" => config.enabled = parse_bool_config(key, value)?,
        "mode" => config.mode = RewardBotMode::from_str(value)?,
        "account_id" => config.account_id = value.to_string(),
        "max_markets" => config.max_markets = parse_u16_config(key, value)?,
        "max_open_orders" => config.max_open_orders = parse_u16_config(key, value)?,
        "per_market_usd" => config.per_market_usd = parse_decimal_config(key, value)?,
        "quote_size_usd" => config.quote_size_usd = parse_decimal_config(key, value)?,
        "min_daily_reward" => config.min_daily_reward = parse_decimal_config(key, value)?,
        "min_market_score" => config.min_market_score = parse_decimal_config(key, value)?,
        "max_spread_cents" => config.max_spread_cents = parse_decimal_config(key, value)?,
        "quote_edge_cents" => config.quote_edge_cents = parse_decimal_config(key, value)?,
        "safety_margin_cents" => config.safety_margin_cents = parse_decimal_config(key, value)?,
        "min_midpoint" => config.min_midpoint = parse_decimal_config(key, value)?,
        "max_midpoint" => config.max_midpoint = parse_decimal_config(key, value)?,
        "stale_book_ms" => config.stale_book_ms = parse_u64_config(key, value)?,
        "min_scoring_check_sec" => config.min_scoring_check_sec = parse_u64_config(key, value)?,
        "max_position_usd" => config.max_position_usd = parse_decimal_config(key, value)?,
        "max_global_position_usd" => {
            config.max_global_position_usd = parse_decimal_config(key, value)?;
        }
        "exit_markup_cents" => config.exit_markup_cents = parse_decimal_config(key, value)?,
        "cancel_on_fill" => config.cancel_on_fill = parse_bool_config(key, value)?,
        _ => {}
    }
    Ok(())
}

fn reward_config_entries(config: &RewardBotConfig) -> Vec<(&'static str, String)> {
    vec![
        ("enabled", config.enabled.to_string()),
        ("mode", config.mode.as_str().to_string()),
        ("account_id", config.account_id.clone()),
        ("max_markets", config.max_markets.to_string()),
        ("max_open_orders", config.max_open_orders.to_string()),
        ("per_market_usd", config.per_market_usd.to_string()),
        ("quote_size_usd", config.quote_size_usd.to_string()),
        ("min_daily_reward", config.min_daily_reward.to_string()),
        ("min_market_score", config.min_market_score.to_string()),
        ("max_spread_cents", config.max_spread_cents.to_string()),
        ("quote_edge_cents", config.quote_edge_cents.to_string()),
        (
            "safety_margin_cents",
            config.safety_margin_cents.to_string(),
        ),
        ("min_midpoint", config.min_midpoint.to_string()),
        ("max_midpoint", config.max_midpoint.to_string()),
        ("stale_book_ms", config.stale_book_ms.to_string()),
        (
            "min_scoring_check_sec",
            config.min_scoring_check_sec.to_string(),
        ),
        ("max_position_usd", config.max_position_usd.to_string()),
        (
            "max_global_position_usd",
            config.max_global_position_usd.to_string(),
        ),
        ("exit_markup_cents", config.exit_markup_cents.to_string()),
        ("cancel_on_fill", config.cancel_on_fill.to_string()),
    ]
}

fn parse_bool_config(key: &str, value: &str) -> Result<bool> {
    match value {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(AppError::invalid_input(
            "REWARD_CONFIG_BOOL_INVALID",
            format!("reward config key {key} must be a boolean"),
        )),
    }
}

fn parse_u16_config(key: &str, value: &str) -> Result<u16> {
    value.parse::<u16>().map_err(|error| {
        AppError::invalid_input(
            "REWARD_CONFIG_U16_INVALID",
            format!("reward config key {key} must be a u16: {error}"),
        )
    })
}

fn parse_u64_config(key: &str, value: &str) -> Result<u64> {
    value.parse::<u64>().map_err(|error| {
        AppError::invalid_input(
            "REWARD_CONFIG_U64_INVALID",
            format!("reward config key {key} must be a u64: {error}"),
        )
    })
}

fn parse_decimal_config(key: &str, value: &str) -> Result<Decimal> {
    Decimal::from_str(value).map_err(|error| {
        AppError::invalid_input(
            "REWARD_CONFIG_DECIMAL_INVALID",
            format!("reward config key {key} must be a decimal: {error}"),
        )
    })
}

async fn insert_reward_order(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    order: &ManagedRewardOrder,
    trace_id: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO reward_managed_orders (
          id,
          account_id,
          condition_id,
          token_id,
          outcome,
          side,
          price,
          size,
          external_order_id,
          status,
          scoring,
          reason,
          created_at,
          updated_at,
          trace_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        ON CONFLICT (id) DO UPDATE
        SET external_order_id = EXCLUDED.external_order_id,
            status = EXCLUDED.status,
            scoring = EXCLUDED.scoring,
            reason = EXCLUDED.reason,
            updated_at = EXCLUDED.updated_at,
            trace_id = EXCLUDED.trace_id
        "#,
    )
    .bind(&order.id)
    .bind(&order.account_id)
    .bind(&order.condition_id)
    .bind(&order.token_id)
    .bind(&order.outcome)
    .bind(order.side.as_str())
    .bind(order.price)
    .bind(order.size)
    .bind(&order.external_order_id)
    .bind(order.status.as_str())
    .bind(order.scoring)
    .bind(&order.reason)
    .bind(order.created_at)
    .bind(order.updated_at)
    .bind(trace_id)
    .execute(&mut **transaction)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_INSERT_FAILED",
            format!("failed to insert reward managed order: {error}"),
        )
    })?;
    Ok(())
}

fn reward_market_from_row(row: &sqlx::postgres::PgRow) -> Result<RewardMarket> {
    let tokens: Json<Vec<RewardToken>> =
        row.try_get("tokens_json").map_err(postgres_decode_error)?;
    Ok(RewardMarket {
        condition_id: row.try_get("condition_id").map_err(postgres_decode_error)?,
        question: row.try_get("question").map_err(postgres_decode_error)?,
        market_slug: row.try_get("market_slug").map_err(postgres_decode_error)?,
        event_slug: row.try_get("event_slug").map_err(postgres_decode_error)?,
        image: row.try_get("image").map_err(postgres_decode_error)?,
        rewards_max_spread: row
            .try_get("rewards_max_spread")
            .map_err(postgres_decode_error)?,
        rewards_min_size: row
            .try_get("rewards_min_size")
            .map_err(postgres_decode_error)?,
        total_daily_rate: row
            .try_get("total_daily_rate")
            .map_err(postgres_decode_error)?,
        tokens: tokens.0,
        active: row.try_get("active").map_err(postgres_decode_error)?,
        updated_at: row.try_get("updated_at").map_err(postgres_decode_error)?,
    })
}

fn reward_order_from_row(row: &sqlx::postgres::PgRow) -> Result<ManagedRewardOrder> {
    let side_raw: String = row.try_get("side").map_err(postgres_decode_error)?;
    let status_raw: String = row.try_get("status").map_err(postgres_decode_error)?;
    Ok(ManagedRewardOrder {
        id: row.try_get("id").map_err(postgres_decode_error)?,
        account_id: row.try_get("account_id").map_err(postgres_decode_error)?,
        condition_id: row.try_get("condition_id").map_err(postgres_decode_error)?,
        token_id: row.try_get("token_id").map_err(postgres_decode_error)?,
        outcome: row.try_get("outcome").map_err(postgres_decode_error)?,
        side: RewardOrderSide::from_str(&side_raw)?,
        price: row.try_get("price").map_err(postgres_decode_error)?,
        size: row.try_get("size").map_err(postgres_decode_error)?,
        external_order_id: row
            .try_get("external_order_id")
            .map_err(postgres_decode_error)?,
        status: ManagedRewardOrderStatus::from_str(&status_raw)?,
        scoring: row.try_get("scoring").map_err(postgres_decode_error)?,
        reason: row.try_get("reason").map_err(postgres_decode_error)?,
        created_at: row.try_get("created_at").map_err(postgres_decode_error)?,
        updated_at: row.try_get("updated_at").map_err(postgres_decode_error)?,
    })
}

fn reward_position_from_row(row: &sqlx::postgres::PgRow) -> Result<RewardPosition> {
    Ok(RewardPosition {
        account_id: row.try_get("account_id").map_err(postgres_decode_error)?,
        condition_id: row.try_get("condition_id").map_err(postgres_decode_error)?,
        token_id: row.try_get("token_id").map_err(postgres_decode_error)?,
        outcome: row.try_get("outcome").map_err(postgres_decode_error)?,
        size: row.try_get("size").map_err(postgres_decode_error)?,
        avg_price: row.try_get("avg_price").map_err(postgres_decode_error)?,
        realized_pnl: row.try_get("realized_pnl").map_err(postgres_decode_error)?,
        updated_at: row.try_get("updated_at").map_err(postgres_decode_error)?,
    })
}

fn reward_event_from_row(row: &sqlx::postgres::PgRow) -> Result<RewardRiskEvent> {
    let severity_raw: String = row.try_get("severity").map_err(postgres_decode_error)?;
    let metadata: Json<Value> = row
        .try_get("metadata_json")
        .map_err(postgres_decode_error)?;
    Ok(RewardRiskEvent {
        id: row.try_get("id").map_err(postgres_decode_error)?,
        account_id: row.try_get("account_id").map_err(postgres_decode_error)?,
        condition_id: row.try_get("condition_id").map_err(postgres_decode_error)?,
        external_order_id: row
            .try_get("external_order_id")
            .map_err(postgres_decode_error)?,
        event_type: row.try_get("event_type").map_err(postgres_decode_error)?,
        severity: RewardRiskSeverity::from_str(&severity_raw)?,
        message: row.try_get("message").map_err(postgres_decode_error)?,
        metadata: metadata.0,
        created_at: row.try_get("created_at").map_err(postgres_decode_error)?,
    })
}

pub type SharedModeStateStore = Arc<InMemoryModeStateStore>;
pub type SharedRiskStateStore = Arc<InMemoryRiskStateStore>;
pub type SharedIdempotencyStore = Arc<InMemoryIdempotencyStore>;
pub type SharedAuditLogSink = Arc<InMemoryAuditLogSink>;
