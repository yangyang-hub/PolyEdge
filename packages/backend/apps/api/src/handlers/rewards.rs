async fn read_reward_bot_snapshot(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<RewardBotSnapshotQuery>,
) -> std::result::Result<Json<ApiResponse<RewardBotSnapshot>>, HttpError> {
    let trace_id = new_trace_id();
    let order_query = RewardOrderListQuery::new(
        query.orders_search.clone(),
        query.orders_status.clone(),
        query.orders_sort_by.clone(),
        query.orders_sort_order.clone(),
        query.orders_page,
        query.orders_page_size,
    );
    let plans_query = RewardQuotePlanListQuery::new(
        query.plans_search.clone(),
        query.plans_eligible,
        query.plans_sort_by.clone(),
        query.plans_sort_order.clone(),
        query.plans_page,
        query.plans_page_size,
    );
    let snapshot = state
        .reward_bot_service
        .snapshot_with_order_query(&order_query, &plans_query)
        .await
        .map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
        })?;

    Ok(Json(ApiResponse::new(snapshot, auth.request_id, trace_id)))
}

async fn update_reward_bot_config(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Json(payload): Json<RewardBotConfigPatch>,
) -> std::result::Result<Json<ApiResponse<RewardBotSnapshot>>, HttpError> {
    let trace_id = new_trace_id();
    state
        .reward_bot_service
        .update_config(payload)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let snapshot =
        state.reward_bot_service.snapshot().await.map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
        })?;

    Ok(Json(ApiResponse::new(snapshot, auth.request_id, trace_id)))
}

async fn run_reward_bot_once(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> std::result::Result<Json<ApiResponse<RewardBotSnapshot>>, HttpError> {
    let trace_id = new_trace_id();
    state
        .reward_bot_service
        .enqueue_control_command(
            RewardControlAction::RunOnce,
            "operator requested one rewards strategy tick",
            &trace_id,
        )
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let snapshot =
        state.reward_bot_service.snapshot().await.map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
        })?;

    Ok(Json(ApiResponse::new(snapshot, auth.request_id, trace_id)))
}

async fn cancel_reward_bot_orders(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> std::result::Result<Json<ApiResponse<RewardBotSnapshot>>, HttpError> {
    let trace_id = new_trace_id();
    state
        .reward_bot_service
        .enqueue_control_command(
            RewardControlAction::CancelAll,
            "operator requested cancelling all rewards orders",
            &trace_id,
        )
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let snapshot =
        state.reward_bot_service.snapshot().await.map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
        })?;

    Ok(Json(ApiResponse::new(snapshot, auth.request_id, trace_id)))
}

async fn reset_reward_bot(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> std::result::Result<Json<ApiResponse<RewardBotSnapshot>>, HttpError> {
    let trace_id = new_trace_id();
    state
        .reward_bot_service
        .enqueue_control_command(
            RewardControlAction::Reset,
            "operator requested resetting rewards validation state",
            &trace_id,
        )
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let snapshot =
        state.reward_bot_service.snapshot().await.map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
        })?;

    Ok(Json(ApiResponse::new(snapshot, auth.request_id, trace_id)))
}
