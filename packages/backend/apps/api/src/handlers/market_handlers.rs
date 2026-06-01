async fn list_markets(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<MarketListQuery>,
) -> std::result::Result<Json<MarketListResponse>, HttpError> {
    let trace_id = new_trace_id();
    let sort_by = query
        .sort_by
        .as_deref()
        .map(MarketSortField::from_str)
        .transpose()
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let sort_order = query
        .sort_order
        .as_deref()
        .map(SortOrder::from_str)
        .transpose()
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let filters = MarketListFilters::new(
        query.status,
        query.tradability_status,
        query.category,
        sort_by,
        sort_order,
        query.offset,
        query.limit,
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    let (markets, total_count) = tokio::try_join!(
        state.market_event_service.list_markets(filters.clone()),
        state.market_event_service.count_markets(filters),
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(MarketListResponse {
        data: markets.into_iter().map(market_to_contract).collect(),
        total_count,
        meta: ApiMeta::new(auth.request_id, trace_id),
    }))
}

async fn get_market(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Path(market_id): Path<String>,
) -> std::result::Result<Json<ApiResponse<MarketData>>, HttpError> {
    let trace_id = new_trace_id();

    let market = state
        .market_event_service
        .get_market(&market_id)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        market_to_contract(market),
        auth.request_id,
        trace_id,
    )))
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

async fn list_market_categories(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> std::result::Result<Json<ApiResponse<Vec<MarketCategoryData>>>, HttpError> {
    let trace_id = new_trace_id();
    let categories = state
        .market_event_service
        .list_market_categories()
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        categories
            .into_iter()
            .map(|cat| MarketCategoryData {
                id: cat.id,
                label: cat.label,
            })
            .collect(),
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

async fn get_orderbook(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Path(token_id): Path<String>,
) -> std::result::Result<Json<ApiResponse<OrderbookData>>, HttpError> {
    let trace_id = new_trace_id();

    let book = state
        .orderbook_cache
        .get_book(&token_id)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    match book {
        Some(book) => Ok(Json(ApiResponse::new(
            OrderbookData {
                token_id: book.token_id,
                bids: book
                    .bids
                    .into_iter()
                    .map(|l| OrderbookLevelData {
                        price: l.price.to_string(),
                        size: l.size.to_string(),
                    })
                    .collect(),
                asks: book
                    .asks
                    .into_iter()
                    .map(|l| OrderbookLevelData {
                        price: l.price.to_string(),
                        size: l.size.to_string(),
                    })
                    .collect(),
                observed_at: book.observed_at,
                source: book.source.to_string(),
            },
            auth.request_id,
            trace_id,
        ))),
        None => Err(HttpError::with_meta(
            AppError::not_found("ORDERBOOK_NOT_FOUND", format!("no orderbook data for token {token_id}")),
            auth.request_id,
            trace_id,
        )),
    }
}