use axum::{
    Router,
    body::{Body, Bytes},
    extract::{Extension, Json, Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    middleware,
    response::Response,
    routing::get,
};
use futures::stream;
use polyedge_application::{
    ArbitrageAnalysisRunListFilters, ArbitrageAnalysisRunView, ArbitrageEventListFilters,
    ArbitrageEventView, ArbitrageOpportunityListFilters, ArbitrageOpportunityStatus,
    ArbitrageOpportunityType, ArbitrageOpportunityValidationView, ArbitrageOpportunityView,
    ArbitrageScanListFilters, ArbitrageScanView, ArbitrageValidationStatus, AuthenticatedActor,
    EventListFilters, EventView, EvidenceListFilters, EvidenceView, ExecutionFillResult,
    ExecutionRequestListFilters, ExecutionRequestView, ExecutionSubmissionReceipt,
    IdempotencyBegin, IdempotencyRequest, KillSwitchReceipt, MarketListFilters, MarketView,
    ModeTransitionCommand, NewsRawEventListFilters, NewsRawEventView, NewsSourceHealthListFilters,
    NewsSourceHealthView, OrderDraftListFilters, OrderDraftView, OrderListFilters, OrderView,
    PositionListFilters, PositionView, ProbabilityEstimateListFilters, ProbabilityEstimateView,
    ReconcileExternalTradeCommand, ReleaseKillSwitchCommand, RewardBookLevel, RewardBotConfigPatch,
    RewardBotSnapshot, RewardMarket, RewardOrderBook, RewardToken, RiskPolicy, RiskStateView,
    SignalListFilters, SignalTransitionListFilters, SignalTransitionView, SignalView,
    SubmitExecutionCommand, SyncExternalOrderStatusCommand, TradeListFilters, TradeView,
    TriggerKillSwitchCommand, select_reward_book_token_ids,
};
use polyedge_connectors::{
    ConnectorOrderStatusUpdate, ConnectorTradeFillUpdate, PolymarketRewardMarket,
    PolymarketRewardOrderBook, PolymarketRewardsConnector,
    normalize_polymarket_order_status_update, normalize_polymarket_trade_fill_update,
};
use polyedge_contracts::{
    AlertSeverity, AlertStatus, ApiResponse, ArbitrageAnalysisRunData,
    ArbitrageAnalysisRunListQuery, ArbitrageOpportunityData, ArbitrageOpportunityListQuery,
    ArbitrageOpportunityValidationData, ArbitrageScanData, ArbitrageScanListQuery, BucketStatus,
    ConnectorOrderStatusCallbackData, ConnectorOrderStatusCallbackRequest,
    ConnectorTradeFillCallbackData, ConnectorTradeFillCallbackRequest, DependencyStatus, EventData,
    EventListQuery, EvidenceData, EvidenceListQuery, ExecutionRequestData,
    ExecutionRequestListQuery, HealthData, KillSwitchData, MarketData, MarketListQuery,
    NewsRawEventData, NewsRawEventListQuery, NewsSourceHealthData, NewsSourceHealthListQuery,
    OrderData, OrderDraftData, OrderDraftListQuery, OrderListQuery,
    PolymarketOrderStatusCallbackRequest, PolymarketTradeFillCallbackRequest, PositionData,
    PositionListQuery, ProbabilityEstimateData, ProbabilityEstimateListQuery, ReadinessData,
    RecomputeSignalData, RecomputeSignalRequest, ReleaseKillSwitchRequest, RiskAlertData,
    RiskAlertListQuery, RiskBucketData, RiskBucketListQuery, RiskStateData, SignalData,
    SignalListQuery, SignalTransitionData, SignalTransitionListQuery, SubmitExecutionData,
    SubmitExecutionRequest, SystemModeData, TradeData, TradeListQuery, TransitionSystemModeRequest,
    TriggerKillSwitchRequest,
};
use polyedge_domain::{
    AppError, Edge, ExposureRatio, OrderStatus, Probability, Quantity, StepUpScope, SystemMode,
    UsdAmount,
};
use polyedge_infrastructure::stores::ExternalEventBegin;
use polyedge_infrastructure::{
    AppState, AuthContext, HttpError, IdempotencyKey, hash_json, new_trace_id,
    request_id_from_headers, require_connector_write_auth, require_console_read_auth,
    require_console_write_auth, require_mode_write_auth,
};
use rust_decimal::Decimal;
use serde_json::{Map, Value, json};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    convert::Infallible,
    str::FromStr,
    time::Duration,
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tower::ServiceBuilder;
use tower_http::{limit::RequestBodyLimitLayer, timeout::TimeoutLayer, trace::TraceLayer};

const CONNECTOR_ORDER_STATUS_SOURCE: &str = "connector.orders.status";
const CONNECTOR_TRADE_FILL_SOURCE: &str = "connector.trades.fill";
const DEFAULT_CONSOLE_LIST_LIMIT: u16 = 100;
const MAX_CONSOLE_LIST_LIMIT: u16 = 200;
const MAX_STREAM_EMITTED_IDS: usize = 1_024;

pub fn build_app(state: AppState) -> Router {
    let system_routes =
        Router::new()
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
            .route(
                "/kill-switch/trigger",
                axum::routing::post(trigger_kill_switch).route_layer(
                    middleware::from_fn_with_state(state.clone(), require_console_write_auth),
                ),
            )
            .route(
                "/kill-switch/release",
                axum::routing::post(release_kill_switch).route_layer(
                    middleware::from_fn_with_state(state.clone(), require_console_write_auth),
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
            "/api/v1/signals",
            get(list_signals).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/signals/{signal_id}/transitions",
            get(list_signal_transitions).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/signals/{signal_id}/recompute",
            axum::routing::post(recompute_signal).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_write_auth,
            )),
        )
        .route(
            "/api/v1/signals/{signal_id}/execution-requests",
            axum::routing::post(submit_execution_request).route_layer(
                middleware::from_fn_with_state(state.clone(), require_console_write_auth),
            ),
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
            "/api/v1/positions",
            get(list_positions).route_layer(middleware::from_fn_with_state(
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
            "/api/v1/arbitrage/scans",
            get(list_arbitrage_scans).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/arbitrage/opportunities",
            get(list_arbitrage_opportunities).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/arbitrage/analysis",
            get(list_arbitrage_analysis_runs).route_layer(middleware::from_fn_with_state(
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
            "/api/v1/risk/state",
            get(read_risk_state).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/risk/alerts",
            get(list_risk_alerts).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/risk/buckets",
            get(list_risk_buckets).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
            )),
        )
        .route(
            "/api/v1/stream/{channel}",
            get(stream_channel).route_layer(middleware::from_fn_with_state(
                state.clone(),
                require_console_read_auth,
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
                    std::time::Duration::from_secs(10),
                )),
        )
}

async fn healthz(headers: HeaderMap) -> Json<ApiResponse<HealthData>> {
    let request_id = request_id_from_headers(&headers);
    Json(ApiResponse::new(
        HealthData {
            status: "ok".to_string(),
        },
        request_id,
        new_trace_id(),
    ))
}

async fn readyz(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> (StatusCode, Json<ApiResponse<ReadinessData>>) {
    let request_id = request_id_from_headers(&headers);
    let trace_id = new_trace_id();
    let postgres_status = match state.dependencies.postgres_ready().await {
        Ok(()) => DependencyStatus {
            status: "ready".to_string(),
            detail: None,
        },
        Err(error) => DependencyStatus {
            status: "not_ready".to_string(),
            detail: Some(error.message().to_string()),
        },
    };
    let redis_status = match state.dependencies.redis_ready().await {
        Ok(()) => DependencyStatus {
            status: "ready".to_string(),
            detail: None,
        },
        Err(error) => DependencyStatus {
            status: "not_ready".to_string(),
            detail: Some(error.message().to_string()),
        },
    };

    let status = if postgres_status.status == "ready" && redis_status.status == "ready" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (
        status,
        Json(ApiResponse::new(
            ReadinessData {
                status: if status == StatusCode::OK {
                    "ready".to_string()
                } else {
                    "degraded".to_string()
                },
                postgres: postgres_status,
                redis: redis_status,
            },
            request_id,
            trace_id,
        )),
    )
}

#[derive(Debug, Clone)]
struct ConsoleRiskSnapshot {
    risk_state: RiskStateView,
    environment: String,
    alerts: Vec<RiskAlertData>,
    buckets: Vec<RiskBucketData>,
}

#[derive(Debug, Clone)]
struct BucketAccumulator {
    exposure: Decimal,
    updated_at: OffsetDateTime,
    version: i64,
}

async fn read_console_risk_snapshot(
    state: &AppState,
) -> polyedge_domain::Result<ConsoleRiskSnapshot> {
    let risk_state = state.risk_service.read_state().await?;
    let mode = state.system_mode_service.read_mode().await?;
    let markets = state
        .market_event_service
        .list_markets(MarketListFilters {
            status: None,
            tradability_status: None,
            limit: u16::MAX,
        })
        .await?;
    let positions = state
        .execution_service
        .list_positions(PositionListFilters {
            market_id: None,
            connector_name: None,
            side: None,
            limit: u16::MAX,
        })
        .await?;
    let markets_by_id = markets
        .iter()
        .map(|market| (market.id.clone(), market.clone()))
        .collect::<HashMap<_, _>>();
    let buckets = derive_risk_buckets(&positions, &markets_by_id)?;
    let alerts = derive_risk_alerts(&risk_state, &buckets, state.risk_service.policy())?;

    Ok(ConsoleRiskSnapshot {
        risk_state,
        environment: mode.environment,
        alerts,
        buckets,
    })
}

fn derive_risk_buckets(
    positions: &[PositionView],
    markets_by_id: &HashMap<String, MarketView>,
) -> polyedge_domain::Result<Vec<RiskBucketData>> {
    let mut grouped = HashMap::<String, BucketAccumulator>::new();

    for position in positions {
        let bucket_name = markets_by_id
            .get(&position.market_id)
            .map(|market| market.category.clone())
            .unwrap_or_else(|| "Uncategorized".to_string());
        let exposure = (position.net_quantity.value() * position.mark_price.value()).abs();

        grouped
            .entry(bucket_name)
            .and_modify(|bucket| {
                bucket.exposure += exposure;
                bucket.updated_at = bucket.updated_at.max(position.updated_at);
                bucket.version = bucket.version.max(position.version);
            })
            .or_insert(BucketAccumulator {
                exposure,
                updated_at: position.updated_at,
                version: position.version,
            });
    }

    let total_exposure = grouped
        .values()
        .fold(Decimal::ZERO, |sum, bucket| sum + bucket.exposure);
    let mut buckets = grouped
        .into_iter()
        .map(|(name, bucket)| {
            let exposure_ratio = if total_exposure > Decimal::ZERO {
                bucket.exposure / total_exposure
            } else {
                Decimal::ZERO
            };
            let limit = category_limit(&name)?;
            let utilization = if limit.value() > Decimal::ZERO {
                exposure_ratio / limit.value()
            } else {
                Decimal::ZERO
            };
            let status = if utilization >= Decimal::ONE {
                BucketStatus::Breach
            } else if utilization >= Decimal::new(85, 2) {
                BucketStatus::Watch
            } else {
                BucketStatus::Healthy
            };

            Ok(RiskBucketData {
                id: format!("bucket_{}", slugify(&name)),
                name,
                exposure: ExposureRatio::new(exposure_ratio)?,
                limit,
                utilization: ExposureRatio::new(utilization)?,
                status,
                updated_at: bucket.updated_at,
                version: bucket.version,
            })
        })
        .collect::<polyedge_domain::Result<Vec<_>>>()?;

    buckets.sort_by(|left, right| right.exposure.cmp(&left.exposure));
    Ok(buckets)
}

fn derive_risk_alerts(
    risk_state: &RiskStateView,
    buckets: &[RiskBucketData],
    policy: &RiskPolicy,
) -> polyedge_domain::Result<Vec<RiskAlertData>> {
    let mut alerts = Vec::new();
    let daily_loss_used = daily_loss_used(risk_state)?;
    let daily_loss_limit = policy.max_daily_loss.value();
    let daily_loss_usage = if daily_loss_limit > Decimal::ZERO {
        daily_loss_used.value() / daily_loss_limit
    } else {
        Decimal::ZERO
    };

    if risk_state.kill_switch {
        alerts.push(RiskAlertData {
            id: "alt_kill_switch_active".to_string(),
            severity: AlertSeverity::Critical,
            reason: "Kill switch is active. Execution remains halted until a protected release completes.".to_string(),
            target: "System Runtime".to_string(),
            status: AlertStatus::Unresolved,
            created_at: risk_state.updated_at,
            updated_at: risk_state.updated_at,
            version: risk_state.version,
        });
    }

    if daily_loss_usage >= Decimal::new(8, 1) {
        alerts.push(RiskAlertData {
            id: "alt_daily_loss_usage".to_string(),
            severity: if daily_loss_usage >= Decimal::new(9, 1) {
                AlertSeverity::Critical
            } else {
                AlertSeverity::Warning
            },
            reason: format!(
                "Daily loss usage reached {}% of the configured budget.",
                (daily_loss_usage * Decimal::new(100, 0)).round_dp(0)
            ),
            target: "Global Risk".to_string(),
            status: if daily_loss_usage >= Decimal::new(9, 1) {
                AlertStatus::Unresolved
            } else {
                AlertStatus::Watching
            },
            created_at: risk_state.updated_at,
            updated_at: risk_state.updated_at,
            version: risk_state.version,
        });
    }

    for bucket in buckets {
        if bucket.status == BucketStatus::Healthy {
            continue;
        }

        alerts.push(RiskAlertData {
            id: format!("alt_bucket_{}", bucket.id),
            severity: if bucket.status == BucketStatus::Breach {
                AlertSeverity::Critical
            } else {
                AlertSeverity::Warning
            },
            reason: if bucket.status == BucketStatus::Breach {
                format!(
                    "{} exposure exceeded its configured concentration limit.",
                    bucket.name
                )
            } else {
                format!(
                    "{} exposure is approaching its configured concentration limit.",
                    bucket.name
                )
            },
            target: format!("{} Bucket", bucket.name),
            status: if bucket.status == BucketStatus::Breach {
                AlertStatus::Unresolved
            } else {
                AlertStatus::Watching
            },
            created_at: bucket.updated_at,
            updated_at: bucket.updated_at,
            version: bucket.version,
        });
    }

    alerts.sort_by(|left, right| {
        alert_severity_rank(left.severity)
            .cmp(&alert_severity_rank(right.severity))
            .then_with(|| right.updated_at.cmp(&left.updated_at))
    });
    Ok(alerts)
}

fn alert_severity_rank(severity: AlertSeverity) -> u8 {
    match severity {
        AlertSeverity::Critical => 0,
        AlertSeverity::Warning => 1,
    }
}

fn category_limit(category: &str) -> polyedge_domain::Result<ExposureRatio> {
    let limit = match category.to_lowercase().as_str() {
        "crypto" => Decimal::new(35, 2),
        "regulation" => Decimal::new(25, 2),
        "macro" => Decimal::new(18, 2),
        _ => Decimal::new(20, 2),
    };

    ExposureRatio::new(limit)
}

fn slugify(value: &str) -> String {
    let slug = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();

    if slug.is_empty() {
        "uncategorized".to_string()
    } else {
        slug
    }
}

fn apply_limit<T>(mut items: Vec<T>, limit: Option<u16>) -> Vec<T> {
    let limit = usize::from(
        limit
            .unwrap_or(DEFAULT_CONSOLE_LIST_LIMIT)
            .min(MAX_CONSOLE_LIST_LIMIT),
    );
    items.truncate(limit);
    items
}

#[derive(Debug, Clone)]
struct SseMessage {
    id: String,
    event: &'static str,
    data: Value,
}

#[derive(Clone)]
struct StreamState {
    app_state: AppState,
    channel: String,
    sequence: u64,
    emitted_ids: HashSet<String>,
    emitted_id_order: VecDeque<String>,
    last_arbitrage_sequence: Option<u64>,
}

async fn stream_channel(
    Extension(_auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Path(channel): Path<String>,
    headers: HeaderMap,
) -> std::result::Result<Response, HttpError> {
    if !matches!(
        channel.as_str(),
        "signals" | "risk" | "events" | "arbitrage"
    ) {
        return Err(HttpError::with_meta(
            AppError::not_found("STREAM_CHANNEL_NOT_FOUND", "unknown stream channel"),
            "unknown",
            new_trace_id(),
        ));
    }

    let mut emitted_ids = HashSet::new();
    let mut emitted_id_order = VecDeque::new();
    let mut last_arbitrage_sequence = None;

    if let Some(last_event_id) = headers
        .get("last-event-id")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if channel == "arbitrage" {
            last_arbitrage_sequence = last_event_id.parse::<u64>().ok();
        }
        emitted_ids.insert(last_event_id.to_string());
        emitted_id_order.push_back(last_event_id.to_string());
    }

    let stream_state = StreamState {
        app_state: state,
        channel,
        sequence: 0,
        emitted_ids,
        emitted_id_order,
        last_arbitrage_sequence,
    };
    let event_stream = stream::unfold(stream_state, |mut stream_state| async move {
        if stream_state.sequence > 0 {
            tokio::time::sleep(Duration::from_secs(5)).await;
        }

        let chunk = match build_stream_chunk(
            &stream_state.app_state,
            &stream_state.channel,
            stream_state.sequence,
            &mut stream_state.emitted_ids,
            &mut stream_state.emitted_id_order,
            &mut stream_state.last_arbitrage_sequence,
        )
        .await
        {
            Ok(chunk) => chunk,
            Err(error) => format!(
                "event: stream.error\ndata: {}\n\n",
                json!({
                    "code": error.code(),
                    "message": error.message(),
                    "retryable": error.retryable(),
                })
            ),
        };

        stream_state.sequence += 1;
        Some((Ok::<Bytes, Infallible>(Bytes::from(chunk)), stream_state))
    });

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-cache, no-transform")
        .header(header::CONNECTION, "keep-alive")
        .header("x-accel-buffering", "no")
        .body(Body::from_stream(event_stream))
        .map_err(|error| {
            HttpError::with_meta(
                AppError::internal(
                    "STREAM_RESPONSE_BUILD_FAILED",
                    format!("failed to build stream response: {error}"),
                ),
                "unknown",
                new_trace_id(),
            )
        })
}

async fn build_stream_chunk(
    state: &AppState,
    channel: &str,
    sequence: u64,
    emitted_ids: &mut HashSet<String>,
    emitted_id_order: &mut VecDeque<String>,
    last_arbitrage_sequence: &mut Option<u64>,
) -> polyedge_domain::Result<String> {
    let messages = match channel {
        "signals" => signal_stream_messages(state).await?,
        "risk" => risk_stream_messages(state).await?,
        "events" => event_stream_messages(state).await?,
        "arbitrage" => arbitrage_stream_messages(state, last_arbitrage_sequence).await?,
        _ => Vec::new(),
    };
    let messages = filter_new_sse_messages(messages, emitted_ids, emitted_id_order);

    if messages.is_empty() {
        return Ok(format!(
            ": polyedge {channel} stream heartbeat {sequence}\n\n"
        ));
    }

    Ok(messages
        .iter()
        .map(format_sse_message)
        .collect::<Vec<_>>()
        .join(""))
}

fn filter_new_sse_messages(
    messages: Vec<SseMessage>,
    emitted_ids: &mut HashSet<String>,
    emitted_id_order: &mut VecDeque<String>,
) -> Vec<SseMessage> {
    messages
        .into_iter()
        .filter(|message| remember_stream_event_id(&message.id, emitted_ids, emitted_id_order))
        .collect()
}

fn remember_stream_event_id(
    event_id: &str,
    emitted_ids: &mut HashSet<String>,
    emitted_id_order: &mut VecDeque<String>,
) -> bool {
    if !emitted_ids.insert(event_id.to_string()) {
        return false;
    }

    emitted_id_order.push_back(event_id.to_string());

    while emitted_ids.len() > MAX_STREAM_EMITTED_IDS {
        let Some(oldest_id) = emitted_id_order.pop_front() else {
            break;
        };
        emitted_ids.remove(&oldest_id);
    }

    true
}

async fn signal_stream_messages(state: &AppState) -> polyedge_domain::Result<Vec<SseMessage>> {
    let markets = state
        .market_event_service
        .list_markets(MarketListFilters::new(None, None, Some(100))?)
        .await?;
    let signals = state
        .market_event_service
        .list_signals(SignalListFilters::new(None, None, None, Some(50))?)
        .await?;

    Ok(signals
        .into_iter()
        .map(|signal| {
            let market = markets.iter().find(|market| market.id == signal.market_id);
            let event = match signal.lifecycle_state.as_str() {
                "new" => "signal.created",
                "invalidated" => "signal.invalidated",
                _ => "signal.updated",
            };

            SseMessage {
                id: format!("signals:{}:{}", signal.id, signal.version),
                event,
                data: json!({
                    "signal_id": signal.id,
                    "market_id": signal.market_id,
                    "market_question": market.map(|market| market.question.clone()),
                    "context_label": market.map(|market| {
                        format!("{} / {}", market.category, market.tradability_status.as_str())
                    }),
                    "version": signal.version,
                    "lifecycle_state": signal.lifecycle_state,
                    "side": signal.side,
                    "fair_price": signal.fair_price,
                    "market_price": signal.market_price,
                    "edge": signal.edge,
                    "confidence": signal.confidence,
                    "reason": signal.reason,
                    "risk_decision": signal.risk_decision,
                    "evidence_lines": Vec::<String>::new(),
                    "updated_at": format_timestamp(signal.updated_at),
                }),
            }
        })
        .collect())
}

async fn risk_stream_messages(state: &AppState) -> polyedge_domain::Result<Vec<SseMessage>> {
    let snapshot = read_console_risk_snapshot(state).await?;
    let open_alerts = snapshot
        .alerts
        .iter()
        .filter(|alert| alert.status != AlertStatus::Contained)
        .count();
    let critical_alerts = snapshot
        .alerts
        .iter()
        .filter(|alert| alert.severity == AlertSeverity::Critical)
        .count();
    let warning_alerts = snapshot
        .alerts
        .iter()
        .filter(|alert| alert.severity == AlertSeverity::Warning)
        .count();
    let risk_state = risk_state_to_contract(
        snapshot.risk_state.clone(),
        snapshot.environment.clone(),
        state.risk_service.policy(),
        Some(open_alerts.try_into().unwrap_or(u32::MAX)),
    )?;
    let mut messages = vec![SseMessage {
        id: format!("risk:{}:{}", risk_state.mode.as_str(), risk_state.version),
        event: "risk.mode_changed",
        data: json!({
            "resource_id": risk_state.id,
            "version": risk_state.version,
            "mode": risk_state.mode,
            "environment": risk_state.environment,
            "kill_switch": risk_state.kill_switch,
            "daily_pnl": risk_state.daily_pnl,
            "gross_exposure": risk_state.gross_exposure,
            "net_exposure": risk_state.net_exposure,
            "daily_loss_limit": risk_state.daily_loss_limit,
            "daily_loss_used": risk_state.daily_loss_used,
            "open_alerts": risk_state.open_alerts,
            "critical_alerts": critical_alerts,
            "warning_alerts": warning_alerts,
            "updated_at": format_timestamp(risk_state.updated_at),
        }),
    }];

    messages.extend(snapshot.alerts.into_iter().map(|alert| {
        let alert_id = alert.id;

        SseMessage {
            id: format!("risk:alert:{}:{}", alert_id, alert.version),
            event: "risk.alerted",
            data: json!({
                "resource_id": alert_id.clone(),
                "version": alert.version,
                "alert_id": alert_id,
                "severity": alert.severity,
                "reason": alert.reason,
                "target": alert.target,
                "status": alert.status,
                "created_at": format_timestamp(alert.created_at),
                "updated_at": format_timestamp(alert.updated_at),
            }),
        }
    }));
    Ok(messages)
}

async fn event_stream_messages(state: &AppState) -> polyedge_domain::Result<Vec<SseMessage>> {
    let events = state
        .market_event_service
        .list_events(EventListFilters::new(None, Some(50))?)
        .await?;

    Ok(events
        .into_iter()
        .map(|event| SseMessage {
            id: format!("events:{}:{}", event.id, event.version),
            event: "event.created",
            data: json!({
                "event_id": event.id,
                "source": event.source,
                "summary": event.summary,
                "confidence": event.confidence,
                "created_at": format_timestamp(event.created_at),
                "version": event.version,
            }),
        })
        .collect())
}

async fn arbitrage_stream_messages(
    state: &AppState,
    last_sequence: &mut Option<u64>,
) -> polyedge_domain::Result<Vec<SseMessage>> {
    let events = state
        .arbitrage_service
        .list_events(ArbitrageEventListFilters::new(*last_sequence, Some(100))?)
        .await?;

    if let Some(sequence) = events.last().map(|event| event.sequence) {
        *last_sequence = Some(sequence);
    }

    Ok(events
        .into_iter()
        .map(|event| SseMessage {
            id: event.sequence.to_string(),
            event: event.event_type.as_str(),
            data: arbitrage_event_sse_data(event),
        })
        .collect())
}

fn arbitrage_event_sse_data(event: ArbitrageEventView) -> Value {
    let mut data = match event.payload {
        Value::Object(map) => map,
        payload => {
            let mut map = Map::new();
            map.insert("payload".to_string(), payload);
            map
        }
    };

    data.insert("sequence".to_string(), json!(event.sequence));
    data.insert("event_id".to_string(), json!(event.id));
    data.insert("event_type".to_string(), json!(event.event_type.as_str()));
    data.insert("resource_type".to_string(), json!(event.resource_type));
    data.insert("resource_id".to_string(), json!(event.resource_id));
    data.insert(
        "occurred_at".to_string(),
        json!(format_timestamp(event.occurred_at)),
    );
    data.insert("trace_id".to_string(), json!(event.trace_id));
    Value::Object(data)
}

fn daily_loss_used(risk_state: &RiskStateView) -> polyedge_domain::Result<UsdAmount> {
    let daily_pnl = risk_state.daily_pnl.value();

    if daily_pnl < Decimal::ZERO {
        return UsdAmount::new(-daily_pnl);
    }

    UsdAmount::new(Decimal::ZERO)
}

fn format_sse_message(message: &SseMessage) -> String {
    format!(
        "id: {}\nevent: {}\ndata: {}\n\n",
        message.id, message.event, message.data
    )
}

fn format_timestamp(timestamp: OffsetDateTime) -> String {
    timestamp
        .format(&Rfc3339)
        .unwrap_or_else(|_| timestamp.to_string())
}

fn console_runtime_mode(mode: SystemMode) -> SystemMode {
    match mode {
        SystemMode::ManualConfirm => SystemMode::PaperTrade,
        other => other,
    }
}

fn normalize_submit_execution_modes(data: &mut SubmitExecutionData) {
    data.execution_request.mode = console_runtime_mode(data.execution_request.mode);
    data.risk_state.mode = console_runtime_mode(data.risk_state.mode);
}

fn normalize_kill_switch_modes(data: &mut KillSwitchData) {
    data.risk_state.mode = console_runtime_mode(data.risk_state.mode);
}

async fn read_system_mode(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> std::result::Result<Json<ApiResponse<SystemModeData>>, HttpError> {
    let trace_id = new_trace_id();
    let snapshot = state
        .system_mode_service
        .read_mode()
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        SystemModeData {
            mode: console_runtime_mode(snapshot.mode),
            environment: snapshot.environment,
            version: snapshot.version,
            replayed: false,
            updated_at: snapshot.updated_at,
        },
        auth.request_id,
        trace_id,
    )))
}

async fn list_markets(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<MarketListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<MarketData>>>, HttpError> {
    let trace_id = new_trace_id();
    let filters = MarketListFilters::new(query.status, query.tradability_status, query.limit)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let markets = state
        .market_event_service
        .list_markets(filters)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

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

async fn read_reward_bot_snapshot(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> std::result::Result<Json<ApiResponse<RewardBotSnapshot>>, HttpError> {
    let trace_id = new_trace_id();
    let snapshot =
        state.reward_bot_service.snapshot().await.map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
        })?;

    Ok(Json(ApiResponse::new(snapshot, auth.request_id, trace_id)))
}

async fn update_reward_bot_config(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Json(payload): Json<RewardBotConfigPatch>,
) -> std::result::Result<Json<ApiResponse<RewardBotSnapshot>>, HttpError> {
    let trace_id = new_trace_id();
    state
        .reward_bot_service
        .update_config(payload)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let snapshot =
        state.reward_bot_service.snapshot().await.map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
        })?;

    Ok(Json(ApiResponse::new(snapshot, auth.request_id, trace_id)))
}

