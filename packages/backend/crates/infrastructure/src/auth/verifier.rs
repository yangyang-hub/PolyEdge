// Internal EdDSA JWT verifier: loads signing keys from settings and validates internal tokens.

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
