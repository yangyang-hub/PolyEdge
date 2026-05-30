use crate::{
    auth::InternalTokenVerifier,
    catalog::{InMemoryMarketEventStore, PostgresMarketEventStore},
    settings::Settings,
    stores::{
        ExternalEventStore, InMemoryAuditLogSink, InMemoryCopyTradeStore, InMemoryExternalEventStore,
        InMemoryIdempotencyStore, InMemoryModeStateStore, InMemoryOrderbookCache,
        InMemoryRewardBotStore, InMemoryRiskStateStore, InMemoryRuntimeConfigStore,
        PostgresAuditLogSink, PostgresCopyTradeStore, PostgresExternalEventStore,
        PostgresIdempotencyStore, PostgresModeStateStore, PostgresRewardBotStore,
        PostgresRiskStateStore, PostgresRuntimeConfigStore, RedisOrderbookCache, RuntimeConfigStore,
    },
};
use polyedge_application::{
    ArbitrageService, ArbitrageStore, AuditLogSink, CopyTradeService, CopyTradeStore,
    ExecutionService, IdempotencyStore, MarketEventService, MarketEventStore, ModeStateStore,
    NewsIngestionService, NewsIngestionStore, OrderbookCache, RewardBotService, RewardBotStore,
    RiskPolicy, RiskService, RiskStateStore, SystemModeService,
};
use polyedge_domain::{AppError, Result, SystemMode};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::sync::Arc;
use tracing::info;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

#[derive(Clone)]
pub struct AppState {
    pub settings: Arc<Settings>,
    pub dependencies: Arc<RuntimeDependencies>,
    pub runtime_config_store: Arc<dyn RuntimeConfigStore>,
    pub auth_verifier: Arc<InternalTokenVerifier>,
    pub idempotency_store: Arc<dyn IdempotencyStore>,
    pub external_event_store: Arc<dyn ExternalEventStore>,
    pub system_mode_service: Arc<SystemModeService>,
    pub market_event_service: Arc<MarketEventService>,
    pub news_ingestion_service: Arc<NewsIngestionService>,
    pub arbitrage_service: Arc<ArbitrageService>,
    pub reward_bot_service: Arc<RewardBotService>,
    pub risk_service: Arc<RiskService>,
    pub execution_service: Arc<ExecutionService>,
    pub copytrade_service: Arc<CopyTradeService>,
    pub orderbook_cache: Arc<dyn OrderbookCache>,
}

pub struct Runtime {
    state: AppState,
}

#[derive(Debug)]
pub struct RuntimeDependencies {
    pub postgres: Option<PgPool>,
    pub redis: Option<redis::Client>,
}

impl Runtime {
    pub async fn load() -> Result<Self> {
        let settings = Settings::load()?;
        Self::from_settings(settings).await
    }

