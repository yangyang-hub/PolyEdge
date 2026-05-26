async fn list_markets(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<MarketListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<MarketData>>>, HttpError> {
    let trace_id = new_trace_id();
    let filters = MarketListFilters::new(query.status, query.tradability_status, query.limit)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    if state.settings.runtime.environment == "test" {
        let markets = state
            .market_event_service
            .list_markets(filters)
            .await
            .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

        return Ok(Json(ApiResponse::new(
            markets.into_iter().map(market_to_contract).collect(),
            auth.request_id,
            trace_id,
        )));
    }

    let connector = PolymarketGammaConnector::new(&state.settings.polymarket.gamma_host)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let markets = connector
        .fetch_markets(filters.limit)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?
        .into_iter()
        .map(polymarket_gamma_market_to_view)
        .filter(|market| {
            filters.status.is_none_or(|status| market.status == status)
                && filters
                    .tradability_status
                    .is_none_or(|status| market.tradability_status == status)
        })
        .collect::<Vec<_>>();

    Ok(Json(ApiResponse::new(
        markets.into_iter().map(market_to_contract).collect(),
        auth.request_id,
        trace_id,
    )))
}

async fn get_market(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Path(market_id): Path<String>,
) -> std::result::Result<Json<ApiResponse<MarketData>>, HttpError> {
    let trace_id = new_trace_id();

    if state.settings.runtime.environment == "test" {
        let market = state
            .market_event_service
            .get_market(&market_id)
            .await
            .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

        return Ok(Json(ApiResponse::new(
            market_to_contract(market),
            auth.request_id,
            trace_id,
        )));
    }

    let connector = PolymarketGammaConnector::new(&state.settings.polymarket.gamma_host)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let market = connector
        .fetch_market(&market_id)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?
        .map(polymarket_gamma_market_to_view)
        .ok_or_else(|| {
            HttpError::with_meta(
                AppError::not_found(
                    "MARKET_NOT_FOUND",
                    format!("market was not found: {market_id}"),
                ),
                auth.request_id.clone(),
                trace_id.clone(),
            )
        })?;

    Ok(Json(ApiResponse::new(
        market_to_contract(market),
        auth.request_id,
        trace_id,
    )))
}

fn polymarket_gamma_market_to_view(market: PolymarketGammaMarket) -> MarketView {
    MarketView {
        id: market.id,
        question: market.question,
        category: market.category,
        status: market.status,
        best_bid: market.best_bid,
        best_ask: market.best_ask,
        mid_price: market.mid_price,
        volume_24h: market.volume_24h,
        ambiguity_level: market.ambiguity_level,
        tradability_status: market.tradability_status,
        resolution_source: market.resolution_source,
        edge_case_notes: market.edge_case_notes,
        polymarket_condition_id: Some(market.condition_id),
        polymarket_yes_asset_id: Some(market.yes_asset_id),
        polymarket_no_asset_id: Some(market.no_asset_id),
        updated_at: market.updated_at,
        version: market.version,
    }
}

async fn list_events(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<EventListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<EventData>>>, HttpError> {
    let trace_id = new_trace_id();
    let filters = EventListFilters::new(query.status, query.limit)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let events = state
        .market_event_service
        .list_events(filters)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        events.into_iter().map(event_to_contract).collect(),
        auth.request_id,
        trace_id,
    )))
}

async fn list_news_source_health(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<NewsSourceHealthListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<NewsSourceHealthData>>>, HttpError> {
    let trace_id = new_trace_id();
    let filters = NewsSourceHealthListFilters::new(query.source_type, query.limit)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let sources = state
        .news_ingestion_service
        .list_source_health(filters)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        sources
            .into_iter()
            .map(news_source_health_to_contract)
            .collect(),
        auth.request_id,
        trace_id,
    )))
}

async fn list_news_raw_events(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<NewsRawEventListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<NewsRawEventData>>>, HttpError> {
    let trace_id = new_trace_id();
    let filters = NewsRawEventListFilters::new(query.source, query.source_type, query.limit)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let events = state
        .news_ingestion_service
        .list_raw_events(filters)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        events.into_iter().map(news_raw_event_to_contract).collect(),
        auth.request_id,
        trace_id,
    )))
}