async fn run_reward_bot_once(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> std::result::Result<Json<ApiResponse<RewardBotSnapshot>>, HttpError> {
    let trace_id = new_trace_id();
    let (markets, books) = fetch_reward_bot_inputs(&state, &trace_id)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    state
        .reward_bot_service
        .run_simulation(markets, books, &trace_id)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let snapshot =
        state.reward_bot_service.snapshot().await.map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
        })?;

    Ok(Json(ApiResponse::new(snapshot, auth.request_id, trace_id)))
}

async fn cancel_reward_bot_orders(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> std::result::Result<Json<ApiResponse<RewardBotSnapshot>>, HttpError> {
    let trace_id = new_trace_id();
    let config = state
        .reward_bot_service
        .read_config()
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    state
        .reward_bot_service
        .cancel_all_orders(
            Some(&config.account_id),
            "operator cancelled all simulated rewards orders",
            &trace_id,
        )
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let snapshot =
        state.reward_bot_service.snapshot().await.map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
        })?;

    Ok(Json(ApiResponse::new(snapshot, auth.request_id, trace_id)))
}

async fn fetch_reward_bot_inputs(
    state: &AppState,
    _trace_id: &str,
) -> polyedge_domain::Result<(Vec<RewardMarket>, HashMap<String, RewardOrderBook>)> {
    let config = state.reward_bot_service.read_config().await?;
    let connector = PolymarketRewardsConnector::new(&state.settings.polymarket.clob_host)?;
    let markets = connector
        .fetch_current_markets()
        .await?
        .into_iter()
        .map(reward_market_from_connector)
        .collect::<Vec<_>>();
    let token_ids = select_reward_book_token_ids(&markets, &config);
    let books = connector
        .fetch_order_books(&token_ids)
        .await?
        .into_iter()
        .map(reward_order_book_from_connector)
        .map(|book| (book.token_id.clone(), book))
        .collect::<HashMap<_, _>>();

    Ok((markets, books))
}

fn reward_market_from_connector(market: PolymarketRewardMarket) -> RewardMarket {
    RewardMarket {
        condition_id: market.condition_id,
        question: market.question,
        market_slug: market.market_slug,
        event_slug: market.event_slug,
        image: market.image,
        rewards_max_spread: market.rewards_max_spread,
        rewards_min_size: market.rewards_min_size,
        total_daily_rate: market.total_daily_rate,
        tokens: market
            .tokens
            .into_iter()
            .map(|token| RewardToken {
                token_id: token.token_id,
                outcome: token.outcome,
                price: token.price,
            })
            .collect(),
        active: market.active,
        updated_at: market.updated_at,
    }
}

fn reward_order_book_from_connector(book: PolymarketRewardOrderBook) -> RewardOrderBook {
    RewardOrderBook {
        token_id: book.token_id,
        bids: book
            .bids
            .into_iter()
            .map(|level| RewardBookLevel {
                price: level.price,
                size: level.size,
            })
            .collect(),
        asks: book
            .asks
            .into_iter()
            .map(|level| RewardBookLevel {
                price: level.price,
                size: level.size,
            })
            .collect(),
        observed_at: book.observed_at,
    }
}

