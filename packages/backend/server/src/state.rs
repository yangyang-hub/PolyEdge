use crate::{
    config::ServerConfig,
    error::Result,
    orderbook::{OrderbookRuntimeConfig, OrderbookSupervisor},
    store::PostgresStore,
    wallet_crypto::WalletCryptoService,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<ServerConfig>,
    pub store: PostgresStore,
    pub orderbooks: Arc<OrderbookSupervisor>,
    pub wallet_crypto: Arc<WalletCryptoService>,
}

impl AppState {
    pub async fn load(config: ServerConfig) -> Result<Self> {
        let wallet_crypto = Arc::new(WalletCryptoService::new(&config.wallet_crypto));
        let store =
            PostgresStore::connect(&config.database_url, config.postgres_max_connections).await?;
        store.migrate().await?;
        store
            .bootstrap_environment_admin(
                &config.bootstrap_admin_username,
                &config.bootstrap_admin_display_name,
                &config.bootstrap_admin_password_hash,
                config.bootstrap_admin_credential_version,
            )
            .await?;
        let orderbooks = Arc::new(OrderbookSupervisor::new(
            store.clone(),
            OrderbookRuntimeConfig {
                clob_host: config.clob_host.clone(),
                max_tokens: config.orderbook_max_tokens,
                poll_interval: config.orderbook_poll_interval,
            },
        )?);
        Ok(Self {
            config: Arc::new(config),
            store,
            orderbooks,
            wallet_crypto,
        })
    }
}
