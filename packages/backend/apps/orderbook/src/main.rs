use axum::{Router, routing::get};
use polyedge_infrastructure::Runtime;
use std::time::Duration;
use tower_http::trace::TraceLayer;
use tracing::info;

mod market_sync;
use market_sync::sync_markets_once;

mod stream;
use stream::run_orderbook_stream;

mod http_api;
use http_api::{
    get_orderbook, get_orderbook_batch, get_orderbook_stats, ingest_books, register_tokens,
    unregister_source,
};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let runtime = Runtime::load().await.expect("failed to load runtime");
    let state = runtime.app_state();

    let port = state.settings.orderbook.port;
    let listen = format!("0.0.0.0:{port}");

    info!(port, "starting polyedge-orderbook service");

    // Run initial market sync so the database is populated before any consumer
    // starts reading.
    {
        let trace_id = polyedge_infrastructure::new_trace_id();
        match sync_markets_once(&state, &trace_id).await {
            Ok(report) => info!(
                general = report.general_upserted,
                reward = report.reward_upserted,
                "initial market sync complete"
            ),
            Err(error) => tracing::warn!(error = %error, "initial market sync failed"),
        }
    }

    // Periodic market sync (general + reward markets → Postgres).
    let sync_state = state.clone();
    let sync_handle = tokio::spawn(async move {
        let interval = Duration::from_secs(
            sync_state.settings.worker.market_sync_interval_secs.max(60),
        );
        loop {
            tokio::time::sleep(interval).await;
            let trace_id = polyedge_infrastructure::new_trace_id();
            match sync_markets_once(&sync_state, &trace_id).await {
                Ok(report) => info!(
                    general = report.general_upserted,
                    reward = report.reward_upserted,
                    "periodic market sync complete"
                ),
                Err(error) => {
                    tracing::warn!(error = %error, "periodic market sync failed")
                }
            }
        }
    });

    // Spawn the WS + poll stream as a restarting background task.
    // It subscribes to tokens registered via the HTTP API by other services.
    let stream_state = state.clone();
    let stream_handle = tokio::spawn(async move {
        loop {
            let token_count = stream_state.orderbook_registry.list_all_tokens().await.len();
            if token_count == 0 {
                info!("no tokens registered yet, waiting 10s before retry");
                tokio::time::sleep(Duration::from_secs(10)).await;
                continue;
            }

            match run_orderbook_stream(&stream_state).await {
                Ok(report) => {
                    info!(
                        subscribed = report.subscribed_tokens,
                        ws_received = report.ws_snapshots_received,
                        poll_reconciliations = report.poll_reconciliations,
                        "orderbook stream stopped, restarting after 5s"
                    );
                }
                Err(error) => {
                    tracing::warn!(error = %error, "orderbook stream failed, restarting after 5s");
                }
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });

    // Build HTTP API.
    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/orderbook/stats", get(get_orderbook_stats))
        .route("/orderbook/batch", axum::routing::post(get_orderbook_batch))
        .route("/orderbook/register", axum::routing::post(register_tokens))
        .route("/orderbook/ingest", axum::routing::post(ingest_books))
        .route(
            "/orderbook/register/{source}",
            axum::routing::delete(unregister_source),
        )
        .route("/orderbook/{token_id}", get(get_orderbook))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&listen)
        .await
        .expect("failed to bind orderbook HTTP listener");

    info!(address = %listen, "orderbook HTTP server listening");

    tokio::select! {
        result = axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()) => {
            if let Err(error) = result {
                tracing::error!(error = %error, "orderbook HTTP server failed");
            }
        }
        _ = stream_handle => {}
        _ = sync_handle => {}
    }

    info!("polyedge-orderbook service shutting down");
}

async fn healthz() -> &'static str {
    "ok"
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("failed to install SIGTERM handler");
        tokio::select! {
            _ = ctrl_c => {}
            _ = sigterm.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = ctrl_c.await;
    }
}
