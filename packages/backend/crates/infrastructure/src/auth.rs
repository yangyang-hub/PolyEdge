use crate::{
    http::{HttpError, new_trace_id},
    runtime::AppState,
    settings::AuthSettings,
};
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use base64::{Engine, engine::general_purpose};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use polyedge_domain::{AppError, Result, StepUpScope, UserRole};
use serde::Deserialize;
use std::{collections::HashMap, collections::HashSet};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

#[derive(Debug, Clone, Copy)]
pub enum RequestKind {
    Read,
    Write,
}

#[derive(Debug, Clone)]
pub struct IdempotencyKey(pub String);

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: String,
    pub session_id: String,
    pub roles: Vec<UserRole>,
    pub request_id: String,
    pub step_up_verified: bool,
    pub step_up_scopes: Vec<StepUpScope>,
    pub step_up_until: Option<OffsetDateTime>,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
}

impl AuthContext {
    pub fn ensure_any_role(&self, accepted_roles: &[UserRole]) -> Result<()> {
        if self.roles.iter().any(|role| accepted_roles.contains(role)) {
            return Ok(());
        }

        Err(AppError::forbidden(
            "AUTH_ROLE_FORBIDDEN",
            "authenticated actor does not have a permitted role for this route",
        ))
    }

    pub fn ensure_scope(&self, required_scope: StepUpScope, now: OffsetDateTime) -> Result<()> {
        if !self.step_up_verified {
            return Err(AppError::forbidden(
                "AUTH_STEP_UP_REQUIRED",
                "step-up verification is required for this action",
            ));
        }

        if !self.step_up_scopes.contains(&required_scope) {
            return Err(AppError::forbidden(
                "AUTH_STEP_UP_SCOPE_MISSING",
                "required step-up scope is missing",
            ));
        }

        let Some(step_up_until) = self.step_up_until else {
            return Err(AppError::forbidden(
                "AUTH_STEP_UP_EXPIRED",
                "step-up verification is missing an expiry timestamp",
            ));
        };

        if step_up_until < now {
            return Err(AppError::forbidden(
                "AUTH_STEP_UP_EXPIRED",
                "step-up verification is no longer valid",
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct JwtHeader {
    alg: String,
    kid: String,
    typ: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JwtClaims {
    iss: String,
    aud: String,
    sub: String,
    iat: i64,
    nbf: i64,
    exp: i64,
    jti: String,
    session_id: String,
    #[serde(default)]
    roles: Vec<UserRole>,
    auth_time: i64,
    request_id: String,
    #[serde(default)]
    step_up_verified: bool,
    #[serde(default)]
    step_up_scope: Vec<StepUpScope>,
    step_up_until: Option<i64>,
}

pub struct InternalTokenVerifier {
    issuer: String,
    audience: String,
    clock_skew_secs: i64,
    max_query_ttl_secs: i64,
    max_write_ttl_secs: i64,
    max_step_up_window_secs: i64,
    revoked_sessions: HashSet<String>,
    force_reauth_after: Option<OffsetDateTime>,
    keys: HashMap<String, VerifyingKey>,
}

impl InternalTokenVerifier {
    pub fn from_settings(settings: &AuthSettings) -> Result<Self> {
        let mut keys = HashMap::new();

        for key in &settings.keys {
            let decoded = general_purpose::STANDARD
                .decode(&key.public_key_base64)
                .map_err(|error| {
                    AppError::internal(
                        "AUTH_PUBLIC_KEY_DECODE_FAILED",
                        format!("failed to decode public key {}: {error}", key.kid),
                    )
                })?;

            let raw_key: [u8; 32] = decoded.try_into().map_err(|_| {
                AppError::internal(
                    "AUTH_PUBLIC_KEY_LENGTH_INVALID",
                    format!("public key {} must be 32 bytes", key.kid),
                )
            })?;

            let verifying_key = VerifyingKey::from_bytes(&raw_key).map_err(|error| {
                AppError::internal(
                    "AUTH_PUBLIC_KEY_INVALID",
                    format!("public key {} is invalid: {error}", key.kid),
                )
            })?;

            keys.insert(key.kid.clone(), verifying_key);
        }

        let force_reauth_after = settings
            .force_reauth_after
            .as_deref()
            .map(|value| OffsetDateTime::parse(value, &Rfc3339))
            .transpose()
            .map_err(|error| {
                AppError::internal(
                    "AUTH_FORCE_REAUTH_AFTER_INVALID",
                    format!("invalid force_reauth_after timestamp: {error}"),
                )
            })?;

        Ok(Self {
            issuer: settings.issuer.clone(),
            audience: settings.audience.clone(),
            clock_skew_secs: settings.clock_skew_secs,
            max_query_ttl_secs: settings.max_query_ttl_secs,
            max_write_ttl_secs: settings.max_write_ttl_secs,
            max_step_up_window_secs: settings.max_step_up_window_secs,
            revoked_sessions: settings.revoked_sessions.iter().cloned().collect(),
            force_reauth_after,
            keys,
        })
    }

    pub fn authenticate(
        &self,
        token: &str,
        request_id_header: &str,
        kind: RequestKind,
        client_ip: Option<String>,
        client_user_agent: Option<String>,
    ) -> Result<AuthContext> {
        let (header_part, claims_part, signature_part) = split_token(token)?;
        let header: JwtHeader = decode_json(header_part)?;
        let claims: JwtClaims = decode_json(claims_part)?;

        if header.alg != "EdDSA" {
            return Err(AppError::unauthorized(
                "AUTH_INVALID_INTERNAL_TOKEN",
                "token algorithm must be EdDSA",
            ));
        }

        if header.typ.as_deref() != Some("JWT") {
            return Err(AppError::unauthorized(
                "AUTH_INVALID_INTERNAL_TOKEN",
                "token type must be JWT",
            ));
        }

        let Some(verifying_key) = self.keys.get(&header.kid) else {
            return Err(AppError::unauthorized(
                "AUTH_INVALID_INTERNAL_TOKEN",
                "token kid is not recognized",
            ));
        };

        let signature_bytes = general_purpose::URL_SAFE_NO_PAD
            .decode(signature_part)
            .map_err(|error| {
                AppError::unauthorized(
                    "AUTH_INVALID_INTERNAL_TOKEN",
                    format!("invalid token signature encoding: {error}"),
                )
            })?;

        let signature = Signature::from_slice(&signature_bytes).map_err(|error| {
            AppError::unauthorized(
                "AUTH_INVALID_INTERNAL_TOKEN",
                format!("invalid token signature bytes: {error}"),
            )
        })?;

        let signed_payload = format!("{header_part}.{claims_part}");
        verifying_key
            .verify(signed_payload.as_bytes(), &signature)
            .map_err(|error| {
                AppError::unauthorized(
                    "AUTH_INVALID_INTERNAL_TOKEN",
                    format!("token signature verification failed: {error}"),
                )
            })?;

        if claims.iss != self.issuer || claims.aud != self.audience {
            return Err(AppError::unauthorized(
                "AUTH_INVALID_AUDIENCE",
                "token issuer or audience does not match",
            ));
        }

        if claims.sub.trim().is_empty()
            || claims.session_id.trim().is_empty()
            || claims.request_id.trim().is_empty()
            || claims.jti.trim().is_empty()
            || claims.roles.is_empty()
        {
            return Err(AppError::unauthorized(
                "AUTH_INVALID_INTERNAL_TOKEN",
                "required token claims are missing",
            ));
        }

        if request_id_header != claims.request_id {
            return Err(AppError::unauthorized(
                "AUTH_INVALID_INTERNAL_TOKEN",
                "request id header does not match token claim",
            ));
        }

        let ttl_secs = claims.exp - claims.iat;
        let max_ttl = match kind {
            RequestKind::Read => self.max_query_ttl_secs,
            RequestKind::Write => self.max_write_ttl_secs,
        };

        if ttl_secs <= 0 || ttl_secs > max_ttl {
            return Err(AppError::unauthorized(
                "AUTH_INVALID_INTERNAL_TOKEN",
                "token ttl is outside the allowed window",
            ));
        }

        let now = OffsetDateTime::now_utc().unix_timestamp();
        let skew = self.clock_skew_secs;
        if claims.nbf > now + skew || claims.exp < now - skew {
            return Err(AppError::unauthorized(
                "AUTH_TOKEN_EXPIRED",
                "token is not currently valid",
            ));
        }

        if self.revoked_sessions.contains(&claims.session_id) {
            return Err(AppError::unauthorized(
                "AUTH_INVALID_SESSION",
                "session has been revoked",
            ));
        }

        let auth_time = OffsetDateTime::from_unix_timestamp(claims.auth_time).map_err(|error| {
            AppError::unauthorized(
                "AUTH_INVALID_INTERNAL_TOKEN",
                format!("invalid auth_time claim: {error}"),
            )
        })?;

        if let Some(force_reauth_after) = self.force_reauth_after {
            if auth_time < force_reauth_after {
                return Err(AppError::unauthorized(
                    "AUTH_INVALID_SESSION",
                    "account requires fresh authentication",
                ));
            }
        }

        let step_up_until = claims
            .step_up_until
            .map(OffsetDateTime::from_unix_timestamp)
            .transpose()
            .map_err(|error| {
                AppError::unauthorized(
                    "AUTH_INVALID_INTERNAL_TOKEN",
                    format!("invalid step_up_until claim: {error}"),
                )
            })?;

        if let Some(step_up_until) = step_up_until {
            let request_start =
                OffsetDateTime::from_unix_timestamp(claims.nbf).map_err(|error| {
                    AppError::unauthorized(
                        "AUTH_INVALID_INTERNAL_TOKEN",
                        format!("invalid nbf claim: {error}"),
                    )
                })?;

            if (step_up_until - request_start).whole_seconds() > self.max_step_up_window_secs {
                return Err(AppError::unauthorized(
                    "AUTH_INVALID_INTERNAL_TOKEN",
                    "step-up window exceeds the allowed duration",
                ));
            }
        }

        Ok(AuthContext {
            user_id: claims.sub,
            session_id: claims.session_id,
            roles: claims.roles,
            request_id: claims.request_id,
            step_up_verified: claims.step_up_verified,
            step_up_scopes: claims.step_up_scope,
            step_up_until,
            ip: client_ip,
            user_agent: client_user_agent,
        })
    }
}

pub async fn require_console_read_auth(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> std::result::Result<Response, HttpError> {
    let auth = authenticate_request(&state, &request, RequestKind::Read)?;
    auth.ensure_any_role(&[
        UserRole::Viewer,
        UserRole::Operator,
        UserRole::RiskAdmin,
        UserRole::Admin,
    ])
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), new_trace_id()))?;

    request.extensions_mut().insert(auth);
    Ok(next.run(request).await)
}

pub async fn require_console_write_auth(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> std::result::Result<Response, HttpError> {
    let auth = authenticate_request(&state, &request, RequestKind::Write)?;
    auth.ensure_any_role(&[UserRole::Operator, UserRole::RiskAdmin, UserRole::Admin])
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), new_trace_id()))?;

    let idempotency_key = extract_idempotency_key(&request, &auth)?;
    request.extensions_mut().insert(auth);
    request
        .extensions_mut()
        .insert(IdempotencyKey(idempotency_key));
    Ok(next.run(request).await)
}

pub async fn require_connector_write_auth(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> std::result::Result<Response, HttpError> {
    let auth = authenticate_request(&state, &request, RequestKind::Write)?;
    auth.ensure_any_role(&[UserRole::Operator, UserRole::RiskAdmin, UserRole::Admin])
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), new_trace_id()))?;