    pub async fn from_settings(mut settings: Settings) -> Result<Self> {
        let postgres = connect_postgres(
            settings.postgres.url.as_deref(),
            settings.postgres.max_connections,
        )
        .await?;
        let redis = connect_redis(settings.redis.url.as_deref()).await?;
        let runtime_config_store: Arc<dyn RuntimeConfigStore> = match postgres.clone() {
            Some(pool) => {
                let store = Arc::new(PostgresRuntimeConfigStore::new(pool));
                store
                    .bootstrap(&settings.runtime_config_value_map())
                    .await?;
                settings.apply_runtime_config_values(store.load_values().await?)?;
                store
            }
            None => Arc::new(InMemoryRuntimeConfigStore::new(
                settings.runtime_config_value_map(),
            )),
        };
        let settings = Arc::new(settings);
        let auth_verifier = Arc::new(InternalTokenVerifier::from_settings(&settings.auth)?);
        let (
            mode_store,
            risk_state_store,
            idempotency_store,
            external_event_store,
            audit_log_sink,
        ): (
            Arc<dyn ModeStateStore>,
            Arc<dyn RiskStateStore>,
            Arc<dyn IdempotencyStore>,
            Arc<dyn ExternalEventStore>,
            Arc<dyn AuditLogSink>,
        ) = if let Some(pool) = postgres.clone() {
            let mode_store = Arc::new(PostgresModeStateStore::new(
                pool.clone(),
                settings.runtime.initial_mode,
                settings.runtime.environment.clone(),
            ));
            mode_store.bootstrap().await?;
            let risk_state_store = Arc::new(PostgresRiskStateStore::new(
                pool.clone(),
                settings.risk.initial_kill_switch,
                settings.risk.initial_daily_pnl,
                settings.risk.initial_gross_exposure,
                settings.risk.initial_net_exposure,
                settings.risk.initial_open_alerts,
            ));
            risk_state_store.bootstrap().await?;
            (
                mode_store,
                risk_state_store,
                Arc::new(PostgresIdempotencyStore::new(pool.clone())),
                Arc::new(PostgresExternalEventStore::new(pool.clone())),
                Arc::new(PostgresAuditLogSink::new(pool)),
            )
        } else {
            (
                Arc::new(InMemoryModeStateStore::new(
                    settings.runtime.initial_mode,
                    settings.runtime.environment.clone(),
                )),
                Arc::new(InMemoryRiskStateStore::new(
                    settings.risk.initial_kill_switch,
                    settings.risk.initial_daily_pnl,
                    settings.risk.initial_gross_exposure,
                    settings.risk.initial_net_exposure,
                    settings.risk.initial_open_alerts,
                )),
                Arc::new(InMemoryIdempotencyStore::new()),
                Arc::new(InMemoryExternalEventStore::new()),
                Arc::new(InMemoryAuditLogSink::new()),
            )
        };
        let (market_event_store, news_ingestion_store, arbitrage_store, reward_bot_store): (
            Arc<dyn MarketEventStore>,
            Arc<dyn NewsIngestionStore>,
            Arc<dyn ArbitrageStore>,
            Arc<dyn RewardBotStore>,
        ) = if let Some(pool) = postgres.clone() {
            let store = Arc::new(PostgresMarketEventStore::new(pool.clone()));
            (
                store.clone(),
                store.clone(),
                store,
                Arc::new(PostgresRewardBotStore::new(pool)),
            )
        } else {
            let store = Arc::new(InMemoryMarketEventStore::new());
            (
                store.clone(),
                store.clone(),
                store,
                Arc::new(InMemoryRewardBotStore::new()),
            )
        };
        let copytrade_store: Arc<dyn CopyTradeStore> = if let Some(pool) = postgres.clone() {
            Arc::new(PostgresCopyTradeStore::new(pool))
        } else {
            Arc::new(InMemoryCopyTradeStore::new())
        };
        let system_mode_service = Arc::new(SystemModeService::new(
            mode_store.clone(),
            idempotency_store.clone(),
            audit_log_sink.clone(),
        ));
        let market_event_service = Arc::new(MarketEventService::new(market_event_store));
        let news_ingestion_service = Arc::new(NewsIngestionService::new(news_ingestion_store));
        let arbitrage_service = Arc::new(ArbitrageService::new(arbitrage_store));
        let reward_bot_service = Arc::new(RewardBotService::new(reward_bot_store, mode_store));
        let copytrade_service = Arc::new(CopyTradeService::new(copytrade_store));
        let execution_audit_log_sink = audit_log_sink.clone();
        let risk_service = Arc::new(RiskService::new(
            risk_policy(&settings),
            risk_state_store,
            market_event_service.clone(),
            system_mode_service.clone(),
            audit_log_sink,
        ));
        let execution_service = Arc::new(ExecutionService::new(
            market_event_service.clone(),
            risk_service.clone(),
            execution_audit_log_sink,
        ));

        let orderbook_cache: Arc<dyn OrderbookCache> = match &redis {
            Some(client) => Arc::new(RedisOrderbookCache::new(
                client.clone(),
                settings.orderbook_stream.book_ttl_secs,
            )),
            None => Arc::new(InMemoryOrderbookCache::new()),
        };

        Ok(Self {
            state: AppState {
                settings,
                dependencies: Arc::new(RuntimeDependencies { postgres, redis }),
                runtime_config_store,
                auth_verifier,
                idempotency_store,
                external_event_store,
                system_mode_service,
                market_event_service,
                news_ingestion_service,
                arbitrage_service,
                reward_bot_service,
                copytrade_service,
                risk_service,
                execution_service,
                orderbook_cache,
            },
        })
    }

