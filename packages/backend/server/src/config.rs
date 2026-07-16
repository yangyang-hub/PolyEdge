use crate::error::{Result, ServerError};
use crate::wallet_crypto::WalletCryptoConfig;
use std::{env, net::SocketAddr, str::FromStr, time::Duration};

const DEFAULT_CLOB_HOST: &str = "https://clob.polymarket.com";
const DEFAULT_DATA_API_HOST: &str = "https://data-api.polymarket.com";

#[derive(Clone)]
pub struct ServerConfig {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub postgres_max_connections: u32,
    pub environment: String,
    pub public_origin: String,
    pub bootstrap_admin_username: String,
    pub bootstrap_admin_display_name: String,
    pub bootstrap_admin_password_hash: String,
    pub bootstrap_admin_credential_version: i64,
    pub session_idle_ttl: Duration,
    pub session_absolute_ttl: Duration,
    pub activation_ttl: Duration,
    pub recent_auth_ttl: Duration,
    pub max_body_bytes: usize,
    pub cors_origins: Vec<String>,
    pub clob_host: String,
    pub data_api_host: String,
    pub chain_id: u64,
    pub orderbook_max_tokens: usize,
    pub orderbook_poll_interval: Duration,
    pub wallet_concurrency: usize,
    pub reconcile_interval: Duration,
    pub wallet_crypto: WalletCryptoConfig,
}

impl ServerConfig {
    pub fn from_env() -> Result<Self> {
        let host = env_value("POLYEDGE_SERVER__HOST").unwrap_or_else(|| "0.0.0.0".to_string());
        let port = parse_env("POLYEDGE_SERVER__PORT", 38_001_u16)?;
        let bind_addr = SocketAddr::from_str(&format!("{host}:{port}")).map_err(|error| {
            ServerError::Configuration(format!("invalid server bind address: {error}"))
        })?;
        let environment = parse_environment(
            env_value("POLYEDGE_RUNTIME__ENVIRONMENT").unwrap_or_else(|| "local".to_string()),
        )?;
        let public_origin = env_value("POLYEDGE_PUBLIC_ORIGIN")
            .unwrap_or_else(|| "http://localhost:33002".to_string());
        validate_exact_origin(
            "POLYEDGE_PUBLIC_ORIGIN",
            &public_origin,
            environment.eq_ignore_ascii_case("production"),
        )?;
        let bootstrap_admin_username = required_env("POLYEDGE_BOOTSTRAP_ADMIN__USERNAME")?;
        let bootstrap_admin_display_name = env_value("POLYEDGE_BOOTSTRAP_ADMIN__DISPLAY_NAME")
            .unwrap_or_else(|| "PolyEdge Administrator".to_string());
        let bootstrap_admin_password_hash =
            required_env("POLYEDGE_BOOTSTRAP_ADMIN__PASSWORD_HASH")?;
        let bootstrap_admin_credential_version =
            parse_env("POLYEDGE_BOOTSTRAP_ADMIN__CREDENTIAL_VERSION", 1_i64)?;
        if bootstrap_admin_credential_version <= 0 {
            return Err(ServerError::Configuration(
                "bootstrap admin credential version must be positive".to_string(),
            ));
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
        let session_idle_seconds = parse_env("POLYEDGE_AUTH__SESSION_IDLE_SECONDS", 1_800_u64)?;
        let session_absolute_seconds =
            parse_env("POLYEDGE_AUTH__SESSION_ABSOLUTE_SECONDS", 28_800_u64)?;
        let activation_seconds = parse_env("POLYEDGE_AUTH__ACTIVATION_TTL_SECONDS", 86_400_u64)?;
        let recent_auth_seconds = parse_env("POLYEDGE_AUTH__RECENT_AUTH_TTL_SECONDS", 300_u64)?;
        if session_idle_seconds == 0
            || session_absolute_seconds < session_idle_seconds
            || activation_seconds == 0
            || recent_auth_seconds == 0
        {
            return Err(ServerError::Configuration(
                "authentication TTLs must be positive and absolute session TTL must be >= idle TTL"
                    .into(),
            ));
        }

        Ok(Self {
            bind_addr,
            database_url,
            postgres_max_connections: parse_env("POLYEDGE_POSTGRES__MAX_CONNECTIONS", 20_u32)?,
            environment,
            public_origin,
            bootstrap_admin_username,
            bootstrap_admin_display_name,
            bootstrap_admin_password_hash,
            bootstrap_admin_credential_version,
            session_idle_ttl: Duration::from_secs(session_idle_seconds),
            session_absolute_ttl: Duration::from_secs(session_absolute_seconds),
            activation_ttl: Duration::from_secs(activation_seconds),
            recent_auth_ttl: Duration::from_secs(recent_auth_seconds),
            max_body_bytes: parse_env("POLYEDGE_SERVER__MAX_BODY_BYTES", 1_048_576_usize)?,
            cors_origins,
            clob_host: env_value("POLYEDGE_POLYMARKET__CLOB_HOST")
                .unwrap_or_else(|| DEFAULT_CLOB_HOST.to_string()),
            data_api_host: env_value("POLYEDGE_POLYMARKET__DATA_API_HOST")
                .unwrap_or_else(|| DEFAULT_DATA_API_HOST.to_string()),
            chain_id: parse_env("POLYEDGE_POLYMARKET__CHAIN_ID", 137_u64)?,
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
            wallet_crypto: WalletCryptoConfig::from_env()?,
        })
    }
}

fn required_env(name: &str) -> Result<String> {
    env_value(name).ok_or_else(|| ServerError::Configuration(format!("{name} is required")))
}

fn env_value(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
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
            validate_exact_origin("CORS origin", origin, false)?;
            Ok(origin.to_string())
        })
        .collect()
}

fn parse_environment(raw: String) -> Result<String> {
    let environment = raw.to_ascii_lowercase();
    if matches!(environment.as_str(), "local" | "production") {
        Ok(environment)
    } else {
        Err(ServerError::Configuration(
            "POLYEDGE_RUNTIME__ENVIRONMENT must be local or production".to_string(),
        ))
    }
}

fn validate_exact_origin(name: &str, origin: &str, require_https: bool) -> Result<()> {
    let (scheme, authority) = origin.split_once("://").ok_or_else(|| {
        ServerError::Configuration(format!("{name} must be an exact HTTP(S) origin"))
    })?;
    if !matches!(scheme, "http" | "https")
        || authority.is_empty()
        || authority.contains(['/', '?', '#'])
        || origin == "*"
    {
        return Err(ServerError::Configuration(format!(
            "{name} must be an exact HTTP(S) origin without path, query, fragment, or wildcard"
        )));
    }
    if require_https && scheme != "https" {
        return Err(ServerError::Configuration(format!(
            "production {name} must use https"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_origin_validation_rejects_paths_and_non_http_schemes() {
        assert!(validate_exact_origin("origin", "https://polyedge.example", true).is_ok());
        assert!(validate_exact_origin("origin", "http://localhost:33002", false).is_ok());
        assert!(validate_exact_origin("origin", "https://polyedge.example/", false).is_err());
        assert!(validate_exact_origin("origin", "javascript://polyedge", false).is_err());
        assert!(validate_exact_origin("origin", "http://localhost", true).is_err());
    }

    #[test]
    fn runtime_environment_rejects_unknown_values() {
        assert_eq!(
            parse_environment("Production".to_string()).unwrap(),
            "production"
        );
        assert!(parse_environment("prodution".to_string()).is_err());
        assert!(parse_environment("staging".to_string()).is_err());
    }
}
