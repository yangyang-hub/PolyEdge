use super::*;
use argon2::{
    ARGON2ID_IDENT, Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use polyedge_contracts::{
    ActivationTokenData, AdminFinanceSummaryData, CreateUserRequest, CreatedUserData,
    UpdateUserRequest,
};
use polyedge_domain::{ActorScope, UserAccount, UserAuthSource, UserRole, UserStatus};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

static AUTH_HASH_CONCURRENCY: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);

#[derive(Debug, Clone)]
pub struct AuthenticatedSession {
    pub session_id: Uuid,
    pub user: UserAccount,
    pub recent_auth_at: Option<OffsetDateTime>,
    pub expires_at: OffsetDateTime,
}

impl AuthenticatedSession {
    #[must_use]
    pub fn actor(&self) -> ActorScope {
        ActorScope {
            user_id: self.user.id,
            role: self.user.role,
        }
    }
}

impl PostgresStore {
    pub async fn check_login_rate_limit(&self, bucket_key: &str, limit: i64) -> Result<()> {
        let key = bucket_key.trim().to_ascii_lowercase();
        let failures: i64 = sqlx::query_scalar(
            r#"SELECT count(*)::bigint FROM auth_login_attempts
               WHERE username_key=$1 AND succeeded=FALSE
                 AND occurred_at>now()-interval '15 minutes'
                 AND occurred_at>COALESCE((
                   SELECT max(ok.occurred_at) FROM auth_login_attempts ok
                   WHERE ok.username_key=$1 AND ok.succeeded=TRUE
                 ), '-infinity'::timestamptz)"#,
        )
        .bind(&key)
        .fetch_one(&self.pool)
        .await?;
        if failures >= limit {
            return Err(ServerError::RateLimited);
        }
        Ok(())
    }

