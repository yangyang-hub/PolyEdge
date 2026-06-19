use polyedge_api::build_app;
use polyedge_common::{bind_service_listener, service_socket_addr, shutdown_signal_then};
use polyedge_infrastructure::{AppState, Runtime, telemetry::init_tracing};
use polyedge_worker::WorkerRuntime;
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
    let addr = service_socket_addr(
        &state.settings.server.host,
        state.settings.server.port,
        "API",
        "API_BIND_ADDR_INVALID",
    )?;
    let listener = bind_service_listener(addr, "API", "API_BIND_FAILED").await?;

    info!(address = %addr, "polyedge api listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal_then(async move {
            worker_runtime.shutdown().await;
        }))
        .await
        .map_err(|error| {
            polyedge_domain::AppError::internal(
                "API_SERVER_FAILED",
                format!("api server failed: {error}"),
            )
        })
}
