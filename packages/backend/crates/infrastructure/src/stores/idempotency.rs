#[derive(Debug, Clone)]
struct IdempotencyRecord {
    request_hash: String,
    request_id: String,
    response_json: Option<String>,
    status: IdempotencyStatus,
    lease_expires_at: Option<OffsetDateTime>,
    expires_at: OffsetDateTime,
    error_code: Option<String>,
}

const IDEMPOTENCY_LEASE: Duration = Duration::minutes(5);
const IDEMPOTENCY_RETENTION: Duration = Duration::hours(24);

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
                        existing.request_id = request.request_id.clone();
                        existing.lease_expires_at = Some(now + IDEMPOTENCY_LEASE);
                        existing.error_code = None;
                        existing.response_json = None;
                        existing.expires_at = now + IDEMPOTENCY_RETENTION;
                        return Ok(IdempotencyBegin::Started);
                    }
                    IdempotencyStatus::Started => {
                        if existing
                            .lease_expires_at
                            .is_none_or(|lease_expires_at| lease_expires_at <= now)
                        {
                            let existing = records
                                .get_mut(&compound_key)
                                .expect("idempotency key exists");
                            existing.lease_expires_at = Some(now + IDEMPOTENCY_LEASE);
                            existing.request_id = request.request_id.clone();
                            existing.expires_at = now + IDEMPOTENCY_RETENTION;
                            existing.error_code = None;
                            existing.response_json = None;
                            return Ok(IdempotencyBegin::Started);
                        }
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
                request_id: request.request_id.clone(),
                response_json: None,
                status: IdempotencyStatus::Started,
                lease_expires_at: Some(now + IDEMPOTENCY_LEASE),
                expires_at: now + IDEMPOTENCY_RETENTION,
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

        if record.request_hash != request.request_hash
            || record.request_id != request.request_id
            || record.status != IdempotencyStatus::Started
        {
            return Err(AppError::conflict(
                "IDEMPOTENCY_FINALIZE_CONFLICT",
                "idempotency record is not owned by the active request",
            ));
        }

        record.status = IdempotencyStatus::Completed;
        record.response_json = Some(response_json.to_string());
        record.lease_expires_at = None;
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

        if record.request_hash != request.request_hash
            || record.request_id != request.request_id
            || record.status != IdempotencyStatus::Started
        {
            return Err(AppError::conflict(
                "IDEMPOTENCY_FINALIZE_CONFLICT",
                "idempotency record is not owned by the active request",
            ));
        }

        record.status = IdempotencyStatus::Failed;
        record.lease_expires_at = None;
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

        sqlx::query(
            r#"
            SELECT pg_advisory_xact_lock(
                hashtextextended($1 || chr(31) || $2, 0)
            )
            "#,
        )
        .bind(&request.scope)
        .bind(&request.idempotency_key)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_IDEMPOTENCY_LOCK_FAILED",
                format!("failed to serialize idempotency key mutation: {error}"),
            )
        })?;

        let existing = sqlx::query(
            r#"
            SELECT request_hash, status, response_json, lease_expires_at, expires_at
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
            let lease_expires_at: Option<OffsetDateTime> =
                row.try_get("lease_expires_at").map_err(|error| {
                    db_error(
                        "POSTGRES_DECODE_FAILED",
                        format!("failed to decode idempotency lease_expires_at: {error}"),
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
                        let result = sqlx::query(
                            r#"
                            UPDATE idempotency_keys
                            SET status = $3, request_id = $4, actor_user_id = $5,
                                actor_session_id = $6, resource_type = $7, resource_id = $8,
                                response_json = NULL, error_code = NULL,
                                lease_expires_at = $9, completed_at = NULL,
                                last_seen_at = $10, expires_at = $11
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
                        .bind(now + IDEMPOTENCY_LEASE)
                        .bind(now)
                        .bind(now + IDEMPOTENCY_RETENTION)
                        .execute(&mut *transaction)
                        .await
                        .map_err(|error| {
                            db_error(
                                "POSTGRES_UPDATE_FAILED",
                                format!("failed to restart failed idempotency row: {error}"),
                            )
                        })?;
                        if result.rows_affected() != 1 {
                            return Err(AppError::conflict(
                                "IDEMPOTENCY_LEASE_LOST",
                                "failed idempotency request changed while it was being restarted",
                            ));
                        }

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
                        if lease_expires_at.is_none_or(|expires_at| expires_at <= now) {
                            let result = sqlx::query(
                                r#"
                                UPDATE idempotency_keys
                                SET request_id = $3,
                                    actor_user_id = $4,
                                    actor_session_id = $5,
                                    resource_type = $6,
                                    resource_id = $7,
                                    response_json = NULL,
                                    error_code = NULL,
                                    lease_expires_at = $8,
                                    completed_at = NULL,
                                    last_seen_at = $9,
                                    expires_at = $10
                                WHERE scope = $1
                                  AND idempotency_key = $2
                                  AND status = 'started'
                                "#,
                            )
                            .bind(&request.scope)
                            .bind(&request.idempotency_key)
                            .bind(&request.request_id)
                            .bind(&request.actor_user_id)
                            .bind(&request.actor_session_id)
                            .bind(&request.resource_type)
                            .bind(&request.resource_id)
                            .bind(now + IDEMPOTENCY_LEASE)
                            .bind(now)
                            .bind(now + IDEMPOTENCY_RETENTION)
                            .execute(&mut *transaction)
                            .await
                            .map_err(|error| {
                                db_error(
                                    "POSTGRES_UPDATE_FAILED",
                                    format!(
                                        "failed to reclaim expired idempotency lease: {error}"
                                    ),
                                )
                            })?;
                            if result.rows_affected() != 1 {
                                return Err(AppError::conflict(
                                    "IDEMPOTENCY_LEASE_LOST",
                                    "idempotency lease changed while it was being reclaimed",
                                ));
                            }
                            transaction.commit().await.map_err(|error| {
                                db_error(
                                    "POSTGRES_TRANSACTION_COMMIT_FAILED",
                                    format!(
                                        "failed to commit idempotency lease reclaim: {error}"
                                    ),
                                )
                            })?;
                            return Ok(IdempotencyBegin::Started);
                        }
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
              error_code,
              first_seen_at,
              last_seen_at,
              lease_expires_at,
              completed_at,
              expires_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NULL, $11, $12, $13, NULL, $14)
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
        .bind(now + IDEMPOTENCY_LEASE)
        .bind(now + IDEMPOTENCY_RETENTION)
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

        let result = sqlx::query(
            r#"
            UPDATE idempotency_keys
            SET status = $3,
                response_json = $4,
                error_code = NULL,
                lease_expires_at = NULL,
                completed_at = $5,
                last_seen_at = $5
            WHERE scope = $1
              AND idempotency_key = $2
              AND request_hash = $6
              AND request_id = $7
              AND status = 'started'
            "#,
        )
        .bind(&request.scope)
        .bind(&request.idempotency_key)
        .bind(IdempotencyStatus::Completed.as_str())
        .bind(Json(response_json))
        .bind(OffsetDateTime::now_utc())
        .bind(&request.request_hash)
        .bind(&request.request_id)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!("failed to complete idempotency row: {error}"),
            )
        })?;
        if result.rows_affected() != 1 {
            return Err(AppError::conflict(
                "IDEMPOTENCY_FINALIZE_CONFLICT",
                "idempotency request could not be completed because its lease was lost",
            ));
        }

        Ok(())
    }

    async fn fail(&self, request: &IdempotencyRequest, error_code: &str) -> Result<()> {
        let result = sqlx::query(
            r#"
            UPDATE idempotency_keys
            SET status = $3,
                last_seen_at = $4,
                error_code = $5,
                lease_expires_at = NULL,
                completed_at = $4
            WHERE scope = $1
              AND idempotency_key = $2
              AND request_hash = $6
              AND request_id = $7
              AND status = 'started'
            "#,
        )
        .bind(&request.scope)
        .bind(&request.idempotency_key)
        .bind(IdempotencyStatus::Failed.as_str())
        .bind(OffsetDateTime::now_utc())
        .bind(error_code)
        .bind(&request.request_hash)
        .bind(&request.request_id)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!("failed to fail idempotency row: {error}"),
            )
        })?;
        if result.rows_affected() != 1 {
            return Err(AppError::conflict(
                "IDEMPOTENCY_FINALIZE_CONFLICT",
                "idempotency request could not be failed because its lease was lost",
            ));
        }

        Ok(())
    }
}
