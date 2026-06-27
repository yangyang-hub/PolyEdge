async fn read_runtime_config(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> std::result::Result<Json<ApiResponse<Vec<RuntimeConfigEntryData>>>, HttpError> {
    let trace_id = new_trace_id();
    let entries = runtime_config_entries_for_state(&state)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        entries
            .into_iter()
            .map(runtime_config_entry_to_contract)
            .collect(),
        auth.request_id,
        trace_id,
    )))
}

async fn update_runtime_config(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateRuntimeConfigRequest>,
) -> std::result::Result<Json<ApiResponse<Vec<RuntimeConfigEntryData>>>, HttpError> {
    let trace_id = new_trace_id();
    polyedge_infrastructure::Settings::validate_runtime_config_keys(&payload.values)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    let sanitized_values = payload
        .values
        .into_iter()
        .map(|(key, value)| (key, value.trim().to_string()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut merged_values = state
        .runtime_config_store
        .load_values()
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    merged_values.extend(sanitized_values.clone());

    let mut candidate = (*state.settings).clone();
    candidate
        .apply_runtime_config_values(merged_values)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    state
        .runtime_config_store
        .save_values(&sanitized_values)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        candidate
            .runtime_config_entries()
            .into_iter()
            .map(runtime_config_entry_to_contract)
            .collect(),
        auth.request_id,
        trace_id,
    )))
}
