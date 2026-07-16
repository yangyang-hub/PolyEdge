use polyedge_connectors::PolymarketDataApiConnector;
use polyedge_server::{
    app,
    config::ServerConfig,
    error::{Result, ServerError},
    execution::{ExecutionRuntimeConfig, RuntimeSupervisor},
    secrets::WalletSecretResolver,
    state::AppState,
};
use std::{sync::Arc, time::Duration};
use tokio::sync::watch;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    dotenvy::dotenv().ok();
    let config = ServerConfig::from_env()?;
    let state = AppState::load(config.clone()).await?;
    let resolver = WalletSecretResolver::new(
        config.chain_id,
        config.clob_host.clone(),
        state.store.clone(),
        Arc::clone(&state.wallet_crypto),
    )?;
    let data_api = PolymarketDataApiConnector::new(&config.data_api_host)?;
    let execution = Arc::new(RuntimeSupervisor::new(
        Arc::new(state.store.clone()),
        Arc::clone(&state.orderbooks),
        resolver,
        data_api,
        ExecutionRuntimeConfig {
            poll_interval: config.reconcile_interval,
            lease_duration: Duration::from_secs(30),
            max_wallet_concurrency: config.wallet_concurrency,
        },
    )?);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let orderbook_task = tokio::spawn(Arc::clone(&state.orderbooks).run(shutdown_rx.clone()));
    let execution_task = tokio::spawn(execution.run(shutdown_rx));
    let listener = tokio::net::TcpListener::bind(config.bind_addr)
        .await
        .map_err(|error| ServerError::Internal(format!("failed to bind server: {error}")))?;
    info!(address = %config.bind_addr, "polyedge-server listening");
    let server = axum::serve(listener, app(state)).with_graceful_shutdown(shutdown_signal());
    let result = server.await;
    let _ = shutdown_tx.send(true);
    let _ = tokio::time::timeout(Duration::from_secs(5), orderbook_task).await;
    let _ = tokio::time::timeout(Duration::from_secs(5), execution_task).await;
    result.map_err(|error| ServerError::Internal(format!("server failed: {error}")))
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
        () = ctrl_c => {},
        () = terminate => {},
    }
}
