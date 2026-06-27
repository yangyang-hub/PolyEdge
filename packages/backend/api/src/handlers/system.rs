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
            mode: snapshot.mode,
            environment: snapshot.environment,
            version: snapshot.version,
            replayed: false,
            updated_at: snapshot.updated_at,
        },
        auth.request_id,
        trace_id,
    )))
}