    pub fn test_app_state(settings: Settings) -> Result<AppState> {
        let settings = Arc::new(settings);
        let auth_verifier = Arc::new(InternalTokenVerifier::from_settings(&settings.auth)?);
        let mode_store: Arc<dyn ModeStateStore> = Arc::new(InMemoryModeStateStore::new(
            settings.runtime.initial_mode,
            settings.runtime.environment.clone(),
        ));
        let risk_state_store: Arc<dyn RiskStateStore> = Arc::new(InMemoryRiskStateStore::new(
            settings.risk.initial_kill_switch,
            settings.risk.initial_daily_pnl,
            settings.risk.initial_gross_exposure,
            settings.risk.initial_net_exposure,
            settings.risk.initial_open_alerts,
        ));
        let idempotency_store: Arc<dyn IdempotencyStore> =
            Arc::new(InMemoryIdempotencyStore::new());
        let runtime_config_store: Arc<dyn RuntimeConfigStore> = Arc::new(
            InMemoryRuntimeConfigStore::new(settings.runtime_config_value_map()),
        );
        let external_event_store: Arc<dyn ExternalEventStore> =
            Arc::new(InMemoryExternalEventStore::new());
        let audit_log_sink: Arc<dyn AuditLogSink> = Arc::new(InMemoryAuditLogSink::new());
        let market_event_store = Arc::new(InMemoryMarketEventStore::new());
        let reward_bot_store = Arc::new(InMemoryRewardBotStore::new());
        let copytrade_store: Arc<dyn CopyTradeStore> = Arc::new(InMemoryCopyTradeStore::new());
        let system_mode_service = Arc::new(SystemModeService::new(
            mode_store.clone(),
            idempotency_store.clone(),
            audit_log_sink.clone(),
        ));
        let market_event_service = Arc::new(MarketEventService::new(market_event_store.clone()));
        let news_ingestion_service =
            Arc::new(NewsIngestionService::new(market_event_store.clone()));
        let arbitrage_service = Arc::new(ArbitrageService::new(market_event_store));
        let reward_bot_service = Arc::new(RewardBotService::new(reward_bot_store, mode_store));
        let copytrade_service = Arc::new(CopyTradeService::new(copytrade_store));
        let execution_audit_log_sink = audit_log_sink.clone();
        let risk_service = Arc::new(RiskService::new(
            risk_policy(&settings),
            risk_state_store,
            market_event_service.clone(),
            system_mode_service.clone(),
            audit_log_sink,
        ));
        let execution_service = Arc::new(ExecutionService::new(
            market_event_service.clone(),
            risk_service.clone(),
            execution_audit_log_sink,
        ));

        let orderbook_cache: Arc<dyn OrderbookCache> = Arc::new(InMemoryOrderbookCache::new());

        Ok(AppState {
            settings,
            dependencies: Arc::new(RuntimeDependencies {
                postgres: None,
                redis: None,
            }),
            runtime_config_store,
            auth_verifier,
            idempotency_store,
            external_event_store,
            system_mode_service,
            market_event_service,
            news_ingestion_service,
            arbitrage_service,
            reward_bot_service,
            copytrade_service,
            risk_service,
            execution_service,
            orderbook_cache,
        })
    }

    #[must_use]
    pub fn app_state(&self) -> AppState {
        self.state.clone()
    }
}