    pub async fn record_login_attempt(&self, bucket_key: &str, succeeded: bool) -> Result<()> {
        let key = bucket_key.trim().to_ascii_lowercase();
        sqlx::query("INSERT INTO auth_login_attempts(username_key,succeeded) VALUES($1,$2)")
            .bind(key)
            .bind(succeeded)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn bootstrap_environment_admin(
        &self,
        username: &str,
        display_name: &str,
        password_hash: &str,
        credential_version: i64,
    ) -> Result<UserAccount> {
        validate_password_hash(password_hash).await?;
        let username = normalize_username(username)?;
        let display_name = validate_display_name(display_name, "bootstrap admin display name")?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(700319, 1)")
            .execute(&mut *tx)
            .await?;
        let user_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO users (
              username, display_name, role, status, auth_source, credential_version
            ) VALUES ($1, $2, 'admin', 'active', 'environment_admin', $3)
            ON CONFLICT ((lower(username))) DO UPDATE SET
              display_name = CASE
                WHEN users.auth_source = 'environment_admin'
                  AND EXCLUDED.credential_version >= users.credential_version
                THEN EXCLUDED.display_name ELSE users.display_name END,
              credential_version = CASE
                WHEN users.auth_source = 'environment_admin'
                THEN GREATEST(users.credential_version, EXCLUDED.credential_version)
                ELSE users.credential_version END,
              updated_at = now()
            RETURNING user_id
            "#,
        )
        .bind(&username)
        .bind(display_name)
        .bind(credential_version)
        .fetch_one(&mut *tx)
        .await?;
        let source: String = sqlx::query_scalar("SELECT auth_source FROM users WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await?;
        if source != "environment_admin" {
            return Err(ServerError::Conflict(
                "bootstrap admin username belongs to a local user".into(),
            ));
        }
        let previous_credential_version = sqlx::query_scalar::<_, i64>(
            "SELECT credential_version FROM user_password_credentials WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO user_password_credentials (user_id, password_hash, credential_version)
            VALUES ($1, $2, $3)
            ON CONFLICT (user_id) DO UPDATE SET
              password_hash = CASE
                WHEN EXCLUDED.credential_version > user_password_credentials.credential_version
                THEN EXCLUDED.password_hash ELSE user_password_credentials.password_hash END,
              credential_version = GREATEST(
                user_password_credentials.credential_version, EXCLUDED.credential_version
              ),
              updated_at = now()
            "#,
        )
        .bind(user_id)
        .bind(password_hash)
        .bind(credential_version)
        .execute(&mut *tx)
        .await?;
        if previous_credential_version.is_some_and(|version| credential_version > version) {
            sqlx::query(
                "UPDATE user_sessions SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL",
            )
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        self.get_user_by_id(user_id).await
    }

    pub async fn get_user_by_id(&self, user_id: i64) -> Result<UserAccount> {
        let row = sqlx::query(
            r#"SELECT user_id, username, display_name, role, status, auth_source,
                      created_by_user_id, credential_version, created_at, updated_at
               FROM users WHERE user_id = $1"#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| ServerError::NotFound(format!("user {user_id}")))?;
        user_from_row(row)
    }

    pub async fn list_users(&self) -> Result<Vec<UserAccount>> {
        let rows = sqlx::query(
            r#"SELECT user_id, username, display_name, role, status, auth_source,
                      created_by_user_id, credential_version, created_at, updated_at
               FROM users ORDER BY created_at DESC, user_id DESC"#,
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(user_from_row).collect()
    }

    pub async fn create_user(
        &self,
        actor: ActorScope,
        request: &CreateUserRequest,
        activation_ttl: Duration,
        request_id: &str,
    ) -> Result<CreatedUserData> {
        require_admin(actor)?;
        let username = normalize_username(&request.username)?;
        let display_name = validate_display_name(&request.display_name, "display_name")?;
        let raw_token = random_token();
        let token_hash = token_hash(&raw_token);
        let expires_at = OffsetDateTime::now_utc() + activation_ttl;
        let mut tx = self.pool.begin().await?;
        let user_id: i64 = sqlx::query_scalar(
            r#"INSERT INTO users (
                 username, display_name, role, status, auth_source, created_by_user_id
               ) VALUES ($1, $2, $3, 'pending', 'local', $4)
               RETURNING user_id"#,
        )
        .bind(username)
        .bind(display_name)
        .bind(request.role.as_str())
        .bind(actor.user_id)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            r#"INSERT INTO user_activation_tokens (
                 user_id, token_hash, expires_at, created_by_user_id
               ) VALUES ($1, $2, $3, $4)"#,
        )
        .bind(user_id)
        .bind(token_hash.as_slice())
        .bind(expires_at)
        .bind(actor.user_id)
        .execute(&mut *tx)
        .await?;
        insert_user_audit(&mut tx, request_id, actor.user_id, "user.create", user_id).await?;
        tx.commit().await?;
        Ok(CreatedUserData {
            user: self.get_user_by_id(user_id).await?,
            activation_token: raw_token,
            activation_expires_at: expires_at,
        })
    }

    pub async fn update_user(
        &self,
        actor: ActorScope,
        user_id: i64,
        request: &UpdateUserRequest,
        request_id: &str,
    ) -> Result<UserAccount> {
        require_admin(actor)?;
        let target = self.get_user_by_id(user_id).await?;
        if target.auth_source == UserAuthSource::EnvironmentAdmin
            && (request.role.is_some() || request.status.is_some())
        {
            return Err(ServerError::Conflict(
                "environment administrator cannot be disabled or demoted".into(),
            ));
        }
        let display_name = request
            .display_name
            .as_deref()
            .map(|value| validate_display_name(value, "display_name"))
            .transpose()?;
        let role_changed = request.role.is_some_and(|role| role != target.role);
        let status_changed = request.status.is_some_and(|status| status != target.status);
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            r#"UPDATE users SET
                 display_name = COALESCE($2, display_name),
                 role = COALESCE($3, role), status = COALESCE($4, status),
                 updated_at = now()
               WHERE user_id = $1"#,
        )
        .bind(user_id)
        .bind(display_name)
        .bind(request.role.map(UserRole::as_str))
        .bind(request.status.map(UserStatus::as_str))
        .execute(&mut *tx)
        .await?;
        if role_changed || status_changed {
            sqlx::query(
                "UPDATE user_sessions SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL",
            )
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        }
        if request
            .status
            .is_some_and(|status| status != UserStatus::Pending)
        {
            sqlx::query(
                "UPDATE user_activation_tokens SET used_at = now() WHERE user_id = $1 AND used_at IS NULL",
            )
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        }
        insert_user_audit(&mut tx, request_id, actor.user_id, "user.update", user_id).await?;
        tx.commit().await?;
        self.get_user_by_id(user_id).await
    }

