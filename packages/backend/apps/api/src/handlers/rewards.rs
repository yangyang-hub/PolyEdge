use std::sync::{Arc, LazyLock};
use tokio::sync::RwLock;
use tracing::warn;

/// Cached authenticated Polymarket connector shared across all API requests.
/// Created lazily on first use; avoids re-authenticating on every snapshot read.
static CACHED_LIVE_CONNECTOR: LazyLock<RwLock<Option<Arc<LivePolymarketConnector>>>> =
    LazyLock::new(|| RwLock::new(None));

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
    let mut snapshot = state
        .reward_bot_service
        .snapshot_with_order_query(&order_query, &plans_query)
        .await
        .map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
        })?;

    // Overlay real Polymarket account data when credentials are configured.
    overlay_live_polymarket_data(&state, &mut snapshot).await;

    Ok(Json(ApiResponse::new(snapshot, auth.request_id, trace_id)))
}

/// Fetch real balance, positions, and open orders from Polymarket and replace
/// the snapshot data.  When credentials are missing or any Polymarket API call
/// fails, the corresponding fields are zeroed rather than falling back to DB.
///
/// **Design note:** This overlay replaces the DB-stored local worker state with
/// live Polymarket data.  As a result, worker-managed metadata (local order IDs
/// with `rew_` prefixes, `reason`, `scoring` flags, pending-cancel / deferred-exit
/// status) is not visible in the API response — the frontend sees only what
/// Polymarket reports.  This is intentional: the snapshot reflects the real
/// external account state, not the internal bookkeeping state.
async fn overlay_live_polymarket_data(state: &AppState, snapshot: &mut RewardBotSnapshot) {
    let settings = &state.settings.polymarket;

    // Always start with empty live data — never show DB-simulated values.
    snapshot.account.available_usd = Decimal::ZERO;
    snapshot.account.reserved_usd = Decimal::ZERO;
    snapshot.account.realized_pnl = Decimal::ZERO;
    snapshot.orders = Vec::new();
    snapshot.positions = Vec::new();

    // --- Balance + Open orders (requires CLOB credentials) ---
    if let Some(connector) = get_cached_live_connector(state).await {
        match connector.balance().await {
            Ok(balance) => {
                snapshot.account.available_usd = balance.balance;
            }
            Err(error) => {
                warn!(error = %error, "failed to fetch live Polymarket balance");
                invalidate_cached_connector().await;
            }
        }

        match connector.list_open_orders().await {
            Ok(live_orders_raw) => {
                let account_id = snapshot.account.account_id.clone();
                let live_orders: Vec<ManagedRewardOrder> = live_orders_raw
                    .iter()
                    .map(|o| polymarket_open_order_to_managed(&account_id, o))
                    .collect();
                snapshot.orders = live_orders;
            }
            Err(error) => {
                warn!(error = %error, "failed to fetch live Polymarket orders");
                invalidate_cached_connector().await;
            }
        }
    } else {
        warn!("Polymarket credentials not configured, skipping live balance/orders");
    }

    // --- Positions from Data API ---
    let wallet_address = &settings.account_id;
    if wallet_address.is_empty() {
        warn!("Polymarket account_id not configured, skipping live positions");
        return;
    }
    match PolymarketDataApiConnector::new(&settings.data_api_host) {
        Ok(data_connector) => {
            match data_connector.fetch_wallet_positions(wallet_address).await {
                Ok(raw_positions) => {
                    let account_id = snapshot.account.account_id.clone();
                    let positions: Vec<RewardPosition> = raw_positions
                        .into_iter()
                        .filter(|p| p.size > Decimal::ZERO)
                        .map(|p| polymarket_position_to_reward(&account_id, &p))
                        .collect();

                    let realized_pnl: Decimal =
                        positions.iter().map(|p| p.realized_pnl).sum();
                    snapshot.account.realized_pnl = realized_pnl;

                    snapshot.positions = positions;
                }
                Err(error) => {
                    warn!(error = %error, "failed to fetch live Polymarket positions");
                }
            }
        }
        Err(error) => {
            warn!(error = %error, "failed to create Polymarket Data API connector");
        }
    }
}

