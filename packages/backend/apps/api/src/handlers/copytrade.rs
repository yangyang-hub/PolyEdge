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
    let (wallet_feeds, books) = fetch_copytrade_inputs(&state)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    state
        .copytrade_service
        .run_copy_cycle(wallet_feeds, books, &trace_id)
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
    let wallet_feeds = fetch_wallet_analysis_inputs(&state)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    state
        .copytrade_service
        .analyze_wallets(wallet_feeds)
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
    let config = state
        .copytrade_service
        .read_config()
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    state
        .copytrade_service
        .cancel_all_orders(
            Some(&config.account_id),
            "operator cancelled all copy-trading orders",
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
        .reset_simulation(&trace_id)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let snapshot = state
        .copytrade_service
        .snapshot()
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(snapshot, auth.request_id, trace_id)))
}

// ── Input fetching (connects Data API connector to application layer) ───────

async fn fetch_copytrade_inputs(
    state: &AppState,
) -> Result<(Vec<WalletFeedInput>, HashMap<String, CopyOrderBook>), polyedge_domain::AppError> {
    let wallet_feeds = fetch_wallet_analysis_inputs(state).await?;

    // Collect unique token IDs from detected source trades to fetch books.
    let mut token_ids = std::collections::HashSet::new();
    for feed in &wallet_feeds {
        for activity in &feed.activities {
            if activity.kind.eq_ignore_ascii_case("TRADE") && !activity.asset.is_empty() {
                token_ids.insert(activity.asset.clone());
            }
        }
    }

    let connector = PolymarketRewardsConnector::new(&state.settings.polymarket.clob_host)?;
    let books = connector
        .fetch_order_books(&token_ids.into_iter().collect::<Vec<_>>())
        .await?
        .into_iter()
        .map(|book| {
            (
                book.token_id.clone(),
                CopyOrderBook {
                    token_id: book.token_id,
                    bids: book
                        .bids
                        .into_iter()
                        .map(|level| CopyBookLevel {
                            price: level.price,
                            size: level.size,
                        })
                        .collect(),
                    asks: book
                        .asks
                        .into_iter()
                        .map(|level| CopyBookLevel {
                            price: level.price,
                            size: level.size,
                        })
                        .collect(),
                    observed_at: book.observed_at,
                },
            )
        })
        .collect::<HashMap<_, _>>();

    Ok((wallet_feeds, books))
}

async fn fetch_wallet_analysis_inputs(
    state: &AppState,
) -> Result<Vec<WalletFeedInput>, polyedge_domain::AppError> {
    let _config = state.copytrade_service.read_config().await?;
    let wallets = state.copytrade_service.snapshot().await?.wallets;
    let active_wallets: Vec<_> = wallets
        .into_iter()
        .filter(|w| w.status == TrackedWalletStatus::Active)
        .collect();

    if active_wallets.is_empty() {
        return Ok(Vec::new());
    }

    let connector = PolymarketDataApiConnector::new(
        &state.settings.polymarket.data_api_host,
    )?;

    let mut feeds = Vec::new();
    for wallet in active_wallets {
        let limit = state.settings.copytrade.wallet_activity_limit;
        let activities = connector
            .fetch_wallet_activity(&wallet.address, limit)
            .await
            .map(|raws| {
                raws.into_iter()
                    .map(|raw| WalletActivityInput {
                        kind: raw.kind,
                        side: raw.side,
                        asset: raw.asset,
                        condition_id: raw.condition_id,
                        outcome: raw.outcome,
                        title: raw.title,
                        slug: raw.slug,
                        price: raw.price,
                        size: raw.size,
                        usdc_size: raw.usdc_size,
                        transaction_hash: raw.transaction_hash,
                        timestamp: raw.timestamp,
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let positions = connector
            .fetch_wallet_positions(&wallet.address)
            .await
            .map(|raws| {
                raws.into_iter()
                    .map(|raw| WalletPositionInput {
                        asset: raw.asset,
                        condition_id: raw.condition_id,
                        outcome: raw.outcome,
                        title: raw.title,
                        slug: raw.slug,
                        size: raw.size,
                        avg_price: raw.avg_price,
                        cur_price: raw.cur_price,
                        realized_pnl: raw.realized_pnl,
                        percent_pnl: raw.percent_pnl,
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        feeds.push(WalletFeedInput {
            address: wallet.address,
            activities,
            positions,
        });
    }

    Ok(feeds)
}
