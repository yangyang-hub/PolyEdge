use crate::error::{Result, ServerError};
use std::{env, net::SocketAddr, str::FromStr, time::Duration};

const DEFAULT_CLOB_HOST: &str = "https://clob.polymarket.com";
const DEFAULT_DATA_API_HOST: &str = "https://data-api.polymarket.com";

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub postgres_max_connections: u32,
    pub environment: String,
    pub auth_disabled: bool,
    pub allow_insecure_private_deploy: bool,
    pub api_token: Option<String>,
    pub step_up_code: Option<String>,
    pub max_body_bytes: usize,
    pub cors_origins: Vec<String>,
    pub clob_host: String,
    pub data_api_host: String,
    pub chain_id: u64,
    pub wallet_secret_env_prefix: String,
    pub orderbook_max_tokens: usize,
    pub orderbook_poll_interval: Duration,
    pub wallet_concurrency: usize,
    pub reconcile_interval: Duration,
}

impl ServerConfig {
    pub fn from_env() -> Result<Self> {
        let host = env_value("POLYEDGE_SERVER__HOST").unwrap_or_else(|| "0.0.0.0".to_string());
        let port = parse_env("POLYEDGE_SERVER__PORT", 38_001_u16)?;
        let bind_addr = SocketAddr::from_str(&format!("{host}:{port}")).map_err(|error| {
            ServerError::Configuration(format!("invalid server bind address: {error}"))
        })?;
        let environment =
            env_value("POLYEDGE_RUNTIME__ENVIRONMENT").unwrap_or_else(|| "local".to_string());
        let auth_disabled = parse_bool_env("POLYEDGE_AUTH__DISABLED", true)?;
        let allow_insecure_private_deploy =
            parse_bool_env("POLYEDGE_AUTH__ALLOW_INSECURE_PRIVATE_DEPLOY", false)?;

        if environment.eq_ignore_ascii_case("production")
            && auth_disabled
            && !allow_insecure_private_deploy
        {
            return Err(ServerError::Configuration(
                "production auth disablement requires POLYEDGE_AUTH__ALLOW_INSECURE_PRIVATE_DEPLOY=true"
                    .to_string(),
            ));
        }

        let api_token = env_value("POLYEDGE_AUTH__API_TOKEN");
        if !auth_disabled {
            let valid = api_token
                .as_deref()
                .map(|token| token.chars().count() >= 32)
                .unwrap_or(false);
            if !valid {
                return Err(ServerError::Configuration(
                    "POLYEDGE_AUTH__API_TOKEN must contain at least 32 characters when authentication is enabled"
                        .to_string(),
                ));
            }
        }
        let step_up_code = env_value("POLYEDGE_AUTH__STEP_UP_CODE");
        if environment.eq_ignore_ascii_case("production") {
            let valid = step_up_code
                .as_deref()
                .map(|code| code.chars().count() >= 16)
                .unwrap_or(false);
            if !valid {
                return Err(ServerError::Configuration(
                    "production requires POLYEDGE_AUTH__STEP_UP_CODE with at least 16 characters"
                        .to_string(),
                ));
            }
        }

        let cors_origins =
            parse_cors_origins(env_value("POLYEDGE_CORS__ALLOWED_ORIGINS").unwrap_or_default())?;
        if environment.eq_ignore_ascii_case("production") && cors_origins.is_empty() {
            return Err(ServerError::Configuration(
                "production requires a non-empty exact CORS allowlist".to_string(),
            ));
        }

        let database_url = env_value("POLYEDGE_POSTGRES__URL").ok_or_else(|| {
            ServerError::Configuration("POLYEDGE_POSTGRES__URL is required".to_string())
        })?;

        Ok(Self {
            bind_addr,
            database_url,
            postgres_max_connections: parse_env("POLYEDGE_POSTGRES__MAX_CONNECTIONS", 20_u32)?,
            environment,
            auth_disabled,
            allow_insecure_private_deploy,
            api_token,
            step_up_code,
            max_body_bytes: parse_env("POLYEDGE_SERVER__MAX_BODY_BYTES", 1_048_576_usize)?,
            cors_origins,
            clob_host: env_value("POLYEDGE_POLYMARKET__CLOB_HOST")
                .unwrap_or_else(|| DEFAULT_CLOB_HOST.to_string()),
            data_api_host: env_value("POLYEDGE_POLYMARKET__DATA_API_HOST")
                .unwrap_or_else(|| DEFAULT_DATA_API_HOST.to_string()),
            chain_id: parse_env("POLYEDGE_POLYMARKET__CHAIN_ID", 137_u64)?,
            wallet_secret_env_prefix: env_value("POLYEDGE_WALLET_SECRETS__ENV_PREFIX")
                .unwrap_or_else(|| "POLYEDGE_WALLET_SECRET__".to_string()),
            orderbook_max_tokens: parse_env(
                "POLYEDGE_TARGETED_ORDERBOOK__MAX_TOKENS",
                1_000_usize,
            )?,
            orderbook_poll_interval: Duration::from_millis(parse_env(
                "POLYEDGE_TARGETED_ORDERBOOK__POLL_INTERVAL_MS",
                10_000_u64,
            )?),
            wallet_concurrency: parse_env("POLYEDGE_EXECUTION__WALLET_CONCURRENCY", 4_usize)?,
            reconcile_interval: Duration::from_millis(parse_env(
                "POLYEDGE_EXECUTION__RECONCILE_INTERVAL_MS",
                2_000_u64,
            )?),
        })
    }
}

fn env_value(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_bool_env(name: &str, default: bool) -> Result<bool> {
    let Some(value) = env_value(name) else {
        return Ok(default);
    };
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(ServerError::Configuration(format!(
            "{name} must be a boolean"
        ))),
    }
}

fn parse_env<T>(name: &str, default: T) -> Result<T>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    let Some(value) = env_value(name) else {
        return Ok(default);
    };
    value
        .parse()
        .map_err(|error| ServerError::Configuration(format!("invalid {name} value: {error}")))
}

fn parse_cors_origins(raw: String) -> Result<Vec<String>> {
    raw.split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .map(|origin| {
            if origin == "*" || origin.contains('?') || origin.contains('#') {
                return Err(ServerError::Configuration(
                    "CORS origins must be exact and may not contain wildcard/query/fragment"
                        .to_string(),
                ));
            }
            let scheme_end = origin.find("://").ok_or_else(|| {
                ServerError::Configuration(format!("invalid CORS origin: {origin}"))
            })?;
            if origin[(scheme_end + 3)..].contains('/') {
                return Err(ServerError::Configuration(format!(
                    "CORS origin may not contain a path: {origin}"
                )));
            }
            Ok(origin.to_string())
        })
        .collect()
}
