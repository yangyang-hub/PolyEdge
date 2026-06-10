use polyedge_api::build_app;
use polyedge_infrastructure::{AppState, Runtime, telemetry::init_tracing};
use polyedge_worker::WorkerRuntime;
use std::net::SocketAddr;
use tracing::info;

#[tokio::main]
async fn main() -> polyedge_domain::Result<()> {
    init_tracing("polyedge_api");
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
    let worker_runtime = WorkerRuntime::start(&state);
    let app = build_app(state.clone());
    let addr: SocketAddr = format!(
        "{}:{}",
        state.settings.server.host, state.settings.server.port
    )
    .parse()
    .map_err(|error| {
        polyedge_domain::AppError::internal(
            "API_BIND_ADDR_INVALID",
            format!("invalid API bind address: {error}"),
        )
    })?;

    let listener = tokio::net::TcpListener::bind(addr).await.map_err(|error| {
        polyedge_domain::AppError::dependency_unavailable(
            "API_BIND_FAILED",
            format!("failed to bind API listener: {error}"),
        )
    })?;

    info!(address = %addr, "polyedge api listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(worker_runtime))
        .await
        .map_err(|error| {
            polyedge_domain::AppError::internal(
                "API_SERVER_FAILED",
                format!("api server failed: {error}"),
            )
        })
}

async fn shutdown_signal(worker_runtime: WorkerRuntime) {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            let _ = signal.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }

    worker_runtime.shutdown().await;
}