    pub async fn reissue_activation_token(
        &self,
        actor: ActorScope,
        user_id: i64,
        activation_ttl: Duration,
        request_id: &str,
    ) -> Result<ActivationTokenData> {
        require_admin(actor)?;
        let raw_token = random_token();
        let digest = token_hash(&raw_token);
        let expires_at = OffsetDateTime::now_utc() + activation_ttl;
        let mut tx = self.pool.begin().await?;
        let eligible = sqlx::query_scalar::<_, bool>(
            r#"SELECT EXISTS (
                 SELECT 1 FROM users
                 WHERE user_id = $1 AND status = 'pending' AND auth_source = 'local'
               )"#,
        )
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await?;
        if !eligible {
            return Err(ServerError::Conflict(
                "activation token can only be issued for a pending local user".into(),
            ));
        }
        sqlx::query(
            "UPDATE user_activation_tokens SET used_at = now() WHERE user_id = $1 AND used_at IS NULL",
        ).bind(user_id).execute(&mut *tx).await?;
        sqlx::query(
            r#"INSERT INTO user_activation_tokens (
                 user_id, token_hash, expires_at, created_by_user_id
               ) VALUES ($1, $2, $3, $4)"#,
        )
        .bind(user_id)
        .bind(digest.as_slice())
        .bind(expires_at)
        .bind(actor.user_id)
        .execute(&mut *tx)
        .await?;
        insert_user_audit(
            &mut tx,
            request_id,
            actor.user_id,
            "user.activation_token.reissue",
            user_id,
        )
        .await?;
        tx.commit().await?;
        Ok(ActivationTokenData {
            user_id,
            activation_token: raw_token,
            activation_expires_at: expires_at,
        })
    }

    pub async fn activate_user(&self, raw_token: &str, password: &str) -> Result<UserAccount> {
        validate_new_password(password)?;
        let digest = token_hash(raw_token);
        let token_exists = sqlx::query_scalar::<_, bool>(
            r#"SELECT EXISTS (
                 SELECT 1 FROM user_activation_tokens t
                 JOIN users u ON u.user_id = t.user_id
                 WHERE t.token_hash = $1 AND t.used_at IS NULL AND t.expires_at > now()
                   AND u.status = 'pending' AND u.auth_source = 'local'
               )"#,
        )
        .bind(digest.as_slice())
        .fetch_one(&self.pool)
        .await?;
        if !token_exists {
            return Err(ServerError::Unauthorized);
        }
        let hash = hash_password(password.to_owned()).await?;
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            r#"SELECT t.activation_token_id, t.user_id
               FROM user_activation_tokens t
               JOIN users u ON u.user_id = t.user_id
               WHERE t.token_hash = $1 AND t.used_at IS NULL AND t.expires_at > now()
                 AND u.status = 'pending' AND u.auth_source = 'local'
               FOR UPDATE"#,
        )
        .bind(digest.as_slice())
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ServerError::Unauthorized)?;
        let activation_id: i64 = row.try_get("activation_token_id")?;
        let user_id: i64 = row.try_get("user_id")?;
        sqlx::query(
            r#"INSERT INTO user_password_credentials (user_id, password_hash, credential_version)
               VALUES ($1, $2, 1)
               ON CONFLICT (user_id) DO UPDATE SET password_hash = EXCLUDED.password_hash,
                 credential_version = user_password_credentials.credential_version + 1,
                 updated_at = now()"#,
        )
        .bind(user_id)
        .bind(hash)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE users SET status = 'active', credential_version = credential_version + 1, updated_at = now() WHERE user_id = $1 AND auth_source = 'local'")
            .bind(user_id).execute(&mut *tx).await?;
        sqlx::query(
            "UPDATE user_activation_tokens SET used_at = now() WHERE activation_token_id = $1",
        )
        .bind(activation_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        self.get_user_by_id(user_id).await
    }

    pub async fn login(
        &self,
        username: &str,
        password: &str,
        idle_ttl: Duration,
        absolute_ttl: Duration,
    ) -> Result<(AuthenticatedSession, String, String)> {
        let row = sqlx::query(
            r#"SELECT u.user_id, p.password_hash
               FROM users u JOIN user_password_credentials p ON p.user_id = u.user_id
               WHERE lower(u.username) = lower($1) AND u.status = 'active'"#,
        )
        .bind(username.trim())
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            verify_dummy_password(password).await;
            return Err(ServerError::Unauthorized);
        };
        let user_id: i64 = row.try_get("user_id")?;
        let password_hash: String = row.try_get("password_hash")?;
        if !verify_password(password.to_owned(), password_hash).await? {
            return Err(ServerError::Unauthorized);
        }
        self.create_session(user_id, idle_ttl, absolute_ttl, true)
            .await
    }

    async fn create_session(
        &self,
        user_id: i64,
        idle_ttl: Duration,
        absolute_ttl: Duration,
        recent: bool,
    ) -> Result<(AuthenticatedSession, String, String)> {
        let session_id = Uuid::now_v7();
        let token = random_token();
        let csrf = random_token();
        let now = OffsetDateTime::now_utc();
        let expires_at = now + idle_ttl;
        let absolute_expires_at = now + absolute_ttl;
        sqlx::query(
            r#"INSERT INTO user_sessions (
                 session_id, user_id, token_hash, csrf_token_hash, recent_auth_at,
                 last_seen_at, expires_at, absolute_expires_at, created_at
               ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#,
        )
        .bind(session_id)
        .bind(user_id)
        .bind(token_hash(&token).as_slice())
        .bind(token_hash(&csrf).as_slice())
        .bind(recent.then_some(now))
        .bind(now)
        .bind(expires_at)
        .bind(absolute_expires_at)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok((
            AuthenticatedSession {
                session_id,
                user: self.get_user_by_id(user_id).await?,
                recent_auth_at: recent.then_some(now),
                expires_at,
            },
            token,
            csrf,
        ))
    }

    pub async fn authenticate_session(
        &self,
        raw_token: &str,
        csrf: Option<&str>,
        require_csrf: bool,
        idle_ttl: Duration,
    ) -> Result<AuthenticatedSession> {
        let row = sqlx::query(
            r#"SELECT s.session_id, s.user_id, s.csrf_token_hash, s.recent_auth_at,
                      s.expires_at, s.absolute_expires_at
               FROM user_sessions s JOIN users u ON u.user_id = s.user_id
               WHERE s.token_hash = $1 AND s.revoked_at IS NULL
                 AND s.expires_at > now() AND s.absolute_expires_at > now()
                 AND u.status = 'active'"#,
        )
        .bind(token_hash(raw_token).as_slice())
        .fetch_optional(&self.pool)
        .await?
        .ok_or(ServerError::Unauthorized)?;
        if require_csrf {
            let supplied = csrf.ok_or(ServerError::Unauthorized)?;
            let expected: Vec<u8> = row.try_get("csrf_token_hash")?;
            if !bool::from(token_hash(supplied).as_slice().ct_eq(expected.as_slice())) {
                return Err(ServerError::Unauthorized);
            }
        }
        let session_id: Uuid = row.try_get("session_id")?;
        let user_id: i64 = row.try_get("user_id")?;
        let absolute: OffsetDateTime = row.try_get("absolute_expires_at")?;
        let expires_at = std::cmp::min(OffsetDateTime::now_utc() + idle_ttl, absolute);
        sqlx::query(
            "UPDATE user_sessions SET last_seen_at = now(), expires_at = $2 WHERE session_id = $1",
        )
        .bind(session_id)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;
        Ok(AuthenticatedSession {
            session_id,
            user: self.get_user_by_id(user_id).await?,
            recent_auth_at: row.try_get("recent_auth_at")?,
            expires_at,
        })
    }

    pub async fn logout(&self, session_id: Uuid) -> Result<()> {
        sqlx::query("UPDATE user_sessions SET revoked_at = now() WHERE session_id = $1")
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn reauthenticate(
        &self,
        session: &AuthenticatedSession,
        password: &str,
        idle_ttl: Duration,
        absolute_ttl: Duration,
    ) -> Result<(AuthenticatedSession, String, String)> {
        let hash: String = sqlx::query_scalar(
            "SELECT password_hash FROM user_password_credentials WHERE user_id = $1",
        )
        .bind(session.user.id)
        .fetch_one(&self.pool)
        .await?;
        if !verify_password(password.to_owned(), hash).await? {
            return Err(ServerError::Unauthorized);
        }
        self.logout(session.session_id).await?;
        self.create_session(session.user.id, idle_ttl, absolute_ttl, true)
            .await
    }

    pub async fn admin_finance_summary(
        &self,
        actor: ActorScope,
    ) -> Result<Vec<AdminFinanceSummaryData>> {
        require_admin(actor)?;
        let rows = sqlx::query(
            r#"SELECT u.user_id, u.username, u.display_name,
                      count(w.wallet_id)::bigint AS wallet_count,
                      COALESCE(sum(COALESCE(e.available_collateral,ws.available_collateral)), 0) AS available_collateral,
                      COALESCE(sum(COALESCE(e.reserved_collateral,ws.reserved_collateral)), 0) AS reserved_collateral,
                      COALESCE(sum(e.position_market_value), 0) AS position_market_value,
                      COALESCE(sum(e.equity), 0) AS equity,
                      COALESCE(sum(e.realized_pnl), 0) AS realized_pnl,
                      COALESCE(sum(e.unrealized_pnl), 0) AS unrealized_pnl,
                      COALESCE(sum(COALESCE(e.total_pnl,0)+COALESCE(f.reward_total,0)-COALESCE(f.fee_total,0)), 0) AS total_pnl,
                      COALESCE(bool_and(e.valuation_complete), false) AS valuation_complete,
                      max(e.observed_at) AS observed_at
               FROM users u LEFT JOIN wallet_accounts w ON w.owner_user_id = u.user_id
               LEFT JOIN wallet_account_state ws ON ws.wallet_id = w.wallet_id
               LEFT JOIN LATERAL (
                 SELECT collateral_balance AS available_collateral,
                        0::numeric AS reserved_collateral,
                        position_market_value,
                        total_equity AS equity,
                        realized_pnl, unrealized_pnl,
                        COALESCE(realized_pnl,0)+COALESCE(unrealized_pnl,0) AS total_pnl,
                        valuation_status='complete' AS valuation_complete, observed_at
                 FROM wallet_equity_snapshots latest
                 WHERE latest.wallet_id=w.wallet_id
                 ORDER BY observed_at DESC,equity_snapshot_id DESC LIMIT 1
               ) e ON TRUE
               LEFT JOIN LATERAL (
                 SELECT COALESCE(sum(amount) FILTER(WHERE flow_type='reward'),0) AS reward_total,
                        COALESCE(sum(amount) FILTER(WHERE flow_type='fee'),0) AS fee_total
                 FROM external_cash_flows flow WHERE flow.wallet_id=w.wallet_id
               ) f ON TRUE
               GROUP BY u.user_id, u.username, u.display_name
               ORDER BY u.username"#,
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(AdminFinanceSummaryData {
                    user_id: row.try_get("user_id")?,
                    username: row.try_get("username")?,
                    display_name: row.try_get("display_name")?,
                    wallet_count: row.try_get("wallet_count")?,
                    available_collateral: row.try_get("available_collateral")?,
                    reserved_collateral: row.try_get("reserved_collateral")?,
                    position_market_value: row.try_get("position_market_value")?,
                    equity: row.try_get("equity")?,
                    realized_pnl: row.try_get("realized_pnl")?,
                    unrealized_pnl: row.try_get("unrealized_pnl")?,
                    total_pnl: row.try_get("total_pnl")?,
                    valuation_complete: row.try_get("valuation_complete")?,
                    observed_at: row.try_get("observed_at")?,
                })
            })
            .collect()
    }
}

