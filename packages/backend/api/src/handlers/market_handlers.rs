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
) -> std::result::Result<Json<ApiResponse<Paginated<EventData>>>, HttpError> {
    let trace_id = new_trace_id();
    let page_query = PageQuery {
        page: query.page.unwrap_or(1),
        page_size: query.page_size.unwrap_or(20),
        sort_order: None,
    };
    let filters = EventListFilters::new(query.status, None)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let result = state
        .market_event_service
        .list_events(filters, &page_query)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        result.map(event_to_contract),
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
) -> std::result::Result<Json<ApiResponse<Paginated<NewsSourceHealthData>>>, HttpError> {
    let trace_id = new_trace_id();
    let page_query = PageQuery {
        page: query.page.unwrap_or(1),
        page_size: query.page_size.unwrap_or(20),
        sort_order: None,
    };
    let filters = NewsSourceHealthListFilters::new(query.source_type, None)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let result = state
        .news_ingestion_service
        .list_source_health(filters, &page_query)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        result.map(news_source_health_to_contract),
        auth.request_id,
        trace_id,
    )))
}

async fn list_news_raw_events(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<NewsRawEventListQuery>,
) -> std::result::Result<Json<ApiResponse<Paginated<NewsRawEventData>>>, HttpError> {
    let trace_id = new_trace_id();
    let page_query = PageQuery {
        page: query.page.unwrap_or(1),
        page_size: query.page_size.unwrap_or(20),
        sort_order: None,
    };
    let filters = NewsRawEventListFilters::new(query.source, query.source_type, None)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let result = state
        .news_ingestion_service
        .list_raw_events(filters, &page_query)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        result.map(news_raw_event_to_contract),
        auth.request_id,
        trace_id,
    )))
}

async fn list_evidences(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<EvidenceListQuery>,
) -> std::result::Result<Json<ApiResponse<Paginated<EvidenceData>>>, HttpError> {
    let trace_id = new_trace_id();
    let page_query = PageQuery {
        page: query.page.unwrap_or(1),
        page_size: query.page_size.unwrap_or(20),
        sort_order: None,
    };
    let filters =
        EvidenceListFilters::new(query.market_id, query.event_id, query.status, None)
            .map_err(|error| {
                HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
            })?;
    let result = state
        .market_event_service
        .list_evidences(filters, &page_query)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        result.map(evidence_to_contract),
        auth.request_id,
        trace_id,
    )))
}

async fn list_probability_estimates(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<ProbabilityEstimateListQuery>,
) -> std::result::Result<Json<ApiResponse<Paginated<ProbabilityEstimateData>>>, HttpError> {
    let trace_id = new_trace_id();
    let page_query = PageQuery {
        page: query.page.unwrap_or(1),
        page_size: query.page_size.unwrap_or(20),
        sort_order: None,
    };
    let filters = ProbabilityEstimateListFilters::new(
        query.market_id,
        query.event_id,
        query.signal_id,
        None,
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let result = state
        .market_event_service
        .list_probability_estimates(filters, &page_query)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        result.map(probability_estimate_to_contract),
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
