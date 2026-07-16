use axum::response::IntoResponse;
use polyedge_contracts::{AuthSessionData, CurrentUserData};

async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> Result<impl IntoResponse> {
    let idle_ttl = time::Duration::try_from(state.config.session_idle_ttl)
        .map_err(|_| ServerError::Configuration("session idle ttl is invalid".into()))?;
    let absolute_ttl = time::Duration::try_from(state.config.session_absolute_ttl)
        .map_err(|_| ServerError::Configuration("session absolute ttl is invalid".into()))?;
    let source = authentication_source(&headers);
    let username_bucket = format!("login:user:{}", request.username.trim().to_ascii_lowercase());
    let source_bucket = format!("{username_bucket}:source:{source}");
    state.store.check_login_rate_limit(&username_bucket, 20).await?;
    state.store.check_login_rate_limit(&source_bucket, 5).await?;
    let login_result = state.store.login(
        &request.username, &request.password, idle_ttl, absolute_ttl,
    ).await;
    state.store.record_login_attempt(&username_bucket, login_result.is_ok()).await?;
    state.store.record_login_attempt(&source_bucket, login_result.is_ok()).await?;
    let (session, token, csrf) = login_result?;
    let context = RequestContext {
        request_id: request_id(&headers), trace_id: trace_id(&headers),
        actor: session.actor(), session_id: session.session_id,
    };
    let data = AuthSessionData {
        user: session.user, csrf_token: csrf.clone(), expires_at: session.expires_at,
        recent_auth_at: session.recent_auth_at,
    };
    let mut response = response(data, &context).into_response();
    let cookie_expires_at = OffsetDateTime::now_utc()
        + time::Duration::try_from(state.config.session_absolute_ttl)
            .map_err(|_| ServerError::Configuration("session absolute ttl is invalid".into()))?;
    append_session_cookies(response.headers_mut(), &state, &token, &csrf, cookie_expires_at)?;
    Ok(response)
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Result<impl IntoResponse> {
    let context = authorize_account_mutation(&state, &headers).await?;
    state.store.logout(context.session_id).await?;
    let mut result = response(
        WriteOperationData { accepted: true, operation_id: format!("op_logout_{}", Uuid::now_v7()),
            resource_id: context.session_id.to_string(), status: "completed".into() }, &context,
    ).into_response();
    clear_session_cookies(result.headers_mut(), &state)?;
    Ok(result)
}

async fn activate_user(
    State(state): State<AppState>, headers: HeaderMap, Json(request): Json<ActivateUserRequest>,
) -> Result<Json<ApiResponse<polyedge_domain::UserAccount>>> {
    let user = state.store.activate_user(&request.token, &request.password).await?;
    let context = anonymous_context(&headers, &user);
    Ok(response(user, &context))
}

async fn reauthenticate(
    State(state): State<AppState>, headers: HeaderMap, Json(request): Json<ReauthenticateRequest>,
) -> Result<impl IntoResponse> {
    let context = authorize_account_mutation(&state, &headers).await?;
    let session = state.store.authenticate_session(
        session_cookie(&headers).ok_or(ServerError::Unauthorized)?,
        csrf_header(&headers), true, time::Duration::try_from(state.config.session_idle_ttl)
            .map_err(|_| ServerError::Configuration("session idle ttl is invalid".into()))?,
    ).await?;
    let idle_ttl = time::Duration::try_from(state.config.session_idle_ttl)
        .map_err(|_| ServerError::Configuration("session idle ttl is invalid".into()))?;
    let absolute_ttl = time::Duration::try_from(state.config.session_absolute_ttl)
        .map_err(|_| ServerError::Configuration("session absolute ttl is invalid".into()))?;
    let rate_limit_key = format!(
        "reauth:{}:{}", session.user.id, authentication_source(&headers),
    );
    state.store.check_login_rate_limit(&rate_limit_key, 5).await?;
    let reauth_result = state.store.reauthenticate(
        &session, &request.password, idle_ttl, absolute_ttl,
    ).await;
    state.store.record_login_attempt(&rate_limit_key, reauth_result.is_ok()).await?;
    let (rotated, token, csrf) = reauth_result?;
    let data = AuthSessionData { user: rotated.user, csrf_token: csrf.clone(),
        expires_at: rotated.expires_at, recent_auth_at: rotated.recent_auth_at };
    let mut result = response(data, &context).into_response();
    let cookie_expires_at = OffsetDateTime::now_utc()+absolute_ttl;
    append_session_cookies(result.headers_mut(), &state, &token, &csrf, cookie_expires_at)?;
    Ok(result)
}

async fn current_user(
    State(state): State<AppState>, headers: HeaderMap,
) -> Result<Json<ApiResponse<CurrentUserData>>> {
    let context = authorize(&state, &headers, None).await?;
    let user = state.store.get_user_by_id(context.actor.user_id).await?;
    let session = state.store.authenticate_session(
        session_cookie(&headers).ok_or(ServerError::Unauthorized)?, None, false,
        time::Duration::try_from(state.config.session_idle_ttl)
            .map_err(|_| ServerError::Configuration("session idle ttl is invalid".into()))?,
    ).await?;
    Ok(response(CurrentUserData { user, recent_auth_at: session.recent_auth_at }, &context))
}

async fn list_users(
    State(state): State<AppState>, headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<polyedge_domain::UserAccount>>>> {
    let context = authorize(&state, &headers, None).await?;
    require_admin_context(&context)?;
    Ok(response(state.store.list_users().await?, &context))
}

async fn create_user(
    State(state): State<AppState>, headers: HeaderMap, Json(request): Json<CreateUserRequest>,
) -> Result<Json<ApiResponse<polyedge_contracts::CreatedUserData>>> {
    let context = authorize(&state, &headers, Some("user_admin")).await?;
    let key = headers.get("idempotency-key").and_then(|value| value.to_str().ok())
        .map(str::trim).filter(|value| !value.is_empty())
        .ok_or_else(|| ServerError::InvalidInput("Idempotency-Key is required".into()))?.to_string();
    let owner_token = match state.store.begin_idempotency(
        context.actor.user_id, "admin.user.create", &key, &hash_json(&request)?,
    ).await? {
        IdempotencyBegin::Started { owner_token } => owner_token,
        IdempotencyBegin::Replay(_) => return Err(ServerError::Conflict(
            "user was already created; activation token is intentionally not replayable".into(),
        )),
    };
    let activation_ttl = time::Duration::try_from(state.config.activation_ttl)
        .map_err(|_| ServerError::Configuration("activation ttl is invalid".into()))?;
    let created = state.store.create_user(
        context.actor, &request, activation_ttl, &context.request_id,
    ).await?;
    state.store.complete_idempotency(
        context.actor.user_id, "admin.user.create", &key, &owner_token,
        &serde_json::json!({"created_user_id": created.user.id}),
    ).await?;
    Ok(response(created, &context))
}

async fn update_user(
    State(state): State<AppState>, headers: HeaderMap, Path(user_id): Path<i64>,
    Json(request): Json<UpdateUserRequest>,
) -> Result<Json<ApiResponse<polyedge_domain::UserAccount>>> {
    let context = authorize(&state, &headers, Some("user_admin")).await?;
    let scope = format!("admin.user.update:{user_id}");
    let lease = begin_identity_write(&state, &headers, &context, &scope, &request).await?;
    if let IdentityWrite::Replay(value) = lease { return Ok(Json(value)); }
    let updated = state.store.update_user(
        context.actor, user_id, &request, &context.request_id,
    ).await?;
    finish_identity_write(&state, &context, &scope, lease, updated).await
}

async fn reissue_activation_token(
    State(state): State<AppState>, headers: HeaderMap, Path(user_id): Path<i64>,
    Json(request): Json<ReissueActivationTokenRequest>,
) -> Result<Json<ApiResponse<polyedge_contracts::ActivationTokenData>>> {
    let context = authorize(&state, &headers, Some("user_admin")).await?;
    require_admin_context(&context)?;
    let key = headers.get("idempotency-key").and_then(|value| value.to_str().ok())
        .map(str::trim).filter(|value| !value.is_empty())
        .ok_or_else(|| ServerError::InvalidInput("Idempotency-Key is required".into()))?.to_string();
    let scope = format!("admin.user.activation_token.reissue:{user_id}");
    let owner_token = match state.store.begin_idempotency(
        context.actor.user_id, &scope, &key, &hash_json(&request)?,
    ).await? {
        IdempotencyBegin::Started { owner_token } => owner_token,
        IdempotencyBegin::Replay(_) => return Err(ServerError::Conflict(
            "activation token was already reissued and is intentionally not replayable".into(),
        )),
    };
    let activation_ttl = time::Duration::try_from(state.config.activation_ttl)
        .map_err(|_| ServerError::Configuration("activation ttl is invalid".into()))?;
    let issued = state.store.reissue_activation_token(
        context.actor, user_id, activation_ttl, &context.request_id,
    ).await?;
    state.store.complete_idempotency(
        context.actor.user_id, &scope, &key, &owner_token,
        &serde_json::json!({"user_id": user_id, "token_issued": true}),
    ).await?;
    Ok(response(issued, &context))
}

async fn admin_finance(
    State(state): State<AppState>, headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<polyedge_contracts::AdminFinanceSummaryData>>>> {
    let context = authorize(&state, &headers, None).await?;
    Ok(response(state.store.admin_finance_summary(context.actor).await?, &context))
}

fn require_admin_context(context: &RequestContext) -> Result<()> {
    if context.actor.is_admin() { Ok(()) } else { Err(ServerError::Forbidden) }
}

enum IdentityWrite<T> {
    Replay(ApiResponse<T>),
    Started { key: String, owner_token: String },
}

async fn begin_identity_write<T: Serialize, R: serde::de::DeserializeOwned>(
    state: &AppState, headers: &HeaderMap, context: &RequestContext, scope: &str, request: &T,
) -> Result<IdentityWrite<R>> {
    let key = headers.get("idempotency-key").and_then(|value| value.to_str().ok())
        .map(str::trim).filter(|value| !value.is_empty())
        .ok_or_else(|| ServerError::InvalidInput("Idempotency-Key is required".into()))?.to_string();
    let request_hash = hash_json(request)?;
    match state.store.begin_idempotency(context.actor.user_id, scope, &key, &request_hash).await? {
        IdempotencyBegin::Replay(value) => Ok(IdentityWrite::Replay(
            serde_json::from_value(value).map_err(|_| ServerError::Internal("invalid identity idempotency response".into()))?
        )),
        IdempotencyBegin::Started { owner_token } => Ok(IdentityWrite::Started { key, owner_token }),
    }
}

async fn finish_identity_write<T: Serialize>(
    state: &AppState, context: &RequestContext, scope: &str, lease: IdentityWrite<T>, data: T,
) -> Result<Json<ApiResponse<T>>> {
    let response = ApiResponse::new(data, &context.request_id, &context.trace_id);
    if let IdentityWrite::Started { key, owner_token } = lease {
        let value = serde_json::to_value(&response)
            .map_err(|_| ServerError::Internal("failed to serialize identity response".into()))?;
        state.store.complete_idempotency(context.actor.user_id, scope, &key, &owner_token, &value).await?;
    }
    Ok(Json(response))
}

fn anonymous_context(headers: &HeaderMap, user: &polyedge_domain::UserAccount) -> RequestContext {
    RequestContext { request_id: request_id(headers), trace_id: trace_id(headers),
        actor: polyedge_domain::ActorScope { user_id: user.id, role: user.role },
        session_id: Uuid::nil() }
}

fn session_cookie(headers: &HeaderMap) -> Option<&str> {
    let cookies = headers.get(header::COOKIE)?.to_str().ok()?;
    cookies.split(';').map(str::trim).find_map(|item| {
        let (name, value) = item.split_once('=')?;
        matches!(name, "__Host-polyedge_session" | "polyedge_session").then_some(value)
    })
}

fn csrf_header(headers: &HeaderMap) -> Option<&str> {
    headers.get("x-csrf-token")
        .or_else(|| headers.get("x-polyedge-csrf-token"))
        .and_then(|value| value.to_str().ok())
}

fn authentication_source(headers: &HeaderMap) -> String {
    headers.get("x-forwarded-for").and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next()).map(str::trim)
        .filter(|value| !value.is_empty() && value.len() <= 64)
        .unwrap_or("direct")
        .replace(|character: char| {
            !character.is_ascii_alphanumeric() && character != '.' && character != ':' && character != '-'
        }, "_")
}

fn append_session_cookies(
    headers: &mut HeaderMap, state: &AppState, token: &str, csrf: &str, expires_at: OffsetDateTime,
) -> Result<()> {
    let max_age = (expires_at - OffsetDateTime::now_utc()).whole_seconds().max(0);
    let production = state.config.environment.eq_ignore_ascii_case("production");
    let (name, secure) = if production { ("__Host-polyedge_session", "; Secure") } else { ("polyedge_session", "") };
    let session = format!("{name}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={max_age}{secure}");
    let csrf_cookie = format!("polyedge_csrf={csrf}; Path=/; SameSite=Strict; Max-Age={max_age}{secure}");
    headers.append(header::SET_COOKIE, HeaderValue::from_str(&session).map_err(|_| ServerError::Internal("invalid session cookie".into()))?);
    headers.append(header::SET_COOKIE, HeaderValue::from_str(&csrf_cookie).map_err(|_| ServerError::Internal("invalid csrf cookie".into()))?);
    Ok(())
}

fn clear_session_cookies(headers: &mut HeaderMap, state: &AppState) -> Result<()> {
    let production = state.config.environment.eq_ignore_ascii_case("production");
    let (name, secure) = if production { ("__Host-polyedge_session", "; Secure") } else { ("polyedge_session", "") };
    for value in [
        format!("{name}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0{secure}"),
        format!("polyedge_csrf=; Path=/; SameSite=Strict; Max-Age=0{secure}"),
    ] {
        headers.append(header::SET_COOKIE, HeaderValue::from_str(&value).map_err(|_| ServerError::Internal("invalid cookie".into()))?);
    }
    Ok(())
}