async fn insert_user_audit(
    tx: &mut Transaction<'_, Postgres>,
    request_id: &str,
    actor_user_id: i64,
    action: &str,
    target_user_id: i64,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO audit_logs (
      request_id,actor_type,actor_user_id,action,resource_owner_user_id,
      resource_type,resource_id,result
    ) VALUES ($1,'user',$2,$3,$4,'user',$5,'succeeded')"#,
    )
    .bind(request_id)
    .bind(actor_user_id)
    .bind(action)
    .bind(target_user_id)
    .bind(target_user_id.to_string())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn user_from_row(row: sqlx::postgres::PgRow) -> Result<UserAccount> {
    Ok(UserAccount {
        id: row.try_get("user_id")?,
        username: row.try_get("username")?,
        display_name: row.try_get("display_name")?,
        role: UserRole::from_str(row.try_get::<&str, _>("role")?)?,
        status: UserStatus::from_str(row.try_get::<&str, _>("status")?)?,
        auth_source: UserAuthSource::from_str(row.try_get::<&str, _>("auth_source")?)?,
        created_by_user_id: row.try_get("created_by_user_id")?,
        credential_version: row.try_get("credential_version")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn require_admin(actor: ActorScope) -> Result<()> {
    if actor.is_admin() {
        Ok(())
    } else {
        Err(ServerError::Forbidden)
    }
}

fn random_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn normalize_username(value: &str) -> Result<String> {
    let value = value.trim().to_ascii_lowercase();
    if !(3..=64).contains(&value.len())
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(ServerError::InvalidInput(
            "username must contain 3 to 64 ASCII letters, digits, '.', '_' or '-'".into(),
        ));
    }
    Ok(value)
}

fn validate_display_name(value: &str, field: &'static str) -> Result<String> {
    let value = required_text(value, field, 120)?;
    if value.chars().any(char::is_control) {
        return Err(ServerError::InvalidInput(format!(
            "{field} may not contain control characters"
        )));
    }
    Ok(value)
}
fn token_hash(value: &str) -> Vec<u8> {
    Sha256::digest(value.as_bytes()).to_vec()
}

fn validate_new_password(password: &str) -> Result<()> {
    if password.len() < 12 || password.len() > 256 {
        return Err(ServerError::InvalidInput(
            "password must contain 12 to 256 characters".into(),
        ));
    }
    Ok(())
}

async fn hash_password(password: String) -> Result<String> {
    let _permit = AUTH_HASH_CONCURRENCY
        .acquire()
        .await
        .map_err(|_| ServerError::Internal("password hashing capacity is unavailable".into()))?;
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|_| ServerError::Internal("password hashing failed".into()))
    })
    .await
    .map_err(|_| ServerError::Internal("password hashing task failed".into()))?
}

