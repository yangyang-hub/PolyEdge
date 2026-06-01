fn apply_plan_filters(
    mut snapshot: RewardBotSnapshot,
    query: &RewardBotSnapshotQuery,
) -> RewardBotSnapshot {
    let mut plans = snapshot.quote_plans;

    // Text search
    if let Some(ref search) = query.plans_search {
        let q = search.trim().to_lowercase();
        if !q.is_empty() {
            plans.retain(|p| {
                p.question.to_lowercase().contains(&q) || p.reason.to_lowercase().contains(&q)
            });
        }
    }

    // Eligibility filter
    if let Some(eligible) = query.plans_eligible {
        plans.retain(|p| p.eligible == eligible);
    }

    // Sort
    let sort_by = query.plans_sort_by.as_deref().unwrap_or("score");
    let desc = query.plans_sort_order.as_deref() != Some("asc");
    plans.sort_by(|a, b| {
        let ord = match sort_by {
            "daily_reward" => a.total_daily_rate.cmp(&b.total_daily_rate),
            "midpoint" => match (a.midpoint, b.midpoint) {
                (Some(a_m), Some(b_m)) => a_m.cmp(&b_m),
                (Some(_), None) => std::cmp::Ordering::Greater,
                (None, Some(_)) => std::cmp::Ordering::Less,
                (None, None) => std::cmp::Ordering::Equal,
            },
            _ => a.score.cmp(&b.score), // default: score
        };
        if desc { ord.reverse() } else { ord }
    });

    snapshot.quote_plans = plans;
    snapshot
}

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
    let snapshot = state
        .reward_bot_service
        .snapshot_with_order_query(&order_query)
        .await
        .map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
        })?;

    let has_plan_filters = query.plans_search.is_some()
        || query.plans_eligible.is_some()
        || query.plans_sort_by.is_some()
        || query.plans_sort_order.is_some();
    let snapshot = if has_plan_filters {
        apply_plan_filters(snapshot, &query)
    } else {
        snapshot
    };

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
