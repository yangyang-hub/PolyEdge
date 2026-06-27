use axum::{
    Router,
    extract::{Extension, Json, Path, Query, State},
    http::{HeaderMap, StatusCode},
    middleware,
    routing::get,
};
use polyedge_application::{
    AddTrackedWalletInput, AuthenticatedActor, CopyControlAction, CopyTradeConfigPatch,
    CopyTradeSnapshot, EventListFilters, EventView, EvidenceListFilters, EvidenceView,
    ExecutionFillResult, ExecutionRequestListFilters, ExecutionRequestView, IdempotencyBegin,
    IdempotencyRequest, MarketListFilters, MarketSortField, MarketView, ModeTransitionCommand,
    NewsRawEventListFilters, NewsRawEventView, NewsSourceHealthListFilters, NewsSourceHealthView,
    OrderDraftListFilters, OrderDraftView, OrderListFilters, OrderView, OrderbookCache, PageQuery,
    Paginated, PositionListFilters, PositionView, ProbabilityEstimateListFilters,
    ProbabilityEstimateView, ReconcileExternalTradeCommand, RewardBotConfigPatch,
    RewardBotSnapshot, RewardControlAction, RewardOrderListQuery, RewardQuotePlanListQuery,
    RewardTokenQuote, RiskPolicy, RiskStateView, SortOrder, SyncExternalOrderStatusCommand,
    TrackedWalletStatus, TradeListFilters, TradeView, WalletActionInput,
};
use polyedge_connectors::{
    ConnectorOrderStatusUpdate, ConnectorTradeFillUpdate, PolymarketChainConnector,
    PolymarketDataApiConnector,
    PolymarketFundingTransferRequest as ConnectorFundingTransferRequest,
    normalize_polymarket_order_status_update, normalize_polymarket_trade_fill_update,
};
use polyedge_contracts::{
    ApiMeta, ApiResponse, ConnectorOrderStatusCallbackData, ConnectorOrderStatusCallbackRequest,
    ConnectorTradeFillCallbackData, ConnectorTradeFillCallbackRequest, DependencyStatus, EventData,
    EventListQuery, EvidenceData, EvidenceListQuery, ExecutionRequestData,
    ExecutionRequestListQuery, FundingStatusData, FundingTokenData, FundingTransferData,
    FundingTransferRequest, HealthData, MarketCategoryData, MarketData, MarketListQuery,
    MarketListResponse, NewsRawEventData, NewsRawEventListQuery, NewsSourceHealthData,
    NewsSourceHealthListQuery, OrderData, OrderDraftData, OrderDraftListQuery, OrderListQuery,
    OrderbookData, OrderbookLevelData, PolymarketOrderStatusCallbackRequest,
    PolymarketTradeFillCallbackRequest, PositionData, ProbabilityEstimateData,
    ProbabilityEstimateListQuery, ReadinessData, RewardBotSnapshotQuery, RiskStateData,
    RuntimeConfigEntryData, SystemModeData, TradeData, TradeListQuery, TransitionSystemModeRequest,
    UpdateRuntimeConfigRequest, WalletActivityData, WalletAnalysisData, WalletAnalysisRequest,
    WalletCategoryData, WalletClosedPositionData, WalletPnlData, WalletProfileData,
    WalletRecentTradeData, WalletRiskData, WalletStyleData, WalletTopMarketData,
};
use polyedge_domain::{AppError, OrderStatus, Probability, Quantity, StepUpScope, UsdAmount};
use polyedge_infrastructure::stores::ExternalEventBegin;
use polyedge_infrastructure::{
    AppState, AuthContext, HttpError, IdempotencyKey, hash_json, new_trace_id,
    request_id_from_headers, require_connector_write_auth, require_console_read_auth,
    require_console_write_auth, require_mode_write_auth,
};
use rust_decimal::Decimal;
use std::{collections::HashMap, str::FromStr};
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer, limit::RequestBodyLimitLayer, timeout::TimeoutLayer, trace::TraceLayer,
};

