// Axum middleware extractors: authenticate console/connector/mode routes, enforce roles and step-up scopes.

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
