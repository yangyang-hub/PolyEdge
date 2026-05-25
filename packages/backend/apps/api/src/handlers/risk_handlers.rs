async fn read_risk_state(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> std::result::Result<Json<ApiResponse<RiskStateData>>, HttpError> {
    let trace_id = new_trace_id();
    let snapshot = read_console_risk_snapshot(&state)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let open_alerts = snapshot
        .alerts
        .iter()
        .filter(|alert| alert.status != AlertStatus::Contained)
        .count()
        .try_into()
        .unwrap_or(u32::MAX);

    Ok(Json(ApiResponse::new(
        risk_state_to_contract(
            snapshot.risk_state,
            snapshot.environment,
            state.risk_service.policy(),
            Some(open_alerts),
        )
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?,
        auth.request_id,
        trace_id,
    )))
}

async fn list_risk_alerts(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<RiskAlertListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<RiskAlertData>>>, HttpError> {
    let trace_id = new_trace_id();
    let snapshot = read_console_risk_snapshot(&state)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let alerts = snapshot
        .alerts
        .into_iter()
        .filter(|alert| query.status.is_none_or(|status| alert.status == status))
        .collect::<Vec<_>>();

    Ok(Json(ApiResponse::new(
        apply_limit(alerts, query.limit),
        auth.request_id,
        trace_id,
    )))
}

async fn list_risk_buckets(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<RiskBucketListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<RiskBucketData>>>, HttpError> {
    let trace_id = new_trace_id();
    let snapshot = read_console_risk_snapshot(&state)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        apply_limit(snapshot.buckets, query.limit),
        auth.request_id,
        trace_id,
    )))
}
