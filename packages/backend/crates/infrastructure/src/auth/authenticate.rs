// Request authentication: header extraction, local-dev shortcut dispatch, and verifier invocation.

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
