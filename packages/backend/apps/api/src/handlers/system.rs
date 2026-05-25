fn console_runtime_mode(mode: SystemMode) -> SystemMode {
    match mode {
        SystemMode::ManualConfirm => SystemMode::PaperTrade,
        other => other,
    }
}

fn normalize_submit_execution_modes(data: &mut SubmitExecutionData) {
    data.execution_request.mode = console_runtime_mode(data.execution_request.mode);
    data.risk_state.mode = console_runtime_mode(data.risk_state.mode);
}

fn normalize_kill_switch_modes(data: &mut KillSwitchData) {
    data.risk_state.mode = console_runtime_mode(data.risk_state.mode);
}

async fn read_system_mode(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> std::result::Result<Json<ApiResponse<SystemModeData>>, HttpError> {
    let trace_id = new_trace_id();
    let snapshot = state
        .system_mode_service
        .read_mode()
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        SystemModeData {
            mode: console_runtime_mode(snapshot.mode),
            environment: snapshot.environment,
            version: snapshot.version,
            replayed: false,
            updated_at: snapshot.updated_at,
        },
        auth.request_id,
        trace_id,
    )))
}