async fn list_signal_transitions(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Path(signal_id): Path<String>,
    Query(query): Query<SignalTransitionListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<SignalTransitionData>>>, HttpError> {
    let trace_id = new_trace_id();
    let filters = SignalTransitionListFilters::new(signal_id, query.limit)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let transitions = state
        .market_event_service
        .list_signal_transitions(filters)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        transitions
            .into_iter()
            .map(signal_transition_to_contract)
            .collect(),
        auth.request_id,
        trace_id,
    )))
}

async fn list_order_drafts(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<OrderDraftListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<OrderDraftData>>>, HttpError> {
    let trace_id = new_trace_id();
    let filters = OrderDraftListFilters::new(
        query.signal_id,
        query.connector_name,
        query.status,
        query.limit,
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let order_drafts = state
        .execution_service
        .list_order_drafts(filters)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        order_drafts
            .into_iter()
            .map(order_draft_to_contract)
            .collect(),
        auth.request_id,
        trace_id,
    )))
}

async fn list_execution_requests(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<ExecutionRequestListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<ExecutionRequestData>>>, HttpError> {
    let trace_id = new_trace_id();
    let filters = ExecutionRequestListFilters::new(
        query.signal_id,
        query.connector_name,
        query.status,
        query.limit,
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let execution_requests = state
        .execution_service
        .list_execution_requests(filters)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        execution_requests
            .into_iter()
            .map(execution_request_to_contract)
            .collect(),
        auth.request_id,
        trace_id,
    )))
}

async fn list_orders(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<OrderListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<OrderData>>>, HttpError> {
    let trace_id = new_trace_id();
    let filters = OrderListFilters::new(
        query.signal_id,
        query.market_id,
        query.connector_name,
        query.status,
        query.limit,
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let orders = state
        .execution_service
        .list_orders(filters)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        orders.into_iter().map(order_to_contract).collect(),
        auth.request_id,
        trace_id,
    )))
}

async fn list_trades(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<TradeListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<TradeData>>>, HttpError> {
    let trace_id = new_trace_id();
    let filters = TradeListFilters::new(
        query.order_id,
        query.signal_id,
        query.market_id,
        query.connector_name,
        query.limit,
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let trades = state
        .execution_service
        .list_trades(filters)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        trades.into_iter().map(trade_to_contract).collect(),
        auth.request_id,
        trace_id,
    )))
}

async fn list_positions(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<PositionListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<PositionData>>>, HttpError> {
    let trace_id = new_trace_id();
    let filters = PositionListFilters::new(
        query.market_id,
        query.connector_name,
        query.side,
        query.limit,
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let positions = state
        .execution_service
        .list_positions(filters)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        positions.into_iter().map(position_to_contract).collect(),
        auth.request_id,
        trace_id,
    )))
}

#[derive(Debug, Clone)]
struct ConnectorOrderStatusInput {
    event_id: String,
    connector_name: String,
    external_order_id: String,
    status: OrderStatus,
}

#[derive(Debug, Clone)]
struct ConnectorTradeFillInput {
    event_id: String,
    connector_name: String,
    external_order_id: String,
    account_id: String,
    external_trade_id: String,
    fill_price: Probability,
    filled_quantity: Quantity,
    fee: UsdAmount,
}

impl From<ConnectorOrderStatusUpdate> for ConnectorOrderStatusInput {
    fn from(value: ConnectorOrderStatusUpdate) -> Self {
        Self {
            event_id: value.event_id,
            connector_name: value.connector_name,
            external_order_id: value.external_order_id,
            status: value.status,
        }
    }
}

impl From<ConnectorTradeFillUpdate> for ConnectorTradeFillInput {
    fn from(value: ConnectorTradeFillUpdate) -> Self {
        Self {
            event_id: value.event_id,
            connector_name: value.connector_name,
            external_order_id: value.external_order_id,
            account_id: value.account_id,
            external_trade_id: value.external_trade_id,
            fill_price: value.fill_price,
            filled_quantity: value.filled_quantity,
            fee: value.fee,
        }
    }
}

async fn connector_order_status_callback(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Json(payload): Json<ConnectorOrderStatusCallbackRequest>,
) -> std::result::Result<Json<ApiResponse<ConnectorOrderStatusCallbackData>>, HttpError> {
    let trace_id = new_trace_id();
    let connector_name = normalize_callback_connector_name(&payload.connector_name)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let callback = ConnectorOrderStatusInput {
        event_id: validate_callback_event_id(&payload.event_id).map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
        })?,
        connector_name,
        external_order_id: payload.external_order_id.clone(),
        status: payload.status,
    };
    let response_data =
        process_connector_order_status_callback(&state, &auth, callback, &payload, &trace_id)
            .await?;

    Ok(Json(ApiResponse::new(
        response_data,
        auth.request_id,
        trace_id,
    )))
}

async fn connector_trade_fill_callback(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Json(payload): Json<ConnectorTradeFillCallbackRequest>,
) -> std::result::Result<Json<ApiResponse<ConnectorTradeFillCallbackData>>, HttpError> {
    let trace_id = new_trace_id();
    let connector_name = normalize_callback_connector_name(&payload.connector_name)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let callback = ConnectorTradeFillInput {
        event_id: validate_callback_event_id(&payload.event_id).map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
        })?,
        connector_name,
        external_order_id: payload.external_order_id.clone(),
        account_id: payload.account_id.clone(),
        external_trade_id: payload.external_trade_id.clone(),
        fill_price: payload.fill_price,
        filled_quantity: payload.filled_quantity,
        fee: payload.fee,
    };
    let response_data =
        process_connector_trade_fill_callback(&state, &auth, callback, &payload, &trace_id).await?;

    Ok(Json(ApiResponse::new(
        response_data,
        auth.request_id,
        trace_id,
    )))
}

async fn polymarket_order_status_callback(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Json(payload): Json<PolymarketOrderStatusCallbackRequest>,
) -> std::result::Result<Json<ApiResponse<ConnectorOrderStatusCallbackData>>, HttpError> {
    let trace_id = new_trace_id();
    let normalized = normalize_polymarket_order_status_update(
        &payload.event_id,
        &payload.order_id,
        payload.status.as_str(),
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let response_data =
        process_connector_order_status_callback(&state, &auth, normalized, &payload, &trace_id)
            .await?;

    Ok(Json(ApiResponse::new(
        response_data,
        auth.request_id,
        trace_id,
    )))
}

async fn polymarket_trade_fill_callback(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Json(payload): Json<PolymarketTradeFillCallbackRequest>,
) -> std::result::Result<Json<ApiResponse<ConnectorTradeFillCallbackData>>, HttpError> {
    let trace_id = new_trace_id();
    let normalized = normalize_polymarket_trade_fill_update(
        &payload.event_id,
        &payload.order_id,
        &payload.account_id,
        &payload.trade_id,
        payload.price,
        payload.size,
        payload.fee,
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let response_data =
        process_connector_trade_fill_callback(&state, &auth, normalized, &payload, &trace_id)
            .await?;

    Ok(Json(ApiResponse::new(
        response_data,
        auth.request_id,
        trace_id,
    )))
}

async fn process_connector_order_status_callback<T: serde::Serialize>(
    state: &AppState,
    auth: &AuthContext,
    callback: impl Into<ConnectorOrderStatusInput>,
    payload: &T,
    trace_id: &str,
) -> std::result::Result<ConnectorOrderStatusCallbackData, HttpError> {
    let callback = callback.into();
    let source_system = callback_source(CONNECTOR_ORDER_STATUS_SOURCE, &callback.connector_name);
    let request_hash = hash_json(payload).map_err(|error| {
        HttpError::with_meta(error, auth.request_id.clone(), trace_id.to_string())
    })?;

    match state
        .external_event_store
        .begin(&source_system, &callback.event_id, &request_hash, trace_id)
        .await
        .map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.to_string())
        })? {
        ExternalEventBegin::Replay => {
            let order = state
                .market_event_service
                .get_order_by_external_ref(
                    callback.connector_name.clone(),
                    callback.external_order_id.clone(),
                )
                .await
                .map_err(|error| {
                    HttpError::with_meta(error, auth.request_id.clone(), trace_id.to_string())
                })?;

            return Ok(connector_order_status_to_contract(order, true));
        }
        ExternalEventBegin::New => {}
    }

    let actor = authenticated_actor(auth);
    let order = match state
        .execution_service
        .sync_external_order_status(SyncExternalOrderStatusCommand {
            connector_name: callback.connector_name.clone(),
            external_order_id: callback.external_order_id.clone(),
            status: callback.status,
            request_id: auth.request_id.clone(),
            trace_id: trace_id.to_string(),
            actor,
        })
        .await
    {
        Ok(order) => order,
        Err(error) => {
            state
                .external_event_store
                .abandon(&source_system, &callback.event_id)
                .await
                .map_err(|abandon_error| {
                    HttpError::with_meta(
                        abandon_error,
                        auth.request_id.clone(),
                        trace_id.to_string(),
                    )
                })?;
            return Err(HttpError::with_meta(
                error,
                auth.request_id.clone(),
                trace_id.to_string(),
            ));
        }
    };

    state
        .external_event_store
        .mark_processed(&source_system, &callback.event_id, trace_id)
        .await
        .map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.to_string())
        })?;

    Ok(connector_order_status_to_contract(order, false))
}

async fn process_connector_trade_fill_callback<T: serde::Serialize>(
    state: &AppState,
    auth: &AuthContext,
    callback: impl Into<ConnectorTradeFillInput>,
    payload: &T,
    trace_id: &str,
) -> std::result::Result<ConnectorTradeFillCallbackData, HttpError> {
    let callback = callback.into();
    let source_system = callback_source(CONNECTOR_TRADE_FILL_SOURCE, &callback.connector_name);
    let request_hash = hash_json(payload).map_err(|error| {
        HttpError::with_meta(error, auth.request_id.clone(), trace_id.to_string())
    })?;

    match state
        .external_event_store
        .begin(&source_system, &callback.event_id, &request_hash, trace_id)
        .await
        .map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.to_string())
        })? {
        ExternalEventBegin::Replay => {
            return build_trade_fill_callback_response(
                state,
                &callback.connector_name,
                &callback.external_order_id,
                &callback.account_id,
                &callback.external_trade_id,
                true,
            )
            .await
            .map_err(|error| {
                HttpError::with_meta(error, auth.request_id.clone(), trace_id.to_string())
            });
        }
        ExternalEventBegin::New => {}
    }

    let actor = authenticated_actor(auth);
    let fill_result = match state
        .execution_service
        .reconcile_external_trade(ReconcileExternalTradeCommand {
            connector_name: callback.connector_name.clone(),
            external_order_id: callback.external_order_id.clone(),
            account_id: callback.account_id.clone(),
            external_trade_id: callback.external_trade_id.clone(),
            fill_price: callback.fill_price,
            filled_quantity: callback.filled_quantity,
            fee: callback.fee,
            request_id: auth.request_id.clone(),
            trace_id: trace_id.to_string(),
            actor,
        })
        .await
    {
        Ok(fill_result) => fill_result,
        Err(error) => {
            state
                .external_event_store
                .abandon(&source_system, &callback.event_id)
                .await
                .map_err(|abandon_error| {
                    HttpError::with_meta(
                        abandon_error,
                        auth.request_id.clone(),
                        trace_id.to_string(),
                    )
                })?;
            return Err(HttpError::with_meta(
                error,
                auth.request_id.clone(),
                trace_id.to_string(),
            ));
        }
    };

    state
        .external_event_store
        .mark_processed(&source_system, &callback.event_id, trace_id)
        .await
        .map_err(|error| {
            HttpError::with_meta(error, auth.request_id.clone(), trace_id.to_string())
        })?;

    let risk_state = state.risk_service.read_state().await.map_err(|error| {
        HttpError::with_meta(error, auth.request_id.clone(), trace_id.to_string())
    })?;

    connector_trade_fill_to_contract(fill_result, risk_state, false, state)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.to_string()))
}

async fn recompute_signal(
    Extension(auth): Extension<AuthContext>,
    Extension(idempotency_key): Extension<IdempotencyKey>,
    State(state): State<AppState>,
    Path(signal_id): Path<String>,
    Json(payload): Json<RecomputeSignalRequest>,
) -> std::result::Result<Json<ApiResponse<RecomputeSignalData>>, HttpError> {
    let trace_id = new_trace_id();
    let request_hash = hash_json(&payload)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let idempotency_request = IdempotencyRequest {
        scope: "signals.recompute".to_string(),
        idempotency_key: idempotency_key.0,
        request_hash,
        request_id: auth.request_id.clone(),
        actor_user_id: Some(auth.user_id.clone()),
        actor_session_id: Some(auth.session_id.clone()),
        resource_type: Some("signal".to_string()),
        resource_id: Some(signal_id.clone()),
    };

    match state
        .idempotency_store
        .begin(&idempotency_request)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?
    {
        IdempotencyBegin::Replay(response_json) => {
            let mut replayed: RecomputeSignalData =
                serde_json::from_str(&response_json).map_err(|error| {
                    HttpError::with_meta(
                        AppError::internal(
                            "SIGNAL_RECOMPUTE_REPLAY_DESERIALIZE_FAILED",
                            format!("failed to deserialize replayed recompute response: {error}"),
                        ),
                        auth.request_id.clone(),
                        trace_id.clone(),
                    )
                })?;
            replayed.replayed = true;

            return Ok(Json(ApiResponse::new(replayed, auth.request_id, trace_id)));
        }
        IdempotencyBegin::Started => {}
    }

    let result = match state
        .market_event_service
        .recompute_signal(signal_id, payload.reason, trace_id.clone())
        .await
    {
        Ok(result) => result,
        Err(error) => {
            state
                .idempotency_store
                .fail(&idempotency_request, error.code())
                .await
                .map_err(|fail_error| {
                    HttpError::with_meta(fail_error, auth.request_id.clone(), trace_id.clone())
                })?;
            return Err(HttpError::with_meta(
                error,
                auth.request_id.clone(),
                trace_id,
            ));
        }
    };

    let response_data = recompute_signal_to_contract(result, false);
    let response_json = serde_json::to_string(&response_data).map_err(|error| {
        HttpError::with_meta(
            AppError::internal(
                "SIGNAL_RECOMPUTE_RESPONSE_SERIALIZE_FAILED",
                format!("failed to serialize signal recompute response: {error}"),
            ),
            auth.request_id.clone(),
            trace_id.clone(),
        )
    })?;

    state
        .idempotency_store
        .complete(&idempotency_request, &response_json)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        response_data,
        auth.request_id,
        trace_id,
    )))
}

async fn read_risk_state(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> std::result::Result<Json<ApiResponse<RiskStateData>>, HttpError> {
    let trace_id = new_trace_id();
    let snapshot = read_console_risk_snapshot(&state)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let open_alerts = snapshot
        .alerts
        .iter()
        .filter(|alert| alert.status != AlertStatus::Contained)
        .count()
        .try_into()
        .unwrap_or(u32::MAX);

    Ok(Json(ApiResponse::new(
        risk_state_to_contract(
            snapshot.risk_state,
            snapshot.environment,
            state.risk_service.policy(),
            Some(open_alerts),
        )
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?,
        auth.request_id,
        trace_id,
    )))
}

async fn list_risk_alerts(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<RiskAlertListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<RiskAlertData>>>, HttpError> {
    let trace_id = new_trace_id();
    let snapshot = read_console_risk_snapshot(&state)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let alerts = snapshot
        .alerts
        .into_iter()
        .filter(|alert| query.status.is_none_or(|status| alert.status == status))
        .collect::<Vec<_>>();

    Ok(Json(ApiResponse::new(
        apply_limit(alerts, query.limit),
        auth.request_id,
        trace_id,
    )))
}

async fn list_risk_buckets(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
    Query(query): Query<RiskBucketListQuery>,
) -> std::result::Result<Json<ApiResponse<Vec<RiskBucketData>>>, HttpError> {
    let trace_id = new_trace_id();
    let snapshot = read_console_risk_snapshot(&state)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        apply_limit(snapshot.buckets, query.limit),
        auth.request_id,
        trace_id,
    )))
}