    request.extensions_mut().insert(auth);
    Ok(next.run(request).await)
}

pub async fn require_mode_write_auth(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> std::result::Result<Response, HttpError> {
    let auth = authenticate_request(&state, &request, RequestKind::Write)?;
    auth.ensure_any_role(&[UserRole::RiskAdmin, UserRole::Admin])
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), new_trace_id()))?;
    auth.ensure_scope(StepUpScope::SystemModeSwitch, OffsetDateTime::now_utc())
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), new_trace_id()))?;

    let idempotency_key = extract_idempotency_key(&request, &auth)?;

    request.extensions_mut().insert(auth);
    request
        .extensions_mut()
        .insert(IdempotencyKey(idempotency_key));
    Ok(next.run(request).await)
}

fn extract_idempotency_key(
    request: &Request,
    auth: &AuthContext,
) -> std::result::Result<String, HttpError> {
    request
        .headers()
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .map(std::borrow::ToOwned::to_owned)
        .ok_or_else(|| {
            HttpError::with_meta(
                AppError::invalid_input(
                    "IDEMPOTENCY_KEY_REQUIRED",
                    "write routes require Idempotency-Key",
                ),
                auth.request_id.clone(),
                new_trace_id(),
            )
        })
}

fn authenticate_request(
    state: &AppState,
    request: &Request,
    kind: RequestKind,
) -> std::result::Result<AuthContext, HttpError> {
    let headers = request.headers();
    let request_id = headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .map(std::borrow::ToOwned::to_owned)
        .ok_or_else(|| {
            HttpError::with_meta(
                AppError::unauthorized(
                    "AUTH_INVALID_INTERNAL_TOKEN",
                    "X-Request-Id header is required for authenticated requests",
                ),
                "unknown",
                new_trace_id(),
            )
        })?;

    if let Some(auth) = authenticate_local_dev_request(state, request, &request_id)? {
        return Ok(auth);
    }

    let token = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            HttpError::with_meta(
                AppError::unauthorized(
                    "AUTH_INVALID_INTERNAL_TOKEN",
                    "Authorization bearer token is required",
                ),
                request_id.clone(),
                new_trace_id(),
            )
        })?;

    let client_ip = headers
        .get("x-client-ip")
        .and_then(|value| value.to_str().ok())
        .map(std::borrow::ToOwned::to_owned);
    let client_user_agent = headers
        .get("x-client-user-agent")
        .and_then(|value| value.to_str().ok())
        .map(std::borrow::ToOwned::to_owned);

    state
        .auth_verifier
        .authenticate(token, &request_id, kind, client_ip, client_user_agent)
        .map_err(|error| HttpError::with_meta(error, request_id, new_trace_id()))
}

