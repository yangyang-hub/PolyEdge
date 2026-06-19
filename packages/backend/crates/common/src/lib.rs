use std::{future::Future, net::SocketAddr};

use polyedge_domain::{AppError, Result};
use tokio::net::TcpListener;

pub fn service_socket_addr(
    host: &str,
    port: u16,
    service_name: &str,
    error_code: &'static str,
) -> Result<SocketAddr> {
    format!("{host}:{port}").parse().map_err(|error| {
        AppError::internal(
            error_code,
            format!("invalid {service_name} bind address: {error}"),
        )
    })
}

pub async fn bind_service_listener(
    addr: SocketAddr,
    service_name: &str,
    error_code: &'static str,
) -> Result<TcpListener> {
    TcpListener::bind(addr).await.map_err(|error| {
        AppError::dependency_unavailable(
            error_code,
            format!("failed to bind {service_name} listener: {error}"),
        )
    })
}

pub async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            let _ = signal.recv().await;
        } else {
            std::future::pending::<()>().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}

pub async fn shutdown_signal_then(after_signal: impl Future<Output = ()>) {
    shutdown_signal().await;
    after_signal.await;
}
