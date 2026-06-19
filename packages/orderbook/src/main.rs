use axum::{Router, routing::get};
use polyedge_application::MarketUpsertOptions;
use polyedge_common::{bind_service_listener, service_socket_addr};
use polyedge_infrastructure::{AppState, Runtime};
use std::time::Duration;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::filter::{FilterExt, FilterFn};
use tracing_subscriber::{EnvFilter, prelude::*};

mod market_sync;
use market_sync::{
    sync_general_markets_once, sync_priority_markets_once, sync_reward_markets_once,
};

mod stream;
use stream::run_orderbook_stream;

mod updates;
use updates::OrderbookUpdateBroadcaster;

mod http_api;
use http_api::{
    OrderbookApiState, get_orderbook, get_orderbook_batch, get_orderbook_stats, ingest_books,
    register_tokens, stream_orderbooks, unregister_source,
};

const MIN_GENERAL_MARKET_SYNC_TIMEOUT_SECS: u64 = 60;
const MAX_GENERAL_MARKET_SYNC_TIMEOUT_SECS: u64 = 240;
const REWARD_MARKET_SYNC_TIMEOUT_SECS: u64 = 45 * 60;
const PRIORITY_MARKET_SYNC_MAX_CONDITION_IDS: usize = 500;
const PRIORITY_REWARD_DISCOVERY_MAX_STALE_MINUTES: u64 = 24 * 60;
const MIN_PRIORITY_MARKET_SYNC_INTERVAL_SECS: u64 = 30;
const MAX_PRIORITY_MARKET_SYNC_INTERVAL_SECS: u64 = 300;
const MAX_PRIORITY_MARKET_SYNC_TIMEOUT_SECS: u64 = 120;
const DEFAULT_MARKET_DATA_MAX_AGE_MINUTES: u64 = 15;
const MIN_GENERAL_MARKET_SYNCED_AT_REFRESH_AFTER_SECS: u64 = 30;

#[tokio::main]
async fn main() -> polyedge_domain::Result<()> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // Suppress expected ERROR-level logs from the SDK's internal WS reconnection
    // cycle. The SDK already retries with exponential backoff, so these errors
    // are normal operational noise. Keep WARN heartbeat diagnostics visible.
    let suppress_sdk_ws_error = FilterFn::new(|metadata| {
        !(metadata
            .target()
            .starts_with("polymarket_client_sdk_v2::ws::connection")
            && *metadata.level() == tracing::Level::ERROR)
    });

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(env_filter.and(suppress_sdk_ws_error)))
        .init();

    let runtime = Runtime::load().await?;
    let state = runtime.app_state();

    let port = state.settings.orderbook.port;
    let addr = service_socket_addr(
        "0.0.0.0",
        port,
        "orderbook HTTP",
        "ORDERBOOK_BIND_ADDR_INVALID",
    )?;

    info!(port, "starting polyedge-orderbook service");

    let broadcaster =
        OrderbookUpdateBroadcaster::with_reward_candles(16_384, state.reward_bot_service.clone());

    // Build and bind HTTP API before any external market sync. Health checks
    // should reflect process readiness, not Polymarket API latency.
    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/orderbook/stats", get(get_orderbook_stats))
        .route("/orderbook/batch", axum::routing::post(get_orderbook_batch))
        .route("/orderbook/stream", get(stream_orderbooks))
        .route("/orderbook/register", axum::routing::post(register_tokens))
        .route("/orderbook/ingest", axum::routing::post(ingest_books))
        .route(
            "/orderbook/register/{source}",
            axum::routing::delete(unregister_source),
        )
        .route("/orderbook/{token_id}", get(get_orderbook))
        .layer(TraceLayer::new_for_http())
        .layer(RequestBodyLimitLayer::new(2 * 1024 * 1024)) // 2 MB
        .with_state(OrderbookApiState::new(state.clone(), broadcaster.clone()));

    let listener = bind_service_listener(addr, "orderbook HTTP", "ORDERBOOK_BIND_FAILED").await?;

    info!(address = %addr, "orderbook HTTP server listening");

    // Periodic market syncs. Gamma market metadata must keep its own cadence:
    // rewards catalog enrichment can take many minutes when CLOB details are
    // rate-limited, but rewards quoting depends on fresh `markets.synced_at`.
    let general_sync_state = state.clone();
    let general_sync_handle = tokio::spawn(async move {
        run_general_market_sync_loop(general_sync_state).await;
    });

    let priority_sync_state = state.clone();
    let priority_sync_handle = tokio::spawn(async move {
        run_priority_market_sync_loop(priority_sync_state).await;
    });

    let reward_sync_state = state.clone();
    let reward_sync_handle = tokio::spawn(async move {
        run_reward_market_sync_loop(reward_sync_state).await;
    });

    // Spawn the WS + poll stream as a restarting background task.
    // It subscribes to tokens registered via the HTTP API by other services.
    let stream_state = state.clone();
    let stream_broadcaster = broadcaster.clone();
    let stream_handle = tokio::spawn(async move {
        let restart_interval = Duration::from_secs(
            stream_state
                .settings
                .orderbook_stream
                .restart_interval_secs
                .max(1),
        );
        loop {
            let token_count = stream_state
                .orderbook_registry
                .list_all_tokens()
                .await
                .len();
            if token_count == 0 {
                info!("no tokens registered yet, waiting 10s before retry");
                tokio::time::sleep(Duration::from_secs(10)).await;
                continue;
            }

            match run_orderbook_stream(&stream_state, &stream_broadcaster).await {
                Ok(report) => {
                    info!(
                        subscribed = report.subscribed_tokens,
                        ws_received = report.ws_snapshots_received,
                        ws_price_changes = report.ws_price_changes_received,
                        poll_reconciliations = report.poll_reconciliations,
                        restart_after_secs = restart_interval.as_secs(),
                        "orderbook stream stopped, restarting"
                    );
                }
                Err(error) => {
                    tracing::warn!(
                        error = %error,
                        restart_after_secs = restart_interval.as_secs(),
                        "orderbook stream failed, restarting"
                    );
                }
            }
            tokio::time::sleep(restart_interval).await;
        }
    });

    tokio::select! {
        result = axum::serve(listener, app).with_graceful_shutdown(polyedge_common::shutdown_signal()) => {
            if let Err(error) = result {
                tracing::error!(error = %error, "orderbook HTTP server failed");
            }
        }
        _ = stream_handle => {}
        _ = general_sync_handle => {}
        _ = priority_sync_handle => {}
        _ = reward_sync_handle => {}
    }

    info!("polyedge-orderbook service shutting down");
    Ok(())
}

