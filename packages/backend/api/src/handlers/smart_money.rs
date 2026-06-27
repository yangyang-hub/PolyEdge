async fn read_smart_money_snapshot(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> std::result::Result<Json<ApiResponse<SmartMoneySnapshot>>, HttpError> {
    let trace_id = new_trace_id();
    let snapshot = state
        .smart_money_service
        .snapshot()
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    Ok(Json(ApiResponse::new(snapshot, auth.request_id, trace_id)))
}

async fn update_smart_money_config(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<SmartMoneyConfigPatch>,
) -> std::result::Result<Json<ApiResponse<SmartMoneySnapshot>>, HttpError> {
    let trace_id = new_trace_id();
    state
        .smart_money_service
        .update_config(payload)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let snapshot = state
        .smart_money_service
        .snapshot()
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    Ok(Json(ApiResponse::new(snapshot, auth.request_id, trace_id)))
}

async fn update_smart_money_candidate_status(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<SmartWalletCandidateStatusUpdate>,
) -> std::result::Result<Json<ApiResponse<SmartMoneySnapshot>>, HttpError> {
    let trace_id = new_trace_id();
    state
        .smart_money_service
        .update_candidate_status(payload)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let snapshot = state
        .smart_money_service
        .snapshot()
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    Ok(Json(ApiResponse::new(snapshot, auth.request_id, trace_id)))
}
