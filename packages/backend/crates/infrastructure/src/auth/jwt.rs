// JWT header/claims shapes and low-level token-segment decoding helpers.

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