async fn healthz() -> &'static str {
    "ok"
}

async fn run_general_market_sync_loop(state: AppState) {
    let interval = Duration::from_secs(state.settings.worker.market_sync_interval_secs.max(60));
    let timeout = general_market_sync_timeout(interval);
    let mut phase = "initial";
    loop {
        let started = std::time::Instant::now();
        run_general_market_sync(&state, phase, timeout).await;
        phase = "periodic";
        let elapsed = started.elapsed();
        if elapsed < interval {
            tokio::time::sleep(interval - elapsed).await;
        }
    }
}

async fn run_reward_market_sync_loop(state: AppState) {
    let interval = Duration::from_secs(state.settings.worker.market_sync_interval_secs.max(60));
    let timeout = Duration::from_secs(REWARD_MARKET_SYNC_TIMEOUT_SECS);
    let mut phase = "initial";
    loop {
        run_reward_market_sync(&state, phase, timeout).await;
        phase = "periodic";
        tokio::time::sleep(interval).await;
    }
}

async fn run_priority_market_sync_loop(state: AppState) {
    let full_interval =
        Duration::from_secs(state.settings.worker.market_sync_interval_secs.max(60));
    let mut phase = "initial";
    loop {
        let (interval, freshness_minutes) =
            priority_market_sync_interval(&state, full_interval).await;
        let timeout = priority_market_sync_timeout(interval);
        let started = std::time::Instant::now();
        run_priority_market_sync(&state, phase, timeout, freshness_minutes).await;
        phase = "periodic";
        let elapsed = started.elapsed();
        if elapsed < interval {
            tokio::time::sleep(interval - elapsed).await;
        }
    }
}

fn general_market_sync_timeout(interval: Duration) -> Duration {
    let timeout_secs = interval
        .as_secs()
        .saturating_mul(4)
        .saturating_div(5)
        .clamp(
            MIN_GENERAL_MARKET_SYNC_TIMEOUT_SECS,
            MAX_GENERAL_MARKET_SYNC_TIMEOUT_SECS,
        );
    Duration::from_secs(timeout_secs)
}

async fn priority_market_sync_interval(
    state: &AppState,
    full_interval: Duration,
) -> (Duration, u64) {
    match state.reward_bot_service.read_config().await {
        Ok(config) => {
            let freshness_minutes = config.max_market_data_age_minutes.max(1);
            let freshness_secs = freshness_minutes.saturating_mul(60);
            let upper = full_interval.as_secs().clamp(
                MIN_PRIORITY_MARKET_SYNC_INTERVAL_SECS,
                MAX_PRIORITY_MARKET_SYNC_INTERVAL_SECS,
            );
            let interval_secs = freshness_secs
                .saturating_div(3)
                .max(1)
                .clamp(MIN_PRIORITY_MARKET_SYNC_INTERVAL_SECS, upper);
            (Duration::from_secs(interval_secs), freshness_minutes)
        }
        Err(error) => {
            tracing::warn!(
                error = %error,
                "failed to read rewards config for priority market sync interval"
            );
            (
                Duration::from_secs(
                    full_interval
                        .as_secs()
                        .clamp(MIN_PRIORITY_MARKET_SYNC_INTERVAL_SECS, 60),
                ),
                0,
            )
        }
    }
}

