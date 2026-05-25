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
