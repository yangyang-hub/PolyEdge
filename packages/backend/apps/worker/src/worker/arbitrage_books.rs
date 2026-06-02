fn arbitrage_validation_config(state: &AppState) -> ArbitrageValidationConfig {
    ArbitrageValidationConfig {
        max_book_age_ms: state.settings.arbitrage.max_book_age_ms,
        min_gross_edge: state.settings.arbitrage.min_gross_edge,
        min_capacity: state.settings.arbitrage.min_capacity,
        fee_buffer: state.settings.arbitrage.fee_buffer,
        slippage_buffer: state.settings.arbitrage.slippage_buffer,
    }
}

fn duration_seconds(seconds: u64) -> TimeDuration {
    TimeDuration::seconds(i64::try_from(seconds).unwrap_or(i64::MAX))
}

fn duration_hours(hours: u64) -> TimeDuration {
    TimeDuration::hours(i64::try_from(hours).unwrap_or(i64::MAX))
}

async fn analyze_arbitrage_opportunities(
    state: &AppState,
    lookback_hours: u16,
    trace_id: &str,
) -> Result<ArbitrageAnalysisRunView> {
    let generated_at = OffsetDateTime::now_utc();
    let observed_after = generated_at - TimeDuration::hours(i64::from(lookback_hours.max(1)));
    let page_query = PageQuery { page: 1, page_size: 500, sort_order: None };
    let opportunities = state
        .arbitrage_service
        .list_opportunities(ArbitrageOpportunityListFilters::new(
            None,
            None,
            None,
            None,
            None,
            Some(observed_after),
            false,
        )?, &page_query)
        .await?;
    let summary = build_arbitrage_analysis(&opportunities.data, lookback_hours.max(1), generated_at);
    let summary_payload = serde_json::to_value(&summary).map_err(|error| {
        AppError::internal(
            "ARBITRAGE_ANALYSIS_ENCODE_FAILED",
            format!("failed to encode arbitrage analysis summary: {error}"),
        )
    })?;
    let analysis = ArbitrageAnalysisRunView {
        id: format!("arb_analysis_{}", trace_id.trim_start_matches("trc_")),
        generated_at,
        lookback_hours: lookback_hours.max(1),
        opportunity_count: summary.opportunity_count,
        market_count: summary.market_count,
        summary_payload,
        trace_id: trace_id.to_string(),
    };

    state.arbitrage_service.record_analysis_run(analysis).await
}

enum ArbitrageBookFeed {
    MarketSnapshot,
    Polymarket(PolymarketBookConnector),
}

fn build_arbitrage_book_feed(state: &AppState) -> Result<ArbitrageBookFeed> {
    match state.settings.arbitrage.book_source.trim() {
        "" | "market_snapshot" => Ok(ArbitrageBookFeed::MarketSnapshot),
        "polymarket" => Ok(ArbitrageBookFeed::Polymarket(PolymarketBookConnector::new(
            &state.settings.polymarket.clob_host,
        )?)),
        other => Err(AppError::invalid_input(
            "ARBITRAGE_BOOK_SOURCE_UNSUPPORTED",
            format!("unsupported arbitrage book_source={other}"),
        )),
    }
}

async fn build_arbitrage_book_snapshot(
    feed: &ArbitrageBookFeed,
    market: &MarketView,
    scan_id: &str,
    trace_id: &str,
) -> Result<MarketBookSnapshotView> {
    match feed {
        ArbitrageBookFeed::MarketSnapshot => {
            build_market_snapshot_book_snapshot(market, scan_id, trace_id)
        }
        ArbitrageBookFeed::Polymarket(connector) => {
            let refs = polymarket_market_refs(market)?;
            let snapshot = connector.fetch_binary_book(&refs).await?;
            build_polymarket_book_snapshot(market, &snapshot, scan_id, trace_id)
        }
    }
}

fn validation_market_book_snapshot_id(
    snapshot: &MarketBookSnapshotView,
    opportunity: &ArbitrageOpportunityView,
) -> String {
    format!(
        "{}_validation_{}_{}",
        snapshot.id,
        opportunity.opportunity_type.as_str(),
        snapshot.observed_at.unix_timestamp_nanos()
    )
}

fn build_market_snapshot_book_snapshot(
    market: &MarketView,
    scan_id: &str,
    trace_id: &str,
) -> Result<MarketBookSnapshotView> {
    let no_bid = Probability::new(Decimal::ONE - market.best_ask.value())?;
    let no_ask = Probability::new(Decimal::ONE - market.best_bid.value())?;
    let zero = zero_quantity();

    Ok(MarketBookSnapshotView {
        id: market_book_snapshot_id(scan_id, &market.id),
        scan_id: scan_id.to_string(),
        connector_name: "market_snapshot".to_string(),
        market_id: market.id.clone(),
        yes_asset_id: market.polymarket_yes_asset_id.clone(),
        no_asset_id: market.polymarket_no_asset_id.clone(),
        yes_bid: Some(market.best_bid),
        yes_ask: Some(market.best_ask),
        yes_bid_size: zero,
        yes_ask_size: zero,
        no_bid: Some(no_bid),
        no_ask: Some(no_ask),
        no_bid_size: zero,
        no_ask_size: zero,
        observed_at: OffsetDateTime::now_utc(),
        raw_payload: json!({
            "source": "market_snapshot",
            "market_best_bid": market.best_bid,
            "market_best_ask": market.best_ask,
            "derived_no_bid": no_bid,
            "derived_no_ask": no_ask,
        }),
        trace_id: trace_id.to_string(),
    })
}

fn build_polymarket_book_snapshot(
    market: &MarketView,
    snapshot: &PolymarketBinaryBookSnapshot,
    scan_id: &str,
    trace_id: &str,
) -> Result<MarketBookSnapshotView> {
    Ok(MarketBookSnapshotView {
        id: market_book_snapshot_id(scan_id, &market.id),
        scan_id: scan_id.to_string(),
        connector_name: POLYMARKET_CONNECTOR_NAME.to_string(),
        market_id: market.id.clone(),
        yes_asset_id: Some(snapshot.yes_asset_id.clone()),
        no_asset_id: Some(snapshot.no_asset_id.clone()),
        yes_bid: book_level_price(&snapshot.yes.best_bid),
        yes_ask: book_level_price(&snapshot.yes.best_ask),
        yes_bid_size: book_level_size(&snapshot.yes.best_bid),
        yes_ask_size: book_level_size(&snapshot.yes.best_ask),
        no_bid: book_level_price(&snapshot.no.best_bid),
        no_ask: book_level_price(&snapshot.no.best_ask),
        no_bid_size: book_level_size(&snapshot.no.best_bid),
        no_ask_size: book_level_size(&snapshot.no.best_ask),
        observed_at: snapshot.observed_at,
        raw_payload: json!({
            "source": "polymarket",
            "condition_id": snapshot.condition_id,
            "yes_asset_id": snapshot.yes_asset_id,
            "no_asset_id": snapshot.no_asset_id,
            "yes_book": snapshot.yes.raw_payload,
            "no_book": snapshot.no.raw_payload,
        }),
        trace_id: trace_id.to_string(),
    })
}

fn book_level_price(level: &Option<PolymarketBookLevel>) -> Option<Probability> {
    level.as_ref().map(|level| level.price)
}

fn book_level_size(level: &Option<PolymarketBookLevel>) -> Quantity {
    level
        .as_ref()
        .map_or_else(zero_quantity, |level| level.size)
}

fn zero_quantity() -> Quantity {
    Quantity::new(Decimal::ZERO).expect("zero quantity must be valid")
}
