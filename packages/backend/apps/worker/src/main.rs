use futures::StreamExt as _;
use polyedge_application::{
    AuthenticatedActor, DispatchExecutionListFilters, ExecutionDispatchCandidate,
    ExecutionReconciliationCandidate, FixtureBundle, FixtureEventRecord, FixtureEvidenceRecord,
    MarkExecutionFailedCommand, MarkExecutionSubmittedCommand, MarketListFilters, MarketView,
    NewsIngestSourceCommand, NewsIngestionItem, NewsRawEventListFilters, NewsRawEventView,
    NewsSourceFailureUpdate, NewsSourceHealthListFilters, NewsSourceHealthView, OrderListFilters,
    ReconcileExecutionListFilters, ReconcileExternalTradeCommand, SyncExternalOrderStatusCommand,
    demo_fixture_bundle,
};
use polyedge_connectors::{
    ConnectorNewsItem, LivePolymarketConfig, LivePolymarketConnector,
    LivePolymarketExecutionOutcome, LivePolymarketOrderRequest, LivePolymarketOrderStatusRequest,
    LivePolymarketTradeSyncRequest, MockPolymarketConnector, MockPolymarketExecutionOutcome,
    MockPolymarketFillRequest, MockPolymarketOrderRequest, MockPolymarketOrderStatusRequest,
    NewsSource, PAPER_ACCOUNT_ID, PAPER_EXECUTOR_NAME, POLYMARKET_ACCOUNT_ID,
    POLYMARKET_CONNECTOR_NAME, PaperExecutionOutcome, PaperExecutor, PaperFillRequest,
    PaperOrderRequest, PaperOrderStatusRequest, PolymarketMarketRefs, PolymarketSignatureScheme,
    RssNewsConnector, RssNewsSourceConfig, normalize_polymarket_order_status_update,
    normalize_polymarket_trade_fill_update, normalize_polymarket_ws_order_message,
    normalize_polymarket_ws_trade_message,
};
use polyedge_domain::{
    AppError, EventStatus, EvidenceDirection, EvidenceStatus, OrderStatus, Probability, Quantity,
    Result, UserRole,
};
use polyedge_infrastructure::{
    AppState, Runtime, new_trace_id,
    settings::{NewsSourceSettings, PolymarketConnectorMode, PolymarketSignatureType},
    telemetry::init_tracing,
};
use polymarket_client_sdk::clob::ws::WsMessage;
use polymarket_client_sdk::types::B256;
use rust_decimal::Decimal;
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};
use time::{Duration as TimeDuration, OffsetDateTime};
use tracing::{info, warn};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct ExecutionDrainReport {
    scanned: usize,
    submitted: usize,
    failed: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct FillReconciliationReport {
    scanned: usize,
    reconciled: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct OrderStatusPollReport {
    scanned: usize,
    opened: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct PolymarketUserEventReport {
    subscribed_markets: usize,
    consumed: usize,
    order_updates_applied: usize,
    trade_updates_applied: usize,
    skipped_unknown_orders: usize,
    skipped_duplicate_trades: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct PolymarketTradeEventReport {
    applied: usize,
    skipped_unknown_orders: usize,
    skipped_duplicate_trades: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct NewsIngestionRunReport {
    sources_scanned: usize,
    sources_succeeded: usize,
    sources_failed: usize,
    fetched: usize,
    inserted: usize,
    deduped: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct NewsPromotionReport {
    scanned: usize,
    promoted: usize,
    evidences_promoted: usize,
    skipped_unmatched: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PolymarketOrderEventOutcome {
    Applied,
    Ignored,
    UnknownOrder,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing("polyedge_worker");
    let runtime = Runtime::load().await?;
    let state = runtime.app_state();
    let mut args = std::env::args().skip(1);
    let command = args.next();

    info!(
        environment = %state.settings.runtime.environment,
        initial_mode = ?state.settings.runtime.initial_mode,
        "polyedge worker runtime initialized",
    );

    match command.as_deref() {
        Some("ingest-fixtures") => {
            let trace_id = new_trace_id();
            let report = state
                .market_event_service
                .ingest_fixture_bundle(demo_fixture_bundle(), &trace_id)
                .await?;
            info!(
                trace_id = %trace_id,
                markets_upserted = report.markets_upserted,
                events_upserted = report.events_upserted,
                evidences_upserted = report.evidences_upserted,
                signals_upserted = report.signals_upserted,
                "seeded demo market/event fixtures",
            );
            Ok(())
        }
        Some("drain-execution-queue") => {
            let connector_name = args
                .next()
                .unwrap_or_else(|| PAPER_EXECUTOR_NAME.to_string());
            let limit = parse_limit_arg(args.next())?;
            let report = drain_execution_queue(&state, Some(connector_name.clone()), limit).await?;
            info!(
                connector_name = %connector_name,
                scanned = report.scanned,
                submitted = report.submitted,
                failed = report.failed,
                "drained queued execution requests",
            );
            Ok(())
        }
        Some("ingest-news-once") => {
            let trace_id = new_trace_id();
            let report = ingest_news_once(&state, &trace_id).await?;
            info!(
                trace_id = %trace_id,
                sources_scanned = report.sources_scanned,
                sources_succeeded = report.sources_succeeded,
                sources_failed = report.sources_failed,
                fetched = report.fetched,
                inserted = report.inserted,
                deduped = report.deduped,
                "ingested configured news sources once",
            );
            Ok(())
        }
        Some("poll-news") => {
            let max_cycles = parse_limit_arg(args.next())?.map(usize::from);
            let report = poll_news(&state, max_cycles).await?;
            info!(
                sources_scanned = report.sources_scanned,
                sources_succeeded = report.sources_succeeded,
                sources_failed = report.sources_failed,
                fetched = report.fetched,
                inserted = report.inserted,
                deduped = report.deduped,
                "news polling stopped",
            );
            Ok(())
        }
        Some("promote-news-events") => {
            let trace_id = new_trace_id();
            let limit = parse_limit_arg(args.next())?;
            let report = promote_news_events(&state, limit, &trace_id).await?;
            info!(
                trace_id = %trace_id,
                scanned = report.scanned,
                promoted = report.promoted,
                evidences_promoted = report.evidences_promoted,
                skipped_unmatched = report.skipped_unmatched,
                "promoted raw news into event and evidence candidates",
            );
            Ok(())
        }
        Some("reconcile-paper-fills") => {
            let connector_name = args
                .next()
                .unwrap_or_else(|| PAPER_EXECUTOR_NAME.to_string());
            let limit = parse_limit_arg(args.next())?;
            let report = reconcile_paper_fills(&state, Some(connector_name.clone()), limit).await?;
            info!(
                connector_name = %connector_name,
                scanned = report.scanned,
                reconciled = report.reconciled,
                "reconciled submitted paper fills",
            );
            Ok(())
        }
        Some("poll-paper-order-statuses") => {
            let connector_name = args
                .next()
                .unwrap_or_else(|| PAPER_EXECUTOR_NAME.to_string());
            let limit = parse_limit_arg(args.next())?;
            let report =
                poll_paper_order_statuses(&state, Some(connector_name.clone()), limit).await?;
            info!(
                connector_name = %connector_name,
                scanned = report.scanned,
                opened = report.opened,
                "polled submitted paper orders into open status",
            );
            Ok(())
        }
        Some("poll-polymarket-order-statuses") => {
            let connector_name = args
                .next()
                .unwrap_or_else(|| POLYMARKET_CONNECTOR_NAME.to_string());
            let limit = polymarket_order_status_limit(&state, parse_limit_arg(args.next())?);
            let report =
                poll_polymarket_order_statuses(&state, Some(connector_name.clone()), limit).await?;
            info!(
                connector_name = %connector_name,
                scanned = report.scanned,
                opened = report.opened,
                "polled submitted polymarket orders into open status",
            );
            Ok(())
        }
        Some("reconcile-polymarket-fills") => {
            let connector_name = args
                .next()
                .unwrap_or_else(|| POLYMARKET_CONNECTOR_NAME.to_string());
            let limit = polymarket_fill_limit(&state, parse_limit_arg(args.next())?);
            let report =
                reconcile_polymarket_fills(&state, Some(connector_name.clone()), limit).await?;
            info!(
                connector_name = %connector_name,
                scanned = report.scanned,
                reconciled = report.reconciled,
                "reconciled submitted polymarket fills",
            );
            Ok(())
        }
        Some("consume-polymarket-user-events") => {
            let connector_name = args
                .next()
                .unwrap_or_else(|| POLYMARKET_CONNECTOR_NAME.to_string());
            let max_events = parse_limit_arg(args.next())?.map(usize::from);
            let report =
                consume_polymarket_user_events(&state, Some(connector_name.clone()), max_events)
                    .await?;
            info!(
                connector_name = %connector_name,
                subscribed_markets = report.subscribed_markets,
                consumed = report.consumed,
                order_updates_applied = report.order_updates_applied,
                trade_updates_applied = report.trade_updates_applied,
                skipped_unknown_orders = report.skipped_unknown_orders,
                skipped_duplicate_trades = report.skipped_duplicate_trades,
                "consumed polymarket authenticated user websocket events",
            );
            Ok(())
        }
        _ => {
            info!("worker job skeleton is ready for ingestion/pricing/risk/dispatch jobs");
            Ok(())
        }
    }
}

async fn drain_execution_queue(
    state: &AppState,
    connector_name: Option<String>,
    limit: Option<u16>,
) -> Result<ExecutionDrainReport> {
    let connector_name = connector_name.unwrap_or_else(|| PAPER_EXECUTOR_NAME.to_string());
    let candidates = state
        .execution_service
        .list_dispatch_candidates(DispatchExecutionListFilters::new(
            Some(connector_name.clone()),
            limit,
        )?)
        .await?;
    let mut report = ExecutionDrainReport {
        scanned: candidates.len(),
        ..ExecutionDrainReport::default()
    };

    match connector_name.as_str() {
        PAPER_EXECUTOR_NAME => {
            let executor = PaperExecutor::new();
            for candidate in candidates {
                dispatch_candidate(state, &executor, candidate)
                    .await
                    .map(|submitted| {
                        if submitted {
                            report.submitted += 1;
                        } else {
                            report.failed += 1;
                        }
                    })?;
            }
        }
        POLYMARKET_CONNECTOR_NAME => {
            ensure_polymarket_enabled(state)?;
            match state.settings.polymarket.mode {
                PolymarketConnectorMode::Mock => {
                    let connector = MockPolymarketConnector::new();
                    for candidate in candidates {
                        dispatch_polymarket_candidate(state, &connector, candidate)
                            .await
                            .map(|submitted| {
                                if submitted {
                                    report.submitted += 1;
                                } else {
                                    report.failed += 1;
                                }
                            })?;
                    }
                }
                PolymarketConnectorMode::Live => {
                    let connector = build_live_polymarket_connector(state).await?;
                    for candidate in candidates {
                        dispatch_live_polymarket_candidate(state, &connector, candidate)
                            .await
                            .map(|submitted| {
                                if submitted {
                                    report.submitted += 1;
                                } else {
                                    report.failed += 1;
                                }
                            })?;
                    }
                }
                PolymarketConnectorMode::Disabled => unreachable!("disabled handled above"),
            }
        }
        other => {
            return Err(AppError::invalid_input(
                "WORKER_CONNECTOR_UNSUPPORTED",
                format!("worker does not support connector_name={other}"),
            ));
        }
    }

    Ok(report)
}

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
        .list_markets(MarketListFilters::new(None, None, Some(200))?)
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

async fn record_news_failure(
    state: &AppState,
    source: &NewsSourceSettings,
    error: &AppError,
    trace_id: &str,
) -> Result<()> {
    state
        .news_ingestion_service
        .record_source_failure(NewsSourceFailureUpdate {
            source: source.id.clone(),
            source_type: source.source_type.clone(),
            reliability: source.reliability,
            error_message: format!("{}: {}", error.code(), error.message()),
            observed_at: OffsetDateTime::now_utc(),
            trace_id: trace_id.to_string(),
        })
        .await
}

fn news_item_to_ingestion_item(item: ConnectorNewsItem) -> NewsIngestionItem {
    NewsIngestionItem {
        source: item.source,
        source_type: item.source_type,
        external_id: item.external_id,
        title: item.title,
        url: item.url,
        author: item.author,
        published_at: item.published_at,
        content_snippet: item.content_snippet,
        raw_payload: item.raw_payload,
    }
}

async fn reconcile_paper_fills(
    state: &AppState,
    connector_name: Option<String>,
    limit: Option<u16>,
) -> Result<FillReconciliationReport> {
    let connector_name = connector_name.unwrap_or_else(|| PAPER_EXECUTOR_NAME.to_string());
    let candidates = state
        .execution_service
        .list_reconciliation_candidates(ReconcileExecutionListFilters::new(
            Some(connector_name),
            limit,
        )?)
        .await?;
    let executor = PaperExecutor::new();
    let mut report = FillReconciliationReport {
        scanned: candidates.len(),
        ..FillReconciliationReport::default()
    };

    for candidate in candidates {
        reconcile_candidate(state, &executor, candidate).await?;
        report.reconciled += 1;
    }

    Ok(report)
}

async fn reconcile_polymarket_fills(
    state: &AppState,
    connector_name: Option<String>,
    limit: Option<u16>,
) -> Result<FillReconciliationReport> {
    ensure_polymarket_enabled(state)?;
    let connector_name = connector_name.unwrap_or_else(|| POLYMARKET_CONNECTOR_NAME.to_string());
    let candidates = state
        .execution_service
        .list_reconciliation_candidates(ReconcileExecutionListFilters::new(
            Some(connector_name),
            limit,
        )?)
        .await?;
    let mut report = FillReconciliationReport {
        scanned: candidates.len(),
        ..FillReconciliationReport::default()
    };

    match state.settings.polymarket.mode {
        PolymarketConnectorMode::Mock => {
            let connector = MockPolymarketConnector::new();
            for candidate in candidates {
                reconcile_polymarket_candidate(state, &connector, candidate).await?;
                report.reconciled += 1;
            }
        }
        PolymarketConnectorMode::Live => {
            let connector = build_live_polymarket_connector(state).await?;
            for candidate in candidates {
                reconcile_live_polymarket_candidate(state, &connector, candidate).await?;
                report.reconciled += 1;
            }
        }
        PolymarketConnectorMode::Disabled => unreachable!("disabled handled above"),
    }

    Ok(report)
}

async fn poll_paper_order_statuses(
    state: &AppState,
    connector_name: Option<String>,
    limit: Option<u16>,
) -> Result<OrderStatusPollReport> {
    let connector_name = connector_name.unwrap_or_else(|| PAPER_EXECUTOR_NAME.to_string());
    let orders = state
        .execution_service
        .list_orders(OrderListFilters::new(
            None,
            None,
            Some(connector_name.clone()),
            Some(OrderStatus::Submitted),
            limit,
        )?)
        .await?;
    let executor = PaperExecutor::new();
    let mut report = OrderStatusPollReport {
        scanned: orders.len(),
        ..OrderStatusPollReport::default()
    };

    for order in orders {
        if poll_order_status_candidate(state, &executor, order).await? {
            report.opened += 1;
        }
    }

    Ok(report)
}

async fn poll_polymarket_order_statuses(
    state: &AppState,
    connector_name: Option<String>,
    limit: Option<u16>,
) -> Result<OrderStatusPollReport> {
    ensure_polymarket_enabled(state)?;
    let connector_name = connector_name.unwrap_or_else(|| POLYMARKET_CONNECTOR_NAME.to_string());
    let orders = state
        .execution_service
        .list_orders(OrderListFilters::new(
            None,
            None,
            Some(connector_name.clone()),
            Some(OrderStatus::Submitted),
            limit,
        )?)
        .await?;
    let mut report = OrderStatusPollReport {
        scanned: orders.len(),
        ..OrderStatusPollReport::default()
    };

    match state.settings.polymarket.mode {
        PolymarketConnectorMode::Mock => {
            let connector = MockPolymarketConnector::new();
            for order in orders {
                if poll_polymarket_order_status_candidate(state, &connector, order).await? {
                    report.opened += 1;
                }
            }
        }
        PolymarketConnectorMode::Live => {
            let connector = build_live_polymarket_connector(state).await?;
            for order in orders {
                if poll_live_polymarket_order_status_candidate(state, &connector, order).await? {
                    report.opened += 1;
                }
            }
        }
        PolymarketConnectorMode::Disabled => unreachable!("disabled handled above"),
    }

    Ok(report)
}

async fn consume_polymarket_user_events(
    state: &AppState,
    connector_name: Option<String>,
    max_events: Option<usize>,
) -> Result<PolymarketUserEventReport> {
    ensure_polymarket_enabled(state)?;
    if state.settings.polymarket.mode != PolymarketConnectorMode::Live {
        return Err(AppError::invalid_input(
            "POLYMARKET_USER_WS_REQUIRES_LIVE_MODE",
            "polymarket authenticated user websocket consumption requires mode=live",
        ));
    }

    let connector_name = connector_name.unwrap_or_else(|| POLYMARKET_CONNECTOR_NAME.to_string());
    if connector_name != POLYMARKET_CONNECTOR_NAME {
        return Err(AppError::invalid_input(
            "WORKER_CONNECTOR_UNSUPPORTED",
            format!("worker does not support connector_name={connector_name}"),
        ));
    }

    let connector = build_live_polymarket_connector(state).await?;
    let subscribed_markets = collect_polymarket_user_event_markets(state, &connector_name).await?;
    let mut report = PolymarketUserEventReport {
        subscribed_markets: subscribed_markets.len(),
        ..PolymarketUserEventReport::default()
    };

    if subscribed_markets.is_empty() {
        info!(
            "skipping polymarket authenticated user websocket because there are no active internal markets to monitor"
        );
        return Ok(report);
    }

    let client = connector.connect_user_ws()?;
    let stream = client
        .subscribe_user_events(subscribed_markets)
        .map_err(|error| {
            AppError::internal(
                "POLYMARKET_USER_WS_SUBSCRIBE_FAILED",
                format!("failed to subscribe to Polymarket user websocket events: {error}"),
            )
        })?;
    let mut stream = Box::pin(stream);

    while let Some(message) = stream.next().await {
        let message = message.map_err(|error| {
            AppError::internal(
                "POLYMARKET_USER_WS_STREAM_FAILED",
                format!("failed to receive Polymarket user websocket event: {error}"),
            )
        })?;
        report.consumed += 1;

        match message {
            WsMessage::Order(order_message) => {
                match apply_polymarket_ws_order_message(state, &order_message).await? {
                    PolymarketOrderEventOutcome::Applied => report.order_updates_applied += 1,
                    PolymarketOrderEventOutcome::UnknownOrder => {
                        report.skipped_unknown_orders += 1;
                    }
                    PolymarketOrderEventOutcome::Ignored => {}
                }
            }
            WsMessage::Trade(trade_message) => {
                let trade_report = apply_polymarket_ws_trade_message(
                    state,
                    connector.account_id(),
                    &trade_message,
                )
                .await?;
                report.trade_updates_applied += trade_report.applied;
                report.skipped_unknown_orders += trade_report.skipped_unknown_orders;
                report.skipped_duplicate_trades += trade_report.skipped_duplicate_trades;
            }
            _ => {}
        }

        if max_events.is_some_and(|limit| report.consumed >= limit) {
            break;
        }
    }

    Ok(report)
}

async fn collect_polymarket_user_event_markets(
    state: &AppState,
    connector_name: &str,
) -> Result<Vec<B256>> {
    if state.settings.polymarket.ws_max_instruments == 0 {
        return Ok(Vec::new());
    }

    let fetch_limit = u16::try_from(
        state
            .settings
            .polymarket
            .ws_max_instruments
            .saturating_mul(4)
            .min(usize::from(u16::MAX)),
    )
    .expect("bounded polymarket websocket fetch limit");
    let mut seen_condition_ids = HashSet::new();
    let mut markets = Vec::new();

    for status in [
        OrderStatus::Submitted,
        OrderStatus::Open,
        OrderStatus::PartiallyFilled,
    ] {
        let orders = state
            .execution_service
            .list_orders(OrderListFilters::new(
                None,
                None,
                Some(connector_name.to_string()),
                Some(status),
                Some(fetch_limit),
            )?)
            .await?;

        for order in orders {
            if markets.len() >= state.settings.polymarket.ws_max_instruments {
                return Ok(markets);
            }

            let market = state
                .market_event_service
                .get_market(&order.market_id)
                .await?;
            let market_refs = match polymarket_market_refs(&market) {
                Ok(market_refs) => market_refs,
                Err(error) => {
                    warn!(
                        market_id = %market.id,
                        order_id = %order.id,
                        error_code = %error.code(),
                        "skipping polymarket websocket market subscription because market refs are incomplete"
                    );
                    continue;
                }
            };
            let condition_key = market_refs.condition_id.clone();
            if !seen_condition_ids.insert(condition_key.clone()) {
                continue;
            }
            match market_refs.condition_id() {
                Ok(condition_id) => markets.push(condition_id),
                Err(error) => {
                    warn!(
                        market_id = %market.id,
                        order_id = %order.id,
                        condition_id = %condition_key,
                        error_code = %error.code(),
                        "skipping polymarket websocket market subscription because condition id is invalid"
                    );
                }
            }
        }
    }

    Ok(markets)
}

async fn apply_polymarket_ws_order_message(
    state: &AppState,
    order_message: &polymarket_client_sdk::clob::ws::OrderMessage,
) -> Result<PolymarketOrderEventOutcome> {
    let Some(update) = normalize_polymarket_ws_order_message(order_message)? else {
        return Ok(PolymarketOrderEventOutcome::Ignored);
    };

    let request_id = new_trace_id();
    let trace_id = new_trace_id();
    let actor = worker_actor(&request_id);
    match state
        .execution_service
        .sync_external_order_status(SyncExternalOrderStatusCommand {
            connector_name: update.connector_name.clone(),
            external_order_id: update.external_order_id.clone(),
            status: update.status,
            request_id,
            trace_id: trace_id.clone(),
            actor,
        })
        .await
    {
        Ok(order) => {
            info!(
                trace_id = %trace_id,
                order_id = %order.id,
                external_order_id = %update.external_order_id,
                status = %update.status.as_str(),
                event_id = %update.event_id,
                "applied polymarket websocket order update",
            );
            Ok(PolymarketOrderEventOutcome::Applied)
        }
        Err(error) if error.code() == "ORDER_NOT_FOUND" => {
            info!(
                trace_id = %trace_id,
                external_order_id = %update.external_order_id,
                event_id = %update.event_id,
                "skipping polymarket websocket order update because no internal order matches the external order id",
            );
            Ok(PolymarketOrderEventOutcome::UnknownOrder)
        }
        Err(error) => Err(error),
    }
}

async fn apply_polymarket_ws_trade_message(
    state: &AppState,
    account_id: &str,
    trade_message: &polymarket_client_sdk::clob::ws::TradeMessage,
) -> Result<PolymarketTradeEventReport> {
    let updates = normalize_polymarket_ws_trade_message(trade_message, account_id)?;
    let mut report = PolymarketTradeEventReport::default();

    for update in updates {
        let request_id = new_trace_id();
        let trace_id = new_trace_id();
        let actor = worker_actor(&request_id);

        match state
            .execution_service
            .reconcile_external_trade(ReconcileExternalTradeCommand {
                connector_name: update.connector_name.clone(),
                external_order_id: update.external_order_id.clone(),
                account_id: update.account_id.clone(),
                external_trade_id: update.external_trade_id.clone(),
                fill_price: update.fill_price,
                filled_quantity: update.filled_quantity,
                fee: update.fee,
                request_id,
                trace_id: trace_id.clone(),
                actor,
            })
            .await
        {
            Ok(result) => {
                report.applied += 1;
                info!(
                    trace_id = %trace_id,
                    order_id = %result.order.id,
                    external_order_id = %update.external_order_id,
                    external_trade_id = %update.external_trade_id,
                    event_id = %update.event_id,
                    "applied polymarket websocket trade update",
                );
            }
            Err(error) if error.code() == "ORDER_NOT_FOUND" => {
                report.skipped_unknown_orders += 1;
                info!(
                    trace_id = %trace_id,
                    external_order_id = %update.external_order_id,
                    external_trade_id = %update.external_trade_id,
                    event_id = %update.event_id,
                    "skipping polymarket websocket trade update because no internal order matches the external order id",
                );
            }
            Err(error) if error.code() == "STATE_TRADE_ALREADY_RECORDED" => {
                report.skipped_duplicate_trades += 1;
                info!(
                    trace_id = %trace_id,
                    external_order_id = %update.external_order_id,
                    external_trade_id = %update.external_trade_id,
                    event_id = %update.event_id,
                    "skipping polymarket websocket trade update because the external trade id was already reconciled",
                );
            }
            Err(error) => return Err(error),
        }
    }

    Ok(report)
}

async fn build_live_polymarket_connector(state: &AppState) -> Result<LivePolymarketConnector> {
    let settings = &state.settings.polymarket;
    let config = LivePolymarketConfig {
        account_id: polymarket_account_id(state).to_string(),
        clob_host: settings.clob_host.clone(),
        ws_host: settings.ws_host.clone(),
        chain_id: settings.chain_id,
        signature_type: polymarket_signature_scheme(settings.signature_type),
        funder: normalize_optional_config_string(settings.funder.as_deref()),
        private_key: normalize_optional_config_string(settings.private_key.as_deref())
            .unwrap_or_default(),
        api_key: normalize_optional_config_string(settings.api_key.as_deref()),
        api_secret: normalize_optional_config_string(settings.api_secret.as_deref()),
        api_passphrase: normalize_optional_config_string(settings.api_passphrase.as_deref()),
    };

    LivePolymarketConnector::connect(&config).await
}

fn normalize_optional_config_string(value: Option<&str>) -> Option<String> {
    value.and_then(|value| {
        let normalized = value.trim();
        if normalized.is_empty() {
            None
        } else {
            Some(normalized.to_string())
        }
    })
}

async fn dispatch_candidate(
    state: &AppState,
    executor: &PaperExecutor,
    candidate: ExecutionDispatchCandidate,
) -> Result<bool> {
    let request_id = new_trace_id();
    let trace_id = new_trace_id();
    let actor = worker_actor(&request_id);
    let execution_request_id = candidate.execution_request.id.clone();

    match executor.submit(&build_paper_order_request(candidate)) {
        Ok(PaperExecutionOutcome::Submitted(acceptance)) => {
            state
                .execution_service
                .mark_execution_submitted(MarkExecutionSubmittedCommand {
                    execution_request_id: execution_request_id.clone(),
                    account_id: PAPER_ACCOUNT_ID.to_string(),
                    external_order_id: acceptance.external_order_id.clone(),
                    request_id,
                    trace_id: trace_id.clone(),
                    actor,
                })
                .await?;
            info!(
                trace_id = %trace_id,
                execution_request_id = %execution_request_id,
                external_order_id = %acceptance.external_order_id,
                submitted_at = %acceptance.submitted_at,
                "paper executor accepted queued execution request",
            );
            Ok(true)
        }
        Ok(PaperExecutionOutcome::Rejected(rejection)) => {
            state
                .execution_service
                .mark_execution_failed(MarkExecutionFailedCommand {
                    execution_request_id: execution_request_id.clone(),
                    failure_code: rejection.code.clone(),
                    failure_message: rejection.message.clone(),
                    request_id,
                    trace_id: trace_id.clone(),
                    actor,
                })
                .await?;
            info!(
                trace_id = %trace_id,
                execution_request_id = %execution_request_id,
                failure_code = %rejection.code,
                "paper executor rejected queued execution request",
            );
            Ok(false)
        }
        Err(error) => {
            state
                .execution_service
                .mark_execution_failed(MarkExecutionFailedCommand {
                    execution_request_id: execution_request_id.clone(),
                    failure_code: error.code().to_string(),
                    failure_message: error.message().to_string(),
                    request_id,
                    trace_id: trace_id.clone(),
                    actor,
                })
                .await?;
            info!(
                trace_id = %trace_id,
                execution_request_id = %execution_request_id,
                failure_code = %error.code(),
                "paper executor dispatch failed before submission",
            );
            Ok(false)
        }
    }
}

async fn dispatch_polymarket_candidate(
    state: &AppState,
    connector: &MockPolymarketConnector,
    candidate: ExecutionDispatchCandidate,
) -> Result<bool> {
    let request_id = new_trace_id();
    let trace_id = new_trace_id();
    let actor = worker_actor(&request_id);
    let execution_request_id = candidate.execution_request.id.clone();

    match connector.submit(&build_polymarket_order_request(candidate)) {
        Ok(MockPolymarketExecutionOutcome::Accepted(acceptance)) => {
            state
                .execution_service
                .mark_execution_submitted(MarkExecutionSubmittedCommand {
                    execution_request_id: execution_request_id.clone(),
                    account_id: polymarket_account_id(state).to_string(),
                    external_order_id: acceptance.order_id.clone(),
                    request_id,
                    trace_id: trace_id.clone(),
                    actor,
                })
                .await?;
            info!(
                trace_id = %trace_id,
                execution_request_id = %execution_request_id,
                external_order_id = %acceptance.order_id,
                accepted_at = %acceptance.accepted_at,
                "mock polymarket connector accepted queued execution request",
            );
            Ok(true)
        }
        Ok(MockPolymarketExecutionOutcome::Rejected(rejection)) => {
            state
                .execution_service
                .mark_execution_failed(MarkExecutionFailedCommand {
                    execution_request_id: execution_request_id.clone(),
                    failure_code: rejection.code.clone(),
                    failure_message: rejection.message.clone(),
                    request_id,
                    trace_id: trace_id.clone(),
                    actor,
                })
                .await?;
            info!(
                trace_id = %trace_id,
                execution_request_id = %execution_request_id,
                failure_code = %rejection.code,
                "mock polymarket connector rejected queued execution request",
            );
            Ok(false)
        }
        Err(error) => {
            state
                .execution_service
                .mark_execution_failed(MarkExecutionFailedCommand {
                    execution_request_id: execution_request_id.clone(),
                    failure_code: error.code().to_string(),
                    failure_message: error.message().to_string(),
                    request_id,
                    trace_id: trace_id.clone(),
                    actor,
                })
                .await?;
            info!(
                trace_id = %trace_id,
                execution_request_id = %execution_request_id,
                failure_code = %error.code(),
                "mock polymarket connector dispatch failed before submission",
            );
            Ok(false)
        }
    }
}

async fn dispatch_live_polymarket_candidate(
    state: &AppState,
    connector: &LivePolymarketConnector,
    candidate: ExecutionDispatchCandidate,
) -> Result<bool> {
    let request_id = new_trace_id();
    let trace_id = new_trace_id();
    let actor = worker_actor(&request_id);
    let execution_request_id = candidate.execution_request.id.clone();
    let market = state
        .market_event_service
        .get_market(&candidate.order_draft.market_id)
        .await?;

    match connector
        .submit(&build_live_polymarket_order_request(candidate, &market)?)
        .await
    {
        Ok(LivePolymarketExecutionOutcome::Accepted(acceptance)) => {
            state
                .execution_service
                .mark_execution_submitted(MarkExecutionSubmittedCommand {
                    execution_request_id: execution_request_id.clone(),
                    account_id: connector.account_id().to_string(),
                    external_order_id: acceptance.order_id.clone(),
                    request_id,
                    trace_id: trace_id.clone(),
                    actor,
                })
                .await?;
            info!(
                trace_id = %trace_id,
                execution_request_id = %execution_request_id,
                external_order_id = %acceptance.order_id,
                accepted_at = %acceptance.accepted_at,
                "live polymarket connector accepted queued execution request",
            );
            Ok(true)
        }
        Ok(LivePolymarketExecutionOutcome::Rejected(rejection)) => {
            state
                .execution_service
                .mark_execution_failed(MarkExecutionFailedCommand {
                    execution_request_id: execution_request_id.clone(),
                    failure_code: rejection.code.clone(),
                    failure_message: rejection.message.clone(),
                    request_id,
                    trace_id: trace_id.clone(),
                    actor,
                })
                .await?;
            info!(
                trace_id = %trace_id,
                execution_request_id = %execution_request_id,
                failure_code = %rejection.code,
                "live polymarket connector rejected queued execution request",
            );
            Ok(false)
        }
        Err(error) => {
            state
                .execution_service
                .mark_execution_failed(MarkExecutionFailedCommand {
                    execution_request_id: execution_request_id.clone(),
                    failure_code: error.code().to_string(),
                    failure_message: error.message().to_string(),
                    request_id,
                    trace_id: trace_id.clone(),
                    actor,
                })
                .await?;
            info!(
                trace_id = %trace_id,
                execution_request_id = %execution_request_id,
                failure_code = %error.code(),
                "live polymarket connector dispatch failed before submission",
            );
            Ok(false)
        }
    }
}

fn build_paper_order_request(candidate: ExecutionDispatchCandidate) -> PaperOrderRequest {
    PaperOrderRequest {
        execution_request_id: candidate.execution_request.id,
        connector_name: candidate.order_draft.connector_name,
        market_id: candidate.order_draft.market_id,
        side: candidate.order_draft.side,
        limit_price: candidate.order_draft.limit_price,
        quantity: candidate.order_draft.quantity,
        notional: candidate.order_draft.notional,
    }
}

fn build_polymarket_order_request(
    candidate: ExecutionDispatchCandidate,
) -> MockPolymarketOrderRequest {
    MockPolymarketOrderRequest {
        execution_request_id: candidate.execution_request.id,
        connector_name: candidate.order_draft.connector_name,
        market_id: candidate.order_draft.market_id,
        side: candidate.order_draft.side,
        limit_price: candidate.order_draft.limit_price,
        quantity: candidate.order_draft.quantity,
        notional: candidate.order_draft.notional,
    }
}

fn build_live_polymarket_order_request(
    candidate: ExecutionDispatchCandidate,
    market: &MarketView,
) -> Result<LivePolymarketOrderRequest> {
    Ok(LivePolymarketOrderRequest {
        execution_request_id: candidate.execution_request.id,
        connector_name: candidate.order_draft.connector_name,
        market_id: candidate.order_draft.market_id,
        side: candidate.order_draft.side,
        limit_price: candidate.order_draft.limit_price,
        quantity: candidate.order_draft.quantity,
        notional: candidate.order_draft.notional,
        market_refs: polymarket_market_refs(market)?,
    })
}

async fn reconcile_candidate(
    state: &AppState,
    executor: &PaperExecutor,
    candidate: ExecutionReconciliationCandidate,
) -> Result<()> {
    let request_id = new_trace_id();
    let trace_id = new_trace_id();
    let actor = worker_actor(&request_id);
    let external_order_id = candidate
        .execution_request
        .external_order_id
        .clone()
        .or(candidate.order_draft.external_order_id.clone())
        .unwrap_or_default();
    let fill = executor.reconcile_fill(&build_paper_fill_request(candidate))?;

    state
        .execution_service
        .reconcile_external_trade(ReconcileExternalTradeCommand {
            connector_name: PAPER_EXECUTOR_NAME.to_string(),
            external_order_id: external_order_id.clone(),
            account_id: fill.account_id.clone(),
            external_trade_id: fill.external_trade_id.clone(),
            fill_price: fill.fill_price,
            filled_quantity: fill.filled_quantity,
            fee: fill.fee,
            request_id,
            trace_id: trace_id.clone(),
            actor,
        })
        .await?;

    info!(
        trace_id = %trace_id,
        external_order_id = %external_order_id,
        external_trade_id = %fill.external_trade_id,
        executed_at = %fill.executed_at,
        "paper executor reconciled submitted execution fill",
    );

    Ok(())
}

async fn reconcile_polymarket_candidate(
    state: &AppState,
    connector: &MockPolymarketConnector,
    candidate: ExecutionReconciliationCandidate,
) -> Result<()> {
    let request_id = new_trace_id();
    let trace_id = new_trace_id();
    let actor = worker_actor(&request_id);
    let fill = connector.reconcile_fill(&build_polymarket_fill_request(state, candidate))?;
    let update = normalize_polymarket_trade_fill_update(
        &fill.event_id,
        &fill.order_id,
        &fill.account_id,
        &fill.trade_id,
        fill.price,
        fill.size,
        fill.fee,
    )?;

    state
        .execution_service
        .reconcile_external_trade(ReconcileExternalTradeCommand {
            connector_name: update.connector_name.clone(),
            external_order_id: update.external_order_id.clone(),
            account_id: update.account_id.clone(),
            external_trade_id: update.external_trade_id.clone(),
            fill_price: update.fill_price,
            filled_quantity: update.filled_quantity,
            fee: update.fee,
            request_id,
            trace_id: trace_id.clone(),
            actor,
        })
        .await?;

    info!(
        trace_id = %trace_id,
        external_order_id = %update.external_order_id,
        external_trade_id = %update.external_trade_id,
        connector_name = %update.connector_name,
        executed_at = %fill.executed_at,
        "mock polymarket connector reconciled submitted execution fill",
    );

    Ok(())
}

async fn reconcile_live_polymarket_candidate(
    state: &AppState,
    connector: &LivePolymarketConnector,
    candidate: ExecutionReconciliationCandidate,
) -> Result<()> {
    let external_order_id = candidate
        .execution_request
        .external_order_id
        .clone()
        .or(candidate.order_draft.external_order_id.clone())
        .unwrap_or_default();

    let updates = connector
        .collect_trade_updates(&LivePolymarketTradeSyncRequest {
            connector_name: candidate.execution_request.connector_name.clone(),
            account_id: connector.account_id().to_string(),
            external_order_id: external_order_id.clone(),
        })
        .await?;

    for update in updates {
        let request_id = new_trace_id();
        let trace_id = new_trace_id();
        let actor = worker_actor(&request_id);
        state
            .execution_service
            .reconcile_external_trade(ReconcileExternalTradeCommand {
                connector_name: update.connector_name.clone(),
                external_order_id: update.external_order_id.clone(),
                account_id: update.account_id.clone(),
                external_trade_id: update.external_trade_id.clone(),
                fill_price: update.fill_price,
                filled_quantity: update.filled_quantity,
                fee: update.fee,
                request_id,
                trace_id: trace_id.clone(),
                actor,
            })
            .await?;

        info!(
            trace_id = %trace_id,
            external_order_id = %update.external_order_id,
            external_trade_id = %update.external_trade_id,
            connector_name = %update.connector_name,
            "live polymarket connector reconciled external trade update",
        );
    }

    Ok(())
}

async fn poll_order_status_candidate(
    state: &AppState,
    executor: &PaperExecutor,
    order: polyedge_application::OrderView,
) -> Result<bool> {
    let request_id = new_trace_id();
    let trace_id = new_trace_id();
    let actor = worker_actor(&request_id);
    let snapshot = executor.poll_order_status(&build_paper_order_status_request(order.clone()))?;

    if snapshot.status == OrderStatus::Open && order.status == OrderStatus::Submitted {
        state
            .execution_service
            .sync_external_order_status(SyncExternalOrderStatusCommand {
                connector_name: order.connector_name.clone(),
                external_order_id: order.external_order_id.clone(),
                status: snapshot.status,
                request_id,
                trace_id: trace_id.clone(),
                actor,
            })
            .await?;
        info!(
            trace_id = %trace_id,
            order_id = %order.id,
            external_order_id = %snapshot.external_order_id,
            observed_at = %snapshot.observed_at,
            "paper executor observed submitted order as open",
        );
        return Ok(true);
    }

    Ok(false)
}

async fn poll_polymarket_order_status_candidate(
    state: &AppState,
    connector: &MockPolymarketConnector,
    order: polyedge_application::OrderView,
) -> Result<bool> {
    let request_id = new_trace_id();
    let trace_id = new_trace_id();
    let actor = worker_actor(&request_id);
    let payload =
        connector.poll_order_status(&build_polymarket_order_status_request(order.clone()))?;
    let update = normalize_polymarket_order_status_update(
        &payload.event_id,
        &payload.order_id,
        &payload.status,
    )?;

    if update.status == OrderStatus::Open && order.status == OrderStatus::Submitted {
        state
            .execution_service
            .sync_external_order_status(SyncExternalOrderStatusCommand {
                connector_name: update.connector_name.clone(),
                external_order_id: update.external_order_id.clone(),
                status: update.status,
                request_id,
                trace_id: trace_id.clone(),
                actor,
            })
            .await?;
        info!(
            trace_id = %trace_id,
            order_id = %order.id,
            external_order_id = %update.external_order_id,
            connector_name = %update.connector_name,
            observed_at = %payload.observed_at,
            "mock polymarket connector observed submitted order as open",
        );
        return Ok(true);
    }

    Ok(false)
}

async fn poll_live_polymarket_order_status_candidate(
    state: &AppState,
    connector: &LivePolymarketConnector,
    order: polyedge_application::OrderView,
) -> Result<bool> {
    let request_id = new_trace_id();
    let trace_id = new_trace_id();
    let actor = worker_actor(&request_id);
    let update = connector
        .poll_order_status(&LivePolymarketOrderStatusRequest {
            connector_name: order.connector_name.clone(),
            external_order_id: order.external_order_id.clone(),
        })
        .await?;

    let Some(update) = update else {
        return Ok(false);
    };

    state
        .execution_service
        .sync_external_order_status(SyncExternalOrderStatusCommand {
            connector_name: update.connector_name.clone(),
            external_order_id: update.external_order_id.clone(),
            status: update.status,
            request_id,
            trace_id: trace_id.clone(),
            actor,
        })
        .await?;

    info!(
        trace_id = %trace_id,
        order_id = %order.id,
        external_order_id = %update.external_order_id,
        connector_name = %update.connector_name,
        status = %update.status.as_str(),
        "live polymarket connector observed external order status change",
    );

    Ok(update.status == OrderStatus::Open && order.status == OrderStatus::Submitted)
}

fn build_paper_fill_request(candidate: ExecutionReconciliationCandidate) -> PaperFillRequest {
    let already_filled_quantity = candidate.order.as_ref().map_or_else(
        || Quantity::new(0.into()).expect("zero quantity"),
        |order| order.filled_quantity,
    );
    PaperFillRequest {
        execution_request_id: candidate.execution_request.id,
        connector_name: candidate.execution_request.connector_name,
        account_id: PAPER_ACCOUNT_ID.to_string(),
        external_order_id: candidate
            .execution_request
            .external_order_id
            .or(candidate.order_draft.external_order_id)
            .unwrap_or_default(),
        market_id: candidate.order_draft.market_id,
        side: candidate.order_draft.side,
        fill_price: candidate.order_draft.limit_price,
        total_quantity: candidate.order_draft.quantity,
        already_filled_quantity,
    }
}

fn build_polymarket_fill_request(
    state: &AppState,
    candidate: ExecutionReconciliationCandidate,
) -> MockPolymarketFillRequest {
    let already_filled_quantity = candidate.order.as_ref().map_or_else(
        || Quantity::new(0.into()).expect("zero quantity"),
        |order| order.filled_quantity,
    );
    MockPolymarketFillRequest {
        execution_request_id: candidate.execution_request.id,
        connector_name: candidate.execution_request.connector_name,
        account_id: polymarket_account_id(state).to_string(),
        external_order_id: candidate
            .execution_request
            .external_order_id
            .or(candidate.order_draft.external_order_id)
            .unwrap_or_default(),
        market_id: candidate.order_draft.market_id,
        side: candidate.order_draft.side,
        fill_price: candidate.order_draft.limit_price,
        total_quantity: candidate.order_draft.quantity,
        already_filled_quantity,
    }
}

fn build_paper_order_status_request(
    order: polyedge_application::OrderView,
) -> PaperOrderStatusRequest {
    PaperOrderStatusRequest {
        connector_name: order.connector_name,
        external_order_id: order.external_order_id,
        current_status: order.status,
    }
}

fn build_polymarket_order_status_request(
    order: polyedge_application::OrderView,
) -> MockPolymarketOrderStatusRequest {
    MockPolymarketOrderStatusRequest {
        connector_name: order.connector_name,
        external_order_id: order.external_order_id,
        current_status: order.status,
    }
}

fn build_promoted_event_record(
    raw_event: &NewsRawEventView,
    related_market_ids: Vec<String>,
    health: Option<&NewsSourceHealthView>,
) -> Result<FixtureEventRecord> {
    let confidence = health
        .map(|health| health.health_score)
        .unwrap_or_else(|| default_news_confidence(&raw_event.source_type));
    let relevance_score =
        promotion_relevance_score(&raw_event.source_type, related_market_ids.len())?;

    Ok(FixtureEventRecord {
        id: promoted_event_id(raw_event),
        raw_event_id: Some(raw_event.id.clone()),
        source: raw_event.source.clone(),
        summary: raw_event.title.clone(),
        relevance_score,
        confidence,
        status: EventStatus::Active,
        related_market_ids,
        reason_trace: format!(
            "Promoted from raw news {} by source/title lexical market matching.",
            raw_event.id
        ),
        created_at: raw_event.event_time,
        updated_at: OffsetDateTime::now_utc(),
        version: 1,
    })
}

fn build_promoted_evidence_record(
    raw_event: &NewsRawEventView,
    market_id: &str,
    event_id: &str,
    health: Option<&NewsSourceHealthView>,
) -> Result<FixtureEvidenceRecord> {
    let direction = promoted_evidence_direction(raw_event);
    let source_reliability = health
        .map(|health| health.reliability)
        .unwrap_or_else(|| default_news_confidence(&raw_event.source_type));

    Ok(FixtureEvidenceRecord {
        id: promoted_evidence_id(raw_event, market_id),
        market_id: market_id.to_string(),
        event_id: event_id.to_string(),
        direction,
        strength: promotion_evidence_strength(&raw_event.source_type, direction),
        source_reliability,
        novelty: promotion_evidence_novelty(&raw_event.source_type),
        resolution_relevance: promotion_evidence_resolution_relevance(
            &raw_event.source_type,
            direction,
        ),
        status: EvidenceStatus::Active,
        expires_at: raw_event.event_time + promotion_evidence_ttl(&raw_event.source_type),
        created_at: raw_event.event_time,
        updated_at: OffsetDateTime::now_utc(),
        version: 1,
    })
}

fn match_raw_news_markets(raw_event: &NewsRawEventView, markets: &[MarketView]) -> Vec<String> {
    let raw_text = format!("{} {}", raw_event.title, raw_event.source);
    let raw_tokens = tokenize_match_text(&raw_text);
    let raw_lower = raw_text.to_ascii_lowercase();
    let mut matches = Vec::new();

    for market in markets {
        let market_text = format!(
            "{} {} {} {}",
            market.question,
            market.category,
            market.resolution_source,
            market.edge_case_notes.join(" ")
        );
        let market_tokens = tokenize_match_text(&market_text);
        let overlap = raw_tokens
            .iter()
            .filter(|token| market_tokens.contains(*token))
            .count();
        let category_match = raw_lower.contains(&market.category.to_ascii_lowercase());

        if overlap >= 2 || category_match || (raw_event.source_type == "official" && overlap >= 1) {
            matches.push(market.id.clone());
        }
    }

    matches
}

fn tokenize_match_text(value: &str) -> HashSet<String> {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .map(|token| token.trim().to_ascii_lowercase())
        .filter(|token| token.len() >= 3 && !is_news_match_stop_word(token))
        .collect()
}

fn is_news_match_stop_word(token: &str) -> bool {
    matches!(
        token,
        "the"
            | "and"
            | "for"
            | "with"
            | "will"
            | "was"
            | "were"
            | "from"
            | "into"
            | "after"
            | "before"
            | "above"
            | "below"
            | "market"
            | "markets"
            | "news"
            | "feed"
            | "watch"
            | "update"
            | "updated"
            | "reports"
            | "publishes"
    )
}

fn promoted_event_id(raw_event: &NewsRawEventView) -> String {
    let suffix = raw_event.hash.chars().take(24).collect::<String>();
    format!("evt_news_{suffix}")
}

fn promoted_evidence_id(raw_event: &NewsRawEventView, market_id: &str) -> String {
    let suffix = raw_event.hash.chars().take(24).collect::<String>();
    format!("evd_news_{market_id}_{suffix}")
}

fn promoted_evidence_direction(raw_event: &NewsRawEventView) -> EvidenceDirection {
    let lower_title = raw_event.title.to_ascii_lowercase();
    let tokens = tokenize_match_text(&lower_title);

    if tokens.iter().any(|token| {
        matches!(
            token.as_str(),
            "reject"
                | "rejects"
                | "rejected"
                | "denies"
                | "denied"
                | "denial"
                | "delay"
                | "delays"
                | "delayed"
                | "postpone"
                | "postpones"
                | "postponed"
                | "retract"
                | "retracted"
                | "withdraw"
                | "withdraws"
                | "withdrawn"
                | "concern"
                | "concerns"
                | "investigation"
                | "lawsuit"
                | "halts"
                | "blocks"
        )
    }) {
        return EvidenceDirection::SupportsNo;
    }

    if lower_title.contains("approval granted")
        || lower_title.contains("approved")
        || lower_title.contains("greenlight")
        || lower_title.contains("green-light")
        || tokens.iter().any(|token| {
            matches!(
                token.as_str(),
                "approve"
                    | "approves"
                    | "grants"
                    | "granted"
                    | "clears"
                    | "accepts"
                    | "authorizes"
                    | "authorized"
            )
        })
    {
        return EvidenceDirection::SupportsYes;
    }

    EvidenceDirection::Background
}

fn default_news_confidence(source_type: &str) -> Probability {
    match source_type {
        "official" => static_probability(78, 2),
        "calendar" => static_probability(66, 2),
        "market" => static_probability(62, 2),
        "social" => static_probability(48, 2),
        _ => static_probability(60, 2),
    }
}

fn promotion_relevance_score(
    source_type: &str,
    matched_market_count: usize,
) -> Result<Probability> {
    let base = match source_type {
        "official" => Decimal::new(72, 2),
        "calendar" => Decimal::new(62, 2),
        "market" => Decimal::new(58, 2),
        "social" => Decimal::new(45, 2),
        _ => Decimal::new(60, 2),
    };
    let boost = Decimal::new(
        (matched_market_count.saturating_sub(1).min(3) as i64) * 5,
        2,
    );

    Probability::new((base + boost).min(Decimal::new(90, 2)))
}

fn promotion_evidence_strength(source_type: &str, direction: EvidenceDirection) -> Probability {
    let is_directional = direction != EvidenceDirection::Background;
    match (source_type, is_directional) {
        ("official", true) => static_probability(34, 2),
        ("official", false) => static_probability(18, 2),
        ("calendar", true) => static_probability(26, 2),
        ("calendar", false) => static_probability(16, 2),
        ("market", true) => static_probability(22, 2),
        ("market", false) => static_probability(14, 2),
        ("social", true) => static_probability(12, 2),
        ("social", false) => static_probability(8, 2),
        (_, true) => static_probability(20, 2),
        (_, false) => static_probability(12, 2),
    }
}

fn promotion_evidence_novelty(source_type: &str) -> Probability {
    match source_type {
        "official" => static_probability(72, 2),
        "calendar" => static_probability(62, 2),
        "market" => static_probability(55, 2),
        "social" => static_probability(40, 2),
        _ => static_probability(50, 2),
    }
}

fn promotion_evidence_resolution_relevance(
    source_type: &str,
    direction: EvidenceDirection,
) -> Probability {
    let directional_boost = if direction == EvidenceDirection::Background {
        Decimal::ZERO
    } else {
        Decimal::new(8, 2)
    };
    let base = match source_type {
        "official" => Decimal::new(76, 2),
        "calendar" => Decimal::new(68, 2),
        "market" => Decimal::new(60, 2),
        "social" => Decimal::new(42, 2),
        _ => Decimal::new(55, 2),
    };

    static_probability_from_decimal((base + directional_boost).min(Decimal::new(90, 2)))
}

fn promotion_evidence_ttl(source_type: &str) -> TimeDuration {
    match source_type {
        "official" => TimeDuration::days(7),
        "calendar" => TimeDuration::days(3),
        "market" => TimeDuration::days(1),
        "social" => TimeDuration::hours(6),
        _ => TimeDuration::days(2),
    }
}

fn static_probability(value: i64, scale: u32) -> Probability {
    Probability::new(Decimal::new(value, scale))
        .expect("static worker probability default must be valid")
}

fn static_probability_from_decimal(value: Decimal) -> Probability {
    Probability::new(value).expect("static worker probability default must be valid")
}

fn worker_actor(request_id: &str) -> AuthenticatedActor {
    AuthenticatedActor {
        user_id: "system:worker".to_string(),
        session_id: "worker-runtime".to_string(),
        roles: vec![UserRole::Admin],
        request_id: request_id.to_string(),
        ip: None,
        user_agent: Some("polyedge-worker/0.1".to_string()),
    }
}

fn parse_limit_arg(raw: Option<String>) -> Result<Option<u16>> {
    raw.map(|value| {
        value.parse::<u16>().map_err(|error| {
            AppError::invalid_input(
                "WORKER_LIMIT_INVALID",
                format!("worker limit must be a valid u16: {error}"),
            )
        })
    })
    .transpose()
}

fn polymarket_account_id(state: &AppState) -> &str {
    let configured = state.settings.polymarket.account_id.trim();
    if configured.is_empty() {
        POLYMARKET_ACCOUNT_ID
    } else {
        configured
    }
}

fn polymarket_order_status_limit(state: &AppState, cli_limit: Option<u16>) -> Option<u16> {
    cli_limit.or(Some(state.settings.polymarket.order_status_poll_limit))
}

fn polymarket_fill_limit(state: &AppState, cli_limit: Option<u16>) -> Option<u16> {
    cli_limit.or(Some(state.settings.polymarket.fill_poll_limit))
}

fn polymarket_signature_scheme(
    signature_type: PolymarketSignatureType,
) -> PolymarketSignatureScheme {
    match signature_type {
        PolymarketSignatureType::Eoa => PolymarketSignatureScheme::Eoa,
        PolymarketSignatureType::Proxy => PolymarketSignatureScheme::Proxy,
        PolymarketSignatureType::GnosisSafe => PolymarketSignatureScheme::GnosisSafe,
    }
}

fn polymarket_market_refs(market: &MarketView) -> Result<PolymarketMarketRefs> {
    let condition_id = market.polymarket_condition_id.clone().ok_or_else(|| {
        AppError::invalid_input(
            "POLYMARKET_CONDITION_ID_MISSING",
            format!("market {} is missing polymarket_condition_id", market.id),
        )
    })?;
    let yes_asset_id = market.polymarket_yes_asset_id.clone().ok_or_else(|| {
        AppError::invalid_input(
            "POLYMARKET_YES_ASSET_ID_MISSING",
            format!("market {} is missing polymarket_yes_asset_id", market.id),
        )
    })?;
    let no_asset_id = market.polymarket_no_asset_id.clone().ok_or_else(|| {
        AppError::invalid_input(
            "POLYMARKET_NO_ASSET_ID_MISSING",
            format!("market {} is missing polymarket_no_asset_id", market.id),
        )
    })?;

    Ok(PolymarketMarketRefs {
        condition_id,
        yes_asset_id,
        no_asset_id,
    })
}

fn ensure_polymarket_enabled(state: &AppState) -> Result<()> {
    match state.settings.polymarket.mode {
        PolymarketConnectorMode::Mock | PolymarketConnectorMode::Live => Ok(()),
        PolymarketConnectorMode::Disabled => Err(AppError::invalid_input(
            "POLYMARKET_CONNECTOR_DISABLED",
            "polymarket connector is disabled in configuration",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polyedge_application::{
        ApproveSignalCommand, EventListFilters, EvidenceListFilters, ExecutionRequestListFilters,
        OrderDraftListFilters, OrderListFilters, PositionListFilters, SignalListFilters,
        SubmitExecutionCommand, SyncExternalOrderStatusCommand, TradeListFilters,
    };
    use polyedge_domain::{
        ExecutionRequestStatus, OrderDraftStatus, OrderStatus, Quantity, SignalLifecycleState,
        SignalSide, SignedUsdAmount, SystemMode,
    };
    use polyedge_infrastructure::{Settings, settings::PolymarketConnectorMode};

    fn test_state(initial_mode: SystemMode) -> AppState {
        Runtime::test_app_state(Settings::for_test(initial_mode, "test", Vec::new()))
            .expect("test app state")
    }

    fn test_state_with_settings(settings: Settings) -> AppState {
        Runtime::test_app_state(settings).expect("test app state")
    }

    fn test_actor(request_id: &str) -> AuthenticatedActor {
        AuthenticatedActor {
            user_id: "usr_test_operator".to_string(),
            session_id: "sess_test_operator".to_string(),
            roles: vec![UserRole::Admin],
            request_id: request_id.to_string(),
            ip: None,
            user_agent: None,
        }
    }

    #[tokio::test]
    async fn promote_news_events_creates_market_linked_event_and_evidence() {
        let state = test_state(SystemMode::ManualConfirm);
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trace_seed")
            .await
            .expect("seed markets");
        let source_reliability = static_probability(92, 2);

        state
            .news_ingestion_service
            .ingest_source_items(NewsIngestSourceCommand {
                source: "sec_feed".to_string(),
                source_type: "official".to_string(),
                reliability: source_reliability,
                items: vec![NewsIngestionItem {
                    source: "sec_feed".to_string(),
                    source_type: "official".to_string(),
                    external_id: Some("entry-promote-1".to_string()),
                    title: "SEC ETF calendar narrows approval window".to_string(),
                    url: Some("https://example.com/sec/entry-promote-1".to_string()),
                    author: None,
                    published_at: Some(OffsetDateTime::UNIX_EPOCH),
                    content_snippet: Some(
                        "Review window narrowed for pending ETF decisions.".to_string(),
                    ),
                    raw_payload: serde_json::json!({"id": "entry-promote-1"}),
                }],
                trace_id: "trc_news_ingest".to_string(),
            })
            .await
            .expect("ingest raw news");

        let report = promote_news_events(&state, Some(10), "trc_promote_news")
            .await
            .expect("promote news events");

        assert_eq!(
            report,
            NewsPromotionReport {
                scanned: 1,
                promoted: 1,
                evidences_promoted: 1,
                skipped_unmatched: 0,
            }
        );

        let promoted_event = state
            .market_event_service
            .list_events(EventListFilters::new(None, Some(200)).expect("event filters"))
            .await
            .expect("list events")
            .into_iter()
            .find(|event| event.summary == "SEC ETF calendar narrows approval window")
            .expect("promoted event");
        assert_eq!(promoted_event.source, "sec_feed");
        assert_eq!(promoted_event.status, EventStatus::Active);
        assert_eq!(promoted_event.related_market_ids, vec!["mkt_121"]);
        assert_eq!(promoted_event.confidence, source_reliability);

        let promoted_evidences = state
            .market_event_service
            .list_evidences(
                EvidenceListFilters::new(
                    Some("mkt_121".to_string()),
                    Some(promoted_event.id.clone()),
                    None,
                    Some(200),
                )
                .expect("evidence filters"),
            )
            .await
            .expect("list evidences");
        assert_eq!(promoted_evidences.len(), 1);
        let promoted_evidence = &promoted_evidences[0];
        assert_eq!(promoted_evidence.status, EvidenceStatus::Active);
        assert_eq!(promoted_evidence.direction, EvidenceDirection::Background);
        assert_eq!(promoted_evidence.source_reliability, source_reliability);
        assert_eq!(promoted_evidence.market_id, "mkt_121");
        assert_eq!(
            promoted_evidence.event_id.as_str(),
            promoted_event.id.as_str()
        );

        let promoted_signals = state
            .market_event_service
            .list_signals(
                SignalListFilters::new(
                    Some("mkt_121".to_string()),
                    Some(promoted_event.id.clone()),
                    None,
                    Some(200),
                )
                .expect("signal filters"),
            )
            .await
            .expect("list signals");
        assert!(promoted_signals.is_empty());
    }

    async fn seed_execution_request_for_connector(
        state: &AppState,
        quantity_units: i64,
        connector_name: &str,
    ) -> polyedge_application::ExecutionSubmissionReceipt {
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trace_seed")
            .await
            .expect("seed fixtures");

        let approval = state
            .risk_service
            .approve_signal(ApproveSignalCommand {
                signal_id: "sig_2411".to_string(),
                reason: "approve fixture signal for worker dispatch test".to_string(),
                expected_version: Some(9),
                request_id: "req_approve".to_string(),
                trace_id: "trace_approve".to_string(),
                actor: test_actor("req_approve"),
            })
            .await
            .expect("approve signal");

        state
            .execution_service
            .submit_execution_request(SubmitExecutionCommand {
                signal_id: approval.signal.id.clone(),
                expected_signal_version: Some(approval.signal.version),
                limit_price: approval.signal.market_price,
                quantity: Quantity::new(quantity_units.into()).expect("quantity"),
                connector_name: Some(connector_name.to_string()),
                reason: "queue execution request for worker dispatch test".to_string(),
                request_id: "req_submit".to_string(),
                trace_id: "trace_submit".to_string(),
                actor: test_actor("req_submit"),
            })
            .await
            .expect("submit execution request")
    }

    async fn seed_execution_request(
        state: &AppState,
        quantity_units: i64,
    ) -> polyedge_application::ExecutionSubmissionReceipt {
        seed_execution_request_for_connector(state, quantity_units, PAPER_EXECUTOR_NAME).await
    }

    #[tokio::test]
    async fn drain_execution_queue_marks_submitted_for_eligible_orders() {
        let state = test_state(SystemMode::ManualConfirm);
        let receipt = seed_execution_request(&state, 3).await;

        let report = drain_execution_queue(&state, None, Some(10))
            .await
            .expect("drain queue");

        assert_eq!(
            report,
            ExecutionDrainReport {
                scanned: 1,
                submitted: 1,
                failed: 0,
            }
        );

        let execution_request = state
            .execution_service
            .list_execution_requests(
                ExecutionRequestListFilters::new(None, None, None, Some(10))
                    .expect("request filters"),
            )
            .await
            .expect("list execution requests")
            .into_iter()
            .find(|item| item.id == receipt.execution_request.id)
            .expect("submitted execution request");
        assert_eq!(execution_request.status, ExecutionRequestStatus::Submitted);
        assert!(
            execution_request
                .external_order_id
                .as_deref()
                .is_some_and(|value| value.starts_with("paper:mkt_120:yes:"))
        );
        assert!(execution_request.submitted_at.is_some());
        assert_eq!(execution_request.failure_code, None);
        assert_eq!(execution_request.failure_message, None);

        let order_draft = state
            .execution_service
            .list_order_drafts(
                OrderDraftListFilters::new(None, None, None, Some(10)).expect("draft filters"),
            )
            .await
            .expect("list order drafts")
            .into_iter()
            .find(|item| item.id == receipt.order_draft.id)
            .expect("submitted order draft");
        assert_eq!(order_draft.status, OrderDraftStatus::Submitted);
        assert_eq!(
            order_draft.external_order_id,
            execution_request.external_order_id
        );
        assert!(order_draft.submitted_at.is_some());

        let orders = state
            .execution_service
            .list_orders(
                OrderListFilters::new(Some("sig_2411".to_string()), None, None, None, Some(10))
                    .expect("order filters"),
            )
            .await
            .expect("list orders");
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].status, OrderStatus::Submitted);
        assert_eq!(orders[0].account_id, PAPER_ACCOUNT_ID);
        assert_eq!(
            orders[0].filled_quantity,
            Quantity::new(0.into()).expect("quantity")
        );
        assert_eq!(orders[0].avg_fill_price.api_string(), "0");
    }

    #[tokio::test]
    async fn poll_paper_order_statuses_promotes_submitted_orders_to_open() {
        let state = test_state(SystemMode::ManualConfirm);
        seed_execution_request(&state, 3).await;
        drain_execution_queue(&state, None, Some(10))
            .await
            .expect("drain queue");

        let report = poll_paper_order_statuses(&state, None, Some(10))
            .await
            .expect("poll order statuses");

        assert_eq!(
            report,
            OrderStatusPollReport {
                scanned: 1,
                opened: 1,
            }
        );

        let orders = state
            .execution_service
            .list_orders(
                OrderListFilters::new(Some("sig_2411".to_string()), None, None, None, Some(10))
                    .expect("order filters"),
            )
            .await
            .expect("list orders");
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].status, OrderStatus::Open);
    }

    #[tokio::test]
    async fn drain_execution_queue_supports_polymarket_mock_connector() {
        let state = test_state(SystemMode::ManualConfirm);
        let receipt =
            seed_execution_request_for_connector(&state, 3, POLYMARKET_CONNECTOR_NAME).await;

        let report = drain_execution_queue(
            &state,
            Some(POLYMARKET_CONNECTOR_NAME.to_string()),
            Some(10),
        )
        .await
        .expect("drain queue");

        assert_eq!(
            report,
            ExecutionDrainReport {
                scanned: 1,
                submitted: 1,
                failed: 0,
            }
        );

        let execution_request = state
            .execution_service
            .list_execution_requests(
                ExecutionRequestListFilters::new(None, None, None, Some(10))
                    .expect("request filters"),
            )
            .await
            .expect("list execution requests")
            .into_iter()
            .find(|item| item.id == receipt.execution_request.id)
            .expect("submitted execution request");
        assert_eq!(execution_request.status, ExecutionRequestStatus::Submitted);
        assert!(
            execution_request
                .external_order_id
                .as_deref()
                .is_some_and(|value| value.starts_with("pm:mkt_120:yes:"))
        );

        let orders = state
            .execution_service
            .list_orders(
                OrderListFilters::new(Some("sig_2411".to_string()), None, None, None, Some(10))
                    .expect("order filters"),
            )
            .await
            .expect("list orders");
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].status, OrderStatus::Submitted);
        assert_eq!(orders[0].account_id, POLYMARKET_ACCOUNT_ID);
        assert_eq!(orders[0].connector_name, POLYMARKET_CONNECTOR_NAME);
    }

    #[tokio::test]
    async fn poll_polymarket_order_statuses_promotes_submitted_orders_to_open() {
        let state = test_state(SystemMode::ManualConfirm);
        seed_execution_request_for_connector(&state, 3, POLYMARKET_CONNECTOR_NAME).await;
        drain_execution_queue(
            &state,
            Some(POLYMARKET_CONNECTOR_NAME.to_string()),
            Some(10),
        )
        .await
        .expect("drain queue");

        let report = poll_polymarket_order_statuses(
            &state,
            Some(POLYMARKET_CONNECTOR_NAME.to_string()),
            Some(10),
        )
        .await
        .expect("poll order statuses");

        assert_eq!(
            report,
            OrderStatusPollReport {
                scanned: 1,
                opened: 1,
            }
        );

        let orders = state
            .execution_service
            .list_orders(
                OrderListFilters::new(Some("sig_2411".to_string()), None, None, None, Some(10))
                    .expect("order filters"),
            )
            .await
            .expect("list orders");
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].status, OrderStatus::Open);
        assert_eq!(orders[0].connector_name, POLYMARKET_CONNECTOR_NAME);
    }

    #[tokio::test]
    async fn polymarket_worker_uses_configured_account_id() {
        let mut settings = Settings::for_test(SystemMode::ManualConfirm, "test", Vec::new());
        settings.polymarket.account_id = "acct_poly_cfg".to_string();
        let state = test_state_with_settings(settings);
        seed_execution_request_for_connector(&state, 3, POLYMARKET_CONNECTOR_NAME).await;

        drain_execution_queue(
            &state,
            Some(POLYMARKET_CONNECTOR_NAME.to_string()),
            Some(10),
        )
        .await
        .expect("drain queue");

        let orders = state
            .execution_service
            .list_orders(
                OrderListFilters::new(Some("sig_2411".to_string()), None, None, None, Some(10))
                    .expect("order filters"),
            )
            .await
            .expect("list orders");
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].account_id, "acct_poly_cfg");
    }

    #[tokio::test]
    async fn polymarket_worker_rejects_disabled_mode() {
        let mut settings = Settings::for_test(SystemMode::ManualConfirm, "test", Vec::new());
        settings.polymarket.mode = PolymarketConnectorMode::Disabled;
        let state = test_state_with_settings(settings);

        let error = poll_polymarket_order_statuses(
            &state,
            Some(POLYMARKET_CONNECTOR_NAME.to_string()),
            Some(10),
        )
        .await
        .expect_err("disabled polymarket connector should fail");

        assert_eq!(error.code(), "POLYMARKET_CONNECTOR_DISABLED");
    }

    #[tokio::test]
    async fn polymarket_live_worker_requires_private_key() {
        let mut settings = Settings::for_test(SystemMode::ManualConfirm, "test", Vec::new());
        settings.polymarket.mode = PolymarketConnectorMode::Live;
        let state = test_state_with_settings(settings);

        let error = poll_polymarket_order_statuses(
            &state,
            Some(POLYMARKET_CONNECTOR_NAME.to_string()),
            Some(10),
        )
        .await
        .expect_err("live polymarket connector should require a private key");

        assert_eq!(error.code(), "POLYMARKET_PRIVATE_KEY_REQUIRED");
    }

    #[tokio::test]
    async fn sync_external_order_status_cancels_open_order_and_request() {
        let state = test_state(SystemMode::ManualConfirm);
        let receipt = seed_execution_request(&state, 3).await;
        drain_execution_queue(&state, None, Some(10))
            .await
            .expect("drain queue");
        poll_paper_order_statuses(&state, None, Some(10))
            .await
            .expect("poll order statuses");

        let order = state
            .execution_service
            .list_orders(
                OrderListFilters::new(Some("sig_2411".to_string()), None, None, None, Some(10))
                    .expect("order filters"),
            )
            .await
            .expect("list orders")
            .into_iter()
            .next()
            .expect("open order");

        let canceled_order = state
            .execution_service
            .sync_external_order_status(SyncExternalOrderStatusCommand {
                connector_name: order.connector_name.clone(),
                external_order_id: order.external_order_id.clone(),
                status: OrderStatus::Canceled,
                request_id: "req_cancel_sync".to_string(),
                trace_id: "trace_cancel_sync".to_string(),
                actor: test_actor("req_cancel_sync"),
            })
            .await
            .expect("cancel order");
        assert_eq!(canceled_order.status, OrderStatus::Canceled);

        let execution_request = state
            .execution_service
            .list_execution_requests(
                ExecutionRequestListFilters::new(None, None, None, Some(10))
                    .expect("request filters"),
            )
            .await
            .expect("list execution requests")
            .into_iter()
            .find(|item| item.id == receipt.execution_request.id)
            .expect("canceled execution request");
        assert_eq!(execution_request.status, ExecutionRequestStatus::Canceled);
    }

    #[tokio::test]
    async fn drain_execution_queue_marks_failed_for_sub_min_notional_orders() {
        let state = test_state(SystemMode::ManualConfirm);
        let receipt = seed_execution_request(&state, 1).await;

        let report = drain_execution_queue(&state, None, Some(10))
            .await
            .expect("drain queue");

        assert_eq!(
            report,
            ExecutionDrainReport {
                scanned: 1,
                submitted: 0,
                failed: 1,
            }
        );

        let execution_request = state
            .execution_service
            .list_execution_requests(
                ExecutionRequestListFilters::new(None, None, None, Some(10))
                    .expect("request filters"),
            )
            .await
            .expect("list execution requests")
            .into_iter()
            .find(|item| item.id == receipt.execution_request.id)
            .expect("failed execution request");
        assert_eq!(execution_request.status, ExecutionRequestStatus::Failed);
        assert_eq!(
            execution_request.failure_code.as_deref(),
            Some("PAPER_MIN_NOTIONAL_NOT_MET")
        );
        assert!(
            execution_request
                .failure_message
                .as_deref()
                .is_some_and(|value| value.contains("notional >= 1.00 USD"))
        );
        assert_eq!(execution_request.external_order_id, None);
        assert_eq!(execution_request.submitted_at, None);

        let order_draft = state
            .execution_service
            .list_order_drafts(
                OrderDraftListFilters::new(None, None, None, Some(10)).expect("draft filters"),
            )
            .await
            .expect("list order drafts")
            .into_iter()
            .find(|item| item.id == receipt.order_draft.id)
            .expect("rejected order draft");
        assert_eq!(order_draft.status, OrderDraftStatus::Rejected);
        assert_eq!(
            order_draft.failure_code.as_deref(),
            Some("PAPER_MIN_NOTIONAL_NOT_MET")
        );
        assert_eq!(order_draft.external_order_id, None);
        assert_eq!(order_draft.submitted_at, None);
    }

    #[tokio::test]
    async fn reconcile_paper_fills_creates_order_trade_position_and_executes_signal() {
        let state = test_state(SystemMode::ManualConfirm);
        let receipt = seed_execution_request(&state, 3).await;
        drain_execution_queue(&state, None, Some(10))
            .await
            .expect("drain queue");

        let first_report = reconcile_paper_fills(&state, None, Some(10))
            .await
            .expect("reconcile fills");

        assert_eq!(
            first_report,
            FillReconciliationReport {
                scanned: 1,
                reconciled: 1,
            }
        );

        let orders = state
            .execution_service
            .list_orders(
                OrderListFilters::new(Some("sig_2411".to_string()), None, None, None, Some(10))
                    .expect("order filters"),
            )
            .await
            .expect("list orders");
        assert_eq!(orders.len(), 1);
        let order = &orders[0];
        assert_eq!(order.execution_request_id, receipt.execution_request.id);
        assert_eq!(order.order_draft_id, receipt.order_draft.id);
        assert_eq!(order.account_id, PAPER_ACCOUNT_ID);
        assert_eq!(order.status, OrderStatus::PartiallyFilled);
        assert_eq!(order.side, SignalSide::Yes);
        assert_eq!(order.quantity, Quantity::new(3.into()).expect("quantity"));
        assert_eq!(
            order.filled_quantity,
            Quantity::new(1.into()).expect("quantity")
        );
        assert!(order.external_order_id.starts_with("paper:mkt_120:yes:"));

        let trades = state
            .execution_service
            .list_trades(
                TradeListFilters::new(None, Some("sig_2411".to_string()), None, None, Some(10))
                    .expect("trade filters"),
            )
            .await
            .expect("list trades");
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].order_id, order.id);
        assert_eq!(trades[0].connector_name, PAPER_EXECUTOR_NAME);
        assert!(
            trades[0]
                .external_trade_id
                .starts_with("paper-trade:mkt_120:yes:")
        );
        assert_eq!(
            trades[0].quantity,
            Quantity::new(1.into()).expect("quantity")
        );
        assert!(trades[0].external_trade_id.ends_with(":1"));

        let positions = state
            .execution_service
            .list_positions(
                PositionListFilters::new(
                    Some("mkt_120".to_string()),
                    Some(PAPER_EXECUTOR_NAME.to_string()),
                    Some(SignalSide::Yes),
                    Some(10),
                )
                .expect("position filters"),
            )
            .await
            .expect("list positions");
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].account_id, PAPER_ACCOUNT_ID);
        assert_eq!(
            positions[0].net_quantity,
            Quantity::new(1.into()).expect("quantity")
        );
        assert_eq!(positions[0].mark_price, order.avg_fill_price);
        assert_eq!(positions[0].avg_cost, order.avg_fill_price);

        let signals = state
            .market_event_service
            .list_signals(
                SignalListFilters::new(Some("mkt_120".to_string()), None, None, Some(10))
                    .expect("signal filters"),
            )
            .await
            .expect("list signals");
        let signal = signals
            .into_iter()
            .find(|item| item.id == "sig_2411")
            .expect("executed signal");
        assert_eq!(signal.lifecycle_state, SignalLifecycleState::Executed);

        let risk_state = state
            .risk_service
            .read_state()
            .await
            .expect("read risk state");
        assert_eq!(
            risk_state.daily_pnl,
            SignedUsdAmount::new(0.into()).expect("daily pnl")
        );
        assert_eq!(risk_state.gross_exposure.api_string(), "0.0052");
        assert_eq!(risk_state.net_exposure.api_string(), "0.0052");

        let second_report = reconcile_paper_fills(&state, None, Some(10))
            .await
            .expect("reconcile fills again");
        assert_eq!(
            second_report,
            FillReconciliationReport {
                scanned: 1,
                reconciled: 1,
            }
        );

        let orders = state
            .execution_service
            .list_orders(
                OrderListFilters::new(Some("sig_2411".to_string()), None, None, None, Some(10))
                    .expect("order filters"),
            )
            .await
            .expect("list orders");
        assert_eq!(orders.len(), 1);
        let order = &orders[0];
        assert_eq!(order.status, OrderStatus::Filled);
        assert_eq!(
            order.filled_quantity,
            Quantity::new(3.into()).expect("quantity")
        );

        let trades = state
            .execution_service
            .list_trades(
                TradeListFilters::new(None, Some("sig_2411".to_string()), None, None, Some(10))
                    .expect("trade filters"),
            )
            .await
            .expect("list trades");
        assert_eq!(trades.len(), 2);
        let mut trade_quantities: Vec<_> = trades
            .iter()
            .map(|trade| {
                assert_eq!(trade.order_id, order.id);
                trade.quantity.value()
            })
            .collect();
        trade_quantities.sort();
        assert_eq!(trade_quantities, vec![1.into(), 2.into()]);
        assert!(
            trades
                .iter()
                .any(|trade| trade.external_trade_id.ends_with(":3"))
        );

        let positions = state
            .execution_service
            .list_positions(
                PositionListFilters::new(
                    Some("mkt_120".to_string()),
                    Some(PAPER_EXECUTOR_NAME.to_string()),
                    Some(SignalSide::Yes),
                    Some(10),
                )
                .expect("position filters"),
            )
            .await
            .expect("list positions");
        assert_eq!(positions.len(), 1);
        assert_eq!(
            positions[0].net_quantity,
            Quantity::new(3.into()).expect("quantity")
        );

        let risk_state = state
            .risk_service
            .read_state()
            .await
            .expect("read risk state");
        assert_eq!(risk_state.gross_exposure.api_string(), "0.0156");
        assert_eq!(risk_state.net_exposure.api_string(), "0.0156");

        let third_report = reconcile_paper_fills(&state, None, Some(10))
            .await
            .expect("reconcile fills final pass");
        assert_eq!(
            third_report,
            FillReconciliationReport {
                scanned: 0,
                reconciled: 0,
            }
        );
    }

    #[tokio::test]
    async fn reconcile_polymarket_fills_creates_trade_and_position() {
        let state = test_state(SystemMode::ManualConfirm);
        let receipt =
            seed_execution_request_for_connector(&state, 3, POLYMARKET_CONNECTOR_NAME).await;
        drain_execution_queue(
            &state,
            Some(POLYMARKET_CONNECTOR_NAME.to_string()),
            Some(10),
        )
        .await
        .expect("drain queue");
        poll_polymarket_order_statuses(
            &state,
            Some(POLYMARKET_CONNECTOR_NAME.to_string()),
            Some(10),
        )
        .await
        .expect("poll polymarket orders");

        let first_report = reconcile_polymarket_fills(
            &state,
            Some(POLYMARKET_CONNECTOR_NAME.to_string()),
            Some(10),
        )
        .await
        .expect("reconcile polymarket fills");

        assert_eq!(
            first_report,
            FillReconciliationReport {
                scanned: 1,
                reconciled: 1,
            }
        );

        let orders = state
            .execution_service
            .list_orders(
                OrderListFilters::new(Some("sig_2411".to_string()), None, None, None, Some(10))
                    .expect("order filters"),
            )
            .await
            .expect("list orders");
        assert_eq!(orders.len(), 1);
        let order = &orders[0];
        assert_eq!(order.execution_request_id, receipt.execution_request.id);
        assert_eq!(order.order_draft_id, receipt.order_draft.id);
        assert_eq!(order.account_id, POLYMARKET_ACCOUNT_ID);
        assert_eq!(order.connector_name, POLYMARKET_CONNECTOR_NAME);
        assert_eq!(order.status, OrderStatus::PartiallyFilled);
        assert_eq!(
            order.filled_quantity,
            Quantity::new(1.into()).expect("quantity")
        );
        assert!(order.external_order_id.starts_with("pm:mkt_120:yes:"));

        let trades = state
            .execution_service
            .list_trades(
                TradeListFilters::new(None, Some("sig_2411".to_string()), None, None, Some(10))
                    .expect("trade filters"),
            )
            .await
            .expect("list trades");
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].order_id, order.id);
        assert_eq!(trades[0].connector_name, POLYMARKET_CONNECTOR_NAME);
        assert!(
            trades[0]
                .external_trade_id
                .starts_with("pm-trade:mkt_120:yes:")
        );

        let positions = state
            .execution_service
            .list_positions(
                PositionListFilters::new(
                    Some("mkt_120".to_string()),
                    Some(POLYMARKET_CONNECTOR_NAME.to_string()),
                    Some(SignalSide::Yes),
                    Some(10),
                )
                .expect("position filters"),
            )
            .await
            .expect("list positions");
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].account_id, POLYMARKET_ACCOUNT_ID);
        assert_eq!(
            positions[0].net_quantity,
            Quantity::new(1.into()).expect("quantity")
        );

        let second_report = reconcile_polymarket_fills(
            &state,
            Some(POLYMARKET_CONNECTOR_NAME.to_string()),
            Some(10),
        )
        .await
        .expect("reconcile polymarket fills again");
        assert_eq!(
            second_report,
            FillReconciliationReport {
                scanned: 1,
                reconciled: 1,
            }
        );

        let orders = state
            .execution_service
            .list_orders(
                OrderListFilters::new(Some("sig_2411".to_string()), None, None, None, Some(10))
                    .expect("order filters"),
            )
            .await
            .expect("list orders");
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].status, OrderStatus::Filled);
        assert_eq!(
            orders[0].filled_quantity,
            Quantity::new(3.into()).expect("quantity")
        );

        let trades = state
            .execution_service
            .list_trades(
                TradeListFilters::new(None, Some("sig_2411".to_string()), None, None, Some(10))
                    .expect("trade filters"),
            )
            .await
            .expect("list trades");
        assert_eq!(trades.len(), 2);
        assert!(
            trades
                .iter()
                .all(|trade| trade.connector_name == POLYMARKET_CONNECTOR_NAME)
        );

        let risk_state = state
            .risk_service
            .read_state()
            .await
            .expect("read risk state");
        assert_eq!(risk_state.gross_exposure.api_string(), "0.0156");
        assert_eq!(risk_state.net_exposure.api_string(), "0.0156");
    }
}
