async fn submit_execution_request(
    Extension(auth): Extension<AuthContext>,
    Extension(idempotency_key): Extension<IdempotencyKey>,
    State(state): State<AppState>,
    Path(signal_id): Path<String>,
    Json(payload): Json<SubmitExecutionRequest>,
) -> std::result::Result<Json<ApiResponse<SubmitExecutionData>>, HttpError> {
    auth.ensure_scope(
        StepUpScope::ExecutionSubmit,
        time::OffsetDateTime::now_utc(),
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), new_trace_id()))?;

    let trace_id = new_trace_id();
    let request_hash = hash_json(&payload)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let idempotency_request = IdempotencyRequest {
        scope: "execution.requests.submit".to_string(),
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
            let mut replayed: SubmitExecutionData =
                serde_json::from_str(&response_json).map_err(|error| {
                    HttpError::with_meta(
                        AppError::internal(
                            "EXECUTION_REQUEST_REPLAY_DESERIALIZE_FAILED",
                            format!(
                                "failed to deserialize replayed execution submission response: {error}"
                            ),
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

    let actor = AuthenticatedActor {
        user_id: auth.user_id.clone(),
        session_id: auth.session_id.clone(),
        roles: auth.roles.clone(),
        request_id: auth.request_id.clone(),
        ip: auth.ip.clone(),
        user_agent: auth.user_agent.clone(),
    };

    let receipt = match state
        .execution_service
        .submit_execution_request(SubmitExecutionCommand {
            signal_id,
            expected_signal_version: payload.expected_signal_version,
            limit_price: payload.limit_price,
            quantity: payload.quantity,
            connector_name: payload.connector_name,
            reason: payload.reason,
            request_id: auth.request_id.clone(),
            trace_id: trace_id.clone(),
            actor,
        })
        .await
    {
        Ok(receipt) => receipt,
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

    let response_data = execution_submission_to_contract(receipt, false, &state)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let response_json = serde_json::to_string(&response_data).map_err(|error| {
        HttpError::with_meta(
            AppError::internal(
                "EXECUTION_REQUEST_RESPONSE_SERIALIZE_FAILED",
                format!("failed to serialize execution submission response: {error}"),
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
