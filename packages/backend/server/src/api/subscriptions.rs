async fn list_strategy_subscriptions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ManualTradingListQuery>,
) -> Result<Json<ApiResponse<Vec<polyedge_contracts::StrategySubscriptionData>>>> {
    let context = authorize(&state, &headers, None).await?;
    Ok(response(
        state
            .store
            .list_strategy_subscriptions(&query, context.actor)
            .await?,
        &context,
    ))
}

async fn create_strategy_subscription(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateStrategySubscriptionRequest>,
) -> Result<Json<ApiResponse<WriteOperationData>>> {
    let context = authorize(&state, &headers, None).await?;
    let lease = begin_write(&state, &headers, "strategy_subscription.create", &request).await?;
    if let WriteLease::Replay(replay) = lease {
        return Ok(Json(replay));
    }
    let subscription = state
        .store
        .create_strategy_subscription(&request, context.actor, &context.request_id)
        .await?;
    finish_write(
        &state,
        "strategy_subscription.create",
        lease,
        completed_operation("strategy_subscription", subscription.subscription.id),
        &context,
    )
    .await
}

async fn update_strategy_subscription(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(subscription_id): Path<i64>,
    Json(request): Json<UpdateStrategySubscriptionRequest>,
) -> Result<Json<ApiResponse<WriteOperationData>>> {
    let context = authorize(&state, &headers, None).await?;
    let scope = format!("strategy_subscription.update:{subscription_id}");
    let lease = begin_write(&state, &headers, &scope, &request).await?;
    if let WriteLease::Replay(replay) = lease {
        return Ok(Json(replay));
    }
    state
        .store
        .update_strategy_subscription(
            subscription_id,
            &request,
            context.actor,
            &context.request_id,
        )
        .await?;
    finish_write(
        &state,
        &scope,
        lease,
        completed_operation("strategy_subscription", subscription_id),
        &context,
    )
    .await
}