fn authenticate_local_dev_request(
    state: &AppState,
    request: &Request,
    request_id: &str,
) -> std::result::Result<Option<AuthContext>, HttpError> {
    if state.settings.runtime.environment != "local" || !state.settings.auth.keys.is_empty() {
        return Ok(None);
    }

    let headers = request.headers();
    let dev_auth = headers
        .get("x-polyedge-dev-auth")
        .and_then(|value| value.to_str().ok());

    if dev_auth != Some("local") {
        return Ok(None);
    }

    let role = headers
        .get("x-polyedge-console-role")
        .and_then(|value| value.to_str().ok())
        .map(parse_dev_role)
        .transpose()
        .map_err(|error| HttpError::with_meta(error, request_id.to_string(), new_trace_id()))?
        .unwrap_or(UserRole::Viewer);
    let actor = headers
        .get("x-polyedge-console-user")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("local-console");
    let actor_id = normalize_dev_actor(actor);
    let step_up_verified = headers
        .get("x-polyedge-step-up-verified")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == "true");
    let step_up_scopes = if step_up_verified {
        headers
            .get("x-polyedge-step-up-scopes")
            .and_then(|value| value.to_str().ok())
            .map(parse_dev_step_up_scopes)
            .transpose()
            .map_err(|error| HttpError::with_meta(error, request_id.to_string(), new_trace_id()))?
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let client_ip = headers
        .get("x-client-ip")
        .and_then(|value| value.to_str().ok())
        .map(std::borrow::ToOwned::to_owned);
    let client_user_agent = headers
        .get("x-client-user-agent")
        .and_then(|value| value.to_str().ok())
        .map(std::borrow::ToOwned::to_owned);
    let now = OffsetDateTime::now_utc();

    Ok(Some(AuthContext {
        user_id: format!("usr_{actor_id}"),
        session_id: format!("sess_local_{actor_id}"),
        roles: vec![role],
        request_id: request_id.to_string(),
        step_up_verified,
        step_up_scopes,
        step_up_until: step_up_verified
            .then(|| now + time::Duration::seconds(state.settings.auth.max_step_up_window_secs)),
        ip: client_ip,
        user_agent: client_user_agent,
    }))
}

