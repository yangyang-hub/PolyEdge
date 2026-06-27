async fn transition_system_mode(
    Extension(auth): Extension<AuthContext>,
    Extension(idempotency_key): Extension<IdempotencyKey>,
    State(state): State<AppState>,
    Json(payload): Json<TransitionSystemModeRequest>,
) -> std::result::Result<Json<ApiResponse<SystemModeData>>, HttpError> {
    if payload.reason.trim().is_empty() {
        return Err(HttpError::with_meta(
            AppError::invalid_input("SYSTEM_MODE_REASON_REQUIRED", "reason must not be empty"),
            auth.request_id.clone(),
            new_trace_id(),
        ));
    }

    let trace_id = new_trace_id();
    let request_hash = hash_json(&payload)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let actor = AuthenticatedActor {
        user_id: auth.user_id.clone(),
        session_id: auth.session_id.clone(),
        roles: auth.roles.clone(),
        request_id: auth.request_id.clone(),
        ip: auth.ip.clone(),
        user_agent: auth.user_agent.clone(),
    };

    let receipt = state
        .system_mode_service
        .transition_mode(ModeTransitionCommand {
            to_mode: payload.to_mode,
            reason: payload.reason,
            request_id: auth.request_id.clone(),
            trace_id: trace_id.clone(),
            idempotency_key: idempotency_key.0,
            request_hash,
            actor,
            required_scope: StepUpScope::SystemModeSwitch,
        })
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        SystemModeData {
            mode: receipt.snapshot.mode,
            environment: receipt.snapshot.environment,
            version: receipt.snapshot.version,
            replayed: receipt.replayed,
            updated_at: receipt.snapshot.updated_at,
        },
        auth.request_id,
        trace_id,
    )))
}
