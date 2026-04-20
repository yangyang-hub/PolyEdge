use config::{Config, Environment, File};
use polyedge_domain::{ExposureRatio, Probability, SignedUsdAmount, SystemMode, UsdAmount};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub server: ServerSettings,
    pub postgres: DatabaseSettings,
    pub redis: RedisSettings,
    pub runtime: RuntimeSettings,
    pub risk: RiskSettings,
    pub polymarket: PolymarketSettings,
    pub auth: AuthSettings,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseSettings {
    pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedisSettings {
    pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeSettings {
    pub environment: String,
    pub initial_mode: SystemMode,
}

#[derive(Debug, Clone, Deserialize)]
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
pub enum PolymarketConnectorMode {
    Disabled,
    Mock,
    Live,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolymarketSignatureType {
    Eoa,
    Proxy,
    GnosisSafe,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PolymarketSettings {
    pub mode: PolymarketConnectorMode,
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

impl Settings {
    pub fn load() -> polyedge_domain::Result<Self> {
        let builder = Config::builder()
            .add_source(File::with_name("config/default").required(false))
            .add_source(Environment::with_prefix("POLYEDGE").separator("__"));

        let config = builder.build().map_err(|error| {
            polyedge_domain::AppError::internal(
                "CONFIG_BUILD_FAILED",
                format!("failed to build configuration: {error}"),
            )
        })?;

        config.try_deserialize().map_err(|error| {
            polyedge_domain::AppError::internal(
                "CONFIG_DESERIALIZE_FAILED",
                format!("failed to deserialize configuration: {error}"),
            )
        })
    }

    #[must_use]
    pub fn for_test(
        initial_mode: SystemMode,
        environment: impl Into<String>,
        public_keys: Vec<AuthKeySettings>,
    ) -> Self {
        Self {
            server: ServerSettings {
                host: "127.0.0.1".to_string(),
                port: 3000,
            },
            postgres: DatabaseSettings { url: None },
            redis: RedisSettings { url: None },
            runtime: RuntimeSettings {
                environment: environment.into(),
                initial_mode,
            },
            risk: RiskSettings {
                exposure_reference_nav: UsdAmount::new(
                    rust_decimal::Decimal::from_str_exact("100.00").expect("usd amount"),
                )
                .expect("exposure reference nav"),
                initial_daily_pnl: SignedUsdAmount::new(rust_decimal::Decimal::ZERO)
                    .expect("signed usd amount"),
                initial_gross_exposure: ExposureRatio::new(rust_decimal::Decimal::ZERO)
                    .expect("gross exposure"),
                initial_net_exposure: ExposureRatio::new(rust_decimal::Decimal::ZERO)
                    .expect("net exposure"),
                initial_open_alerts: 0,
                initial_kill_switch: false,
                min_signal_confidence: Probability::new(
                    rust_decimal::Decimal::from_str_exact("0.55").expect("probability"),
                )
                .expect("min confidence"),
                min_edge_to_execute: Probability::new(
                    rust_decimal::Decimal::from_str_exact("0.03").expect("probability"),
                )
                .expect("min edge"),
                max_open_alerts: 3,
                max_daily_loss: UsdAmount::new(
                    rust_decimal::Decimal::from_str_exact("5000.00").expect("usd amount"),
                )
                .expect("max daily loss"),
                max_gross_exposure: ExposureRatio::new(
                    rust_decimal::Decimal::from_str_exact("0.50").expect("gross limit"),
                )
                .expect("gross limit"),
                max_net_exposure: ExposureRatio::new(
                    rust_decimal::Decimal::from_str_exact("0.30").expect("net limit"),
                )
                .expect("net limit"),
            },
            polymarket: PolymarketSettings {
                mode: PolymarketConnectorMode::Mock,
                account_id: "polymarket_account".to_string(),
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
            },
            auth: AuthSettings {
                issuer: "polyedge-nextjs".to_string(),
                audience: "polyedge-rust-api".to_string(),
                clock_skew_secs: 30,
                max_query_ttl_secs: 60,
                max_write_ttl_secs: 30,
                max_step_up_window_secs: 600,
                revoked_sessions: Vec::new(),
                force_reauth_after: None,
                keys: public_keys,
            },
        }
    }
}