async fn verify_password(password: String, encoded: String) -> Result<bool> {
    let _permit = AUTH_HASH_CONCURRENCY.acquire().await.map_err(|_| {
        ServerError::Internal("password verification capacity is unavailable".into())
    })?;
    tokio::task::spawn_blocking(move || {
        let parsed = PasswordHash::new(&encoded)
            .map_err(|_| ServerError::Internal("stored password hash is invalid".into()))?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    })
    .await
    .map_err(|_| ServerError::Internal("password verification task failed".into()))?
}

async fn validate_password_hash(encoded: &str) -> Result<()> {
    let owned = encoded.to_owned();
    let _permit = AUTH_HASH_CONCURRENCY
        .acquire()
        .await
        .map_err(|_| ServerError::Internal("password validation capacity is unavailable".into()))?;
    tokio::task::spawn_blocking(move || {
        let parsed = PasswordHash::new(&owned).map_err(|_| {
            ServerError::InvalidInput("bootstrap password hash is not valid Argon2id PHC".into())
        })?;
        let memory = parsed.params.get_decimal("m");
        let iterations = parsed.params.get_decimal("t");
        let parallelism = parsed.params.get_decimal("p");
        if parsed.algorithm != ARGON2ID_IDENT
            || parsed.version != Some(19)
            || memory.is_none_or(|value| !(8_192..=262_144).contains(&value))
            || iterations.is_none_or(|value| !(1..=10).contains(&value))
            || parallelism.is_none_or(|value| !(1..=8).contains(&value))
        {
            return Err(ServerError::InvalidInput(
                "bootstrap password hash must use Argon2id v=19 with bounded m/t/p parameters"
                    .into(),
            ));
        }
        Ok(())
    })
    .await
    .map_err(|_| ServerError::Internal("password hash validation task failed".into()))?
}

