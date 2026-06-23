async fn scan_arbitrage_once(state: &AppState, trace_id: &str) -> Result<ArbitrageScanRunReport> {
    let started_at = OffsetDateTime::now_utc();
    let scan_id = format!("scan_{}", trace_id.trim_start_matches("trc_"));
    let book_source = state.settings.arbitrage.book_source.trim();
    let scan = ArbitrageScanView {
        id: scan_id.clone(),
        started_at,
        finished_at: None,
        market_count: 0,
        snapshot_count: 0,
        opportunity_count: 0,
        scanner_version: state.settings.arbitrage.scanner_version.clone(),
        metadata: json!({
            "book_source": book_source,
            "scan_limit": state.settings.arbitrage.scan_limit,
        }),
        trace_id: trace_id.to_string(),
    };
    state.arbitrage_service.start_scan(scan).await?;

    let validation_config = arbitrage_validation_config(state);
    let mut markets = state
        .market_event_service
        .list_markets(MarketListFilters::new(
            Some(MarketStatus::Open),
            None,
            None,
            None,
            None,
            None,
            Some(state.settings.arbitrage.scan_limit),
        )?)
        .await?;

    if markets.is_empty() {
        match fetch_gamma_markets(state).await {
            Ok(gamma_markets) => {
                if !gamma_markets.is_empty() {
                    let count = gamma_markets.len();
                    if let Err(error) = state
                        .market_event_service
                        .upsert_markets(&gamma_markets, trace_id)
                        .await
                    {
                        warn!(
                            trace_id,
                            error = %error,
                            "failed to upsert gamma markets during arbitrage fallback",
                        );
                    }
                    info!(
                        trace_id,
                        count,
                        "loaded markets from Polymarket Gamma (database empty)",
                    );
                    markets = gamma_markets;
                }
            }
            Err(error) => {
                warn!(
                    trace_id,
                    error = %error,
                    "failed to fetch markets from Polymarket Gamma fallback",
                );
            }
        }
    }

    let book_feed = build_arbitrage_book_feed(state)?;
    let mut report = ArbitrageScanRunReport {
        markets_scanned: markets.len(),
        ..ArbitrageScanRunReport::default()
    };
    let expired_before =
        started_at - duration_seconds(state.settings.arbitrage.opportunity_ttl_secs);
    let expired = state
        .arbitrage_service
        .expire_opportunities(expired_before, trace_id)
        .await?;
    report.opportunities_expired += expired.len();

    for market in markets {
        let snapshot =
            match build_arbitrage_book_snapshot(&book_feed, &market, &scan_id, trace_id).await {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    report.failed_books += 1;
                    warn!(
                        trace_id,
                        market_id = %market.id,
                        error = %error,
                        "failed to build arbitrage book snapshot",
                    );
                    continue;
                }
            };

        let opportunities = state
            .arbitrage_service
            .record_snapshot_and_detect(snapshot.clone())
            .await?;
        report.snapshots_recorded += 1;
        report.opportunities_recorded += opportunities.len();

        for opportunity in &opportunities {
            let validation_snapshot = match build_arbitrage_book_snapshot(
                &book_feed, &market, &scan_id, trace_id,
            )
            .await
            {
                Ok(mut snapshot) => {
                    report.validation_books_refetched += 1;
                    snapshot.id = validation_market_book_snapshot_id(&snapshot, opportunity);
                    state
                        .arbitrage_service
                        .record_book_snapshot(snapshot.clone())
                        .await?;
                    report.snapshots_recorded += 1;
                    snapshot
                }
                Err(error) => {
                    report.validation_book_failures += 1;
                    warn!(
                        trace_id,
                        scan_id = %scan_id,
                        market_id = %market.id,
                        opportunity_id = %opportunity.id,
                        error = %error,
                        "failed to refetch arbitrage book for validation",
                    );
                    continue;
                }
            };
            let validation = state
                .arbitrage_service
                .validate_opportunity(
                    opportunity,
                    &validation_snapshot,
                    &validation_config,
                    OffsetDateTime::now_utc(),
                )
                .await?;
            report.validations_recorded += 1;
            info!(
                trace_id,
                scan_id = %scan_id,
                market_id = %market.id,
                opportunity_id = %opportunity.id,
                validation_id = %validation.id,
                validation_status = %validation.status.as_str(),
                net_edge = %validation.net_edge.value(),
                book_age_ms = validation.book_age_ms,
                "validated arbitrage opportunity",
            );
        }
    }

    state
        .arbitrage_service
        .complete_scan(
            &scan_id,
            OffsetDateTime::now_utc(),
            u32::try_from(report.markets_scanned).unwrap_or(u32::MAX),
            u32::try_from(report.snapshots_recorded).unwrap_or(u32::MAX),
            u32::try_from(report.opportunities_recorded).unwrap_or(u32::MAX),
        )
        .await?;

    let event_retention_cutoff =
        started_at - duration_hours(state.settings.arbitrage.event_retention_hours);
    report.events_pruned = state
        .arbitrage_service
        .prune_events(event_retention_cutoff)
        .await?;
    let history_pruned = state
        .arbitrage_service
        .prune_scan_history(event_retention_cutoff)
        .await?;
    report.scans_pruned = history_pruned.scans_deleted;
    report.snapshots_pruned = history_pruned.snapshots_deleted;
    report.scan_opportunities_pruned = history_pruned.opportunities_deleted;

    Ok(report)
}

