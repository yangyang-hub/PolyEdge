use config::{Config, Environment};
use polyedge_domain::{
    AppError, Edge, ExposureRatio, Probability, Quantity, SignedUsdAmount, SystemMode, UsdAmount,
};
use serde::{Deserialize, Serialize};

const AUTH_KEYS_JSON_ENV: &str = "POLYEDGE_AUTH__KEYS_JSON";
const NEWS_SOURCES_JSON_ENV: &str = "POLYEDGE_NEWS__SOURCES_JSON";

mod runtime_config;
pub use runtime_config::{RuntimeConfigEntry, RuntimeConfigValueType};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub server: ServerSettings,
    pub postgres: DatabaseSettings,
    pub redis: RedisSettings,
    pub runtime: RuntimeSettings,
    pub risk: RiskSettings,
    pub polymarket: PolymarketSettings,
    pub arbitrage: ArbitrageSettings,
    pub rewards: RewardsSettings,
    pub news: NewsSettings,
    pub worker: WorkerSettings,
    pub auth: AuthSettings,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DatabaseSettings {
    pub url: Option<String>,
    pub max_connections: u32,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct RedisSettings {
    pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct RuntimeSettings {
    pub environment: String,
    pub initial_mode: SystemMode,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct RiskSettings {
    pub exposure_reference_nav: UsdAmount,
    pub initial_daily_pnl: SignedUsdAmount,
    pub initial_gross_exposure: ExposureRatio,
    pub initial_net_exposure: ExposureRatio,
    pub initial_open_alerts: u32,
    pub initial_kill_switch: bool,
    pub min_signal_confidence: Probability,
    pub min_edge_to_execute: Probability,
    pub max_open_alerts: u32,
    pub max_daily_loss: UsdAmount,
    pub max_gross_exposure: ExposureRatio,
    pub max_net_exposure: ExposureRatio,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolymarketSignatureType {
    Eoa,
    Proxy,
    GnosisSafe,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PolymarketSettings {
    pub account_id: String,
    pub chain_id: u64,
    pub signature_type: PolymarketSignatureType,
    pub funder: Option<String>,
    pub private_key: Option<String>,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub api_passphrase: Option<String>,
    pub clob_host: String,
    pub ws_host: String,
    pub gamma_host: String,
    pub data_api_host: String,
    pub order_status_poll_limit: u16,
    pub fill_poll_limit: u16,
    pub ws_max_instruments: usize,
    pub ws_idle_warn_secs: u64,
    pub ws_stale_after_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ArbitrageSettings {
    pub enabled: bool,
    pub poll_interval_secs: u64,
    pub scan_limit: u16,
    pub scanner_version: String,
    pub book_source: String,
    pub analysis_lookback_hours: u16,
    pub max_book_age_ms: u64,
    pub opportunity_ttl_secs: u64,
    pub event_retention_hours: u64,
    pub min_gross_edge: Edge,
    pub min_capacity: Quantity,
    pub fee_buffer: Edge,
    pub slippage_buffer: Edge,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct RewardsSettings {
    pub enabled: bool,
    pub poll_interval_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct NewsSettings {
    pub enabled: bool,
    pub poll_interval_secs: u64,
    pub request_timeout_secs: u64,
    pub max_items_per_source: usize,
    #[serde(default)]
    pub sources: Vec<NewsSourceSettings>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct NewsSourceSettings {
    pub id: String,
    pub source_type: String,
    pub url: String,
    pub reliability: Probability,
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct WorkerSettings {
    pub poll_news: bool,
    pub promote_news_events: bool,
    pub poll_arbitrage_radar: bool,
    pub analyze_arbitrage_opportunities: bool,
    pub poll_reward_bot: bool,
    pub drain_execution_queue: bool,
    pub poll_paper_order_statuses: bool,
    pub reconcile_paper_fills: bool,
    pub poll_polymarket_order_statuses: bool,
    pub reconcile_polymarket_fills: bool,
    pub consume_polymarket_user_events: bool,
    pub poll_market_sync: bool,
    pub news_promotion_interval_secs: u64,
    pub arbitrage_analysis_interval_secs: u64,
    pub execution_drain_interval_secs: u64,
    pub order_status_poll_interval_secs: u64,
    pub fill_reconciliation_interval_secs: u64,
    pub polymarket_user_event_restart_interval_secs: u64,
    pub market_sync_interval_secs: u64,
    pub task_limit: u16,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AuthSettings {
    pub issuer: String,
    pub audience: String,
    pub clock_skew_secs: i64,
    pub max_query_ttl_secs: i64,
    pub max_write_ttl_secs: i64,
    pub max_step_up_window_secs: i64,
    #[serde(default)]
    pub revoked_sessions: Vec<String>,
    pub force_reauth_after: Option<String>,
    #[serde(default)]
    pub keys: Vec<AuthKeySettings>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthKeySettings {
    pub kid: String,
    pub public_key_base64: String,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 38001,
        }
    }
}

impl Default for DatabaseSettings {
    fn default() -> Self {
        Self {
            url: None,
            max_connections: 20,
        }
    }
}

impl Default for RuntimeSettings {
    fn default() -> Self {
        Self {
            environment: "local".to_string(),
            initial_mode: SystemMode::PaperTrade,
        }
    }
}

impl Default for RiskSettings {
    fn default() -> Self {
        Self {
            exposure_reference_nav: usd_amount("100.00"),
            initial_daily_pnl: signed_usd_amount("0.00"),
            initial_gross_exposure: exposure_ratio("0"),
            initial_net_exposure: exposure_ratio("0"),
            initial_open_alerts: 0,
            initial_kill_switch: false,
            min_signal_confidence: probability("0.55"),
            min_edge_to_execute: probability("0.03"),
            max_open_alerts: 3,
            max_daily_loss: usd_amount("5000.00"),
            max_gross_exposure: exposure_ratio("0.50"),
            max_net_exposure: exposure_ratio("0.30"),
        }
    }
}

impl Default for PolymarketSignatureType {
    fn default() -> Self {
        Self::Eoa
    }
}

impl Default for PolymarketSettings {
    fn default() -> Self {
        Self {
            account_id: String::new(),
            chain_id: 137,
            signature_type: PolymarketSignatureType::Eoa,
            funder: None,
            private_key: None,
            api_key: None,
            api_secret: None,
            api_passphrase: None,
            clob_host: "https://clob.polymarket.com".to_string(),
            ws_host: "wss://ws-subscriptions-clob.polymarket.com/ws/market".to_string(),
            gamma_host: "https://gamma-api.polymarket.com".to_string(),
            data_api_host: "https://data-api.polymarket.com".to_string(),
            order_status_poll_limit: 100,
            fill_poll_limit: 100,
            ws_max_instruments: 100,
            ws_idle_warn_secs: 15,
            ws_stale_after_secs: 60,
        }
    }
}

impl Default for ArbitrageSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            poll_interval_secs: 5,
            scan_limit: 100,
            scanner_version: "v1".to_string(),
            book_source: "market_snapshot".to_string(),
            analysis_lookback_hours: 24,
            max_book_age_ms: 10_000,
            opportunity_ttl_secs: 60,
            event_retention_hours: 24,
            min_gross_edge: edge("0.005"),
            min_capacity: quantity("1"),
            fee_buffer: edge("0.005"),
            slippage_buffer: edge("0.005"),
        }
    }
}

impl Default for RewardsSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            poll_interval_secs: 60,
        }
    }
}

impl Default for NewsSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            poll_interval_secs: 60,
            request_timeout_secs: 10,
            max_items_per_source: 50,
            sources: Vec::new(),
        }
    }
}

impl Default for NewsSourceSettings {
    fn default() -> Self {
        Self {
            id: String::new(),
            source_type: "news".to_string(),
            url: String::new(),
            reliability: probability("0.50"),
            enabled: true,
        }
    }
}

impl Default for WorkerSettings {
    fn default() -> Self {
        Self {
            poll_news: false,
            promote_news_events: false,
            poll_arbitrage_radar: false,
            analyze_arbitrage_opportunities: false,
            poll_reward_bot: false,
            drain_execution_queue: false,
            poll_paper_order_statuses: false,
            reconcile_paper_fills: false,
            poll_polymarket_order_statuses: false,
            reconcile_polymarket_fills: false,
            consume_polymarket_user_events: false,
            poll_market_sync: true,
            news_promotion_interval_secs: 60,
            arbitrage_analysis_interval_secs: 300,
            execution_drain_interval_secs: 5,
            order_status_poll_interval_secs: 15,
            fill_reconciliation_interval_secs: 15,
            polymarket_user_event_restart_interval_secs: 5,
            market_sync_interval_secs: 300,
            task_limit: 100,
        }
    }
}

impl Default for AuthSettings {
    fn default() -> Self {
        Self {
            issuer: "polyedge-nextjs".to_string(),
            audience: "polyedge-rust-api".to_string(),
            clock_skew_secs: 30,
            max_query_ttl_secs: 60,
            max_write_ttl_secs: 30,
            max_step_up_window_secs: 600,
            revoked_sessions: Vec::new(),
            force_reauth_after: Some("2026-01-01T00:00:00Z".to_string()),
            keys: Vec::new(),
        }
    }
}

impl Settings {
    pub fn load() -> polyedge_domain::Result<Self> {
        if let Err(error) = dotenvy::dotenv() {
            if !error.not_found() {
                return Err(AppError::internal(
                    "CONFIG_DOTENV_FAILED",
                    format!("failed to load .env file: {error}"),
                ));
            }
        }

        Self::load_from_environment(
            environment_source(),
            std::env::var(AUTH_KEYS_JSON_ENV).ok(),
            std::env::var(NEWS_SOURCES_JSON_ENV).ok(),
        )
    }

    #[must_use]
    pub fn for_test(
        initial_mode: SystemMode,
        environment: impl Into<String>,
        public_keys: Vec<AuthKeySettings>,
    ) -> Self {
        let mut settings = Self::default();
        settings.server.port = 38001;
        settings.runtime.environment = environment.into();
        settings.runtime.initial_mode = initial_mode;
        settings.auth.force_reauth_after = None;
        settings.auth.keys = public_keys;
        settings
    }

    fn load_from_environment(
        source: Environment,
        auth_keys_json: Option<String>,
        news_sources_json: Option<String>,
    ) -> polyedge_domain::Result<Self> {
        let config = Config::builder()
            .add_source(source)
            .build()
            .map_err(|error| {
                AppError::internal(
                    "CONFIG_BUILD_FAILED",
                    format!("failed to build configuration: {error}"),
                )
            })?;

        let mut settings = Self::from_config(config)?;

        if let Some(raw_keys) = auth_keys_json.filter(|value| !value.trim().is_empty()) {
            settings.auth.keys = serde_json::from_str(&raw_keys).map_err(|error| {
                AppError::internal(
                    "CONFIG_AUTH_KEYS_JSON_INVALID",
                    format!("failed to parse {AUTH_KEYS_JSON_ENV}: {error}"),
                )
            })?;
        }

        if let Some(raw_sources) = news_sources_json.filter(|value| !value.trim().is_empty()) {
            settings.news.sources = serde_json::from_str(&raw_sources).map_err(|error| {
                AppError::internal(
                    "CONFIG_NEWS_SOURCES_JSON_INVALID",
                    format!("failed to parse {NEWS_SOURCES_JSON_ENV}: {error}"),
                )
            })?;
        }

        Ok(settings)
    }

    fn from_config(config: Config) -> polyedge_domain::Result<Self> {
        config.try_deserialize().map_err(|error| {
            AppError::internal(
                "CONFIG_DESERIALIZE_FAILED",
                format!("failed to deserialize configuration: {error}"),
            )
        })
    }
}

fn environment_source() -> Environment {
    Environment::with_prefix("POLYEDGE")
        .prefix_separator("_")
        .separator("__")
        .ignore_empty(true)
        .try_parsing(true)
        .list_separator(",")
        .with_list_parse_key("auth.revoked_sessions")
}

fn decimal(value: &str) -> rust_decimal::Decimal {
    rust_decimal::Decimal::from_str_exact(value)
        .expect("static backend configuration default must be a valid decimal")
}

fn probability(value: &str) -> Probability {
    Probability::new(decimal(value)).expect("static backend configuration default must be valid")
}

fn edge(value: &str) -> Edge {
    Edge::new(decimal(value)).expect("static backend configuration default must be valid")
}

fn quantity(value: &str) -> Quantity {
    Quantity::new(decimal(value)).expect("static backend configuration default must be valid")
}

fn exposure_ratio(value: &str) -> ExposureRatio {
    ExposureRatio::new(decimal(value)).expect("static backend configuration default must be valid")
}

fn usd_amount(value: &str) -> UsdAmount {
    UsdAmount::new(decimal(value)).expect("static backend configuration default must be valid")
}

fn signed_usd_amount(value: &str) -> SignedUsdAmount {
    SignedUsdAmount::new(decimal(value))
        .expect("static backend configuration default must be valid")
}

#[cfg(test)]
mod tests {
    use super::{Settings, edge, environment_source, quantity};
    use std::collections::HashMap;

    #[test]
    fn settings_defaults_match_runtime_defaults() {
        let settings = Settings::from_config(config::Config::builder().build().expect("config"))
            .expect("settings");

        assert_eq!(settings.server.host, "0.0.0.0");
        assert_eq!(settings.server.port, 38001);
        assert_eq!(settings.runtime.environment, "local");
        assert_eq!(
            settings.runtime.initial_mode,
            polyedge_domain::SystemMode::PaperTrade
        );
        assert!(settings.polymarket.account_id.is_empty());
        assert!(!settings.news.enabled);
        assert_eq!(settings.news.poll_interval_secs, 60);
        assert_eq!(settings.news.request_timeout_secs, 10);
        assert_eq!(settings.news.max_items_per_source, 50);
        assert!(settings.news.sources.is_empty());
        assert!(!settings.rewards.enabled);
        assert_eq!(settings.rewards.poll_interval_secs, 60);
        assert!(!settings.worker.poll_news);
        assert!(!settings.worker.promote_news_events);
        assert!(!settings.worker.poll_arbitrage_radar);
        assert!(!settings.worker.analyze_arbitrage_opportunities);
        assert!(!settings.worker.poll_reward_bot);
        assert!(!settings.worker.drain_execution_queue);
        assert!(!settings.worker.poll_paper_order_statuses);
        assert!(!settings.worker.reconcile_paper_fills);
        assert!(!settings.worker.poll_polymarket_order_statuses);
        assert!(!settings.worker.reconcile_polymarket_fills);
        assert!(!settings.worker.consume_polymarket_user_events);
        assert_eq!(settings.worker.news_promotion_interval_secs, 60);
        assert_eq!(settings.worker.arbitrage_analysis_interval_secs, 300);
        assert_eq!(settings.worker.execution_drain_interval_secs, 5);
        assert_eq!(settings.worker.order_status_poll_interval_secs, 15);
        assert_eq!(settings.worker.fill_reconciliation_interval_secs, 15);
        assert_eq!(
            settings.worker.polymarket_user_event_restart_interval_secs,
            5
        );
        assert_eq!(settings.worker.task_limit, 100);
        assert!(!settings.arbitrage.enabled);
        assert_eq!(settings.arbitrage.poll_interval_secs, 5);
        assert_eq!(settings.arbitrage.scan_limit, 100);
        assert_eq!(settings.arbitrage.scanner_version, "v1");
        assert_eq!(settings.arbitrage.book_source, "market_snapshot");
        assert_eq!(settings.arbitrage.analysis_lookback_hours, 24);
        assert_eq!(settings.arbitrage.max_book_age_ms, 10_000);
        assert_eq!(settings.arbitrage.opportunity_ttl_secs, 60);
        assert_eq!(settings.arbitrage.event_retention_hours, 24);
        assert_eq!(settings.arbitrage.min_gross_edge, edge("0.005"));
        assert_eq!(settings.arbitrage.min_capacity, quantity("1"));
        assert_eq!(settings.arbitrage.fee_buffer, edge("0.005"));
        assert_eq!(settings.arbitrage.slippage_buffer, edge("0.005"));
        assert!(settings.postgres.url.is_none());
        assert_eq!(settings.postgres.max_connections, 20);
        assert!(settings.redis.url.is_none());
        assert_eq!(
            settings.auth.force_reauth_after.as_deref(),
            Some("2026-01-01T00:00:00Z")
        );
    }

    #[test]
    fn settings_can_be_loaded_from_environment_variables() {
        let source = environment_source().source(Some(HashMap::from([
            ("POLYEDGE_SERVER__PORT".to_string(), "9090".to_string()),
            (
                "POLYEDGE_POSTGRES__URL".to_string(),
                "postgres://postgres:postgres@localhost:5432/polyedge".to_string(),
            ),
            (
                "POLYEDGE_POSTGRES__MAX_CONNECTIONS".to_string(),
                "32".to_string(),
            ),
            (
                "POLYEDGE_RUNTIME__ENVIRONMENT".to_string(),
                "staging".to_string(),
            ),
            (
                "POLYEDGE_RUNTIME__INITIAL_MODE".to_string(),
                "live_auto".to_string(),
            ),
            (
                "POLYEDGE_RISK__INITIAL_KILL_SWITCH".to_string(),
                "true".to_string(),
            ),
            (
                "POLYEDGE_POLYMARKET__ACCOUNT_ID".to_string(),
                "acct_poly".to_string(),
            ),
            (
                "POLYEDGE_ARBITRAGE__ENABLED".to_string(),
                "true".to_string(),
            ),
            (
                "POLYEDGE_ARBITRAGE__POLL_INTERVAL_SECS".to_string(),
                "7".to_string(),
            ),
            (
                "POLYEDGE_ARBITRAGE__SCAN_LIMIT".to_string(),
                "42".to_string(),
            ),
            (
                "POLYEDGE_ARBITRAGE__SCANNER_VERSION".to_string(),
                "v_test".to_string(),
            ),
            (
                "POLYEDGE_ARBITRAGE__BOOK_SOURCE".to_string(),
                "polymarket".to_string(),
            ),
            (
                "POLYEDGE_ARBITRAGE__ANALYSIS_LOOKBACK_HOURS".to_string(),
                "12".to_string(),
            ),
            (
                "POLYEDGE_ARBITRAGE__MAX_BOOK_AGE_MS".to_string(),
                "2500".to_string(),
            ),
            (
                "POLYEDGE_ARBITRAGE__OPPORTUNITY_TTL_SECS".to_string(),
                "15".to_string(),
            ),
            (
                "POLYEDGE_ARBITRAGE__EVENT_RETENTION_HOURS".to_string(),
                "6".to_string(),
            ),
            (
                "POLYEDGE_ARBITRAGE__MIN_GROSS_EDGE".to_string(),
                "0.02".to_string(),
            ),
            (
                "POLYEDGE_ARBITRAGE__MIN_CAPACITY".to_string(),
                "50".to_string(),
            ),
            (
                "POLYEDGE_ARBITRAGE__FEE_BUFFER".to_string(),
                "0.003".to_string(),
            ),
            (
                "POLYEDGE_ARBITRAGE__SLIPPAGE_BUFFER".to_string(),
                "0.004".to_string(),
            ),
            ("POLYEDGE_REWARDS__ENABLED".to_string(), "true".to_string()),
            (
                "POLYEDGE_REWARDS__POLL_INTERVAL_SECS".to_string(),
                "45".to_string(),
            ),
            ("POLYEDGE_WORKER__POLL_NEWS".to_string(), "true".to_string()),
            (
                "POLYEDGE_WORKER__PROMOTE_NEWS_EVENTS".to_string(),
                "true".to_string(),
            ),
            (
                "POLYEDGE_WORKER__POLL_ARBITRAGE_RADAR".to_string(),
                "true".to_string(),
            ),
            (
                "POLYEDGE_WORKER__ANALYZE_ARBITRAGE_OPPORTUNITIES".to_string(),
                "true".to_string(),
            ),
            (
                "POLYEDGE_WORKER__POLL_REWARD_BOT".to_string(),
                "true".to_string(),
            ),
            (
                "POLYEDGE_WORKER__DRAIN_EXECUTION_QUEUE".to_string(),
                "true".to_string(),
            ),
            (
                "POLYEDGE_WORKER__POLL_PAPER_ORDER_STATUSES".to_string(),
                "true".to_string(),
            ),
            (
                "POLYEDGE_WORKER__RECONCILE_PAPER_FILLS".to_string(),
                "true".to_string(),
            ),
            (
                "POLYEDGE_WORKER__POLL_POLYMARKET_ORDER_STATUSES".to_string(),
                "true".to_string(),
            ),
            (
                "POLYEDGE_WORKER__RECONCILE_POLYMARKET_FILLS".to_string(),
                "true".to_string(),
            ),
            (
                "POLYEDGE_WORKER__CONSUME_POLYMARKET_USER_EVENTS".to_string(),
                "true".to_string(),
            ),
            (
                "POLYEDGE_WORKER__NEWS_PROMOTION_INTERVAL_SECS".to_string(),
                "30".to_string(),
            ),
            (
                "POLYEDGE_WORKER__ARBITRAGE_ANALYSIS_INTERVAL_SECS".to_string(),
                "120".to_string(),
            ),
            (
                "POLYEDGE_WORKER__EXECUTION_DRAIN_INTERVAL_SECS".to_string(),
                "6".to_string(),
            ),
            (
                "POLYEDGE_WORKER__ORDER_STATUS_POLL_INTERVAL_SECS".to_string(),
                "20".to_string(),
            ),
            (
                "POLYEDGE_WORKER__FILL_RECONCILIATION_INTERVAL_SECS".to_string(),
                "25".to_string(),
            ),
            (
                "POLYEDGE_WORKER__POLYMARKET_USER_EVENT_RESTART_INTERVAL_SECS".to_string(),
                "10".to_string(),
            ),
            ("POLYEDGE_WORKER__TASK_LIMIT".to_string(), "33".to_string()),
            (
                "POLYEDGE_POLYMARKET__PRIVATE_KEY".to_string(),
                "".to_string(),
            ),
            (
                "POLYEDGE_AUTH__REVOKED_SESSIONS".to_string(),
                "sess_alpha,sess_beta".to_string(),
            ),
            ("POLYEDGE_AUTH__KEYS_JSON".to_string(), "[]".to_string()),
        ])));

        let settings = Settings::load_from_environment(
            source,
            Some(
                r#"[{"kid":"local-dev","public_key_base64":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="}]"#
                    .to_string(),
            ),
            Some(
                r#"[{"id":"sec_feed","source_type":"official","url":"https://example.com/rss","reliability":"0.95","enabled":true}]"#
                    .to_string(),
            ),
        )
        .expect("settings");

        assert_eq!(settings.server.port, 9090);
        assert_eq!(
            settings.postgres.url.as_deref(),
            Some("postgres://postgres:postgres@localhost:5432/polyedge"),
        );
        assert_eq!(settings.postgres.max_connections, 32);
        assert_eq!(settings.runtime.environment, "staging");
        assert_eq!(
            settings.runtime.initial_mode,
            polyedge_domain::SystemMode::LiveAuto
        );
        assert!(settings.risk.initial_kill_switch);
        assert_eq!(settings.polymarket.account_id, "acct_poly");
        assert!(settings.polymarket.private_key.is_none());
        assert!(settings.arbitrage.enabled);
        assert_eq!(settings.arbitrage.poll_interval_secs, 7);
        assert_eq!(settings.arbitrage.scan_limit, 42);
        assert_eq!(settings.arbitrage.scanner_version, "v_test");
        assert_eq!(settings.arbitrage.book_source, "polymarket");
        assert_eq!(settings.arbitrage.analysis_lookback_hours, 12);
        assert_eq!(settings.arbitrage.max_book_age_ms, 2500);
        assert_eq!(settings.arbitrage.opportunity_ttl_secs, 15);
        assert_eq!(settings.arbitrage.event_retention_hours, 6);
        assert_eq!(settings.arbitrage.min_gross_edge, edge("0.02"));
        assert_eq!(settings.arbitrage.min_capacity, quantity("50"));
        assert_eq!(settings.arbitrage.fee_buffer, edge("0.003"));
        assert_eq!(settings.arbitrage.slippage_buffer, edge("0.004"));
        assert!(settings.rewards.enabled);
        assert_eq!(settings.rewards.poll_interval_secs, 45);
        assert!(settings.worker.poll_news);
        assert!(settings.worker.promote_news_events);
        assert!(settings.worker.poll_arbitrage_radar);
        assert!(settings.worker.analyze_arbitrage_opportunities);
        assert!(settings.worker.poll_reward_bot);
        assert!(settings.worker.drain_execution_queue);
        assert!(settings.worker.poll_paper_order_statuses);
        assert!(settings.worker.reconcile_paper_fills);
        assert!(settings.worker.poll_polymarket_order_statuses);
        assert!(settings.worker.reconcile_polymarket_fills);
        assert!(settings.worker.consume_polymarket_user_events);
        assert_eq!(settings.worker.news_promotion_interval_secs, 30);
        assert_eq!(settings.worker.arbitrage_analysis_interval_secs, 120);
        assert_eq!(settings.worker.execution_drain_interval_secs, 6);
        assert_eq!(settings.worker.order_status_poll_interval_secs, 20);
        assert_eq!(settings.worker.fill_reconciliation_interval_secs, 25);
        assert_eq!(
            settings.worker.polymarket_user_event_restart_interval_secs,
            10
        );
        assert_eq!(settings.worker.task_limit, 33);
        assert_eq!(
            settings.auth.revoked_sessions,
            vec!["sess_alpha".to_string(), "sess_beta".to_string()],
        );
        assert_eq!(settings.auth.keys.len(), 1);
        assert_eq!(settings.auth.keys[0].kid, "local-dev");
        assert_eq!(settings.news.sources.len(), 1);
        assert_eq!(settings.news.sources[0].id, "sec_feed");
        assert_eq!(settings.news.sources[0].source_type, "official");
        assert_eq!(settings.news.sources[0].url, "https://example.com/rss");
        assert!(settings.news.sources[0].enabled);
    }

    #[test]
    fn runtime_config_values_override_runtime_settings() {
        let mut settings = Settings::default();
        settings
            .apply_runtime_config_values(std::collections::BTreeMap::from([
                ("arbitrage.enabled".to_string(), "true".to_string()),
                ("arbitrage.scan_limit".to_string(), "25".to_string()),
                (
                    "polymarket.account_id".to_string(),
                    "acct_runtime".to_string(),
                ),
                ("worker.poll_news".to_string(), "true".to_string()),
                (
                    "news.sources_json".to_string(),
                    r#"[{"id":"sec","source_type":"official","url":"https://example.com/rss","reliability":"0.9","enabled":true}]"#
                        .to_string(),
                ),
            ]))
            .expect("runtime config values");

        assert!(settings.arbitrage.enabled);
        assert_eq!(settings.arbitrage.scan_limit, 25);
        assert_eq!(settings.polymarket.account_id, "acct_runtime");
        assert!(settings.worker.poll_news);
        assert_eq!(settings.news.sources.len(), 1);
        assert_eq!(settings.news.sources[0].id, "sec");
    }

    #[test]
    fn runtime_config_rejects_unknown_keys() {
        let values =
            std::collections::BTreeMap::from([("server.port".to_string(), "38002".to_string())]);

        let error = Settings::validate_runtime_config_keys(&values).expect_err("unknown key");

        assert_eq!(error.code(), "CONFIG_RUNTIME_KEY_UNSUPPORTED");
    }
}
