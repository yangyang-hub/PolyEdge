// Request authentication: header extraction, local-dev shortcut dispatch, and verifier invocation.

fn authenticate_request(
    state: &AppState,
    request: &Request,
    kind: RequestKind,
) -> std::result::Result<AuthContext, HttpError> {
    let headers = request.headers();
    if state.settings.auth.disabled {
        return Ok(auth_disabled_context(request));
    }

    let is_local_mode =
        state.settings.runtime.environment == "local" && state.settings.auth.keys.is_empty();

    // In local mode, generate a fallback request ID when the header is missing
    // (e.g. EventSource connections cannot set custom headers).
    let request_id = headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .map(std::borrow::ToOwned::to_owned)
        .or_else(|| is_local_mode.then(|| format!("req_local_{}", new_trace_id())))
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

    // In local mode with no auth keys configured, requests without dev-auth headers
    // (e.g. EventSource which cannot set custom headers) are granted viewer access.
    if is_local_mode {
        let client_ip = headers
            .get("x-client-ip")
            .and_then(|value| value.to_str().ok())
            .map(std::borrow::ToOwned::to_owned);
        let client_user_agent = headers
            .get("x-client-user-agent")
            .and_then(|value| value.to_str().ok())
            .map(std::borrow::ToOwned::to_owned);
        return Ok(AuthContext {
            user_id: "usr_local_anonymous".to_string(),
            session_id: "sess_local_anonymous".to_string(),
            roles: vec![UserRole::Viewer],
            request_id,
            step_up_verified: false,
            step_up_scopes: Vec::new(),
            step_up_until: None,
            ip: client_ip,
            user_agent: client_user_agent,
        });
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

    // In local mode: require X-PolyEdge-Dev-Auth header.
    // EventSource cannot set custom headers, so requests without the header
    // are rejected here and fall through to JWT auth (which will also fail).
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

    // Step-up verification: compare client-sent code against configured secret.
    let configured_code = state.settings.auth.step_up_code.trim();
    let step_up_code = headers
        .get("x-polyedge-step-up-code")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .trim();
    let step_up_verified = !configured_code.is_empty()
        && !step_up_code.is_empty()
        && step_up_code == configured_code;

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

fn auth_disabled_context(request: &Request) -> AuthContext {
    let headers = request.headers();
    let request_id = headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .map(std::borrow::ToOwned::to_owned)
        .unwrap_or_else(|| format!("req_intranet_{}", new_trace_id()));
    let client_ip = headers
        .get("x-client-ip")
        .and_then(|value| value.to_str().ok())
        .map(std::borrow::ToOwned::to_owned);
    let client_user_agent = headers
        .get("x-client-user-agent")
        .and_then(|value| value.to_str().ok())
        .map(std::borrow::ToOwned::to_owned);

    AuthContext {
        user_id: "usr_intranet_admin".to_string(),
        session_id: "sess_intranet_admin".to_string(),
        roles: vec![UserRole::Admin],
        request_id,
        step_up_verified: true,
        step_up_scopes: vec![
            StepUpScope::SignalApprove,
            StepUpScope::SignalReject,
            StepUpScope::ExecutionSubmit,
            StepUpScope::OrderCancelForce,
            StepUpScope::SystemModeSwitch,
            StepUpScope::SystemKillSwitchTrigger,
            StepUpScope::SystemKillSwitchRelease,
            StepUpScope::RiskThresholdUpdate,
            StepUpScope::FundingTransfer,
        ],
        step_up_until: Some(OffsetDateTime::now_utc() + time::Duration::days(3650)),
        ip: client_ip,
        user_agent: client_user_agent,
    }
}