async fn poll_arbitrage_radar(
    state: &AppState,
    max_cycles: Option<usize>,
) -> Result<ArbitrageScanRunReport> {
    let mut total = ArbitrageScanRunReport::default();
    let mut cycles = 0usize;
    let interval = Duration::from_secs(state.settings.arbitrage.poll_interval_secs.max(1));

    loop {
        let trace_id = new_trace_id();
        let report = scan_arbitrage_once(state, &trace_id).await?;
        total.markets_scanned += report.markets_scanned;
        total.snapshots_recorded += report.snapshots_recorded;
        total.opportunities_recorded += report.opportunities_recorded;
        total.validations_recorded += report.validations_recorded;
        total.validation_books_refetched += report.validation_books_refetched;
        total.validation_book_failures += report.validation_book_failures;
        total.opportunities_expired += report.opportunities_expired;
        total.events_pruned = total.events_pruned.saturating_add(report.events_pruned);
        total.scans_pruned = total.scans_pruned.saturating_add(report.scans_pruned);
        total.snapshots_pruned = total
            .snapshots_pruned
            .saturating_add(report.snapshots_pruned);
        total.scan_opportunities_pruned = total
            .scan_opportunities_pruned
            .saturating_add(report.scan_opportunities_pruned);
        total.failed_books += report.failed_books;
        cycles += 1;

        info!(
            trace_id = %trace_id,
            cycle = cycles,
            markets_scanned = report.markets_scanned,
            snapshots_recorded = report.snapshots_recorded,
            opportunities_recorded = report.opportunities_recorded,
            validations_recorded = report.validations_recorded,
            validation_books_refetched = report.validation_books_refetched,
            validation_book_failures = report.validation_book_failures,
            opportunities_expired = report.opportunities_expired,
            events_pruned = report.events_pruned,
            scans_pruned = report.scans_pruned,
            snapshots_pruned = report.snapshots_pruned,
            scan_opportunities_pruned = report.scan_opportunities_pruned,
            failed_books = report.failed_books,
            "completed arbitrage radar polling cycle",
        );

        if max_cycles.is_some_and(|limit| cycles >= limit) {
            break;
        }

        tokio::select! {
            () = tokio::time::sleep(interval) => {}
            shutdown = tokio::signal::ctrl_c() => {
                if let Err(error) = shutdown {
                    warn!(error = %error, "failed to listen for ctrl-c during arbitrage polling");
                }
                break;
            }
        }
    }

    Ok(total)
}

async fn fetch_gamma_markets(state: &AppState) -> Result<Vec<MarketView>> {
    let connector =
        PolymarketGammaConnector::new(&state.settings.polymarket.gamma_host)?;
    let page_size = state.settings.arbitrage.scan_limit;
    let markets = connector.fetch_markets(page_size).await?;
    Ok(markets
        .into_iter()
        .map(gamma_market_to_view)
        .filter(|market| market.status == MarketStatus::Open)
        .collect())
}
