use futures::StreamExt as _;
use polyedge_application::{
    ArbitrageAnalysisRunView, ArbitrageOpportunityListFilters, ArbitrageOpportunityView,
    ArbitrageScanView, ArbitrageValidationConfig, AuthenticatedActor, DispatchExecutionListFilters,
    ExecutionDispatchCandidate, ExecutionReconciliationCandidate, FixtureBundle,
    FixtureEventRecord, FixtureEvidenceRecord, MarkExecutionFailedCommand,
    MarkExecutionSubmittedCommand, MarketBookSnapshotView, MarketListFilters, MarketView,
    NewsIngestSourceCommand, NewsIngestionItem, NewsRawEventListFilters, NewsRawEventView,
    NewsSourceFailureUpdate, NewsSourceHealthListFilters, NewsSourceHealthView, OrderListFilters,
    ReconcileExecutionListFilters, ReconcileExternalTradeCommand, RewardBookLevel,
    RewardBotRunReport, RewardMarket, RewardOrderBook, RewardToken, SyncExternalOrderStatusCommand,
    build_arbitrage_analysis, market_book_snapshot_id, select_reward_book_token_ids,
};
use polyedge_connectors::{
    ConnectorNewsItem, LivePolymarketConfig, LivePolymarketConnector,
    LivePolymarketExecutionOutcome, LivePolymarketOrderRequest, LivePolymarketOrderStatusRequest,
    LivePolymarketTradeSyncRequest, NewsSource, PAPER_ACCOUNT_ID, PAPER_EXECUTOR_NAME,
    POLYMARKET_CONNECTOR_NAME, PaperExecutionOutcome, PaperExecutor, PaperFillRequest,
    PaperOrderRequest, PaperOrderStatusRequest, PolymarketBinaryBookSnapshot,
    PolymarketBookConnector, PolymarketBookLevel, PolymarketGammaConnector, PolymarketGammaMarket,
    PolymarketMarketRefs, PolymarketRewardMarket,
    PolymarketRewardOrderBook, PolymarketRewardsConnector, PolymarketSignatureScheme,
    RssNewsConnector, RssNewsSourceConfig, normalize_polymarket_ws_order_message,
    normalize_polymarket_ws_trade_message,
};
use polyedge_domain::{
    AppError, EventStatus, EvidenceDirection, EvidenceStatus, MarketStatus, OrderStatus,
    Probability, Quantity, Result, UserRole,
};
use polyedge_infrastructure::{
    AppState, Runtime, new_trace_id,
    settings::{NewsSourceSettings, PolymarketSignatureType},
    telemetry::init_tracing,
};
use polymarket_client_sdk::clob::ws::WsMessage;
use polymarket_client_sdk::types::B256;
use rust_decimal::Decimal;
use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
    future::Future,
    time::Duration,
};
use time::{Duration as TimeDuration, OffsetDateTime};
use tokio::{sync::watch, task::JoinHandle};
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct ArbitrageScanRunReport {
    markets_scanned: usize,
    snapshots_recorded: usize,
    opportunities_recorded: usize,
    validations_recorded: usize,
    validation_books_refetched: usize,
    validation_book_failures: usize,
    opportunities_expired: usize,
    events_pruned: u64,
    failed_books: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PolymarketOrderEventOutcome {
    Applied,
    Ignored,
    UnknownOrder,
}

const WORKER_SERVICE_COMMAND: &str = "run";

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
        None | Some(WORKER_SERVICE_COMMAND) => run_worker_service(state).await,
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
        Some("scan-arbitrage-once") => {
            let trace_id = new_trace_id();
            let report = scan_arbitrage_once(&state, &trace_id).await?;
            info!(
                trace_id = %trace_id,
                markets_scanned = report.markets_scanned,
                snapshots_recorded = report.snapshots_recorded,
                opportunities_recorded = report.opportunities_recorded,
                validations_recorded = report.validations_recorded,
                validation_books_refetched = report.validation_books_refetched,
                validation_book_failures = report.validation_book_failures,
                opportunities_expired = report.opportunities_expired,
                events_pruned = report.events_pruned,
                failed_books = report.failed_books,
                "scanned arbitrage radar once",
            );
            Ok(())
        }
        Some("poll-arbitrage-radar") => {
            let max_cycles = parse_limit_arg(args.next())?.map(usize::from);
            let report = poll_arbitrage_radar(&state, max_cycles).await?;
            info!(
                markets_scanned = report.markets_scanned,
                snapshots_recorded = report.snapshots_recorded,
                opportunities_recorded = report.opportunities_recorded,
                validations_recorded = report.validations_recorded,
                validation_books_refetched = report.validation_books_refetched,
                validation_book_failures = report.validation_book_failures,
                opportunities_expired = report.opportunities_expired,
                events_pruned = report.events_pruned,
                failed_books = report.failed_books,
                "arbitrage radar polling stopped",
            );
            Ok(())
        }
        Some("analyze-arbitrage-opportunities") => {
            let trace_id = new_trace_id();
            let lookback_hours = parse_limit_arg(args.next())?
                .unwrap_or(state.settings.arbitrage.analysis_lookback_hours);
            let analysis =
                analyze_arbitrage_opportunities(&state, lookback_hours, &trace_id).await?;
            info!(
                trace_id = %trace_id,
                analysis_id = %analysis.id,
                lookback_hours = analysis.lookback_hours,
                opportunity_count = analysis.opportunity_count,
                market_count = analysis.market_count,
                summary = %analysis.summary_payload,
                "analyzed arbitrage opportunity history",
            );
            Ok(())
        }
        Some("scan-rewards-once") => {
            let trace_id = new_trace_id();
            let report = run_reward_bot_once(&state, &trace_id).await?;
            info!(
                trace_id = %trace_id,
                markets_scanned = report.markets_scanned,
                books_fetched = report.books_fetched,
                plans_built = report.plans_built,
                eligible_plans = report.eligible_plans,
                simulated_orders = report.simulated_orders,
                cancelled_orders = report.cancelled_orders,
                "ran rewards bot simulation once",
            );
            Ok(())
        }
        Some("poll-reward-bot") => {
            let max_cycles = parse_limit_arg(args.next())?.map(usize::from);
            let report = poll_reward_bot(&state, max_cycles).await?;
            info!(
                markets_scanned = report.markets_scanned,
                books_fetched = report.books_fetched,
                plans_built = report.plans_built,
                eligible_plans = report.eligible_plans,
                simulated_orders = report.simulated_orders,
                cancelled_orders = report.cancelled_orders,
                "reward bot polling stopped",
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
        Some(other) => Err(AppError::invalid_input(
            "WORKER_COMMAND_UNSUPPORTED",
            format!("unsupported polyedge-worker command: {other}"),
        )),
    }
}

include!("worker/service.rs");
include!("worker/execution_queue.rs");
include!("worker/news.rs");
include!("worker/arbitrage.rs");
include!("worker/rewards.rs");
include!("worker/arbitrage_books.rs");
include!("worker/news_helpers.rs");
include!("worker/execution_reconcile.rs");
include!("worker/polymarket_events.rs");
include!("worker/polymarket_config.rs");
include!("worker/execution_dispatch.rs");
include!("worker/news_promotion.rs");
include!("worker/shared.rs");

#[cfg(test)]
mod tests;
