//! Backend configuration: strongly-typed settings structs loaded from environment
//! variables. Struct definitions and the `Settings` loader live here; `Default`
//! impls, value parsers, and tests are split out and inlined with `include!`
//! (sub-files share this module's imports). Runtime-config overrides live in the
//! `runtime_config` submodule.

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
    pub orderbook_stream: OrderbookStreamSettings,
    pub orderbook: OrderbookServiceSettings,
    pub auth: AuthSettings,
    pub copytrade: CopyTradeSettings,
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
    #[serde(rename = "poly_1271", alias = "poly1271", alias = "POLY_1271")]
    Poly1271,
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
    pub polygon_rpc_url: String,
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
    pub ai_openai_api_key: Option<String>,
    pub ai_anthropic_api_key: Option<String>,
    pub ai_openai_base_url: String,
    pub ai_anthropic_base_url: String,
    pub ai_model: String,
    pub ai_min_confidence_bps: u16,
    pub ai_request_timeout_secs: u64,
    pub ai_max_markets_per_cycle: u16,
    pub info_risk_interval_secs: u64,
    pub info_risk_max_markets_per_cycle: u16,
    pub info_risk_min_confidence_bps: u16,
    pub info_risk_web_search_enabled: bool,
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
    pub poll_reward_info_risks: bool,
    pub drain_execution_queue: bool,
    pub poll_paper_order_statuses: bool,
    pub reconcile_paper_fills: bool,
    pub poll_polymarket_order_statuses: bool,
    pub reconcile_polymarket_fills: bool,
    pub consume_polymarket_user_events: bool,
    pub poll_market_sync: bool,
    pub consume_orderbook_stream: bool,
    pub poll_copytrade: bool,
    pub analyze_wallets: bool,
    pub recompute_signals: bool,
    pub news_promotion_interval_secs: u64,
    pub signal_recompute_interval_secs: u64,
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
pub struct OrderbookStreamSettings {
    pub max_tokens: usize,
    pub max_levels_per_side: usize,
    pub poll_reconcile_interval_secs: u64,
    pub stale_threshold_ms: u64,
    pub book_ttl_ms: u64,
    pub token_refresh_interval_secs: u64,
    pub restart_interval_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct OrderbookServiceSettings {
    /// HTTP port for the standalone orderbook service.
    pub port: u16,
    /// URL of the orderbook service for other services to connect to.
    pub service_url: String,
    /// Shared secret required by orderbook write/register endpoints.
    pub write_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CopyTradeSettings {
    pub enabled: bool,
    pub poll_interval_secs: u64,
    pub analysis_interval_secs: u64,
    pub wallet_activity_limit: u16,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AuthSettings {
    pub disabled: bool,
    pub issuer: String,
    pub audience: String,
    pub clock_skew_secs: i64,
    pub max_query_ttl_secs: i64,
    pub max_write_ttl_secs: i64,
    pub max_step_up_window_secs: i64,
    pub step_up_code: String,
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

include!("settings/defaults.rs");
include!("settings/parsers.rs");
include!("settings/tests.rs");
