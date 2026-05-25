async fn healthz(headers: HeaderMap) -> Json<ApiResponse<HealthData>> {
    let request_id = request_id_from_headers(&headers);
    Json(ApiResponse::new(
        HealthData {
            status: "ok".to_string(),
        },
        request_id,
        new_trace_id(),
    ))
}

async fn readyz(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> (StatusCode, Json<ApiResponse<ReadinessData>>) {
    let request_id = request_id_from_headers(&headers);
    let trace_id = new_trace_id();
    let postgres_status = match state.dependencies.postgres_ready().await {
        Ok(()) => DependencyStatus {
            status: "ready".to_string(),
            detail: None,
        },
        Err(error) => DependencyStatus {
            status: "not_ready".to_string(),
            detail: Some(error.message().to_string()),
        },
    };
    let redis_status = match state.dependencies.redis_ready().await {
        Ok(()) => DependencyStatus {
            status: "ready".to_string(),
            detail: None,
        },
        Err(error) => DependencyStatus {
            status: "not_ready".to_string(),
            detail: Some(error.message().to_string()),
        },
    };

    let status = if postgres_status.status == "ready" && redis_status.status == "ready" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (
        status,
        Json(ApiResponse::new(
            ReadinessData {
                status: if status == StatusCode::OK {
                    "ready".to_string()
                } else {
                    "degraded".to_string()
                },
                postgres: postgres_status,
                redis: redis_status,
            },
            request_id,
            trace_id,
        )),
    )
}
