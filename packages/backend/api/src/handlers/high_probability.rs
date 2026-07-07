async fn read_high_probability_snapshot(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> std::result::Result<Json<ApiResponse<HighProbabilitySnapshot>>, HttpError> {
    let trace_id = new_trace_id();
    let snapshot = state
        .high_probability_service
        .snapshot()
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(snapshot, auth.request_id, trace_id)))
}

async fn read_high_probability_config(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> std::result::Result<Json<ApiResponse<HighProbabilityConfig>>, HttpError> {
    let trace_id = new_trace_id();
    let config = state
        .high_probability_service
        .read_config()
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(config, auth.request_id, trace_id)))
}

async fn read_high_probability_buckets(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> std::result::Result<Json<ApiResponse<Vec<HighProbabilityBucketStats>>>, HttpError> {
    let trace_id = new_trace_id();
    let snapshot = state
        .high_probability_service
        .snapshot()
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        snapshot.bucket_stats,
        auth.request_id,
        trace_id,
    )))
}

async fn read_high_probability_report(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> std::result::Result<Json<ApiResponse<HighProbabilityResearchReport>>, HttpError> {
    let trace_id = new_trace_id();
    let report = state
        .high_probability_service
        .research_report()
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(report, auth.request_id, trace_id)))
}

async fn read_high_probability_backtests(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> std::result::Result<Json<ApiResponse<HighProbabilityBacktestReport>>, HttpError> {
    let trace_id = new_trace_id();
    let report = state
        .high_probability_service
        .backtest_report()
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(report, auth.request_id, trace_id)))
}

async fn read_high_probability_backtest_runs(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<HashMap<String, String>>,
) -> std::result::Result<Json<ApiResponse<Vec<HighProbabilityBacktestRun>>>, HttpError> {
    let trace_id = new_trace_id();
    let limit = parse_high_probability_limit_query(&query, &auth, &trace_id)?;
    let runs = state
        .high_probability_service
        .list_backtest_runs(limit)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(runs, auth.request_id, trace_id)))
}

async fn read_high_probability_fair_values(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<HashMap<String, String>>,
) -> std::result::Result<Json<ApiResponse<Vec<FairValueEstimate>>>, HttpError> {
    let trace_id = new_trace_id();
    let limit = parse_high_probability_limit_query(&query, &auth, &trace_id)?;
    let fair_values = state
        .high_probability_service
        .list_fair_values(limit)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(fair_values, auth.request_id, trace_id)))
}

async fn read_high_probability_backtest_trades(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Path(run_id): Path<i64>,
    Query(query): Query<HashMap<String, String>>,
) -> std::result::Result<Json<ApiResponse<Vec<HighProbabilityBacktestTrade>>>, HttpError> {
    let trace_id = new_trace_id();
    let limit = parse_high_probability_limit_query(&query, &auth, &trace_id)?;
    let trades = state
        .high_probability_service
        .list_backtest_trades(run_id, limit)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(trades, auth.request_id, trace_id)))
}

fn parse_high_probability_limit_query(
    query: &HashMap<String, String>,
    auth: &AuthContext,
    trace_id: &str,
) -> std::result::Result<Option<u16>, HttpError> {
    let Some(raw_limit) = query.get("limit") else {
        return Ok(None);
    };
    raw_limit
        .parse::<u16>()
        .map(Some)
        .map_err(|error| {
            HttpError::with_meta(
                AppError::invalid_input(
                    "HIGH_PROBABILITY_LIMIT_INVALID",
                    format!("limit must be a positive integer: {error}"),
                ),
                auth.request_id.clone(),
                trace_id.to_string(),
            )
        })
}