async fn submit_execution_request(
    Extension(auth): Extension<AuthContext>,
    Extension(idempotency_key): Extension<IdempotencyKey>,
    State(state): State<AppState>,
    Path(signal_id): Path<String>,
    Json(payload): Json<SubmitExecutionRequest>,
) -> std::result::Result<Json<ApiResponse<SubmitExecutionData>>, HttpError> {
    auth.ensure_scope(
        StepUpScope::ExecutionSubmit,
        time::OffsetDateTime::now_utc(),
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), new_trace_id()))?;

    let trace_id = new_trace_id();
    let request_hash = hash_json(&payload)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let idempotency_request = IdempotencyRequest {
        scope: "execution.requests.submit".to_string(),
        idempotency_key: idempotency_key.0,
        request_hash,
        request_id: auth.request_id.clone(),
        actor_user_id: Some(auth.user_id.clone()),
        actor_session_id: Some(auth.session_id.clone()),
        resource_type: Some("signal".to_string()),
        resource_id: Some(signal_id.clone()),
    };

    match state
        .idempotency_store
        .begin(&idempotency_request)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?
    {
        IdempotencyBegin::Replay(response_json) => {
            let mut replayed: SubmitExecutionData =
                serde_json::from_str(&response_json).map_err(|error| {
                    HttpError::with_meta(
                        AppError::internal(
                            "EXECUTION_REQUEST_REPLAY_DESERIALIZE_FAILED",
                            format!(
                                "failed to deserialize replayed execution submission response: {error}"
                            ),
                        ),
                        auth.request_id.clone(),
                        trace_id.clone(),
                    )
                })?;
            replayed.replayed = true;
            normalize_submit_execution_modes(&mut replayed);

            return Ok(Json(ApiResponse::new(replayed, auth.request_id, trace_id)));
        }
        IdempotencyBegin::Started => {}
    }

    let actor = AuthenticatedActor {
        user_id: auth.user_id.clone(),
        session_id: auth.session_id.clone(),
        roles: auth.roles.clone(),
        request_id: auth.request_id.clone(),
        ip: auth.ip.clone(),
        user_agent: auth.user_agent.clone(),
    };

    let receipt = match state
        .execution_service
        .submit_execution_request(SubmitExecutionCommand {
            signal_id,
            expected_signal_version: payload.expected_signal_version,
            limit_price: payload.limit_price,
            quantity: payload.quantity,
            connector_name: payload.connector_name,
            reason: payload.reason,
            request_id: auth.request_id.clone(),
            trace_id: trace_id.clone(),
            actor,
        })
        .await
    {
        Ok(receipt) => receipt,
        Err(error) => {
            state
                .idempotency_store
                .fail(&idempotency_request, error.code())
                .await
                .map_err(|fail_error| {
                    HttpError::with_meta(fail_error, auth.request_id.clone(), trace_id.clone())
                })?;
            return Err(HttpError::with_meta(
                error,
                auth.request_id.clone(),
                trace_id,
            ));
        }
    };

    let response_data = execution_submission_to_contract(receipt, false, &state)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let response_json = serde_json::to_string(&response_data).map_err(|error| {
        HttpError::with_meta(
            AppError::internal(
                "EXECUTION_REQUEST_RESPONSE_SERIALIZE_FAILED",
                format!("failed to serialize execution submission response: {error}"),
            ),
            auth.request_id.clone(),
            trace_id.clone(),
        )
    })?;

    state
        .idempotency_store
        .complete(&idempotency_request, &response_json)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        response_data,
        auth.request_id,
        trace_id,
    )))
}

async fn trigger_kill_switch(
    Extension(auth): Extension<AuthContext>,
    Extension(idempotency_key): Extension<IdempotencyKey>,
    State(state): State<AppState>,
    Json(payload): Json<TriggerKillSwitchRequest>,
) -> std::result::Result<Json<ApiResponse<KillSwitchData>>, HttpError> {
    auth.ensure_scope(
        StepUpScope::SystemKillSwitchTrigger,
        time::OffsetDateTime::now_utc(),
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), new_trace_id()))?;

    let trace_id = new_trace_id();
    let request_hash = hash_json(&payload)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let idempotency_request = IdempotencyRequest {
        scope: "system.kill_switch.trigger".to_string(),
        idempotency_key: idempotency_key.0,
        request_hash,
        request_id: auth.request_id.clone(),
        actor_user_id: Some(auth.user_id.clone()),
        actor_session_id: Some(auth.session_id.clone()),
        resource_type: Some("risk_state".to_string()),
        resource_id: Some("global".to_string()),
    };

    match state
        .idempotency_store
        .begin(&idempotency_request)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?
    {
        IdempotencyBegin::Replay(response_json) => {
            let mut replayed: KillSwitchData =
                serde_json::from_str(&response_json).map_err(|error| {
                    HttpError::with_meta(
                        AppError::internal(
                            "KILL_SWITCH_REPLAY_DESERIALIZE_FAILED",
                            format!("failed to deserialize replayed kill switch response: {error}"),
                        ),
                        auth.request_id.clone(),
                        trace_id.clone(),
                    )
                })?;
            replayed.replayed = true;
            normalize_kill_switch_modes(&mut replayed);

            return Ok(Json(ApiResponse::new(replayed, auth.request_id, trace_id)));
        }
        IdempotencyBegin::Started => {}
    }

    let actor = AuthenticatedActor {
        user_id: auth.user_id.clone(),
        session_id: auth.session_id.clone(),
        roles: auth.roles.clone(),
        request_id: auth.request_id.clone(),
        ip: auth.ip.clone(),
        user_agent: auth.user_agent.clone(),
    };

    let receipt = match state
        .risk_service
        .trigger_kill_switch(TriggerKillSwitchCommand {
            reason: payload.reason,
            expected_version: payload.expected_version,
            request_id: auth.request_id.clone(),
            trace_id: trace_id.clone(),
            actor,
        })
        .await
    {
        Ok(receipt) => receipt,
        Err(error) => {
            state
                .idempotency_store
                .fail(&idempotency_request, error.code())
                .await
                .map_err(|fail_error| {
                    HttpError::with_meta(fail_error, auth.request_id.clone(), trace_id.clone())
                })?;
            return Err(HttpError::with_meta(
                error,
                auth.request_id.clone(),
                trace_id,
            ));
        }
    };

    let response_data = kill_switch_to_contract(receipt, false, &state)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let response_json = serde_json::to_string(&response_data).map_err(|error| {
        HttpError::with_meta(
            AppError::internal(
                "KILL_SWITCH_RESPONSE_SERIALIZE_FAILED",
                format!("failed to serialize kill switch response: {error}"),
            ),
            auth.request_id.clone(),
            trace_id.clone(),
        )
    })?;

    state
        .idempotency_store
        .complete(&idempotency_request, &response_json)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        response_data,
        auth.request_id,
        trace_id,
    )))
}

async fn release_kill_switch(
    Extension(auth): Extension<AuthContext>,
    Extension(idempotency_key): Extension<IdempotencyKey>,
    State(state): State<AppState>,
    Json(payload): Json<ReleaseKillSwitchRequest>,
) -> std::result::Result<Json<ApiResponse<KillSwitchData>>, HttpError> {
    auth.ensure_scope(
        StepUpScope::SystemKillSwitchRelease,
        time::OffsetDateTime::now_utc(),
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), new_trace_id()))?;

    let trace_id = new_trace_id();
    let request_hash = hash_json(&payload)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let idempotency_request = IdempotencyRequest {
        scope: "system.kill_switch.release".to_string(),
        idempotency_key: idempotency_key.0,
        request_hash,
        request_id: auth.request_id.clone(),
        actor_user_id: Some(auth.user_id.clone()),
        actor_session_id: Some(auth.session_id.clone()),
        resource_type: Some("risk_state".to_string()),
        resource_id: Some("global".to_string()),
    };

    match state
        .idempotency_store
        .begin(&idempotency_request)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?
    {
        IdempotencyBegin::Replay(response_json) => {
            let mut replayed: KillSwitchData =
                serde_json::from_str(&response_json).map_err(|error| {
                    HttpError::with_meta(
                        AppError::internal(
                            "KILL_SWITCH_REPLAY_DESERIALIZE_FAILED",
                            format!("failed to deserialize replayed kill switch response: {error}"),
                        ),
                        auth.request_id.clone(),
                        trace_id.clone(),
                    )
                })?;
            replayed.replayed = true;
            normalize_kill_switch_modes(&mut replayed);

            return Ok(Json(ApiResponse::new(replayed, auth.request_id, trace_id)));
        }
        IdempotencyBegin::Started => {}
    }

    let actor = AuthenticatedActor {
        user_id: auth.user_id.clone(),
        session_id: auth.session_id.clone(),
        roles: auth.roles.clone(),
        request_id: auth.request_id.clone(),
        ip: auth.ip.clone(),
        user_agent: auth.user_agent.clone(),
    };

    let receipt = match state
        .risk_service
        .release_kill_switch(ReleaseKillSwitchCommand {
            reason: payload.reason,
            to_mode: payload.to_mode,
            expected_version: payload.expected_version,
            request_id: auth.request_id.clone(),
            trace_id: trace_id.clone(),
            actor,
        })
        .await
    {
        Ok(receipt) => receipt,
        Err(error) => {
            state
                .idempotency_store
                .fail(&idempotency_request, error.code())
                .await
                .map_err(|fail_error| {
                    HttpError::with_meta(fail_error, auth.request_id.clone(), trace_id.clone())
                })?;
            return Err(HttpError::with_meta(
                error,
                auth.request_id.clone(),
                trace_id,
            ));
        }
    };

    let response_data = kill_switch_to_contract(receipt, false, &state)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let response_json = serde_json::to_string(&response_data).map_err(|error| {
        HttpError::with_meta(
            AppError::internal(
                "KILL_SWITCH_RESPONSE_SERIALIZE_FAILED",
                format!("failed to serialize kill switch response: {error}"),
            ),
            auth.request_id.clone(),
            trace_id.clone(),
        )
    })?;

    state
        .idempotency_store
        .complete(&idempotency_request, &response_json)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        response_data,
        auth.request_id,
        trace_id,
    )))
}

async fn transition_system_mode(
    Extension(auth): Extension<AuthContext>,
    Extension(idempotency_key): Extension<IdempotencyKey>,
    State(state): State<AppState>,
    Json(payload): Json<TransitionSystemModeRequest>,
) -> std::result::Result<Json<ApiResponse<SystemModeData>>, HttpError> {
    if payload.reason.trim().is_empty() {
        return Err(HttpError::with_meta(
            AppError::invalid_input("SYSTEM_MODE_REASON_REQUIRED", "reason must not be empty"),
            auth.request_id.clone(),
            new_trace_id(),
        ));
    }

    let trace_id = new_trace_id();
    let request_hash = hash_json(&payload)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let actor = AuthenticatedActor {
        user_id: auth.user_id.clone(),
        session_id: auth.session_id.clone(),
        roles: auth.roles.clone(),
        request_id: auth.request_id.clone(),
        ip: auth.ip.clone(),
        user_agent: auth.user_agent.clone(),
    };

    let receipt = state
        .system_mode_service
        .transition_mode(ModeTransitionCommand {
            to_mode: payload.to_mode,
            reason: payload.reason,
            request_id: auth.request_id.clone(),
            trace_id: trace_id.clone(),
            idempotency_key: idempotency_key.0,
            request_hash,
            actor,
            required_scope: StepUpScope::SystemModeSwitch,
        })
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        SystemModeData {
            mode: console_runtime_mode(receipt.snapshot.mode),
            environment: receipt.snapshot.environment,
            version: receipt.snapshot.version,
            replayed: receipt.replayed,
            updated_at: receipt.snapshot.updated_at,
        },
        auth.request_id,
        trace_id,
    )))
}

fn market_to_contract(market: MarketView) -> MarketData {
    MarketData {
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
        polymarket_condition_id: market.polymarket_condition_id,
        polymarket_yes_asset_id: market.polymarket_yes_asset_id,
        polymarket_no_asset_id: market.polymarket_no_asset_id,
        updated_at: market.updated_at,
        version: market.version,
    }
}

fn event_to_contract(event: EventView) -> EventData {
    EventData {
        id: event.id,
        source: event.source,
        summary: event.summary,
        relevance_score: event.relevance_score,
        confidence: event.confidence,
        status: event.status,
        related_market_ids: event.related_market_ids,
        reason_trace: event.reason_trace,
        created_at: event.created_at,
        updated_at: event.updated_at,
        version: event.version,
    }
}

fn news_source_health_to_contract(source: NewsSourceHealthView) -> NewsSourceHealthData {
    NewsSourceHealthData {
        source: source.source,
        source_type: source.source_type,
        enabled: source.enabled,
        reliability: source.reliability,
        last_success_at: source.last_success_at,
        last_error_at: source.last_error_at,
        consecutive_failures: source.consecutive_failures,
        items_fetched: source.items_fetched,
        items_inserted: source.items_inserted,
        items_deduped: source.items_deduped,
        health_score: source.health_score,
        last_error: source.last_error,
        updated_at: source.updated_at,
    }
}

fn news_raw_event_to_contract(event: NewsRawEventView) -> NewsRawEventData {
    NewsRawEventData {
        id: event.id,
        source: event.source,
        source_type: event.source_type,
        external_id: event.external_id,
        title: event.title,
        url: event.url,
        author: event.author,
        published_at: event.published_at,
        event_time: event.event_time,
        hash: event.hash,
        raw_payload: event.raw_payload,
        ingested_at: event.ingested_at,
        trace_id: event.trace_id,
    }
}

fn evidence_to_contract(evidence: EvidenceView) -> EvidenceData {
    EvidenceData {
        id: evidence.id,
        market_id: evidence.market_id,
        event_id: evidence.event_id,
        direction: evidence.direction,
        strength: evidence.strength,
        source_reliability: evidence.source_reliability,
        novelty: evidence.novelty,
        resolution_relevance: evidence.resolution_relevance,
        status: evidence.status,
        expires_at: evidence.expires_at,
        created_at: evidence.created_at,
        updated_at: evidence.updated_at,
        version: evidence.version,
    }
}

fn signal_to_contract(signal: SignalView) -> SignalData {
    SignalData {
        id: signal.id,
        market_id: signal.market_id,
        event_id: signal.event_id,
        action: signal.action,
        side: signal.side,
        market_price: signal.market_price,
        fair_price: signal.fair_price,
        edge: signal.edge,
        confidence: signal.confidence,
        lifecycle_state: signal.lifecycle_state,
        reason: signal.reason,
        risk_decision: signal.risk_decision,
        evidence_ids: signal.evidence_ids,
        approved_by_user_id: signal.approved_by_user_id,
        approved_at: signal.approved_at,
        rejected_by_user_id: signal.rejected_by_user_id,
        rejected_at: signal.rejected_at,
        updated_at: signal.updated_at,
        version: signal.version,
    }
}

fn order_draft_to_contract(order_draft: OrderDraftView) -> OrderDraftData {
    OrderDraftData {
        id: order_draft.id,
        signal_id: order_draft.signal_id,
        signal_version: order_draft.signal_version,
        market_id: order_draft.market_id,
        connector_name: order_draft.connector_name,
        side: order_draft.side,
        limit_price: order_draft.limit_price,
        quantity: order_draft.quantity,
        notional: order_draft.notional,
        status: order_draft.status,
        created_by_user_id: order_draft.created_by_user_id,
        created_at: order_draft.created_at,
        external_order_id: order_draft.external_order_id,
        submitted_at: order_draft.submitted_at,
        failure_code: order_draft.failure_code,
        failure_message: order_draft.failure_message,
        updated_at: order_draft.updated_at,
        version: order_draft.version,
    }
}

fn execution_request_to_contract(execution_request: ExecutionRequestView) -> ExecutionRequestData {
    ExecutionRequestData {
        id: execution_request.id,
        signal_id: execution_request.signal_id,
        signal_version: execution_request.signal_version,
        order_draft_id: execution_request.order_draft_id,
        connector_name: execution_request.connector_name,
        mode: console_runtime_mode(execution_request.mode),
        requested_by_user_id: execution_request.requested_by_user_id,
        status: execution_request.status,
        reason: execution_request.reason,
        created_at: execution_request.created_at,
        external_order_id: execution_request.external_order_id,
        submitted_at: execution_request.submitted_at,
        failure_code: execution_request.failure_code,
        failure_message: execution_request.failure_message,
        updated_at: execution_request.updated_at,
        version: execution_request.version,
    }
}

