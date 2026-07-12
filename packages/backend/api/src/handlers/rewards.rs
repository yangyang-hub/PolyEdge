async fn read_reward_bot_snapshot(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<RewardBotSnapshotQuery>,
) -> std::result::Result<Json<ApiResponse<RewardBotSnapshot>>, HttpError> {
    let trace_id = new_trace_id();
    let order_query = RewardOrderListQuery::new(
        String::new(), // account_id injected by service from config
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
    let mut snapshot = state
        .reward_bot_service
        .snapshot_with_order_query(&order_query, &plans_query)
        .await
        .map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
        })?;
    enrich_reward_bot_snapshot(&mut snapshot, state.orderbook_cache.as_ref()).await;

    Ok(Json(ApiResponse::new(snapshot, auth.request_id, trace_id)))
}

async fn list_reward_strategy_runs(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<RewardStrategyRunsQuery>,
) -> std::result::Result<Json<ApiResponse<polyedge_application::RewardStrategyRunPage>>, HttpError>
{
    let trace_id = new_trace_id();
    let query = RewardStrategyRunListQuery::new(
        query.account_id,
        query.status,
        query.page,
        query.page_size,
    );
    let page = state
        .reward_bot_service
        .list_strategy_runs(&query)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    Ok(Json(ApiResponse::new(page, auth.request_id, trace_id)))
}

async fn read_reward_strategy_run(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Path(run_id): Path<i64>,
) -> std::result::Result<Json<ApiResponse<polyedge_application::RewardStrategyRun>>, HttpError> {
    let trace_id = new_trace_id();
    let run = state
        .reward_bot_service
        .get_strategy_run(run_id)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?
        .ok_or_else(|| {
            HttpError::with_meta(
                AppError::not_found(
                    "REWARD_STRATEGY_RUN_NOT_FOUND",
                    format!("reward strategy run {run_id} was not found"),
                ),
                auth.request_id.clone(),
                trace_id.clone(),
            )
        })?;
    Ok(Json(ApiResponse::new(run, auth.request_id, trace_id)))
}

async fn list_reward_strategy_decisions(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Path(run_id): Path<i64>,
    Query(query): Query<RewardStrategyDecisionsQuery>,
) -> std::result::Result<
    Json<ApiResponse<polyedge_application::RewardStrategyDecisionPage>>,
    HttpError,
> {
    let trace_id = new_trace_id();
    let query = RewardStrategyDecisionListQuery::new(
        query.search,
        query.eligible,
        query.page,
        query.page_size,
    );
    let page = state
        .reward_bot_service
        .list_strategy_decisions(run_id, &query)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    Ok(Json(ApiResponse::new(page, auth.request_id, trace_id)))
}

async fn list_reward_strategy_actions(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Path(run_id): Path<i64>,
    Query(query): Query<RewardStrategyActionsQuery>,
) -> std::result::Result<
    Json<ApiResponse<polyedge_application::RewardStrategyActionPage>>,
    HttpError,
> {
    let trace_id = new_trace_id();
    let query = RewardStrategyActionListQuery::new(
        query.status,
        query.action_type,
        query.page,
        query.page_size,
    );
    let page = state
        .reward_bot_service
        .list_strategy_actions(run_id, &query)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    Ok(Json(ApiResponse::new(page, auth.request_id, trace_id)))
}

async fn list_reward_order_transitions(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Path(managed_order_id): Path<String>,
    Query(query): Query<RewardOrderTransitionsQuery>,
) -> std::result::Result<
    Json<ApiResponse<polyedge_application::RewardOrderTransitionPage>>,
    HttpError,
> {
    let trace_id = new_trace_id();
    let query = RewardOrderTransitionListQuery::new(query.page, query.page_size);
    let page = state
        .reward_bot_service
        .list_order_transitions(&managed_order_id, &query)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    Ok(Json(ApiResponse::new(page, auth.request_id, trace_id)))
}

