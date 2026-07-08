use futures::StreamExt as _;
use polyedge_application::{
    AuthenticatedActor, BookSnapshot, BookSource, CachedOrderBook, DatabaseMaintenanceReport,
    DispatchExecutionListFilters, ExecutionDispatchCandidate, ExecutionReconciliationCandidate,
    FixtureBundle, FixtureEventRecord, FixtureEvidenceRecord, ManagedRewardOrder,
    ManagedRewardOrderStatus, MarkExecutionFailedCommand, MarkExecutionSubmittedCommand,
    MarketListFilters, MarketView, NewsIngestSourceCommand, NewsIngestionItem,
    NewsRawEventListFilters, NewsRawEventView, NewsSourceFailureUpdate,
    NewsSourceHealthListFilters, NewsSourceHealthView, OrderListFilters, PageQuery,
    PostFillStrategy, REWARD_AI_CANDLE_SOURCE_INTERVAL_SEC,
    REWARD_AI_CANDLE_SOURCE_LIMIT_PER_TOKEN, ReconcileExecutionListFilters,
    ReconcileExternalTradeCommand, RewardAccountState, RewardAiAdvisoryRequest,
    RewardAiSuitability, RewardBookLevel, RewardBotConfig, RewardBotRunReport,
    RewardCandidateMarket, RewardControlAction, RewardControlCommand, RewardEventWindowStatus,
    RewardExitStrategySource, RewardFairValueEstimate, RewardFill, RewardFillRole,
    RewardInfoRiskAssessmentRequest, RewardLiveCycle, RewardLiveQuoteMaterialization,
    RewardLlmCallRecord, RewardMarket, RewardMarketAdvisory, RewardMarketInfoRisk,
    RewardMergeIntent, RewardMergeIntentStatus, RewardOrderBook, RewardOrderSide,
    RewardPlanQuoteMode, RewardPosition, RewardProviderPreLlmCandidateKind, RewardQuoteLeg,
    RewardQuotePlan, RewardQuoteReadiness, RewardRiskEvent, RewardRiskSeverity,
    RewardStrategyBucket, RewardStrategyProfile, RewardTickOutcome, RewardToken,
    SyncExternalOrderStatusCommand, apply_first_quote_entry_gates, apply_reward_ai_advisories,
    apply_reward_fair_value_to_quote_plan, apply_reward_fair_values_to_quote_plans,
    apply_reward_info_risks, apply_reward_opportunity_metrics_to_quote_plans,
    build_reward_ai_advisory_request, build_reward_info_risk_assessment_request,
    materialize_reward_quote_plan_for_live_orderbook, new_risk_event,
    refresh_reward_opportunity_metrics_for_quote_plans,
    reward_ai_strategy_hint_max_condition_notional_usd, reward_condition_has_active_exposure,
    reward_market_books_available, reward_order_counts_as_external_open,
    reward_provider_cache_refresh_due, reward_provider_plan_passes_pre_llm_gate,
    reward_provider_pre_llm_candidate_kind, reward_quote_plan_event_window_blocks_new_buy,
    reward_quote_plan_event_window_cancels_open_buy, scale_double_legs_for_budget,
    scale_single_leg_for_budget, select_reward_book_token_ids,
};
use polyedge_connectors::{
    ConnectorNewsItem, ConnectorOrderStatusUpdate, ConnectorTradeFillUpdate,
    LivePolymarketCancelOrderRequest, LivePolymarketCancelOutcome, LivePolymarketConfig,
    LivePolymarketConnector, LivePolymarketExecutionOutcome, LivePolymarketOrderRequest,
    LivePolymarketOrderStatusRequest, LivePolymarketTokenOrderRequest,
    LivePolymarketTradeSyncOutcome, LivePolymarketTradeSyncRequest, NewsSource,
    OrderbookStreamClient, PAPER_ACCOUNT_ID, PAPER_EXECUTOR_NAME, POLYMARKET_CONNECTOR_NAME,
    PaperExecutionOutcome, PaperExecutor, PaperFillRequest, PaperOrderRequest,
    PaperOrderStatusRequest, PolymarketAcceptedOrderStatus, PolymarketChainConnector,
    PolymarketDataApiConnector, PolymarketGammaConnector, PolymarketGammaMarket,
    PolymarketMarketRefs, PolymarketMergePositionsRequest, PolymarketOpenOrder,
    PolymarketOrderRejection, PolymarketRewardMarket, PolymarketRewardsConnector,
    PolymarketSignatureScheme, PolymarketTokenOrderSide, PolymarketWalletActivity,
    PolymarketWalletPosition, RssNewsConnector, RssNewsSourceConfig,
    normalize_polymarket_ws_order_message, normalize_polymarket_ws_trade_message,
};
use polyedge_domain::{
    AppError, EventStatus, EvidenceDirection, EvidenceStatus, OrderStatus, Probability, Quantity,
    Result, UsdAmount, UserRole,
};
use polyedge_infrastructure::{
    AppState, Runtime, new_trace_id,
    settings::{NewsSourceSettings, PolymarketSignatureType},
    telemetry::init_tracing,
};
use polymarket_client_sdk::clob::ws::WsMessage;
use polymarket_client_sdk::types::B256;
use rust_decimal::{Decimal, RoundingStrategy};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    future::Future,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use time::{Duration as TimeDuration, OffsetDateTime};
use tokio::{
    sync::{RwLock, mpsc, watch},
    task::JoinHandle,
};
use tracing::{debug, info, warn};
use uuid::Uuid;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct MarketSyncReport {
    fetched: usize,
    upserted: usize,
}

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
struct RewardInfoRiskScanReport {
    candidates: usize,
    cache_hits: usize,
    requested: usize,
    saved: usize,
    failures: usize,
    skipped_missing_market: usize,
    applied_plans: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PolymarketOrderEventOutcome {
    Applied,
    Ignored,
    UnknownOrder,
}

const WORKER_SERVICE_COMMAND: &str = "run";
const REWARD_PRICE_TICK: Decimal = Decimal::from_parts(1, 0, 0, false, 2);
const REWARD_BOOK_HISTORY_LIMIT: usize = 240;

pub async fn run_cli() -> Result<()> {
    init_tracing("polyedge_worker");
    let runtime = Runtime::load().await?;
    let state = {
        let base = runtime.app_state();
        let url = &base.settings.orderbook.service_url;
        let client = std::sync::Arc::new(polyedge_connectors::OrderbookHttpClient::new(
            url,
            base.settings.orderbook.write_token.as_deref(),
        ));
        AppState {
            orderbook_cache: client.clone(),
            orderbook_registry: client,
            ..base
        }
    };
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
        Some("run-database-maintenance-once") => {
            let report = run_database_maintenance_once(&state).await?;
            log_database_maintenance_report(report, "completed database maintenance once");
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
                placed_orders = report.placed_orders,
                cancelled_orders = report.cancelled_orders,
                "ran rewards bot once",
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
                placed_orders = report.placed_orders,
                cancelled_orders = report.cancelled_orders,
                "reward bot polling stopped",
            );
            Ok(())
        }
        Some("scan-reward-info-risks-once") => {
            let trace_id = new_trace_id();
            let report = scan_reward_info_risks_once(&state, &trace_id).await?;
            info!(
                trace_id = %trace_id,
                candidates = report.candidates,
                cache_hits = report.cache_hits,
                requested = report.requested,
                saved = report.saved,
                applied_plans = report.applied_plans,
                "scanned reward market info risks once",
            );
            Ok(())
        }
        Some("poll-reward-info-risks") => {
            let max_cycles = parse_limit_arg(args.next())?.map(usize::from);
            let report = poll_reward_info_risks(&state, max_cycles).await?;
            info!(
                candidates = report.candidates,
                cache_hits = report.cache_hits,
                requested = report.requested,
                saved = report.saved,
                applied_plans = report.applied_plans,
                "reward info risk polling stopped",
            );
            Ok(())
        }
        Some("sync-markets-once") => {
            let trace_id = new_trace_id();
            let report = sync_markets_once(&state, &trace_id).await?;
            info!(
                trace_id = %trace_id,
                fetched = report.fetched,
                upserted = report.upserted,
                "synced markets from Polymarket Gamma once",
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
include!("worker/database_maintenance.rs");
include!("worker/orderbook_registration.rs");
include!("worker/service_info_risk.rs");
include!("worker/execution_queue.rs");
include!("worker/news.rs");
include!("worker/market_sync.rs");
include!("worker/rewards.rs");
include!("worker/news_helpers.rs");
include!("worker/execution_reconcile.rs");
include!("worker/polymarket_events.rs");
include!("worker/polymarket_config.rs");
include!("worker/execution_dispatch.rs");
include!("worker/news_promotion.rs");
include!("worker/shared.rs");

#[cfg(test)]
mod tests;