fn order_to_contract(order: OrderView) -> OrderData {
    OrderData {
        id: order.id,
        signal_id: order.signal_id,
        execution_request_id: order.execution_request_id,
        order_draft_id: order.order_draft_id,
        market_id: order.market_id,
        connector_name: order.connector_name,
        account_id: order.account_id,
        external_order_id: order.external_order_id,
        side: order.side,
        limit_price: order.limit_price,
        quantity: order.quantity,
        filled_quantity: order.filled_quantity,
        avg_fill_price: order.avg_fill_price,
        status: order.status,
        submitted_at: order.submitted_at,
        updated_at: order.updated_at,
        version: order.version,
    }
}

fn trade_to_contract(trade: TradeView) -> TradeData {
    TradeData {
        id: trade.id,
        order_id: trade.order_id,
        signal_id: trade.signal_id,
        market_id: trade.market_id,
        connector_name: trade.connector_name,
        external_trade_id: trade.external_trade_id,
        side: trade.side,
        price: trade.price,
        quantity: trade.quantity,
        fee: trade.fee,
        executed_at: trade.executed_at,
    }
}

fn position_to_contract(position: PositionView) -> PositionData {
    PositionData {
        id: position.id,
        market_id: position.market_id,
        connector_name: position.connector_name,
        account_id: position.account_id,
        side: position.side,
        net_quantity: position.net_quantity,
        avg_cost: position.avg_cost,
        mark_price: position.mark_price,
        unrealized_pnl: position.unrealized_pnl,
        realized_pnl: position.realized_pnl,
        updated_at: position.updated_at,
        version: position.version,
    }
}

fn risk_state_to_contract(
    risk_state: RiskStateView,
    environment: String,
    policy: &RiskPolicy,
    open_alerts_override: Option<u32>,
) -> polyedge_domain::Result<RiskStateData> {
    Ok(RiskStateData {
        id: "risk_state_global".to_string(),
        mode: console_runtime_mode(risk_state.mode),
        environment,
        kill_switch: risk_state.kill_switch,
        daily_pnl: risk_state.daily_pnl,
        gross_exposure: risk_state.gross_exposure,
        net_exposure: risk_state.net_exposure,
        open_alerts: open_alerts_override.unwrap_or(risk_state.open_alerts),
        daily_loss_limit: policy.max_daily_loss,
        daily_loss_used: daily_loss_used(&risk_state)?,
        updated_at: risk_state.updated_at,
        version: risk_state.version,
    })
}

fn probability_estimate_to_contract(estimate: ProbabilityEstimateView) -> ProbabilityEstimateData {
    ProbabilityEstimateData {
        id: estimate.id,
        market_id: estimate.market_id,
        event_id: estimate.event_id,
        signal_id: estimate.signal_id,
        prior_price: estimate.prior_price,
        posterior_price: estimate.posterior_price,
        fair_price: estimate.fair_price,
        market_price: estimate.market_price,
        edge: estimate.edge,
        confidence: estimate.confidence,
        time_horizon: estimate.time_horizon,
        model_version: estimate.model_version,
        reason_codes: estimate.reason_codes,
        evidence_count: estimate.evidence_count,
        created_at: estimate.created_at,
    }
}

fn arbitrage_scan_to_contract(scan: ArbitrageScanView) -> ArbitrageScanData {
    ArbitrageScanData {
        id: scan.id,
        started_at: scan.started_at,
        finished_at: scan.finished_at,
        market_count: scan.market_count,
        snapshot_count: scan.snapshot_count,
        opportunity_count: scan.opportunity_count,
        scanner_version: scan.scanner_version,
        metadata: scan.metadata,
        trace_id: scan.trace_id,
    }
}

fn arbitrage_opportunity_to_contract(
    opportunity: ArbitrageOpportunityView,
) -> ArbitrageOpportunityData {
    ArbitrageOpportunityData {
        id: opportunity.id,
        scan_id: opportunity.scan_id,
        market_id: opportunity.market_id,
        opportunity_type: opportunity.opportunity_type.as_str().to_string(),
        status: opportunity.status.as_str().to_string(),
        gross_edge: opportunity.gross_edge,
        price_sum: opportunity.price_sum.to_string(),
        capacity: opportunity.capacity,
        yes_price: opportunity.yes_price,
        no_price: opportunity.no_price,
        yes_size: opportunity.yes_size,
        no_size: opportunity.no_size,
        observed_at: opportunity.observed_at,
        reason_codes: opportunity.reason_codes,
        analysis_payload: opportunity.analysis_payload,
        trace_id: opportunity.trace_id,
        validation: opportunity.validation.map(arbitrage_validation_to_contract),
    }
}

fn arbitrage_validation_to_contract(
    validation: ArbitrageOpportunityValidationView,
) -> ArbitrageOpportunityValidationData {
    ArbitrageOpportunityValidationData {
        id: validation.id,
        opportunity_id: validation.opportunity_id,
        status: validation.status.as_str().to_string(),
        gross_edge: validation.gross_edge,
        net_edge: validation.net_edge,
        fee_estimate: validation.fee_estimate,
        slippage_buffer: validation.slippage_buffer,
        validated_capacity: validation.validated_capacity,
        book_age_ms: validation.book_age_ms,
        reason_codes: validation.reason_codes,
        validation_payload: validation.validation_payload,
        validated_at: validation.validated_at,
        trace_id: validation.trace_id,
    }
}

fn arbitrage_analysis_run_to_contract(
    analysis: ArbitrageAnalysisRunView,
) -> ArbitrageAnalysisRunData {
    ArbitrageAnalysisRunData {
        id: analysis.id,
        generated_at: analysis.generated_at,
        lookback_hours: analysis.lookback_hours,
        opportunity_count: analysis.opportunity_count,
        market_count: analysis.market_count,
        summary_payload: analysis.summary_payload,
        trace_id: analysis.trace_id,
    }
}

fn signal_transition_to_contract(transition: SignalTransitionView) -> SignalTransitionData {
    SignalTransitionData {
        id: transition.id,
        signal_id: transition.signal_id,
        from_state: transition.from_state,
        to_state: transition.to_state,
        trigger_type: transition.trigger_type,
        trigger_payload: transition.trigger_payload,
        created_at: transition.created_at,
    }
}

fn recompute_signal_to_contract(
    result: polyedge_application::RecomputeSignalResult,
    replayed: bool,
) -> RecomputeSignalData {
    RecomputeSignalData {
        signal: signal_to_contract(result.signal),
        estimate: probability_estimate_to_contract(result.estimate),
        transition: result.transition.map(signal_transition_to_contract),
        replayed,
    }
}

fn risk_state_to_contract_for_state(
    state: &AppState,
    risk_state: RiskStateView,
) -> polyedge_domain::Result<RiskStateData> {
    risk_state_to_contract(
        risk_state,
        state.settings.runtime.environment.clone(),
        state.risk_service.policy(),
        None,
    )
}

fn execution_submission_to_contract(
    receipt: ExecutionSubmissionReceipt,
    replayed: bool,
    state: &AppState,
) -> polyedge_domain::Result<SubmitExecutionData> {
    Ok(SubmitExecutionData {
        order_draft: order_draft_to_contract(receipt.order_draft),
        execution_request: execution_request_to_contract(receipt.execution_request),
        risk_state: risk_state_to_contract_for_state(state, receipt.risk_state)?,
        replayed,
    })
}

fn kill_switch_to_contract(
    receipt: KillSwitchReceipt,
    replayed: bool,
    state: &AppState,
) -> polyedge_domain::Result<KillSwitchData> {
    Ok(KillSwitchData {
        risk_state: risk_state_to_contract_for_state(state, receipt.risk_state)?,
        replayed,
    })
}

fn connector_order_status_to_contract(
    order: OrderView,
    replayed: bool,
) -> ConnectorOrderStatusCallbackData {
    ConnectorOrderStatusCallbackData {
        order: order_to_contract(order),
        replayed,
    }
}

fn connector_trade_fill_to_contract(
    result: ExecutionFillResult,
    risk_state: RiskStateView,
    replayed: bool,
    state: &AppState,
) -> polyedge_domain::Result<ConnectorTradeFillCallbackData> {
    Ok(ConnectorTradeFillCallbackData {
        order: order_to_contract(result.order),
        trade: trade_to_contract(result.trade),
        position: position_to_contract(result.position),
        risk_state: risk_state_to_contract_for_state(state, risk_state)?,
        replayed,
    })
}

fn authenticated_actor(auth: &AuthContext) -> AuthenticatedActor {
    AuthenticatedActor {
        user_id: auth.user_id.clone(),
        session_id: auth.session_id.clone(),
        roles: auth.roles.clone(),
        request_id: auth.request_id.clone(),
        ip: auth.ip.clone(),
        user_agent: auth.user_agent.clone(),
    }
}

fn normalize_callback_connector_name(connector_name: &str) -> polyedge_domain::Result<String> {
    let normalized = connector_name.trim().to_ascii_lowercase();

    if normalized.is_empty() {
        return Err(AppError::invalid_input(
            "CONNECTOR_NAME_REQUIRED",
            "connector_name must not be empty",
        ));
    }

    Ok(normalized)
}

fn validate_callback_event_id(event_id: &str) -> polyedge_domain::Result<String> {
    let normalized = event_id.trim().to_string();

    if normalized.is_empty() {
        return Err(AppError::invalid_input(
            "EXTERNAL_EVENT_ID_REQUIRED",
            "event_id must not be empty",
        ));
    }

    Ok(normalized)
}

fn callback_source(prefix: &str, connector_name: &str) -> String {
    format!("{prefix}.{connector_name}")
}

