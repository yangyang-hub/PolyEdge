async fn ingest_news_once(state: &AppState, trace_id: &str) -> Result<NewsIngestionRunReport> {
    let settings = &state.settings.news;
    if !settings.enabled {
        return Err(AppError::invalid_input(
            "NEWS_INGESTION_DISABLED",
            "news ingestion is disabled; set POLYEDGE_NEWS__ENABLED=true",
        ));
    }

    let enabled_sources: Vec<_> = settings
        .sources
        .iter()
        .filter(|source| source.enabled)
        .collect();

    if enabled_sources.is_empty() {
        return Err(AppError::invalid_input(
            "NEWS_SOURCES_REQUIRED",
            "news ingestion requires at least one enabled source",
        ));
    }

    let timeout = Duration::from_secs(settings.request_timeout_secs.max(1));
    let mut report = NewsIngestionRunReport {
        sources_scanned: enabled_sources.len(),
        ..NewsIngestionRunReport::default()
    };

    for source in enabled_sources {
        let connector = match RssNewsConnector::new(
            RssNewsSourceConfig {
                id: source.id.clone(),
                source_type: source.source_type.clone(),
                url: source.url.clone(),
            },
            timeout,
        ) {
            Ok(connector) => connector,
            Err(error) => {
                record_news_failure(state, source, &error, trace_id).await?;
                report.sources_failed += 1;
                warn!(
                    source = %source.id,
                    error = %error,
                    "news source configuration failed",
                );
                continue;
            }
        };

        let fetched_items = match connector.fetch().await {
            Ok(items) => items,
            Err(error) => {
                record_news_failure(state, source, &error, trace_id).await?;
                report.sources_failed += 1;
                warn!(
                    source = %source.id,
                    error = %error,
                    "news source fetch failed",
                );
                continue;
            }
        };

        let items: Vec<_> = fetched_items
            .into_iter()
            .take(settings.max_items_per_source)
            .map(news_item_to_ingestion_item)
            .collect();
        let source_report = match state
            .news_ingestion_service
            .ingest_source_items(NewsIngestSourceCommand {
                source: source.id.clone(),
                source_type: source.source_type.clone(),
                reliability: source.reliability,
                items,
                trace_id: trace_id.to_string(),
            })
            .await
        {
            Ok(source_report) => source_report,
            Err(error) => {
                record_news_failure(state, source, &error, trace_id).await?;
                report.sources_failed += 1;
                warn!(
                    source = %source.id,
                    error = %error,
                    "news source ingestion failed",
                );
                continue;
            }
        };

        report.sources_succeeded += 1;
        report.fetched += source_report.fetched;
        report.inserted += source_report.inserted;
        report.deduped += source_report.deduped;
    }

    Ok(report)
}

async fn poll_news(state: &AppState, max_cycles: Option<usize>) -> Result<NewsIngestionRunReport> {
    let mut total = NewsIngestionRunReport::default();
    let mut cycles = 0usize;
    let interval = Duration::from_secs(state.settings.news.poll_interval_secs.max(1));

    loop {
        let trace_id = new_trace_id();
        let report = ingest_news_once(state, &trace_id).await?;
        total.sources_scanned += report.sources_scanned;
        total.sources_succeeded += report.sources_succeeded;
        total.sources_failed += report.sources_failed;
        total.fetched += report.fetched;
        total.inserted += report.inserted;
        total.deduped += report.deduped;
        cycles += 1;

        info!(
            trace_id = %trace_id,
            cycle = cycles,
            sources_scanned = report.sources_scanned,
            sources_succeeded = report.sources_succeeded,
            sources_failed = report.sources_failed,
            fetched = report.fetched,
            inserted = report.inserted,
            deduped = report.deduped,
            "completed news polling cycle",
        );

        if max_cycles.is_some_and(|limit| cycles >= limit) {
            break;
        }

        tokio::select! {
            () = tokio::time::sleep(interval) => {}
            shutdown = tokio::signal::ctrl_c() => {
                if let Err(error) = shutdown {
                    warn!(error = %error, "failed to listen for ctrl-c during news polling");
                }
                break;
            }
        }
    }

    Ok(total)
}

async fn promote_news_events(
    state: &AppState,
    limit: Option<u16>,
    trace_id: &str,
) -> Result<NewsPromotionReport> {
    let raw_events = state
        .news_ingestion_service
        .list_raw_events(NewsRawEventListFilters::new(None, None, limit)?)
        .await?;
    let markets = state
        .market_event_service
        .list_markets(MarketListFilters::new(None, None, None, None, None, None, Some(200))?)
        .await?;
    let source_health = state
        .news_ingestion_service
        .list_source_health(NewsSourceHealthListFilters::new(None, Some(200))?)
        .await?
        .into_iter()
        .map(|health| (health.source.clone(), health))
        .collect::<HashMap<_, _>>();
    let mut report = NewsPromotionReport {
        scanned: raw_events.len(),
        ..NewsPromotionReport::default()
    };
    let mut promoted_events = Vec::new();
    let mut promoted_evidences = Vec::new();

    for raw_event in raw_events {
        let related_market_ids = match_raw_news_markets(&raw_event, &markets);

        if related_market_ids.is_empty() {
            report.skipped_unmatched += 1;
            continue;
        }

        let health = source_health.get(&raw_event.source);
        let promoted_event =
            build_promoted_event_record(&raw_event, related_market_ids.clone(), health)?;
        for market_id in &related_market_ids {
            promoted_evidences.push(build_promoted_evidence_record(
                &raw_event,
                market_id,
                &promoted_event.id,
                health,
            )?);
        }
        promoted_events.push(promoted_event);
    }

    report.promoted = promoted_events.len();
    report.evidences_promoted = promoted_evidences.len();

    if promoted_events.is_empty() {
        return Ok(report);
    }

    state
        .market_event_service
        .ingest_fixture_bundle(
            FixtureBundle {
                markets: Vec::new(),
                events: promoted_events,
                evidences: promoted_evidences,
                signals: Vec::new(),
            },
            trace_id,
        )
        .await?;

    Ok(report)
}