fn priority_market_sync_timeout(interval: Duration) -> Duration {
    Duration::from_secs(interval.as_secs().clamp(
        MIN_PRIORITY_MARKET_SYNC_INTERVAL_SECS,
        MAX_PRIORITY_MARKET_SYNC_TIMEOUT_SECS,
    ))
}

async fn run_general_market_sync(state: &AppState, phase: &'static str, timeout: Duration) {
    let trace_id = polyedge_infrastructure::new_trace_id();
    let upsert_options = general_market_upsert_options(state).await;
    let refresh_synced_at_after_secs = upsert_options.refresh_synced_at_after_secs.unwrap_or(0);
    let started = std::time::Instant::now();
    match tokio::time::timeout(
        timeout,
        sync_general_markets_once(state, &trace_id, upsert_options),
    )
    .await
    {
        Ok(Ok(written)) => info!(
            phase,
            written,
            refresh_synced_at_after_secs,
            elapsed_ms = started.elapsed().as_millis() as u64,
            "orderbook general market sync complete"
        ),
        Ok(Err(error)) => tracing::warn!(
            phase,
            error = %error,
            elapsed_ms = started.elapsed().as_millis() as u64,
            "orderbook general market sync failed"
        ),
        Err(_) => tracing::warn!(
            phase,
            timeout_secs = timeout.as_secs(),
            "orderbook general market sync timed out"
        ),
    }
}

async fn general_market_upsert_options(state: &AppState) -> MarketUpsertOptions {
    let freshness_minutes = state
        .reward_bot_service
        .read_config()
        .await
        .map(|config| config.max_market_data_age_minutes.max(1))
        .unwrap_or(DEFAULT_MARKET_DATA_MAX_AGE_MINUTES);
    MarketUpsertOptions::refresh_when_older_than(general_synced_at_refresh_after_secs(
        freshness_minutes,
    ))
}

fn general_synced_at_refresh_after_secs(freshness_minutes: u64) -> u64 {
    let max_age_secs = freshness_minutes.max(1).saturating_mul(60);
    let refresh_after_secs = max_age_secs.saturating_mul(2).saturating_div(3);
    let upper = max_age_secs
        .saturating_sub(1)
        .max(MIN_GENERAL_MARKET_SYNCED_AT_REFRESH_AFTER_SECS);
    refresh_after_secs.clamp(MIN_GENERAL_MARKET_SYNCED_AT_REFRESH_AFTER_SECS, upper)
}

async fn run_priority_market_sync(
    state: &AppState,
    phase: &'static str,
    timeout: Duration,
    freshness_minutes: u64,
) {
    let trace_id = polyedge_infrastructure::new_trace_id();
    let started = std::time::Instant::now();
    match tokio::time::timeout(
        timeout,
        sync_priority_markets_once(
            state,
            &trace_id,
            PRIORITY_MARKET_SYNC_MAX_CONDITION_IDS,
            PRIORITY_REWARD_DISCOVERY_MAX_STALE_MINUTES,
        ),
    )
    .await
    {
        Ok(Ok(report)) if report.condition_ids > 0 => info!(
            phase,
            freshness_minutes,
            condition_ids = report.condition_ids,
            fetched = report.fetched,
            upserted = report.upserted,
            elapsed_ms = started.elapsed().as_millis() as u64,
            "orderbook priority market sync complete"
        ),
        Ok(Ok(_)) => tracing::debug!(
            phase,
            freshness_minutes,
            elapsed_ms = started.elapsed().as_millis() as u64,
            "orderbook priority market sync skipped with no priority markets"
        ),
        Ok(Err(error)) => tracing::warn!(
            phase,
            freshness_minutes,
            error = %error,
            elapsed_ms = started.elapsed().as_millis() as u64,
            "orderbook priority market sync failed"
        ),
        Err(_) => tracing::warn!(
            phase,
            freshness_minutes,
            timeout_secs = timeout.as_secs(),
            "orderbook priority market sync timed out"
        ),
    }
}

async fn run_reward_market_sync(state: &AppState, phase: &'static str, timeout: Duration) {
    let started = std::time::Instant::now();
    match tokio::time::timeout(timeout, sync_reward_markets_once(state)).await {
        Ok(Ok(reward)) => info!(
            phase,
            reward,
            elapsed_ms = started.elapsed().as_millis() as u64,
            "orderbook reward market sync complete"
        ),
        Ok(Err(error)) => tracing::warn!(
            phase,
            error = %error,
            elapsed_ms = started.elapsed().as_millis() as u64,
            "orderbook reward market sync failed; preserving prior reward catalog"
        ),
        Err(_) => tracing::warn!(
            phase,
            timeout_secs = timeout.as_secs(),
            "orderbook reward market sync timed out; preserving prior reward catalog"
        ),
    }
}