impl RuntimeDependencies {
    pub async fn postgres_ready(&self) -> Result<()> {
        let Some(pool) = &self.postgres else {
            return Ok(());
        };

        sqlx::query("SELECT 1")
            .execute(pool)
            .await
            .map_err(|error| {
                AppError::dependency_unavailable(
                    "POSTGRES_NOT_READY",
                    format!("postgres readiness check failed: {error}"),
                )
            })?;

        Ok(())
    }

    pub async fn redis_ready(&self) -> Result<()> {
        let Some(client) = &self.redis else {
            return Ok(());
        };

        let mut connection = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|error| {
                AppError::dependency_unavailable(
                    "REDIS_NOT_READY",
                    format!("redis connection failed: {error}"),
                )
            })?;

        let pong: String = redis::cmd("PING")
            .query_async(&mut connection)
            .await
            .map_err(|error| {
                AppError::dependency_unavailable(
                    "REDIS_NOT_READY",
                    format!("redis ping failed: {error}"),
                )
            })?;

        if pong != "PONG" {
            return Err(AppError::dependency_unavailable(
                "REDIS_NOT_READY",
                format!("unexpected redis ping response: {pong}"),
            ));
        }

        Ok(())
    }
}

fn risk_policy(settings: &Settings) -> RiskPolicy {
    RiskPolicy {
        exposure_reference_nav: settings.risk.exposure_reference_nav,
        min_signal_confidence: settings.risk.min_signal_confidence,
        min_edge_to_execute: settings.risk.min_edge_to_execute,
        max_open_alerts: settings.risk.max_open_alerts,
        max_daily_loss: settings.risk.max_daily_loss,
        max_gross_exposure: settings.risk.max_gross_exposure,
        max_net_exposure: settings.risk.max_net_exposure,
    }
}

async fn connect_postgres(url: Option<&str>, max_connections: u32) -> Result<Option<PgPool>> {
    let Some(url) = url.filter(|value| !value.trim().is_empty()) else {
        info!("postgres connection is not configured");
        return Ok(None);
    };

    if max_connections == 0 {
        return Err(AppError::invalid_input(
            "POSTGRES_MAX_CONNECTIONS_INVALID",
            "postgres max_connections must be greater than zero",
        ));
    }

    let pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(url)
        .await
        .map_err(|error| {
            AppError::dependency_unavailable(
                "POSTGRES_CONNECT_FAILED",
                format!("failed to connect to postgres: {error}"),
            )
        })?;

    MIGRATOR.run(&pool).await.map_err(|error| {
        AppError::dependency_unavailable(
            "POSTGRES_MIGRATION_FAILED",
            format!("failed to run embedded postgres migrations: {error}"),
        )
    })?;

    Ok(Some(pool))
}

async fn connect_redis(url: Option<&str>) -> Result<Option<redis::Client>> {
    let Some(url) = url.filter(|value| !value.trim().is_empty()) else {
        info!("redis connection is not configured");
        return Ok(None);
    };

    let client = redis::Client::open(url).map_err(|error| {
        AppError::dependency_unavailable(
            "REDIS_CONNECT_FAILED",
            format!("failed to construct redis client: {error}"),
        )
    })?;

    let mut connection = client
        .get_multiplexed_async_connection()
        .await
        .map_err(|error| {
            AppError::dependency_unavailable(
                "REDIS_CONNECT_FAILED",
                format!("failed to connect to redis: {error}"),
            )
        })?;

    let pong: String = redis::cmd("PING")
        .query_async(&mut connection)
        .await
        .map_err(|error| {
            AppError::dependency_unavailable(
                "REDIS_CONNECT_FAILED",
                format!("failed to ping redis: {error}"),
            )
        })?;

    if pong != "PONG" {
        return Err(AppError::dependency_unavailable(
            "REDIS_CONNECT_FAILED",
            format!("unexpected redis ping response: {pong}"),
        ));
    }

    Ok(Some(client))
}

#[allow(dead_code)]
fn _assert_mode_copy(mode: SystemMode) -> SystemMode {
    mode
}