const CONNECTOR_ORDER_STATUS_SOURCE: &str = "connector.orders.status";
const CONNECTOR_TRADE_FILL_SOURCE: &str = "connector.trades.fill";
pub fn build_app(state: AppState) -> Router {
    let system_routes = Router::new()
        .route(
            "/mode",
            get(read_system_mode).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/mode",
            axum::routing::post(transition_system_mode).route_layer(
                middleware::from_fn_with_state(state.clone(), require_mode_write_auth),
            ),
        )
        .with_state(state.clone());

    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route(
            "/api/v1/markets",
            get(list_markets).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/markets/{market_id}",
            get(get_market).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/market-categories",
            get(list_market_categories).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/orderbook/{token_id}",
            get(get_orderbook).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/funding",
            get(read_funding_status).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/funding/transfer",
            axum::routing::post(submit_funding_transfer).route_layer(
                middleware::from_fn_with_state(state.clone(), require_console_write_auth),
            ),
        )
        .route(
            "/api/v1/events",
            get(list_events).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/news/source-health",
            get(list_news_source_health).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/news/raw-events",
            get(list_news_raw_events).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/evidences",
            get(list_evidences).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/orders/drafts",
            get(list_order_drafts).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/orders",
            get(list_orders).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/trades",
            get(list_trades).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/execution/requests",
            get(list_execution_requests).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/connectors/callbacks/orders/status",
            axum::routing::post(connector_order_status_callback).route_layer(
                middleware::from_fn_with_state(state.clone(), require_connector_write_auth),
            ),
        )
        .route(
            "/api/v1/connectors/callbacks/trades/fill",
            axum::routing::post(connector_trade_fill_callback).route_layer(
                middleware::from_fn_with_state(state.clone(), require_connector_write_auth),
            ),
        )
        .route(
            "/api/v1/connectors/polymarket/callbacks/orders/status",
            axum::routing::post(polymarket_order_status_callback).route_layer(
                middleware::from_fn_with_state(state.clone(), require_connector_write_auth),
            ),
        )
        .route(
            "/api/v1/connectors/polymarket/callbacks/trades/fill",
            axum::routing::post(polymarket_trade_fill_callback).route_layer(
                middleware::from_fn_with_state(state.clone(), require_connector_write_auth),
            ),
        )
        .route(
            "/api/v1/pricing/estimates",
            get(list_probability_estimates).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/rewards-bot",
            get(read_reward_bot_snapshot).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/rewards-bot/config",
            axum::routing::post(update_reward_bot_config).route_layer(
                middleware::from_fn_with_state(state.clone(), require_console_write_auth),
            ),
        )
        .route(
            "/api/v1/rewards-bot/run",
            axum::routing::post(run_reward_bot_once).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_write_auth,
            )),
        )
        .route(
            "/api/v1/rewards-bot/cancel-all",
            axum::routing::post(cancel_reward_bot_orders).route_layer(
                middleware::from_fn_with_state(state.clone(), require_console_write_auth),
            ),
        )
        .route(
            "/api/v1/rewards-bot/reset",
            axum::routing::post(reset_reward_bot).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_write_auth,
            )),
        )
        // ── Copy Trading ─────────────────────────────────────────────
        .route(
            "/api/v1/copy-trading",
            get(read_copytrade_snapshot).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/copy-trading/config",
            axum::routing::post(update_copytrade_config).route_layer(
                middleware::from_fn_with_state(state.clone(), require_console_write_auth),
            ),
        )
        .route(
            "/api/v1/copy-trading/wallets",
            axum::routing::post(add_copytrade_wallet).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_write_auth,
            )),
        )
        .route(
            "/api/v1/copy-trading/wallets/remove",
            axum::routing::post(remove_copytrade_wallet).route_layer(
                middleware::from_fn_with_state(state.clone(), require_console_write_auth),
            ),
        )
        .route(
            "/api/v1/copy-trading/wallets/status",
            axum::routing::post(set_copytrade_wallet_status).route_layer(
                middleware::from_fn_with_state(state.clone(), require_console_write_auth),
            ),
        )
        .route(
            "/api/v1/copy-trading/run",
            axum::routing::post(run_copytrade_once).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_write_auth,
            )),
        )
        .route(
            "/api/v1/copy-trading/analyze",
            axum::routing::post(analyze_copytrade_wallets).route_layer(
                middleware::from_fn_with_state(state.clone(), require_console_write_auth),
            ),
        )
        .route(
            "/api/v1/copy-trading/cancel-all",
            axum::routing::post(cancel_copytrade_orders).route_layer(
                middleware::from_fn_with_state(state.clone(), require_console_write_auth),
            ),
        )
        .route(
            "/api/v1/copy-trading/reset",
            axum::routing::post(reset_copytrade).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_write_auth,
            )),
        )
        // ── Wallet Analysis ─────────────────────────────────────────
        .route(
            "/api/v1/wallet-analysis",
            axum::routing::post(analyze_wallet).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/runtime-config",
            get(read_runtime_config).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/runtime-config",
            axum::routing::post(update_runtime_config).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_write_auth,
            )),
        )
        .nest("/api/v1/system", system_routes)
        .with_state(state)
        .layer(
            ServiceBuilder::new()
                .layer(RequestBodyLimitLayer::new(1024 * 1024))
                .layer(TraceLayer::new_for_http())
                .layer(TimeoutLayer::with_status_code(
                    StatusCode::REQUEST_TIMEOUT,
                    std::time::Duration::from_secs(30),
                )),
        )
        .layer(CorsLayer::permissive())
}

include!("handlers/health.rs");
include!("handlers/system.rs");
include!("handlers/market_handlers.rs");
include!("handlers/funding.rs");
include!("handlers/rewards.rs");
include!("handlers/copytrade.rs");
include!("handlers/runtime_config.rs");
include!("handlers/runtime_config_helpers.rs");
include!("handlers/execution_lists.rs");
include!("handlers/callbacks.rs");
include!("handlers/mode_control.rs");
include!("handlers/mappers.rs");
include!("handlers/callback_helpers.rs");
include!("handlers/wallet_analysis.rs");

#[cfg(test)]
mod tests;
