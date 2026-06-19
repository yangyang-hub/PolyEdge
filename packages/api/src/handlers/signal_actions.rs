async fn recompute_signal(
    Extension(auth): Extension<AuthContext>,
    Extension(idempotency_key): Extension<IdempotencyKey>,
    State(state): State<AppState>,
    Path(signal_id): Path<String>,
    Json(payload): Json<RecomputeSignalRequest>,
) -> std::result::Result<Json<ApiResponse<RecomputeSignalData>>, HttpError> {
    let trace_id = new_trace_id();
    let request_hash = hash_json(&payload)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let idempotency_request = IdempotencyRequest {
        scope: "signals.recompute".to_string(),
        idempotency_key: idempotency_key.0,
        request_hash,
        request_id: auth.request_id.clone(),
        actor_user_id: Some(auth.user_id.clone()),
        actor_session_id: Some(auth.session_id.clone()),
        resource_type: Some("signal".to_string()),
        resource_id: Some(signal_id.clone()),
    };

    match state
        .idempotency_store
        .begin(&idempotency_request)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?
    {
        IdempotencyBegin::Replay(response_json) => {
            let mut replayed: RecomputeSignalData =
                serde_json::from_str(&response_json).map_err(|error| {
                    HttpError::with_meta(
                        AppError::internal(
                            "SIGNAL_RECOMPUTE_REPLAY_DESERIALIZE_FAILED",
                            format!("failed to deserialize replayed recompute response: {error}"),
                        ),
                        auth.request_id.clone(),
                        trace_id.clone(),
                    )
                })?;
            replayed.replayed = true;

            return Ok(Json(ApiResponse::new(replayed, auth.request_id, trace_id)));
        }
        IdempotencyBegin::Started => {}
    }

    let result = match state
        .market_event_service
        .recompute_signal(signal_id, payload.reason, trace_id.clone())
        .await
    {
        Ok(result) => result,
        Err(error) => {
            state
                .idempotency_store
                .fail(&idempotency_request, error.code())
                .await
                .map_err(|fail_error| {
                    HttpError::with_meta(fail_error, auth.request_id.clone(), trace_id.clone())
                })?;
            return Err(HttpError::with_meta(
                error,
                auth.request_id.clone(),
                trace_id,
            ));
        }
    };

    let response_data = recompute_signal_to_contract(result, false);
    let response_json = serde_json::to_string(&response_data).map_err(|error| {
        HttpError::with_meta(
            AppError::internal(
                "SIGNAL_RECOMPUTE_RESPONSE_SERIALIZE_FAILED",
                format!("failed to serialize signal recompute response: {error}"),
            ),
            auth.request_id.clone(),
            trace_id.clone(),
        )
    })?;

    state
        .idempotency_store
        .complete(&idempotency_request, &response_json)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        response_data,
        auth.request_id,
        trace_id,
    )))
}
