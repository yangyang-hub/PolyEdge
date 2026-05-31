async fn read_copytrade_snapshot(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> std::result::Result<Json<ApiResponse<CopyTradeSnapshot>>, HttpError> {
    let trace_id = new_trace_id();
    let snapshot = state
        .copytrade_service
        .snapshot()
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(snapshot, auth.request_id, trace_id)))
}

async fn update_copytrade_config(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Json(payload): Json<CopyTradeConfigPatch>,
) -> std::result::Result<Json<ApiResponse<CopyTradeSnapshot>>, HttpError> {
    let trace_id = new_trace_id();
    state
        .copytrade_service
        .update_config(payload)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let snapshot = state
        .copytrade_service
        .snapshot()
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(snapshot, auth.request_id, trace_id)))
}

async fn add_copytrade_wallet(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Json(payload): Json<AddTrackedWalletInput>,
) -> std::result::Result<Json<ApiResponse<CopyTradeSnapshot>>, HttpError> {
    let trace_id = new_trace_id();
    state
        .copytrade_service
        .add_wallet(&payload)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let snapshot = state
        .copytrade_service
        .snapshot()
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(snapshot, auth.request_id, trace_id)))
}

async fn remove_copytrade_wallet(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Json(payload): Json<WalletActionInput>,
) -> std::result::Result<Json<ApiResponse<CopyTradeSnapshot>>, HttpError> {
    let trace_id = new_trace_id();
    state
        .copytrade_service
        .remove_wallet(&payload.address)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let snapshot = state
        .copytrade_service
        .snapshot()
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(snapshot, auth.request_id, trace_id)))
}

async fn set_copytrade_wallet_status(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> std::result::Result<Json<ApiResponse<CopyTradeSnapshot>>, HttpError> {
    let trace_id = new_trace_id();
    let address = match payload.get("address").and_then(|v| v.as_str()) {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => {
            return Err(HttpError::with_meta(
                AppError::invalid_input(
                    "WALLET_ADDRESS_REQUIRED",
                    "wallet address must not be empty",
                ),
                auth.request_id.clone(),
                trace_id,
            ));
        }
    };
    let status_str = payload
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("paused");
    let status = match status_str {
        "active" => TrackedWalletStatus::Active,
        "paused" => TrackedWalletStatus::Paused,
        other => {
            return Err(HttpError::with_meta(
                AppError::invalid_input(
                    "INVALID_WALLET_STATUS",
                    format!("invalid wallet status '{other}': expected 'active' or 'paused'"),
                ),
                auth.request_id.clone(),
                trace_id,
            ));
        }
    };
    state
        .copytrade_service
        .set_wallet_status(&address, status)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let snapshot = state
        .copytrade_service
        .snapshot()
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(snapshot, auth.request_id, trace_id)))
}

async fn run_copytrade_once(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> std::result::Result<Json<ApiResponse<CopyTradeSnapshot>>, HttpError> {
    let trace_id = new_trace_id();
    state
        .copytrade_service
        .enqueue_control_command(
            CopyControlAction::RunOnce,
            "operator requested one copytrade cycle",
            &trace_id,
        )
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let snapshot = state
        .copytrade_service
        .snapshot()
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(snapshot, auth.request_id, trace_id)))
}

async fn analyze_copytrade_wallets(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> std::result::Result<Json<ApiResponse<CopyTradeSnapshot>>, HttpError> {
    let trace_id = new_trace_id();
    state
        .copytrade_service
        .enqueue_control_command(
            CopyControlAction::AnalyzeWallets,
            "operator requested copytrade wallet analysis",
            &trace_id,
        )
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let snapshot = state
        .copytrade_service
        .snapshot()
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(snapshot, auth.request_id, trace_id)))
}

async fn cancel_copytrade_orders(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> std::result::Result<Json<ApiResponse<CopyTradeSnapshot>>, HttpError> {
    let trace_id = new_trace_id();
    state
        .copytrade_service
        .enqueue_control_command(
            CopyControlAction::CancelAll,
            "operator requested cancelling all copy-trading orders",
            &trace_id,
        )
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let snapshot = state
        .copytrade_service
        .snapshot()
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(snapshot, auth.request_id, trace_id)))
}

async fn reset_copytrade(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> std::result::Result<Json<ApiResponse<CopyTradeSnapshot>>, HttpError> {
    let trace_id = new_trace_id();
    state
        .copytrade_service
        .enqueue_control_command(
            CopyControlAction::Reset,
            "operator requested resetting copytrade simulation account",
            &trace_id,
        )
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let snapshot = state
        .copytrade_service
        .snapshot()
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(snapshot, auth.request_id, trace_id)))
}