async fn update_reward_bot_config(
    Extension(auth): Extension<AuthContext>,
    Extension(idempotency_key): Extension<IdempotencyKey>,
    State(state): State<AppState>,
    Json(mut payload): Json<UpdateRewardBotConfigRequest>,
) -> std::result::Result<Json<ApiResponse<RewardBotSnapshot>>, HttpError> {
    let trace_id = new_trace_id();
    payload.operator_note = normalize_reward_operator_note(payload.operator_note).map_err(|error| {
        HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
    })?;
    let patch = reward_config_patch_from_request(&payload).map_err(|error| {
        HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
    })?;

    if patch.enabled == Some(true) {
        auth.ensure_scope(
            StepUpScope::RewardsLiveTradingEnable,
            time::OffsetDateTime::now_utc(),
        )
        .map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
        })?;
    }
    if patch.balanced_merge_auto_execute_enabled == Some(true) {
        auth.ensure_scope(
            StepUpScope::RewardsMergeAutoExecute,
            time::OffsetDateTime::now_utc(),
        )
        .map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
        })?;
    }

    let (idempotency_request, replay) = begin_reward_write(
        &state,
        &auth,
        idempotency_key,
        &trace_id,
        REWARD_CONFIG_IDEMPOTENCY_SCOPE,
        "config",
        &payload,
    )
    .await?;
    if let Some(response) = replay {
        return Ok(Json(response));
    }

    if let Err(error) = state
        .reward_bot_service
        .update_config(patch)
        .await
    {
        return Err(fail_reward_write(
            &state,
            &auth,
            &trace_id,
            &idempotency_request,
            error,
        )
        .await);
    }
    let mut snapshot =
        match state.reward_bot_service.snapshot().await {
            Ok(snapshot) => snapshot,
            Err(error) => {
                return Err(fail_reward_write(
                    &state,
                    &auth,
                    &trace_id,
                    &idempotency_request,
                    error,
                )
                .await);
            }
        };
    enrich_reward_bot_snapshot(&mut snapshot, state.orderbook_cache.as_ref()).await;
    let response = ApiResponse::new(snapshot, auth.request_id.clone(), trace_id.clone());
    let reason = reward_operator_reason(
        "operator updated rewards configuration",
        payload.operator_note.as_deref(),
    );
    if let Err(error) = append_reward_audit(
        &state,
        &auth,
        &trace_id,
        "rewards.config.update",
        "reward_bot_config",
        "global",
        &reason,
        AuditResult::Succeeded,
    )
    .await
    {
        return Err(fail_reward_write(
            &state,
            &auth,
            &trace_id,
            &idempotency_request,
            error,
        )
        .await);
    }
    complete_reward_write(&state, &auth, &trace_id, &idempotency_request, &response).await?;

    Ok(Json(response))
}