async fn build_trade_fill_callback_response(
    state: &AppState,
    connector_name: &str,
    external_order_id: &str,
    account_id: &str,
    external_trade_id: &str,
    replayed: bool,
) -> polyedge_domain::Result<ConnectorTradeFillCallbackData> {
    let order = state
        .market_event_service
        .get_order_by_external_ref(connector_name.to_string(), external_order_id.to_string())
        .await?;
    let trades = state
        .execution_service
        .list_trades(TradeListFilters::new(
            Some(order.id.clone()),
            Some(order.signal_id.clone()),
            Some(order.market_id.clone()),
            Some(order.connector_name.clone()),
            Some(100),
        )?)
        .await?;
    let trade = trades
        .into_iter()
        .find(|trade| trade.external_trade_id == external_trade_id)
        .ok_or_else(|| {
            AppError::not_found(
                "EXTERNAL_TRADE_NOT_FOUND",
                "external trade callback replay could not find a matching trade",
            )
        })?;
    let positions = state
        .execution_service
        .list_positions(PositionListFilters::new(
            Some(order.market_id.clone()),
            Some(order.connector_name.clone()),
            Some(order.side),
            Some(100),
        )?)
        .await?;
    let position = positions
        .into_iter()
        .find(|position| position.account_id == account_id)
        .ok_or_else(|| {
            AppError::not_found(
                "POSITION_NOT_FOUND",
                "external trade callback replay could not find a matching position",
            )
        })?;
    let risk_state = state.risk_service.read_state().await?;

    Ok(ConnectorTradeFillCallbackData {
        order: order_to_contract(order),
        trade: trade_to_contract(trade),
        position: position_to_contract(position),
        risk_state: risk_state_to_contract_for_state(state, risk_state)?,
        replayed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Router,
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use base64::{Engine, engine::general_purpose};
    use ed25519_dalek::{Signer, SigningKey};
    use polyedge_application::{
        ArbitrageAnalysisRunView, ArbitrageScanView, ArbitrageValidationConfig, AuthenticatedActor,
        MarkExecutionSubmittedCommand, MarketBookSnapshotView, NewsIngestSourceCommand,
        NewsIngestionItem, build_arbitrage_analysis, demo_fixture_bundle,
    };
    use polyedge_domain::{Edge, Probability, Quantity, StepUpScope, SystemMode, UserRole};
    use polyedge_infrastructure::{AppState, AuthKeySettings, Runtime, Settings};
    use serde::Serialize;
    use tower::util::ServiceExt;
    use uuid::Uuid;

    #[derive(Serialize)]
    struct TestHeader<'a> {
        alg: &'a str,
        kid: &'a str,
        typ: &'a str,
    }

    #[derive(Serialize)]
    struct TestClaims {
        iss: String,
        aud: String,
        sub: String,
        iat: i64,
        nbf: i64,
        exp: i64,
        jti: String,
        session_id: String,
        roles: Vec<UserRole>,
        auth_time: i64,
        request_id: String,
        step_up_verified: bool,
        step_up_scope: Vec<polyedge_domain::StepUpScope>,
        step_up_until: Option<i64>,
    }

    #[test]
    fn sse_message_filter_keeps_new_resource_versions_only() {
        let mut emitted_ids = HashSet::new();
        let mut emitted_id_order = VecDeque::new();
        let first_batch = filter_new_sse_messages(
            vec![
                SseMessage {
                    id: "signals:sig_1:1".to_string(),
                    event: "signal.updated",
                    data: json!({ "signal_id": "sig_1", "version": 1 }),
                },
                SseMessage {
                    id: "signals:sig_1:1".to_string(),
                    event: "signal.updated",
                    data: json!({ "signal_id": "sig_1", "version": 1 }),
                },
                SseMessage {
                    id: "signals:sig_2:1".to_string(),
                    event: "signal.created",
                    data: json!({ "signal_id": "sig_2", "version": 1 }),
                },
            ],
            &mut emitted_ids,
            &mut emitted_id_order,
        );

        assert_eq!(
            first_batch
                .iter()
                .map(|message| message.id.as_str())
                .collect::<Vec<_>>(),
            vec!["signals:sig_1:1", "signals:sig_2:1"]
        );

        let second_batch = filter_new_sse_messages(
            vec![
                SseMessage {
                    id: "signals:sig_1:1".to_string(),
                    event: "signal.updated",
                    data: json!({ "signal_id": "sig_1", "version": 1 }),
                },
                SseMessage {
                    id: "signals:sig_1:2".to_string(),
                    event: "signal.updated",
                    data: json!({ "signal_id": "sig_1", "version": 2 }),
                },
            ],
            &mut emitted_ids,
            &mut emitted_id_order,
        );

        assert_eq!(
            second_batch
                .iter()
                .map(|message| message.id.as_str())
                .collect::<Vec<_>>(),
            vec!["signals:sig_1:2"]
        );
    }

    #[test]
    fn sse_message_filter_bounds_seen_id_cache() {
        let mut emitted_ids = HashSet::new();
        let mut emitted_id_order = VecDeque::new();
        let messages = (0..=MAX_STREAM_EMITTED_IDS)
            .map(|index| SseMessage {
                id: format!("signals:sig_{index}:1"),
                event: "signal.updated",
                data: json!({ "signal_id": format!("sig_{index}"), "version": 1 }),
            })
            .collect::<Vec<_>>();

        let filtered = filter_new_sse_messages(messages, &mut emitted_ids, &mut emitted_id_order);

        assert_eq!(filtered.len(), MAX_STREAM_EMITTED_IDS + 1);
        assert_eq!(emitted_ids.len(), MAX_STREAM_EMITTED_IDS);
        assert!(!emitted_ids.contains("signals:sig_0:1"));
        assert!(emitted_ids.contains(&format!("signals:sig_{MAX_STREAM_EMITTED_IDS}:1")));
    }

    fn issue_token_with(
        signing_key: &SigningKey,
        kid: &str,
        request_id: &str,
        roles: Vec<UserRole>,
        step_up_scope: Vec<StepUpScope>,
    ) -> String {
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let header = serde_json::to_vec(&TestHeader {
            alg: "EdDSA",
            kid,
            typ: "JWT",
        })
        .expect("serialize header");
        let claims = serde_json::to_vec(&TestClaims {
            iss: "polyedge-nextjs".to_string(),
            aud: "polyedge-rust-api".to_string(),
            sub: "usr_123".to_string(),
            iat: now,
            nbf: now,
            exp: now + 20,
            jti: format!("jit_{}", Uuid::now_v7()),
            session_id: "sess_123".to_string(),
            roles,
            auth_time: now - 30,
            request_id: request_id.to_string(),
            step_up_verified: true,
            step_up_scope,
            step_up_until: Some(now + 120),
        })
        .expect("serialize claims");
        let header_b64 = general_purpose::URL_SAFE_NO_PAD.encode(header);
        let claims_b64 = general_purpose::URL_SAFE_NO_PAD.encode(claims);
        let message = format!("{header_b64}.{claims_b64}");
        let signature = signing_key.sign(message.as_bytes());
        let signature_b64 = general_purpose::URL_SAFE_NO_PAD.encode(signature.to_bytes());
        format!("{message}.{signature_b64}")
    }

    fn issue_token(signing_key: &SigningKey, kid: &str, request_id: &str) -> String {
        issue_token_with(
            signing_key,
            kid,
            request_id,
            vec![UserRole::RiskAdmin],
            vec![StepUpScope::SystemModeSwitch],
        )
    }

    fn test_actor(request_id: &str) -> AuthenticatedActor {
        AuthenticatedActor {
            user_id: "usr_123".to_string(),
            session_id: "sess_123".to_string(),
            roles: vec![UserRole::RiskAdmin],
            request_id: request_id.to_string(),
            ip: None,
            user_agent: Some("api-tests".to_string()),
        }
    }

    async fn submit_execution_for_test(
        app: Router,
        signing_key: &SigningKey,
        signal_id: &str,
        connector_name: &str,
    ) -> SubmitExecutionData {
        let submit_request_id = format!("req_{}", Uuid::now_v7());
        let submit_token = issue_token_with(
            signing_key,
            "test-key",
            &submit_request_id,
            vec![UserRole::RiskAdmin],
            vec![StepUpScope::ExecutionSubmit],
        );
        let submit_body = serde_json::to_vec(&serde_json::json!({
            "limit_price": "0.48",
            "quantity": "25",
            "reason": "queue manual execution request for connector callback flow",
            "expected_signal_version": 9,
            "connector_name": connector_name
        }))
        .expect("serialize execution body");
        let submit_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/signals/{signal_id}/execution-requests"))
                    .header("Authorization", format!("Bearer {submit_token}"))
                    .header("X-Request-Id", &submit_request_id)
                    .header("Idempotency-Key", format!("idem-submit-{signal_id}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(submit_body))
                    .expect("submit request"),
            )
            .await
            .expect("submit response");
        assert_eq!(submit_response.status(), StatusCode::OK);
        let submit_response_body = to_bytes(submit_response.into_body(), usize::MAX)
            .await
            .expect("read submit body");
        let submit_payload: ApiResponse<SubmitExecutionData> =
            serde_json::from_slice(&submit_response_body).expect("deserialize submit response");

        submit_payload.data
    }

    async fn dispatch_execution(
        state: &AppState,
        execution_request_id: &str,
        account_id: &str,
        external_order_id: &str,
    ) {
        let request_id = format!("req_dispatch_{}", Uuid::now_v7());
        state
            .execution_service
            .mark_execution_submitted(MarkExecutionSubmittedCommand {
                execution_request_id: execution_request_id.to_string(),
                account_id: account_id.to_string(),
                external_order_id: external_order_id.to_string(),
                request_id: request_id.clone(),
                trace_id: format!("trc_{}", Uuid::now_v7()),
                actor: test_actor(&request_id),
            })
            .await
            .expect("dispatch execution");
    }

    #[tokio::test]
    async fn healthz_is_available_without_authentication() {
        let signing_key = SigningKey::from_bytes(&[11_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let app = build_app(Runtime::test_app_state(settings).expect("state"));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn protected_read_route_requires_valid_token() {
        let signing_key = SigningKey::from_bytes(&[11_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let app = build_app(Runtime::test_app_state(settings).expect("state"));
        let request_id = "req_test_1";
        let token = issue_token(&signing_key, "test-key", request_id);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/system/mode")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn markets_route_returns_seeded_market_list() {
        let signing_key = SigningKey::from_bytes(&[12_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_markets")
            .await
            .expect("seed fixtures");
        let app = build_app(state);
        let request_id = format!("req_{}", Uuid::now_v7());
        let token = issue_token(&signing_key, "test-key", &request_id);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/markets?tradability_status=manual_review")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let payload: ApiResponse<Vec<MarketData>> =
            serde_json::from_slice(&body).expect("deserialize response");
        assert_eq!(payload.data.len(), 1);
        assert_eq!(payload.data[0].id, "mkt_121");
    }

    #[tokio::test]
    async fn arbitrage_routes_return_recorded_opportunities() {
        let signing_key = SigningKey::from_bytes(&[42_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        let started_at = OffsetDateTime::now_utc();
        let scan_id = "scan_api_test_arbitrage";
        state
            .arbitrage_service
            .start_scan(ArbitrageScanView {
                id: scan_id.to_string(),
                started_at,
                finished_at: None,
                market_count: 0,
                snapshot_count: 0,
                opportunity_count: 0,
                scanner_version: "api-test".to_string(),
                metadata: json!({ "mode": "test" }),
                trace_id: "trc_api_test_arbitrage".to_string(),
            })
            .await
            .expect("start scan");
        let observed_at = started_at + time::Duration::seconds(1);
        let snapshot = MarketBookSnapshotView {
            id: "book_api_test_mkt_120".to_string(),
            scan_id: scan_id.to_string(),
            connector_name: "polymarket".to_string(),
            market_id: "mkt_120".to_string(),
            yes_asset_id: Some("asset_yes".to_string()),
            no_asset_id: Some("asset_no".to_string()),
            yes_bid: None,
            yes_ask: Some(Probability::new(Decimal::new(45, 2)).expect("yes ask")),
            yes_bid_size: Quantity::new(Decimal::ZERO).expect("zero yes bid size"),
            yes_ask_size: Quantity::new(Decimal::new(900, 0)).expect("yes ask size"),
            no_bid: None,
            no_ask: Some(Probability::new(Decimal::new(51, 2)).expect("no ask")),
            no_bid_size: Quantity::new(Decimal::ZERO).expect("zero no bid size"),
            no_ask_size: Quantity::new(Decimal::new(850, 0)).expect("no ask size"),
            observed_at,
            raw_payload: json!({ "fixture": true }),
            trace_id: "trc_api_test_arbitrage".to_string(),
        };
        let opportunities = state
            .arbitrage_service
            .record_snapshot_and_detect(snapshot.clone())
            .await
            .expect("record snapshot and detect");
        state
            .arbitrage_service
            .validate_opportunity(
                &opportunities[0],
                &snapshot,
                &ArbitrageValidationConfig {
                    max_book_age_ms: 10_000,
                    min_gross_edge: Edge::new(Decimal::new(5, 3)).expect("min edge"),
                    min_capacity: Quantity::new(Decimal::ONE).expect("min capacity"),
                    fee_buffer: Edge::new(Decimal::new(5, 3)).expect("fee buffer"),
                    slippage_buffer: Edge::new(Decimal::new(5, 3)).expect("slippage buffer"),
                },
                observed_at + time::Duration::milliseconds(50),
            )
            .await
            .expect("validate opportunity");
        let unvalidated_snapshot = MarketBookSnapshotView {
            id: "book_api_test_mkt_121".to_string(),
            scan_id: scan_id.to_string(),
            connector_name: "polymarket".to_string(),
            market_id: "mkt_121".to_string(),
            yes_asset_id: Some("asset_yes_121".to_string()),
            no_asset_id: Some("asset_no_121".to_string()),
            yes_bid: None,
            yes_ask: Some(Probability::new(Decimal::new(46, 2)).expect("yes ask")),
            yes_bid_size: Quantity::new(Decimal::ZERO).expect("zero yes bid size"),
            yes_ask_size: Quantity::new(Decimal::new(500, 0)).expect("yes ask size"),
            no_bid: None,
            no_ask: Some(Probability::new(Decimal::new(50, 2)).expect("no ask")),
            no_bid_size: Quantity::new(Decimal::ZERO).expect("zero no bid size"),
            no_ask_size: Quantity::new(Decimal::new(450, 0)).expect("no ask size"),
            observed_at: observed_at + time::Duration::milliseconds(100),
            raw_payload: json!({ "fixture": "unvalidated" }),
            trace_id: "trc_api_test_arbitrage".to_string(),
        };
        let unvalidated_opportunities = state
            .arbitrage_service
            .record_snapshot_and_detect(unvalidated_snapshot)
            .await
            .expect("record unvalidated snapshot and detect");
        assert_eq!(unvalidated_opportunities.len(), 1);
        state
            .arbitrage_service
            .complete_scan(scan_id, started_at + time::Duration::seconds(2), 2, 2, 2)
            .await
            .expect("complete scan");
        let summary =
            build_arbitrage_analysis(&opportunities, 24, started_at + time::Duration::seconds(3));
        state
            .arbitrage_service
            .record_analysis_run(ArbitrageAnalysisRunView {
                id: "arb_analysis_api_test".to_string(),
                generated_at: summary.generated_at,
                lookback_hours: summary.lookback_hours,
                opportunity_count: summary.opportunity_count,
                market_count: summary.market_count,
                summary_payload: serde_json::to_value(&summary).expect("serialize summary"),
                trace_id: "trc_api_test_arbitrage_analysis".to_string(),
            })
            .await
            .expect("record analysis");

        let mut emitted_ids = HashSet::new();
        let mut emitted_id_order = VecDeque::new();
        let mut last_arbitrage_sequence = None;
        let stream_chunk = build_stream_chunk(
            &state,
            "arbitrage",
            0,
            &mut emitted_ids,
            &mut emitted_id_order,
            &mut last_arbitrage_sequence,
        )
        .await
        .expect("build arbitrage stream");
        assert!(stream_chunk.contains("event: arbitrage.scan.started"));
        assert!(stream_chunk.contains("event: arbitrage.validation.passed"));
        assert!(last_arbitrage_sequence.is_some());

        let mut resumed_ids = HashSet::new();
        let mut resumed_id_order = VecDeque::new();
        let mut resumed_sequence = Some(1);
        let resumed_stream_chunk = build_stream_chunk(
            &state,
            "arbitrage",
            0,
            &mut resumed_ids,
            &mut resumed_id_order,
            &mut resumed_sequence,
        )
        .await
        .expect("build resumed arbitrage stream");
        assert!(!resumed_stream_chunk.contains("event: arbitrage.scan.started"));
        assert!(resumed_stream_chunk.contains("event: arbitrage.opportunity.observed"));

        let second_stream_chunk = build_stream_chunk(
            &state,
            "arbitrage",
            1,
            &mut emitted_ids,
            &mut emitted_id_order,
            &mut last_arbitrage_sequence,
        )
        .await
        .expect("build arbitrage stream heartbeat");
        assert!(second_stream_chunk.contains("polyedge arbitrage stream heartbeat"));

        let app = build_app(state);
        let request_id = format!("req_{}", Uuid::now_v7());
        let token = issue_token(&signing_key, "test-key", &request_id);

        let opportunities_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/arbitrage/opportunities?market_id=mkt_120&opportunity_type=binary_buy_both&validation_status=valid&active_only=true&min_net_edge=0.01")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("opportunities response");

        assert_eq!(opportunities_response.status(), StatusCode::OK);
        let opportunities_body = to_bytes(opportunities_response.into_body(), usize::MAX)
            .await
            .expect("read opportunities body");
        let opportunities_payload: ApiResponse<Vec<ArbitrageOpportunityData>> =
            serde_json::from_slice(&opportunities_body).expect("deserialize opportunities");
        assert_eq!(opportunities_payload.data.len(), 1);
        assert_eq!(
            opportunities_payload.data[0].opportunity_type,
            "binary_buy_both"
        );
        assert_eq!(opportunities_payload.data[0].status, "observed");
        assert_eq!(opportunities_payload.data[0].price_sum, "0.96");
        assert_eq!(
            opportunities_payload.data[0].reason_codes,
            vec!["yes_ask_plus_no_ask_below_one"]
        );
        let validation = opportunities_payload.data[0]
            .validation
            .as_ref()
            .expect("validation");
        assert_eq!(validation.status, "valid");
        assert_eq!(validation.book_age_ms, 50);

        let high_edge_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/arbitrage/opportunities?market_id=mkt_120&validation_status=valid&active_only=true&min_net_edge=0.035")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("high edge opportunities response");

        assert_eq!(high_edge_response.status(), StatusCode::OK);
        let high_edge_body = to_bytes(high_edge_response.into_body(), usize::MAX)
            .await
            .expect("read high edge opportunities body");
        let high_edge_payload: ApiResponse<Vec<ArbitrageOpportunityData>> =
            serde_json::from_slice(&high_edge_body).expect("deserialize high edge opportunities");
        assert!(high_edge_payload.data.is_empty());

        let unvalidated_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/arbitrage/opportunities?market_id=mkt_121&validation_status=unvalidated")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("unvalidated opportunities response");

        assert_eq!(unvalidated_response.status(), StatusCode::OK);
        let unvalidated_body = to_bytes(unvalidated_response.into_body(), usize::MAX)
            .await
            .expect("read unvalidated opportunities body");
        let unvalidated_payload: ApiResponse<Vec<ArbitrageOpportunityData>> =
            serde_json::from_slice(&unvalidated_body)
                .expect("deserialize unvalidated opportunities");
        assert_eq!(unvalidated_payload.data.len(), 1);
        assert_eq!(unvalidated_payload.data[0].market_id, "mkt_121");
        assert!(unvalidated_payload.data[0].validation.is_none());

        let scans_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/arbitrage/scans?limit=1")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("scans response");

        assert_eq!(scans_response.status(), StatusCode::OK);
        let scans_body = to_bytes(scans_response.into_body(), usize::MAX)
            .await
            .expect("read scans body");
        let scans_payload: ApiResponse<Vec<ArbitrageScanData>> =
            serde_json::from_slice(&scans_body).expect("deserialize scans");
        assert_eq!(scans_payload.data[0].id, scan_id);
        assert_eq!(scans_payload.data[0].opportunity_count, 2);

        let analysis_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/arbitrage/analysis?limit=1")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("analysis response");

        assert_eq!(analysis_response.status(), StatusCode::OK);
        let analysis_body = to_bytes(analysis_response.into_body(), usize::MAX)
            .await
            .expect("read analysis body");
        let analysis_payload: ApiResponse<Vec<ArbitrageAnalysisRunData>> =
            serde_json::from_slice(&analysis_body).expect("deserialize analysis");
        assert_eq!(analysis_payload.data[0].opportunity_count, 1);
        assert_eq!(analysis_payload.data[0].market_count, 1);
    }

    #[tokio::test]
    async fn events_route_filters_by_status() {
        let signing_key = SigningKey::from_bytes(&[13_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_events")
            .await
            .expect("seed fixtures");
        let app = build_app(state);
        let request_id = format!("req_{}", Uuid::now_v7());
        let token = issue_token(&signing_key, "test-key", &request_id);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/events?status=active")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let payload: ApiResponse<Vec<EventData>> =
            serde_json::from_slice(&body).expect("deserialize response");
        assert_eq!(payload.data.len(), 1);
        assert_eq!(payload.data[0].id, "evt_9001");
    }

    #[tokio::test]
    async fn news_source_health_route_filters_by_source_type() {
        let signing_key = SigningKey::from_bytes(&[17_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        let reliability = Probability::new(Decimal::new(95, 2)).expect("probability");
        state
            .news_ingestion_service
            .ingest_source_items(NewsIngestSourceCommand {
                source: "sec_feed".to_string(),
                source_type: "official".to_string(),
                reliability,
                items: vec![NewsIngestionItem {
                    source: "sec_feed".to_string(),
                    source_type: "official".to_string(),
                    external_id: Some("entry-1".to_string()),
                    title: "SEC publishes ETF calendar update".to_string(),
                    url: Some("https://example.com/sec-calendar".to_string()),
                    author: None,
                    published_at: None,
                    content_snippet: Some("Window narrowed".to_string()),
                    raw_payload: serde_json::json!({"id": "entry-1"}),
                }],
                trace_id: "trc_seed_news_health".to_string(),
            })
            .await
            .expect("seed official source health");
        state
            .news_ingestion_service
            .ingest_source_items(NewsIngestSourceCommand {
                source: "wire_feed".to_string(),
                source_type: "news".to_string(),
                reliability,
                items: vec![NewsIngestionItem {
                    source: "wire_feed".to_string(),
                    source_type: "news".to_string(),
                    external_id: Some("wire-1".to_string()),
                    title: "Wire reports crypto policy hearing".to_string(),
                    url: Some("https://example.com/wire-policy".to_string()),
                    author: None,
                    published_at: None,
                    content_snippet: Some("Hearing scheduled".to_string()),
                    raw_payload: serde_json::json!({"id": "wire-1"}),
                }],
                trace_id: "trc_seed_wire_health".to_string(),
            })
            .await
            .expect("seed news source health");

        let app = build_app(state);
        let request_id = format!("req_{}", Uuid::now_v7());
        let token = issue_token(&signing_key, "test-key", &request_id);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/news/source-health?source_type=official")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let payload: ApiResponse<Vec<NewsSourceHealthData>> =
            serde_json::from_slice(&body).expect("deserialize response");
        assert_eq!(payload.data.len(), 1);
        assert_eq!(payload.data[0].source, "sec_feed");
        assert_eq!(payload.data[0].source_type, "official");
        assert_eq!(payload.data[0].items_fetched, 1);
        assert_eq!(payload.data[0].items_inserted, 1);
        assert_eq!(payload.data[0].consecutive_failures, 0);

        let raw_request_id = format!("req_{}", Uuid::now_v7());
        let raw_token = issue_token(&signing_key, "test-key", &raw_request_id);
        let raw_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/news/raw-events?source_type=official")
                    .header("Authorization", format!("Bearer {raw_token}"))
                    .header("X-Request-Id", &raw_request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(raw_response.status(), StatusCode::OK);
        let raw_body = to_bytes(raw_response.into_body(), usize::MAX)
            .await
            .expect("read raw body");
        let raw_payload: ApiResponse<Vec<NewsRawEventData>> =
            serde_json::from_slice(&raw_body).expect("deserialize raw response");
        assert_eq!(raw_payload.data.len(), 1);
        assert_eq!(raw_payload.data[0].source, "sec_feed");
        assert_eq!(
            raw_payload.data[0].title,
            "SEC publishes ETF calendar update"
        );
        assert_eq!(raw_payload.data[0].external_id.as_deref(), Some("entry-1"));
    }

    #[tokio::test]
    async fn evidences_route_filters_by_market() {
        let signing_key = SigningKey::from_bytes(&[14_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_evidences")
            .await
            .expect("seed fixtures");
        let app = build_app(state);
        let request_id = format!("req_{}", Uuid::now_v7());
        let token = issue_token(&signing_key, "test-key", &request_id);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/evidences?market_id=mkt_121")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let payload: ApiResponse<Vec<EvidenceData>> =
            serde_json::from_slice(&body).expect("deserialize response");
        assert_eq!(payload.data.len(), 2);
        assert!(payload.data.iter().all(|item| item.market_id == "mkt_121"));
    }

    #[tokio::test]
    async fn signals_route_filters_by_lifecycle_state_alias() {
        let signing_key = SigningKey::from_bytes(&[15_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_signals")
            .await
            .expect("seed fixtures");
        let app = build_app(state);
        let request_id = format!("req_{}", Uuid::now_v7());
        let token = issue_token(&signing_key, "test-key", &request_id);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/signals?status=active")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let payload: ApiResponse<Vec<SignalData>> =
            serde_json::from_slice(&body).expect("deserialize response");
        assert_eq!(payload.data.len(), 1);
        assert_eq!(payload.data[0].id, "sig_2411");
    }

    #[tokio::test]
    async fn risk_state_route_returns_current_snapshot() {
        let signing_key = SigningKey::from_bytes(&[18_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let app = build_app(Runtime::test_app_state(settings).expect("state"));
        let request_id = format!("req_{}", Uuid::now_v7());
        let token = issue_token(&signing_key, "test-key", &request_id);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/risk/state")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let payload: ApiResponse<RiskStateData> =
            serde_json::from_slice(&body).expect("deserialize response");
        assert_eq!(payload.data.mode, SystemMode::PaperTrade);
        assert_eq!(payload.data.id, "risk_state_global");
        assert_eq!(payload.data.environment, "test");
        assert!(!payload.data.kill_switch);
        assert_eq!(payload.data.open_alerts, 0);
    }

    #[tokio::test]
    async fn console_risk_routes_return_derived_resources() {
        let signing_key = SigningKey::from_bytes(&[28_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_console_risk")
            .await
            .expect("seed fixtures");
        let app = build_app(state);

        let alerts_request_id = format!("req_{}", Uuid::now_v7());
        let alerts_token = issue_token(&signing_key, "test-key", &alerts_request_id);
        let alerts_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/risk/alerts")
                    .header("Authorization", format!("Bearer {alerts_token}"))
                    .header("X-Request-Id", &alerts_request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(alerts_response.status(), StatusCode::OK);
        let alerts_body = to_bytes(alerts_response.into_body(), usize::MAX)
            .await
            .expect("read alerts body");
        let alerts_payload: ApiResponse<Vec<RiskAlertData>> =
            serde_json::from_slice(&alerts_body).expect("deserialize alerts response");
        assert!(
            alerts_payload
                .data
                .iter()
                .all(|alert| alert.id != "alt_pending_signal_approvals")
        );
    }

    #[tokio::test]
    async fn submit_execution_request_requires_execution_submit_scope() {
        let signing_key = SigningKey::from_bytes(&[25_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_execution_scope")
            .await
            .expect("seed fixtures");
        let app = build_app(state);
        let request_id = format!("req_{}", Uuid::now_v7());
        let token = issue_token(&signing_key, "test-key", &request_id);
        let body = serde_json::to_vec(&serde_json::json!({
            "limit_price": "0.48",
            "quantity": "25",
            "reason": "scope check",
            "expected_signal_version": 9,
            "connector_name": "paper_executor"
        }))
        .expect("serialize body");

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/signals/sig_2412/execution-requests")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .header("Idempotency-Key", "idem-execution-scope")
                    .header("Content-Type", "application/json")
                    .body(Body::from(body))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn submit_execution_request_is_idempotent_and_lists_created_records() {
        let signing_key = SigningKey::from_bytes(&[26_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_execution")
            .await
            .expect("seed fixtures");
        let app = build_app(state);

        let submit_request_id = format!("req_{}", Uuid::now_v7());
        let submit_token = issue_token_with(
            &signing_key,
            "test-key",
            &submit_request_id,
            vec![UserRole::RiskAdmin],
            vec![StepUpScope::ExecutionSubmit],
        );
        let submit_body = serde_json::to_vec(&serde_json::json!({
            "limit_price": "0.48",
            "quantity": "25",
            "reason": "queue manual execution request",
            "expected_signal_version": 9,
            "connector_name": "paper_executor"
        }))
        .expect("serialize body");

        let submit_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/signals/sig_2412/execution-requests")
                    .header("Authorization", format!("Bearer {submit_token}"))
                    .header("X-Request-Id", &submit_request_id)
                    .header("Idempotency-Key", "idem-execution-submit")
                    .header("Content-Type", "application/json")
                    .body(Body::from(submit_body.clone()))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(submit_response.status(), StatusCode::OK);
        let submit_response_body = to_bytes(submit_response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let submit_payload: ApiResponse<SubmitExecutionData> =
            serde_json::from_slice(&submit_response_body).expect("deserialize response");
        assert_eq!(submit_payload.data.order_draft.signal_id, "sig_2412");
        assert_eq!(submit_payload.data.order_draft.status.as_str(), "queued");
        assert_eq!(
            submit_payload.data.execution_request.status.as_str(),
            "queued"
        );
        assert_eq!(
            submit_payload.data.execution_request.mode,
            SystemMode::PaperTrade
        );
        assert_eq!(
            submit_payload.data.execution_request.connector_name,
            "paper_executor"
        );
        assert!(!submit_payload.data.replayed);

        let submit_replay = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/signals/sig_2412/execution-requests")
                    .header("Authorization", format!("Bearer {submit_token}"))
                    .header("X-Request-Id", &submit_request_id)
                    .header("Idempotency-Key", "idem-execution-submit")
                    .header("Content-Type", "application/json")
                    .body(Body::from(submit_body))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(submit_replay.status(), StatusCode::OK);
        let submit_replay_body = to_bytes(submit_replay.into_body(), usize::MAX)
            .await
            .expect("read body");
        let submit_replay_payload: ApiResponse<SubmitExecutionData> =
            serde_json::from_slice(&submit_replay_body).expect("deserialize response");
        assert!(submit_replay_payload.data.replayed);

        let list_request_id = format!("req_{}", Uuid::now_v7());
        let list_token = issue_token(&signing_key, "test-key", &list_request_id);

        let order_drafts_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/orders/drafts?signal_id=sig_2412")
                    .header("Authorization", format!("Bearer {list_token}"))
                    .header("X-Request-Id", &list_request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(order_drafts_response.status(), StatusCode::OK);
        let order_drafts_body = to_bytes(order_drafts_response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let order_drafts_payload: ApiResponse<Vec<OrderDraftData>> =
            serde_json::from_slice(&order_drafts_body).expect("deserialize response");
        assert_eq!(order_drafts_payload.data.len(), 1);
        assert_eq!(order_drafts_payload.data[0].signal_id, "sig_2412");

        let execution_requests_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/execution/requests?signal_id=sig_2412")
                    .header("Authorization", format!("Bearer {list_token}"))
                    .header("X-Request-Id", &list_request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(execution_requests_response.status(), StatusCode::OK);
        let execution_requests_body = to_bytes(execution_requests_response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let execution_requests_payload: ApiResponse<Vec<ExecutionRequestData>> =
            serde_json::from_slice(&execution_requests_body).expect("deserialize response");
        assert_eq!(execution_requests_payload.data.len(), 1);
        assert_eq!(execution_requests_payload.data[0].signal_id, "sig_2412");
    }

    #[tokio::test]
    async fn connector_order_status_callback_is_deduplicated_without_idempotency_key() {
        let signing_key = SigningKey::from_bytes(&[27_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_connector_callbacks")
            .await
            .expect("seed fixtures");
        let app = build_app(state.clone());
        let submission =
            submit_execution_for_test(app.clone(), &signing_key, "sig_2412", "paper_executor")
                .await;
        let external_order_id = "paper_ord_callback_001";

        dispatch_execution(
            &state,
            &submission.execution_request.id,
            "acct_paper_main",
            external_order_id,
        )
        .await;

        let callback_request_id = format!("req_{}", Uuid::now_v7());
        let callback_token = issue_token_with(
            &signing_key,
            "test-key",
            &callback_request_id,
            vec![UserRole::RiskAdmin],
            Vec::new(),
        );
        let callback_body = serde_json::to_vec(&serde_json::json!({
            "event_id": "evt_connector_order_open_1",
            "connector_name": "paper_executor",
            "external_order_id": external_order_id,
            "status": "open"
        }))
        .expect("serialize callback body");

        let first_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/connectors/callbacks/orders/status")
                    .header("Authorization", format!("Bearer {callback_token}"))
                    .header("X-Request-Id", &callback_request_id)
                    .header("Content-Type", "application/json")
                    .body(Body::from(callback_body.clone()))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(first_response.status(), StatusCode::OK);
        let first_response_body = to_bytes(first_response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let first_payload: ApiResponse<ConnectorOrderStatusCallbackData> =
            serde_json::from_slice(&first_response_body).expect("deserialize response");
        assert_eq!(
            first_payload.data.order.external_order_id,
            external_order_id
        );
        assert_eq!(first_payload.data.order.status.as_str(), "open");
        assert!(!first_payload.data.replayed);

        let replay_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/connectors/callbacks/orders/status")
                    .header("Authorization", format!("Bearer {callback_token}"))
                    .header("X-Request-Id", &callback_request_id)
                    .header("Content-Type", "application/json")
                    .body(Body::from(callback_body))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(replay_response.status(), StatusCode::OK);
        let replay_response_body = to_bytes(replay_response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let replay_payload: ApiResponse<ConnectorOrderStatusCallbackData> =
            serde_json::from_slice(&replay_response_body).expect("deserialize response");
        assert!(replay_payload.data.replayed);
        assert_eq!(
            replay_payload.data.order.external_order_id,
            external_order_id
        );
        assert_eq!(replay_payload.data.order.status.as_str(), "open");
    }

    #[tokio::test]
    async fn connector_trade_fill_callback_is_deduplicated_without_duplicate_trades() {
        let signing_key = SigningKey::from_bytes(&[28_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_connector_trade_callback")
            .await
            .expect("seed fixtures");
        let app = build_app(state.clone());
        let submission =
            submit_execution_for_test(app.clone(), &signing_key, "sig_2412", "paper_executor")
                .await;
        let external_order_id = "paper_ord_callback_002";

        dispatch_execution(
            &state,
            &submission.execution_request.id,
            "acct_paper_main",
            external_order_id,
        )
        .await;

        let open_request_id = format!("req_{}", Uuid::now_v7());
        let open_token = issue_token_with(
            &signing_key,
            "test-key",
            &open_request_id,
            vec![UserRole::RiskAdmin],
            Vec::new(),
        );
        let open_body = serde_json::to_vec(&serde_json::json!({
            "event_id": "evt_connector_order_open_2",
            "connector_name": "paper_executor",
            "external_order_id": external_order_id,
            "status": "open"
        }))
        .expect("serialize order open body");
        let open_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/connectors/callbacks/orders/status")
                    .header("Authorization", format!("Bearer {open_token}"))
                    .header("X-Request-Id", &open_request_id)
                    .header("Content-Type", "application/json")
                    .body(Body::from(open_body))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(open_response.status(), StatusCode::OK);

        let trade_request_id = format!("req_{}", Uuid::now_v7());
        let trade_token = issue_token_with(
            &signing_key,
            "test-key",
            &trade_request_id,
            vec![UserRole::RiskAdmin],
            Vec::new(),
        );
        let trade_body = serde_json::to_vec(&serde_json::json!({
            "event_id": "evt_connector_trade_fill_1",
            "connector_name": "paper_executor",
            "external_order_id": external_order_id,
            "account_id": "acct_paper_main",
            "external_trade_id": "paper_trade_callback_001",
            "fill_price": "0.48",
            "filled_quantity": "1",
            "fee": "0.00"
        }))
        .expect("serialize trade body");

        let first_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/connectors/callbacks/trades/fill")
                    .header("Authorization", format!("Bearer {trade_token}"))
                    .header("X-Request-Id", &trade_request_id)
                    .header("Content-Type", "application/json")
                    .body(Body::from(trade_body.clone()))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(first_response.status(), StatusCode::OK);
        let first_response_body = to_bytes(first_response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let first_payload: ApiResponse<ConnectorTradeFillCallbackData> =
            serde_json::from_slice(&first_response_body).expect("deserialize response");
        assert_eq!(
            first_payload.data.trade.external_trade_id,
            "paper_trade_callback_001"
        );
        assert_eq!(
            first_payload.data.order.external_order_id,
            external_order_id
        );
        assert_eq!(first_payload.data.position.account_id, "acct_paper_main");
        assert!(!first_payload.data.replayed);

        let replay_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/connectors/callbacks/trades/fill")
                    .header("Authorization", format!("Bearer {trade_token}"))
                    .header("X-Request-Id", &trade_request_id)
                    .header("Content-Type", "application/json")
                    .body(Body::from(trade_body))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(replay_response.status(), StatusCode::OK);
        let replay_response_body = to_bytes(replay_response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let replay_payload: ApiResponse<ConnectorTradeFillCallbackData> =
            serde_json::from_slice(&replay_response_body).expect("deserialize response");
        assert!(replay_payload.data.replayed);
        assert_eq!(
            replay_payload.data.trade.external_trade_id,
            "paper_trade_callback_001"
        );

        let trades_request_id = format!("req_{}", Uuid::now_v7());
        let trades_token = issue_token(&signing_key, "test-key", &trades_request_id);
        let trades_response = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/v1/trades?order_id={}",
                        first_payload.data.order.id
                    ))
                    .header("Authorization", format!("Bearer {trades_token}"))
                    .header("X-Request-Id", &trades_request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(trades_response.status(), StatusCode::OK);
        let trades_response_body = to_bytes(trades_response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let trades_payload: ApiResponse<Vec<TradeData>> =
            serde_json::from_slice(&trades_response_body).expect("deserialize response");
        assert_eq!(trades_payload.data.len(), 1);
        assert_eq!(
            trades_payload.data[0].external_trade_id,
            "paper_trade_callback_001"
        );
    }

    #[tokio::test]
    async fn polymarket_order_status_callback_normalizes_live_to_open() {
        let signing_key = SigningKey::from_bytes(&[29_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_polymarket_status")
            .await
            .expect("seed fixtures");
        let app = build_app(state.clone());
        let submission = submit_execution_for_test(
            app.clone(),
            &signing_key,
            "sig_2412",
            polyedge_connectors::POLYMARKET_CONNECTOR_NAME,
        )
        .await;
        let external_order_id = "pm_ord_callback_001";

        dispatch_execution(
            &state,
            &submission.execution_request.id,
            "acct_poly_main",
            external_order_id,
        )
        .await;

        let request_id = format!("req_{}", Uuid::now_v7());
        let token = issue_token_with(
            &signing_key,
            "test-key",
            &request_id,
            vec![UserRole::RiskAdmin],
            Vec::new(),
        );
        let body = serde_json::to_vec(&serde_json::json!({
            "event_id": "evt_pm_order_open_1",
            "order_id": external_order_id,
            "status": "live"
        }))
        .expect("serialize body");

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/connectors/polymarket/callbacks/orders/status")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .header("Content-Type", "application/json")
                    .body(Body::from(body))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let response_body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let payload: ApiResponse<ConnectorOrderStatusCallbackData> =
            serde_json::from_slice(&response_body).expect("deserialize response");
        assert_eq!(
            payload.data.order.connector_name,
            polyedge_connectors::POLYMARKET_CONNECTOR_NAME
        );
        assert_eq!(payload.data.order.status.as_str(), "open");
        assert!(!payload.data.replayed);
    }

    #[tokio::test]
    async fn polymarket_trade_fill_callback_normalizes_trade_payload() {
        let signing_key = SigningKey::from_bytes(&[30_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_polymarket_fill")
            .await
            .expect("seed fixtures");
        let app = build_app(state.clone());
        let submission = submit_execution_for_test(
            app.clone(),
            &signing_key,
            "sig_2412",
            polyedge_connectors::POLYMARKET_CONNECTOR_NAME,
        )
        .await;
        let external_order_id = "pm_ord_callback_002";

        dispatch_execution(
            &state,
            &submission.execution_request.id,
            "acct_poly_main",
            external_order_id,
        )
        .await;

        let open_request_id = format!("req_{}", Uuid::now_v7());
        let open_token = issue_token_with(
            &signing_key,
            "test-key",
            &open_request_id,
            vec![UserRole::RiskAdmin],
            Vec::new(),
        );
        let open_body = serde_json::to_vec(&serde_json::json!({
            "event_id": "evt_pm_order_open_2",
            "order_id": external_order_id,
            "status": "live"
        }))
        .expect("serialize open body");
        let open_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/connectors/polymarket/callbacks/orders/status")
                    .header("Authorization", format!("Bearer {open_token}"))
                    .header("X-Request-Id", &open_request_id)
                    .header("Content-Type", "application/json")
                    .body(Body::from(open_body))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(open_response.status(), StatusCode::OK);

        let trade_request_id = format!("req_{}", Uuid::now_v7());
        let trade_token = issue_token_with(
            &signing_key,
            "test-key",
            &trade_request_id,
            vec![UserRole::RiskAdmin],
            Vec::new(),
        );
        let trade_body = serde_json::to_vec(&serde_json::json!({
            "event_id": "evt_pm_trade_fill_1",
            "order_id": external_order_id,
            "account_id": "acct_poly_main",
            "trade_id": "pm_trade_callback_001",
            "price": "0.48",
            "size": "1",
            "fee": "0.00"
        }))
        .expect("serialize trade body");

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/connectors/polymarket/callbacks/trades/fill")
                    .header("Authorization", format!("Bearer {trade_token}"))
                    .header("X-Request-Id", &trade_request_id)
                    .header("Content-Type", "application/json")
                    .body(Body::from(trade_body))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let response_body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let payload: ApiResponse<ConnectorTradeFillCallbackData> =
            serde_json::from_slice(&response_body).expect("deserialize response");
        assert_eq!(
            payload.data.order.connector_name,
            polyedge_connectors::POLYMARKET_CONNECTOR_NAME
        );
        assert_eq!(
            payload.data.trade.external_trade_id,
            "pm_trade_callback_001"
        );
        assert_eq!(payload.data.position.account_id, "acct_poly_main");
        assert!(!payload.data.replayed);
    }

    #[tokio::test]
    async fn trigger_kill_switch_requires_specific_scope() {
        let signing_key = SigningKey::from_bytes(&[21_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let app = build_app(Runtime::test_app_state(settings).expect("state"));
        let request_id = format!("req_{}", Uuid::now_v7());
        let token = issue_token_with(
            &signing_key,
            "test-key",
            &request_id,
            vec![UserRole::RiskAdmin],
            vec![StepUpScope::SystemModeSwitch],
        );
        let body = serde_json::to_vec(&TriggerKillSwitchRequest {
            reason: "operator initiated stop".to_string(),
            expected_version: Some(1),
        })
        .expect("serialize body");

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/system/kill-switch/trigger")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .header("Idempotency-Key", "idem-kill-trigger-scope")
                    .header("Content-Type", "application/json")
                    .body(Body::from(body))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn kill_switch_trigger_and_release_are_idempotent() {
        let signing_key = SigningKey::from_bytes(&[22_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let app = build_app(Runtime::test_app_state(settings).expect("state"));

        let trigger_request_id = format!("req_{}", Uuid::now_v7());
        let trigger_token = issue_token_with(
            &signing_key,
            "test-key",
            &trigger_request_id,
            vec![UserRole::RiskAdmin],
            vec![StepUpScope::SystemKillSwitchTrigger],
        );
        let trigger_body = serde_json::to_vec(&TriggerKillSwitchRequest {
            reason: "operator initiated stop".to_string(),
            expected_version: Some(1),
        })
        .expect("serialize body");

        let trigger_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/system/kill-switch/trigger")
                    .header("Authorization", format!("Bearer {trigger_token}"))
                    .header("X-Request-Id", &trigger_request_id)
                    .header("Idempotency-Key", "idem-kill-trigger-1")
                    .header("Content-Type", "application/json")
                    .body(Body::from(trigger_body.clone()))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(trigger_response.status(), StatusCode::OK);
        let trigger_response_body = to_bytes(trigger_response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let trigger_payload: ApiResponse<KillSwitchData> =
            serde_json::from_slice(&trigger_response_body).expect("deserialize response");
        assert_eq!(
            trigger_payload.data.risk_state.mode,
            SystemMode::KillSwitchLocked
        );
        assert!(trigger_payload.data.risk_state.kill_switch);
        assert_eq!(trigger_payload.data.risk_state.version, 2);
        assert!(!trigger_payload.data.replayed);

        let trigger_replay = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/system/kill-switch/trigger")
                    .header("Authorization", format!("Bearer {trigger_token}"))
                    .header("X-Request-Id", &trigger_request_id)
                    .header("Idempotency-Key", "idem-kill-trigger-1")
                    .header("Content-Type", "application/json")
                    .body(Body::from(trigger_body))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(trigger_replay.status(), StatusCode::OK);
        let trigger_replay_body = to_bytes(trigger_replay.into_body(), usize::MAX)
            .await
            .expect("read body");
        let trigger_replay_payload: ApiResponse<KillSwitchData> =
            serde_json::from_slice(&trigger_replay_body).expect("deserialize response");
        assert!(trigger_replay_payload.data.replayed);

        let release_request_id = format!("req_{}", Uuid::now_v7());
        let release_token = issue_token_with(
            &signing_key,
            "test-key",
            &release_request_id,
            vec![UserRole::RiskAdmin],
            vec![StepUpScope::SystemKillSwitchRelease],
        );
        let release_body = serde_json::to_vec(&ReleaseKillSwitchRequest {
            reason: "resume controlled paper trading".to_string(),
            to_mode: SystemMode::PaperTrade,
            expected_version: Some(2),
        })
        .expect("serialize body");

        let release_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/system/kill-switch/release")
                    .header("Authorization", format!("Bearer {release_token}"))
                    .header("X-Request-Id", &release_request_id)
                    .header("Idempotency-Key", "idem-kill-release-1")
                    .header("Content-Type", "application/json")
                    .body(Body::from(release_body.clone()))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(release_response.status(), StatusCode::OK);
        let release_response_body = to_bytes(release_response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let release_payload: ApiResponse<KillSwitchData> =
            serde_json::from_slice(&release_response_body).expect("deserialize response");
        assert_eq!(release_payload.data.risk_state.mode, SystemMode::PaperTrade);
        assert!(!release_payload.data.risk_state.kill_switch);
        assert_eq!(release_payload.data.risk_state.version, 3);
        assert!(!release_payload.data.replayed);

        let release_replay = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/system/kill-switch/release")
                    .header("Authorization", format!("Bearer {release_token}"))
                    .header("X-Request-Id", &release_request_id)
                    .header("Idempotency-Key", "idem-kill-release-1")
                    .header("Content-Type", "application/json")
                    .body(Body::from(release_body))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(release_replay.status(), StatusCode::OK);
        let release_replay_body = to_bytes(release_replay.into_body(), usize::MAX)
            .await
            .expect("read body");
        let release_replay_payload: ApiResponse<KillSwitchData> =
            serde_json::from_slice(&release_replay_body).expect("deserialize response");
        assert!(release_replay_payload.data.replayed);
        assert_eq!(release_replay_payload.data.risk_state.version, 3);
    }

    #[tokio::test]
    async fn recompute_signal_route_is_idempotent_and_creates_estimate() {
        let signing_key = SigningKey::from_bytes(&[16_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_recompute")
            .await
            .expect("seed fixtures");
        let app = build_app(state);
        let request_id = format!("req_{}", Uuid::now_v7());
        let token = issue_token(&signing_key, "test-key", &request_id);
        let body = serde_json::to_vec(&RecomputeSignalRequest {
            reason: "manual pricing refresh after official update".to_string(),
        })
        .expect("serialize body");

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/signals/sig_2412/recompute")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .header("Idempotency-Key", "idem-signal-1")
                    .header("Content-Type", "application/json")
                    .body(Body::from(body.clone()))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        let first_body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let first_payload: ApiResponse<RecomputeSignalData> =
            serde_json::from_slice(&first_body).expect("deserialize response");
        assert_eq!(first_payload.data.signal.id, "sig_2412");
        assert_eq!(
            first_payload.data.signal.side,
            polyedge_domain::SignalSide::No
        );
        assert_eq!(
            first_payload.data.signal.lifecycle_state,
            polyedge_domain::SignalLifecycleState::New
        );
        assert!(first_payload.data.transition.is_none());
        assert!(!first_payload.data.replayed);

        let replay = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/signals/sig_2412/recompute")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .header("Idempotency-Key", "idem-signal-1")
                    .header("Content-Type", "application/json")
                    .body(Body::from(body))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(replay.status(), StatusCode::OK);
        let replay_body = to_bytes(replay.into_body(), usize::MAX)
            .await
            .expect("read body");
        let replay_payload: ApiResponse<RecomputeSignalData> =
            serde_json::from_slice(&replay_body).expect("deserialize response");
        assert!(replay_payload.data.replayed);
        assert_eq!(
            replay_payload.data.estimate.id,
            first_payload.data.estimate.id
        );

        let estimates_request_id = format!("req_{}", Uuid::now_v7());
        let estimates_token = issue_token(&signing_key, "test-key", &estimates_request_id);
        let estimates_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/pricing/estimates?signal_id=sig_2412")
                    .header("Authorization", format!("Bearer {estimates_token}"))
                    .header("X-Request-Id", &estimates_request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(estimates_response.status(), StatusCode::OK);
        let estimates_body = to_bytes(estimates_response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let estimates_payload: ApiResponse<Vec<ProbabilityEstimateData>> =
            serde_json::from_slice(&estimates_body).expect("deserialize response");
        assert_eq!(estimates_payload.data.len(), 1);
        assert_eq!(
            estimates_payload.data[0].signal_id.as_deref(),
            Some("sig_2412")
        );
    }

    #[tokio::test]
    async fn signal_transitions_route_returns_recompute_transition() {
        let signing_key = SigningKey::from_bytes(&[17_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let state = Runtime::test_app_state(settings).expect("state");
        state
            .market_event_service
            .ingest_fixture_bundle(demo_fixture_bundle(), "trc_seeded_transitions")
            .await
            .expect("seed fixtures");
        let app = build_app(state);
        let request_id = format!("req_{}", Uuid::now_v7());
        let token = issue_token(&signing_key, "test-key", &request_id);
        let body = serde_json::to_vec(&RecomputeSignalRequest {
            reason: "refresh transition history".to_string(),
        })
        .expect("serialize body");

        let recompute = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/signals/sig_2411/recompute")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", &request_id)
                    .header("Idempotency-Key", "idem-signal-2")
                    .header("Content-Type", "application/json")
                    .body(Body::from(body))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(recompute.status(), StatusCode::OK);

        let transitions_request_id = format!("req_{}", Uuid::now_v7());
        let transitions_token = issue_token(&signing_key, "test-key", &transitions_request_id);
        let transitions_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/signals/sig_2411/transitions?limit=10")
                    .header("Authorization", format!("Bearer {transitions_token}"))
                    .header("X-Request-Id", &transitions_request_id)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(transitions_response.status(), StatusCode::OK);
        let transitions_body = to_bytes(transitions_response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let payload: ApiResponse<Vec<SignalTransitionData>> =
            serde_json::from_slice(&transitions_body).expect("deserialize response");
        assert_eq!(payload.data.len(), 1);
        assert_eq!(payload.data[0].signal_id, "sig_2411");
        assert_eq!(
            payload.data[0].from_state,
            polyedge_domain::SignalLifecycleState::Active
        );
        assert_eq!(
            payload.data[0].to_state,
            polyedge_domain::SignalLifecycleState::Weakened
        );
    }

    #[tokio::test]
    async fn mode_transition_is_idempotent() {
        let signing_key = SigningKey::from_bytes(&[11_u8; 32]);
        let settings = Settings::for_test(
            SystemMode::ManualConfirm,
            "test",
            vec![AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: general_purpose::STANDARD
                    .encode(signing_key.verifying_key().as_bytes()),
            }],
        );
        let app = build_app(Runtime::test_app_state(settings).expect("state"));
        let request_id = "req_test_2";
        let token = issue_token(&signing_key, "test-key", request_id);
        let body = serde_json::to_vec(&TransitionSystemModeRequest {
            to_mode: SystemMode::Research,
            reason: "operator switched to research mode".to_string(),
        })
        .expect("serialize body");

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/system/mode")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", request_id)
                    .header("Idempotency-Key", "idem-1")
                    .header("Content-Type", "application/json")
                    .body(Body::from(body.clone()))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);

        let replay = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/system/mode")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("X-Request-Id", request_id)
                    .header("Idempotency-Key", "idem-1")
                    .header("Content-Type", "application/json")
                    .body(Body::from(body))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(replay.status(), StatusCode::OK);

        let replay_body = to_bytes(replay.into_body(), usize::MAX)
            .await
            .expect("read body");
        let payload: ApiResponse<SystemModeData> =
            serde_json::from_slice(&replay_body).expect("deserialize response");
        assert!(payload.data.replayed);
    }
}
