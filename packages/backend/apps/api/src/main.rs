use polyedge_api::build_app;
use polyedge_infrastructure::{Runtime, telemetry::init_tracing};
use std::net::SocketAddr;
use tracing::info;

#[tokio::main]
async fn main() -> polyedge_domain::Result<()> {
    init_tracing("polyedge_api");
    let runtime = Runtime::load().await?;
    let state = runtime.app_state();
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
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|error| {
            polyedge_domain::AppError::internal(
                "API_SERVER_FAILED",
                format!("api server failed: {error}"),
            )
        })
}

async fn shutdown_signal() {
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
}