async fn run_reward_bot_once(
    Extension(auth): Extension<AuthContext>,
    Extension(idempotency_key): Extension<IdempotencyKey>,
    State(state): State<AppState>,
    Json(mut payload): Json<RewardBotControlRequest>,
) -> std::result::Result<Json<ApiResponse<RewardBotSnapshot>>, HttpError> {
    let trace_id = new_trace_id();
    auth.ensure_scope(
        StepUpScope::RewardsRunOnce,
        time::OffsetDateTime::now_utc(),
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    payload.operator_note = normalize_reward_operator_note(payload.operator_note).map_err(|error| {
        HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
    })?;
    execute_reward_control_write(
        &state,
        &auth,
        idempotency_key,
        &trace_id,
        REWARD_RUN_IDEMPOTENCY_SCOPE,
        RewardControlAction::RunOnce,
        "operator requested one rewards strategy tick",
        payload,
    )
    .await
}

async fn cancel_reward_bot_orders(
    Extension(auth): Extension<AuthContext>,
    Extension(idempotency_key): Extension<IdempotencyKey>,
    State(state): State<AppState>,
    Json(mut payload): Json<RewardBotControlRequest>,
) -> std::result::Result<Json<ApiResponse<RewardBotSnapshot>>, HttpError> {
    let trace_id = new_trace_id();
    payload.operator_note = normalize_reward_operator_note(payload.operator_note).map_err(|error| {
        HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
    })?;
    execute_reward_control_write(
        &state,
        &auth,
        idempotency_key,
        &trace_id,
        REWARD_CANCEL_IDEMPOTENCY_SCOPE,
        RewardControlAction::CancelAll,
        "operator requested cancelling all rewards orders",
        payload,
    )
    .await
}

async fn reset_reward_bot(
    Extension(auth): Extension<AuthContext>,
    Extension(idempotency_key): Extension<IdempotencyKey>,
    State(state): State<AppState>,
    Json(mut payload): Json<RewardBotControlRequest>,
) -> std::result::Result<Json<ApiResponse<RewardBotSnapshot>>, HttpError> {
    let trace_id = new_trace_id();
    auth.ensure_scope(
        StepUpScope::RewardsStateReset,
        time::OffsetDateTime::now_utc(),
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    payload.operator_note = normalize_reward_operator_note(payload.operator_note).map_err(|error| {
        HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
    })?;
    execute_reward_control_write(
        &state,
        &auth,
        idempotency_key,
        &trace_id,
        REWARD_RESET_IDEMPOTENCY_SCOPE,
        RewardControlAction::Reset,
        "operator requested resetting rewards state",
        payload,
    )
    .await
}

const REWARD_CONFIG_IDEMPOTENCY_SCOPE: &str = "rewards.config.update";
const REWARD_RUN_IDEMPOTENCY_SCOPE: &str = "rewards.control.run_once";
const REWARD_CANCEL_IDEMPOTENCY_SCOPE: &str = "rewards.control.cancel_all";
const REWARD_RESET_IDEMPOTENCY_SCOPE: &str = "rewards.control.reset";
const REWARD_OPERATOR_NOTE_MAX_CHARS: usize = 500;

fn reward_config_patch_from_request(
    payload: &UpdateRewardBotConfigRequest,
) -> polyedge_domain::Result<RewardBotConfigPatch> {
    let value = serde_json::Value::Object(payload.patch.clone().into_iter().collect());
    serde_json::from_value(value).map_err(|error| {
        AppError::invalid_input(
            "REWARD_CONFIG_PATCH_INVALID",
            format!("invalid rewards configuration patch: {error}"),
        )
    })
}

fn normalize_reward_operator_note(note: Option<String>) -> polyedge_domain::Result<Option<String>> {
    let Some(note) = note else {
        return Ok(None);
    };
    let note = note.trim();
    if note.is_empty() {
        return Ok(None);
    }
    if note.chars().count() > REWARD_OPERATOR_NOTE_MAX_CHARS {
        return Err(AppError::invalid_input(
            "REWARD_OPERATOR_NOTE_TOO_LONG",
            format!(
                "operator_note cannot exceed {REWARD_OPERATOR_NOTE_MAX_CHARS} characters"
            ),
        ));
    }
    if note.chars().any(char::is_control) {
        return Err(AppError::invalid_input(
            "REWARD_OPERATOR_NOTE_INVALID",
            "operator_note must be a single printable line",
        ));
    }
    Ok(Some(note.to_string()))
}

fn reward_operator_reason(default_reason: &str, operator_note: Option<&str>) -> String {
    operator_note.map_or_else(
        || default_reason.to_string(),
        |note| format!("{default_reason}; operator note: {note}"),
    )
}

async fn execute_reward_control_write(
    state: &AppState,
    auth: &AuthContext,
    idempotency_key: IdempotencyKey,
    trace_id: &str,
    scope: &str,
    action: RewardControlAction,
    default_reason: &str,
    payload: RewardBotControlRequest,
) -> std::result::Result<Json<ApiResponse<RewardBotSnapshot>>, HttpError> {
    let (idempotency_request, replay) = begin_reward_write(
        state,
        auth,
        idempotency_key,
        trace_id,
        scope,
        action.as_str(),
        &payload,
    )
    .await?;
    if let Some(response) = replay {
        return Ok(Json(response));
    }

    let reason = reward_operator_reason(default_reason, payload.operator_note.as_deref());
    if let Err(error) = state
        .reward_bot_service
        .enqueue_control_command(action, &reason, trace_id)
        .await
    {
        return Err(fail_reward_write(state, auth, trace_id, &idempotency_request, error).await);
    }
    let mut snapshot = match state.reward_bot_service.snapshot().await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            return Err(
                fail_reward_write(state, auth, trace_id, &idempotency_request, error).await,
            );
        }
    };
    enrich_reward_bot_snapshot(&mut snapshot, state.orderbook_cache.as_ref()).await;
    let response = ApiResponse::new(snapshot, auth.request_id.clone(), trace_id.to_string());
    let audit_action = format!("rewards.control.{}", action.as_str());
    if let Err(error) = append_reward_audit(
        state,
        auth,
        trace_id,
        &audit_action,
        "reward_control_command",
        action.as_str(),
        &reason,
        AuditResult::Accepted,
    )
    .await
    {
        return Err(fail_reward_write(state, auth, trace_id, &idempotency_request, error).await);
    }
    complete_reward_write(state, auth, trace_id, &idempotency_request, &response).await?;
    Ok(Json(response))
}

async fn begin_reward_write<T: serde::Serialize>(
    state: &AppState,
    auth: &AuthContext,
    idempotency_key: IdempotencyKey,
    trace_id: &str,
    scope: &str,
    resource_id: &str,
    payload: &T,
) -> std::result::Result<
    (IdempotencyRequest, Option<ApiResponse<RewardBotSnapshot>>),
    HttpError,
> {
    let request_hash = hash_json(payload).map_err(|error| {
        HttpError::with_meta(error, auth.request_id.clone(), trace_id.to_string())
    })?;
    let request = IdempotencyRequest {
        scope: scope.to_string(),
        idempotency_key: idempotency_key.0,
        request_hash,
        request_id: auth.request_id.clone(),
        actor_user_id: Some(auth.user_id.clone()),
        actor_session_id: Some(auth.session_id.clone()),
        resource_type: Some("rewards_bot".to_string()),
        resource_id: Some(resource_id.to_string()),
    };
    let replay = match state.idempotency_store.begin(&request).await.map_err(|error| {
        HttpError::with_meta(error, auth.request_id.clone(), trace_id.to_string())
    })? {
        IdempotencyBegin::Started => None,
        IdempotencyBegin::Replay(response_json) => Some(
            serde_json::from_str::<ApiResponse<RewardBotSnapshot>>(&response_json).map_err(
                |error| {
                    HttpError::with_meta(
                        AppError::internal(
                            "REWARD_IDEMPOTENCY_REPLAY_DESERIALIZE_FAILED",
                            format!("failed to deserialize replayed rewards response: {error}"),
                        ),
                        auth.request_id.clone(),
                        trace_id.to_string(),
                    )
                },
            )?,
        ),
    };
    Ok((request, replay))
}

