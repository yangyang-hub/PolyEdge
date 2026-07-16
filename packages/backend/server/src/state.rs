use crate::{
    config::ServerConfig,
    error::Result,
    orderbook::{OrderbookRuntimeConfig, OrderbookSupervisor},
    store::PostgresStore,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<ServerConfig>,
    pub store: PostgresStore,
    pub orderbooks: Arc<OrderbookSupervisor>,
}

impl AppState {
    pub async fn load(config: ServerConfig) -> Result<Self> {
        let store =
            PostgresStore::connect(&config.database_url, config.postgres_max_connections).await?;
        store.migrate().await?;
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
        })
    }
}
