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
    State(state): State<AppState>,
    Json(payload): Json<RewardBotConfigPatch>,
) -> std::result::Result<Json<ApiResponse<RewardBotSnapshot>>, HttpError> {
    let trace_id = new_trace_id();
    state
        .reward_bot_service
        .update_config(payload)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let mut snapshot =
        state.reward_bot_service.snapshot().await.map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
        })?;
    enrich_reward_bot_snapshot(&mut snapshot, state.orderbook_cache.as_ref()).await;

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
    let mut snapshot =
        state.reward_bot_service.snapshot().await.map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
        })?;
    enrich_reward_bot_snapshot(&mut snapshot, state.orderbook_cache.as_ref()).await;

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
    let mut snapshot =
        state.reward_bot_service.snapshot().await.map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
        })?;
    enrich_reward_bot_snapshot(&mut snapshot, state.orderbook_cache.as_ref()).await;

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
            "operator requested resetting rewards state",
            &trace_id,
        )
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let mut snapshot =
        state.reward_bot_service.snapshot().await.map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
        })?;
    enrich_reward_bot_snapshot(&mut snapshot, state.orderbook_cache.as_ref()).await;

    Ok(Json(ApiResponse::new(snapshot, auth.request_id, trace_id)))
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