fn parse_dev_role(value: &str) -> Result<UserRole> {
    match value {
        "viewer" => Ok(UserRole::Viewer),
        "operator" => Ok(UserRole::Operator),
        "risk_admin" => Ok(UserRole::RiskAdmin),
        "admin" => Ok(UserRole::Admin),
        _ => Err(AppError::unauthorized(
            "AUTH_DEV_ROLE_INVALID",
            format!("invalid local dev role: {value}"),
        )),
    }
}

fn parse_dev_step_up_scopes(value: &str) -> Result<Vec<StepUpScope>> {
    value
        .split(',')
        .filter(|scope| !scope.trim().is_empty())
        .map(|scope| match scope.trim() {
            "signal_approve" => Ok(StepUpScope::SignalApprove),
            "signal_reject" => Ok(StepUpScope::SignalReject),
            "execution_submit" => Ok(StepUpScope::ExecutionSubmit),
            "order_cancel_force" => Ok(StepUpScope::OrderCancelForce),
            "system_mode_switch" => Ok(StepUpScope::SystemModeSwitch),
            "system_kill_switch_trigger" => Ok(StepUpScope::SystemKillSwitchTrigger),
            "system_kill_switch_release" => Ok(StepUpScope::SystemKillSwitchRelease),
            "risk_threshold_update" => Ok(StepUpScope::RiskThresholdUpdate),
            other => Err(AppError::unauthorized(
                "AUTH_DEV_STEP_UP_SCOPE_INVALID",
                format!("invalid local dev step-up scope: {other}"),
            )),
        })
        .collect()
}

