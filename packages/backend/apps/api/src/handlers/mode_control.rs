async fn trigger_kill_switch(
    Extension(auth): Extension<AuthContext>,
    Extension(idempotency_key): Extension<IdempotencyKey>,
    State(state): State<AppState>,
    Json(payload): Json<TriggerKillSwitchRequest>,
) -> std::result::Result<Json<ApiResponse<KillSwitchData>>, HttpError> {
    auth.ensure_scope(
        StepUpScope::SystemKillSwitchTrigger,
        time::OffsetDateTime::now_utc(),
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), new_trace_id()))?;

    let trace_id = new_trace_id();
    let request_hash = hash_json(&payload)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let idempotency_request = IdempotencyRequest {
        scope: "system.kill_switch.trigger".to_string(),
        idempotency_key: idempotency_key.0,
        request_hash,
        request_id: auth.request_id.clone(),
        actor_user_id: Some(auth.user_id.clone()),
        actor_session_id: Some(auth.session_id.clone()),
        resource_type: Some("risk_state".to_string()),
        resource_id: Some("global".to_string()),
    };

    match state
        .idempotency_store
        .begin(&idempotency_request)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?
    {
        IdempotencyBegin::Replay(response_json) => {
            let mut replayed: KillSwitchData =
                serde_json::from_str(&response_json).map_err(|error| {
                    HttpError::with_meta(
                        AppError::internal(
                            "KILL_SWITCH_REPLAY_DESERIALIZE_FAILED",
                            format!("failed to deserialize replayed kill switch response: {error}"),
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
        .risk_service
        .trigger_kill_switch(TriggerKillSwitchCommand {
            reason: payload.reason,
            expected_version: payload.expected_version,
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

    let response_data = kill_switch_to_contract(receipt, false, &state)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let response_json = serde_json::to_string(&response_data).map_err(|error| {
        HttpError::with_meta(
            AppError::internal(
                "KILL_SWITCH_RESPONSE_SERIALIZE_FAILED",
                format!("failed to serialize kill switch response: {error}"),
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

async fn release_kill_switch(
    Extension(auth): Extension<AuthContext>,
    Extension(idempotency_key): Extension<IdempotencyKey>,
    State(state): State<AppState>,
    Json(payload): Json<ReleaseKillSwitchRequest>,
) -> std::result::Result<Json<ApiResponse<KillSwitchData>>, HttpError> {
    auth.ensure_scope(
        StepUpScope::SystemKillSwitchRelease,
        time::OffsetDateTime::now_utc(),
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), new_trace_id()))?;

    let trace_id = new_trace_id();
    let request_hash = hash_json(&payload)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let idempotency_request = IdempotencyRequest {
        scope: "system.kill_switch.release".to_string(),
        idempotency_key: idempotency_key.0,
        request_hash,
        request_id: auth.request_id.clone(),
        actor_user_id: Some(auth.user_id.clone()),
        actor_session_id: Some(auth.session_id.clone()),
        resource_type: Some("risk_state".to_string()),
        resource_id: Some("global".to_string()),
    };

    match state
        .idempotency_store
        .begin(&idempotency_request)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?
    {
        IdempotencyBegin::Replay(response_json) => {
            let mut replayed: KillSwitchData =
                serde_json::from_str(&response_json).map_err(|error| {
                    HttpError::with_meta(
                        AppError::internal(
                            "KILL_SWITCH_REPLAY_DESERIALIZE_FAILED",
                            format!("failed to deserialize replayed kill switch response: {error}"),
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
        .risk_service
        .release_kill_switch(ReleaseKillSwitchCommand {
            reason: payload.reason,
            to_mode: payload.to_mode,
            expected_version: payload.expected_version,
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

    let response_data = kill_switch_to_contract(receipt, false, &state)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let response_json = serde_json::to_string(&response_data).map_err(|error| {
        HttpError::with_meta(
            AppError::internal(
                "KILL_SWITCH_RESPONSE_SERIALIZE_FAILED",
                format!("failed to serialize kill switch response: {error}"),
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

async fn transition_system_mode(
    Extension(auth): Extension<AuthContext>,
    Extension(idempotency_key): Extension<IdempotencyKey>,
    State(state): State<AppState>,
    Json(payload): Json<TransitionSystemModeRequest>,
) -> std::result::Result<Json<ApiResponse<SystemModeData>>, HttpError> {
    if payload.reason.trim().is_empty() {
        return Err(HttpError::with_meta(
            AppError::invalid_input("SYSTEM_MODE_REASON_REQUIRED", "reason must not be empty"),
            auth.request_id.clone(),
            new_trace_id(),
        ));
    }

    let trace_id = new_trace_id();
    let request_hash = hash_json(&payload)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let actor = AuthenticatedActor {
        user_id: auth.user_id.clone(),
        session_id: auth.session_id.clone(),
        roles: auth.roles.clone(),
        request_id: auth.request_id.clone(),
        ip: auth.ip.clone(),
        user_agent: auth.user_agent.clone(),
    };

    let receipt = state
        .system_mode_service
        .transition_mode(ModeTransitionCommand {
            to_mode: payload.to_mode,
            reason: payload.reason,
            request_id: auth.request_id.clone(),
            trace_id: trace_id.clone(),
            idempotency_key: idempotency_key.0,
            request_hash,
            actor,
            required_scope: StepUpScope::SystemModeSwitch,
        })
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        SystemModeData {
            mode: receipt.snapshot.mode,
            environment: receipt.snapshot.environment,
            version: receipt.snapshot.version,
            replayed: receipt.replayed,
            updated_at: receipt.snapshot.updated_at,
        },
        auth.request_id,
        trace_id,
    )))
}