async fn complete_reward_write(
    state: &AppState,
    auth: &AuthContext,
    trace_id: &str,
    request: &IdempotencyRequest,
    response: &ApiResponse<RewardBotSnapshot>,
) -> std::result::Result<(), HttpError> {
    let response_json = serde_json::to_string(response).map_err(|error| {
        HttpError::with_meta(
            AppError::internal(
                "REWARD_RESPONSE_SERIALIZE_FAILED",
                format!("failed to serialize rewards write response: {error}"),
            ),
            auth.request_id.clone(),
            trace_id.to_string(),
        )
    })?;
    state
        .idempotency_store
        .complete(request, &response_json)
        .await
        .map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.to_string())
        })
}

async fn fail_reward_write(
    state: &AppState,
    auth: &AuthContext,
    trace_id: &str,
    request: &IdempotencyRequest,
    error: AppError,
) -> HttpError {
    let error_code = error.code().to_string();
    if let Err(fail_error) = state.idempotency_store.fail(request, &error_code).await {
        return HttpError::with_meta(
            fail_error,
            auth.request_id.clone(),
            trace_id.to_string(),
        );
    }
    HttpError::with_meta(error, auth.request_id.clone(), trace_id.to_string())
}

async fn append_reward_audit(
    state: &AppState,
    auth: &AuthContext,
    trace_id: &str,
    action: &str,
    resource_type: &str,
    resource_id: &str,
    reason: &str,
    result: AuditResult,
) -> polyedge_domain::Result<()> {
    state
        .audit_log_sink
        .append(AuditLogEntry {
            occurred_at: time::OffsetDateTime::now_utc(),
            request_id: auth.request_id.clone(),
            trace_id: trace_id.to_string(),
            actor: AuthenticatedActor {
                user_id: auth.user_id.clone(),
                session_id: auth.session_id.clone(),
                roles: auth.roles.clone(),
                request_id: auth.request_id.clone(),
                ip: auth.ip.clone(),
                user_agent: auth.user_agent.clone(),
            },
            action: action.to_string(),
            resource_type: resource_type.to_string(),
            resource_id: resource_id.to_string(),
            reason: reason.to_string(),
            result,
            error_code: None,
        })
        .await
}

/// Best-effort: attach per-token live quotes (best bid / best ask / mid mark)
/// to a rewards snapshot, drawn from the orderbook cache. Positions and orders
/// only carry `token_id`, so the frontend looks quotes up by token. Never fails
/// the request: on any orderbook error the map is left unset and the frontend
/// renders "—" placeholders, then the next snapshot refresh retries.
async fn enrich_reward_bot_snapshot(snapshot: &mut RewardBotSnapshot, cache: &dyn OrderbookCache) {
    let mut token_ids: Vec<String> = snapshot
        .positions
        .iter()
        .map(|position| position.token_id.clone())
        .chain(snapshot.orders.iter().map(|order| order.token_id.clone()))
        .collect();
    if token_ids.is_empty() {
        return;
    }
    token_ids.sort();
    token_ids.dedup();

    let books = match cache.get_books(&token_ids).await {
        Ok(books) => books,
        Err(error) => {
            tracing::warn!(
                error = %error,
                "rewards snapshot orderbook enrichment failed; token_quotes left empty"
            );
            return;
        }
    };

    // Cached bids are sorted descending and asks ascending, but take the
    // defensive max/min so a malformed cache entry can never invert the quote.
    let mut quotes = HashMap::with_capacity(books.len());
    for book in books {
        let best_bid = book.bids.iter().map(|level| level.price).max();
        let best_ask = book.asks.iter().map(|level| level.price).min();
        let mark_price = match (best_bid, best_ask) {
            (Some(bid), Some(ask)) => Some(((bid + ask) / Decimal::from(2)).round_dp(4)),
            (Some(only), None) | (None, Some(only)) => Some(only),
            (None, None) => None,
        };
        quotes.insert(
            book.token_id.clone(),
            RewardTokenQuote {
                best_bid,
                best_ask,
                mark_price,
            },
        );
    }

    if !quotes.is_empty() {
        snapshot.token_quotes = Some(quotes);
    }
}