fn normalize_dev_actor(value: &str) -> String {
    let normalized: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let normalized = normalized.trim_matches('_');

    if normalized.is_empty() {
        "local_console".to_string()
    } else {
        normalized.to_string()
    }
}

fn split_token(token: &str) -> Result<(&str, &str, &str)> {
    let mut parts = token.split('.');
    let Some(header) = parts.next() else {
        return Err(AppError::unauthorized(
            "AUTH_INVALID_INTERNAL_TOKEN",
            "token must contain header, payload and signature",
        ));
    };
    let Some(payload) = parts.next() else {
        return Err(AppError::unauthorized(
            "AUTH_INVALID_INTERNAL_TOKEN",
            "token must contain header, payload and signature",
        ));
    };
    let Some(signature) = parts.next() else {
        return Err(AppError::unauthorized(
            "AUTH_INVALID_INTERNAL_TOKEN",
            "token must contain header, payload and signature",
        ));
    };

    if parts.next().is_some() {
        return Err(AppError::unauthorized(
            "AUTH_INVALID_INTERNAL_TOKEN",
            "token must contain exactly three segments",
        ));
    }

    Ok((header, payload, signature))
}

fn decode_json<T: for<'de> Deserialize<'de>>(part: &str) -> Result<T> {
    let decoded = general_purpose::URL_SAFE_NO_PAD
        .decode(part)
        .map_err(|error| {
            AppError::unauthorized(
                "AUTH_INVALID_INTERNAL_TOKEN",
                format!("failed to decode token segment: {error}"),
            )
        })?;

    serde_json::from_slice(&decoded).map_err(|error| {
        AppError::unauthorized(
            "AUTH_INVALID_INTERNAL_TOKEN",
            format!("failed to decode token json: {error}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use ed25519_dalek::{Signer, SigningKey};
    use serde::Serialize;

    #[derive(Serialize)]
    struct TestHeader<'a> {
        alg: &'a str,
        kid: &'a str,
        typ: &'a str,
    }

    #[derive(Serialize)]
    struct TestClaims {
        iss: String,
        aud: String,
        sub: String,
        iat: i64,
        nbf: i64,
        exp: i64,
        jti: String,
        session_id: String,
        roles: Vec<UserRole>,
        auth_time: i64,
        request_id: String,
        step_up_verified: bool,
        step_up_scope: Vec<StepUpScope>,
        step_up_until: Option<i64>,
    }

    fn issue_token(
        signing_key: &SigningKey,
        kid: &str,
        request_id: &str,
        include_scope: bool,
    ) -> String {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        let header = serde_json::to_vec(&TestHeader {
            alg: "EdDSA",
            kid,
            typ: "JWT",
        })
        .expect("serialize header");
        let claims = serde_json::to_vec(&TestClaims {
            iss: "polyedge-nextjs".to_string(),
            aud: "polyedge-rust-api".to_string(),
            sub: "usr_123".to_string(),
            iat: now,
            nbf: now,
            exp: now + 30,
            jti: "jit_123".to_string(),
            session_id: "sess_123".to_string(),
            roles: vec![UserRole::RiskAdmin],
            auth_time: now - 60,
            request_id: request_id.to_string(),
            step_up_verified: include_scope,
            step_up_scope: if include_scope {
                vec![StepUpScope::SystemModeSwitch]
            } else {
                Vec::new()
            },
            step_up_until: if include_scope { Some(now + 60) } else { None },
        })
        .expect("serialize claims");

        let header_b64 = general_purpose::URL_SAFE_NO_PAD.encode(header);
        let claims_b64 = general_purpose::URL_SAFE_NO_PAD.encode(claims);
        let message = format!("{header_b64}.{claims_b64}");
        let signature = signing_key.sign(message.as_bytes());
        let signature_b64 = general_purpose::URL_SAFE_NO_PAD.encode(signature.to_bytes());
        format!("{message}.{signature_b64}")
    }

    #[test]
    fn verifier_accepts_valid_token() {
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let public_key = general_purpose::STANDARD.encode(signing_key.verifying_key().as_bytes());
        let settings = AuthSettings {
            issuer: "polyedge-nextjs".to_string(),
            audience: "polyedge-rust-api".to_string(),
            clock_skew_secs: 30,
            max_query_ttl_secs: 60,
            max_write_ttl_secs: 30,
            max_step_up_window_secs: 600,
            revoked_sessions: Vec::new(),
            force_reauth_after: None,
            keys: vec![crate::settings::AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: public_key,
            }],
        };
        let verifier = InternalTokenVerifier::from_settings(&settings).expect("verifier");
        let token = issue_token(&signing_key, "test-key", "req_123", true);

        let auth = verifier
            .authenticate(
                &token,
                "req_123",
                RequestKind::Write,
                Some("127.0.0.1".to_string()),
                Some("test-agent".to_string()),
            )
            .expect("authenticate");

        assert_eq!(auth.user_id, "usr_123");
        assert!(auth.step_up_verified);
    }

    #[test]
    fn verifier_rejects_request_id_mismatch() {
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let public_key = general_purpose::STANDARD.encode(signing_key.verifying_key().as_bytes());
        let settings = AuthSettings {
            issuer: "polyedge-nextjs".to_string(),
            audience: "polyedge-rust-api".to_string(),
            clock_skew_secs: 30,
            max_query_ttl_secs: 60,
            max_write_ttl_secs: 30,
            max_step_up_window_secs: 600,
            revoked_sessions: Vec::new(),
            force_reauth_after: None,
            keys: vec![crate::settings::AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: public_key,
            }],
        };
        let verifier = InternalTokenVerifier::from_settings(&settings).expect("verifier");
        let token = issue_token(&signing_key, "test-key", "req_123", false);

        let error = verifier
            .authenticate(&token, "req_other", RequestKind::Read, None, None)
            .expect_err("request id mismatch should fail");

        assert_eq!(error.code(), "AUTH_INVALID_INTERNAL_TOKEN");
    }
}