/// Return a cached `LivePolymarketConnector`, creating one on first use.
/// Returns `None` if Polymarket credentials are not configured.
async fn get_cached_live_connector(state: &AppState) -> Option<Arc<LivePolymarketConnector>> {
    // Fast path: read lock to check existing connector.
    {
        let cache = CACHED_LIVE_CONNECTOR.read().await;
        if let Some(connector) = cache.as_ref() {
            return Some(Arc::clone(connector));
        }
    }

    // Slow path: build config and authenticate.
    let config = build_live_connector_config(state)?;
    let connector = LivePolymarketConnector::connect(&config).await.map_err(|error| {
        warn!(error = %error, "failed to connect to Polymarket for live data overlay");
        error
    }).ok()?;

    let connector = Arc::new(connector);
    {
        let mut cache = CACHED_LIVE_CONNECTOR.write().await;
        // Only store if another request didn't race us.
        if cache.is_none() {
            *cache = Some(Arc::clone(&connector));
        }
    }

    Some(connector)
}

/// Clear the cached connector so the next request will re-authenticate.
/// Called when API calls fail, in case the connector's session is stale.
async fn invalidate_cached_connector() {
    let mut cache = CACHED_LIVE_CONNECTOR.write().await;
    *cache = None;
}

/// Build a `LivePolymarketConfig` from current settings, or return `None` if
/// credentials are not configured.
fn build_live_connector_config(state: &AppState) -> Option<LivePolymarketConfig> {
    let settings = &state.settings.polymarket;
    let private_key = settings.private_key.as_deref()?.trim();
    if private_key.is_empty() {
        return None;
    }
    let api_key = settings.api_key.as_deref().unwrap_or("").trim().to_string();
    let api_secret = settings.api_secret.as_deref().unwrap_or("").trim().to_string();
    let api_passphrase = settings
        .api_passphrase
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_string();
    let funder = settings
        .funder
        .as_deref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let signature_type = match settings.signature_type {
        PolymarketSignatureType::Eoa => PolymarketSignatureScheme::Eoa,
        PolymarketSignatureType::Proxy => PolymarketSignatureScheme::Proxy,
        PolymarketSignatureType::GnosisSafe => PolymarketSignatureScheme::GnosisSafe,
        PolymarketSignatureType::Poly1271 => PolymarketSignatureScheme::Poly1271,
    };

    Some(LivePolymarketConfig {
        account_id: settings.account_id.trim().to_string(),
        clob_host: settings.clob_host.clone(),
        ws_host: settings.ws_host.clone(),
        chain_id: settings.chain_id,
        signature_type,
        funder,
        private_key: private_key.to_string(),
        api_key: Some(api_key).filter(|s| !s.is_empty()),
        api_secret: Some(api_secret).filter(|s| !s.is_empty()),
        api_passphrase: Some(api_passphrase).filter(|s| !s.is_empty()),
    })
}

fn polymarket_position_to_reward(
    account_id: &str,
    pos: &PolymarketWalletPosition,
) -> RewardPosition {
    RewardPosition {
        account_id: account_id.to_string(),
        condition_id: pos.condition_id.clone(),
        token_id: pos.asset.clone(),
        outcome: pos.outcome.clone(),
        size: pos.size,
        avg_price: pos.avg_price,
        realized_pnl: pos.realized_pnl,
        updated_at: OffsetDateTime::now_utc(),
    }
}

fn polymarket_open_order_to_managed(
    account_id: &str,
    order: &PolymarketOpenOrder,
) -> ManagedRewardOrder {
    use polyedge_connectors::PolymarketTokenOrderSide;

    let side = match order.side {
        PolymarketTokenOrderSide::Buy => RewardOrderSide::Buy,
        PolymarketTokenOrderSide::Sell => RewardOrderSide::Sell,
    };
    let remaining = order.original_size - order.size_matched;
    let status = if remaining <= Decimal::ZERO {
        ManagedRewardOrderStatus::Filled
    } else {
        ManagedRewardOrderStatus::Open
    };
    let now = OffsetDateTime::now_utc();

    ManagedRewardOrder {
        id: order.id.clone(),
        account_id: account_id.to_string(),
        condition_id: order.market.clone(),
        token_id: order.asset_id.clone(),
        outcome: order.outcome.clone(),
        side,
        price: order.price,
        size: order.original_size,
        external_order_id: Some(order.id.clone()),
        status,
        scoring: false,
        reason: "live polymarket order".to_string(),
        filled_size: order.size_matched,
        reward_earned: Decimal::ZERO,
        last_scored_at: None,
        created_at: now,
        updated_at: now,
    }
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

    overlay_live_polymarket_data(&state, &mut snapshot).await;

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

    overlay_live_polymarket_data(&state, &mut snapshot).await;

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

    overlay_live_polymarket_data(&state, &mut snapshot).await;

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

    overlay_live_polymarket_data(&state, &mut snapshot).await;

    Ok(Json(ApiResponse::new(snapshot, auth.request_id, trace_id)))
}