async fn list_evidences(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<EvidenceListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<EvidenceData>>>, HttpError> {
    let trace_id = new_trace_id();
    let filters =
        EvidenceListFilters::new(query.market_id, query.event_id, query.status, query.limit)
            .map_err(|error| {
                HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
            })?;
    let evidences = state
        .market_event_service
        .list_evidences(filters)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        evidences.into_iter().map(evidence_to_contract).collect(),
        auth.request_id,
        trace_id,
    )))
}

async fn list_signals(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<SignalListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<SignalData>>>, HttpError> {
    let trace_id = new_trace_id();
    let filters = SignalListFilters::new(
        query.market_id,
        query.event_id,
        query.lifecycle_state,
        query.limit,
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let signals = state
        .market_event_service
        .list_signals(filters)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        signals.into_iter().map(signal_to_contract).collect(),
        auth.request_id,
        trace_id,
    )))
}

async fn list_probability_estimates(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<ProbabilityEstimateListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<ProbabilityEstimateData>>>, HttpError> {
    let trace_id = new_trace_id();
    let filters = ProbabilityEstimateListFilters::new(
        query.market_id,
        query.event_id,
        query.signal_id,
        query.limit,
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let estimates = state
        .market_event_service
        .list_probability_estimates(filters)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        estimates
            .into_iter()
            .map(probability_estimate_to_contract)
            .collect(),
        auth.request_id,
        trace_id,
    )))
}

async fn list_arbitrage_scans(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<ArbitrageScanListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<ArbitrageScanData>>>, HttpError> {
    let trace_id = new_trace_id();
    let filters = ArbitrageScanListFilters::new(query.limit)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let scans = state
        .arbitrage_service
        .list_scans(filters)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        scans.into_iter().map(arbitrage_scan_to_contract).collect(),
        auth.request_id,
        trace_id,
    )))
}

async fn list_arbitrage_opportunities(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<ArbitrageOpportunityListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<ArbitrageOpportunityData>>>, HttpError> {
    let trace_id = new_trace_id();
    let opportunity_type = query
        .opportunity_type
        .as_deref()
        .map(ArbitrageOpportunityType::from_str)
        .transpose()
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let status = query
        .status
        .as_deref()
        .map(ArbitrageOpportunityStatus::from_str)
        .transpose()
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let validation_status = query
        .validation_status
        .as_deref()
        .map(ArbitrageValidationStatus::from_str)
        .transpose()
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let min_net_edge = query
        .min_net_edge
        .as_deref()
        .map(|value| {
            Decimal::from_str(value)
                .map_err(|error| {
                    AppError::invalid_input(
                        "ARBITRAGE_MIN_NET_EDGE_INVALID",
                        format!("min_net_edge must be decimal: {error}"),
                    )
                })
                .and_then(Edge::new)
        })
        .transpose()
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let observed_after = query
        .observed_after
        .as_deref()
        .map(|value| {
            OffsetDateTime::parse(value, &Rfc3339).map_err(|error| {
                AppError::invalid_input(
                    "ARBITRAGE_OBSERVED_AFTER_INVALID",
                    format!("observed_after must be RFC3339: {error}"),
                )
            })
        })
        .transpose()
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let filters = ArbitrageOpportunityListFilters::new(
        query.market_id,
        opportunity_type,
        status,
        validation_status,
        min_net_edge,
        observed_after,
        query.active_only.unwrap_or(false),
        query.limit,
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let opportunities = state
        .arbitrage_service
        .list_opportunities(filters)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        opportunities
            .into_iter()
            .map(arbitrage_opportunity_to_contract)
            .collect(),
        auth.request_id,
        trace_id,
    )))
}

async fn list_arbitrage_analysis_runs(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<ArbitrageAnalysisRunListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<ArbitrageAnalysisRunData>>>, HttpError> {
    let trace_id = new_trace_id();
    let filters = ArbitrageAnalysisRunListFilters::new(query.limit)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let runs = state
        .arbitrage_service
        .list_analysis_runs(filters)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        runs.into_iter()
            .map(arbitrage_analysis_run_to_contract)
            .collect(),
        auth.request_id,
        trace_id,
    )))
}