async fn verify_dummy_password(password: &str) {
    const DUMMY: &str = "$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHQxMjM0NTY3OA$0dZsT3Wzj6wF+Q+gnwo1GQb/lyrT/sqv4h6XQmczRiU";
    let _ = verify_password(password.to_owned(), DUMMY.to_owned()).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bootstrap_password_hash_requires_bounded_argon2id() -> Result<()> {
        let valid = hash_password("test-password-with-enough-length".to_string()).await?;
        validate_password_hash(&valid).await?;

        let wrong_algorithm = valid.replacen("argon2id", "argon2i", 1);
        assert!(validate_password_hash(&wrong_algorithm).await.is_err());

        let excessive_memory = valid.replacen("m=19456", "m=999999", 1);
        assert!(validate_password_hash(&excessive_memory).await.is_err());
        Ok(())
    }

    #[test]
    fn usernames_are_canonical_ascii_identifiers() -> Result<()> {
        assert_eq!(normalize_username(" Maker.Primary-1 ")?, "maker.primary-1");
        assert!(normalize_username("ab").is_err());
        assert!(normalize_username("admin\nroot").is_err());
        assert!(normalize_username("管理员").is_err());
        assert!(validate_display_name("Admin\nRoot", "display_name").is_err());
        Ok(())
    }
}
