//! Wallet credential resolution.
//!
//! Database rows contain only a provider/locator reference. For the supported
//! environment provider the actual secret is JSON in
//! `POLYEDGE_WALLET_SECRET__<NORMALIZED_LOCATOR>`. Secret values are never
//! included in errors, `Debug`, or tracing fields.

use polyedge_connectors::{LivePolymarketConfig, PolymarketSignatureScheme};
use polyedge_domain::{AppError, CredentialProvider, Result, WalletAccount, WalletCredentialRef};
use serde::Deserialize;
use std::{env, fmt};

#[derive(Clone)]
pub struct WalletSecretResolver {
    chain_id: u64,
    clob_host: String,
    env_prefix: String,
}

impl fmt::Debug for WalletSecretResolver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WalletSecretResolver")
            .field("chain_id", &self.chain_id)
            .field("clob_host", &self.clob_host)
            .field("env_prefix", &self.env_prefix)
            .finish()
    }
}

impl Default for WalletSecretResolver {
    fn default() -> Self {
        Self {
            chain_id: 137,
            clob_host: env::var("POLYEDGE_POLYMARKET__CLOB_HOST")
                .unwrap_or_else(|_| "https://clob.polymarket.com".to_string()),
            env_prefix: env::var("POLYEDGE_WALLET_SECRETS__ENV_PREFIX")
                .unwrap_or_else(|_| "POLYEDGE_WALLET_SECRET__".to_string()),
        }
    }
}

impl WalletSecretResolver {
    pub fn new(
        chain_id: u64,
        clob_host: impl Into<String>,
        env_prefix: impl Into<String>,
    ) -> Result<Self> {
        let clob_host = non_empty("clob_host", clob_host.into())?;
        let env_prefix = non_empty("env_prefix", env_prefix.into())?;
        Ok(Self {
            chain_id,
            clob_host,
            env_prefix,
        })
    }

    pub fn resolve(
        &self,
        wallet: &WalletAccount,
        credential: &WalletCredentialRef,
    ) -> Result<LivePolymarketConfig> {
        if credential.provider != CredentialProvider::Environment {
            return Err(AppError::dependency_unavailable(
                "WALLET_CREDENTIAL_PROVIDER_UNSUPPORTED",
                format!("wallet {} uses unsupported credential provider", wallet.id),
            ));
        }
        let normalized = normalize_locator(&credential.locator)?;
        let variable = format!("{}{normalized}", self.env_prefix);
        let raw = env::var(&variable).map_err(|_| {
            AppError::dependency_unavailable(
                "WALLET_SECRET_MISSING",
                format!("credential environment variable {variable} is not configured"),
            )
        })?;
        let secret = serde_json::from_str::<WalletSecret>(&raw).map_err(|error| {
            // Deliberately do not include the raw value in the error.
            AppError::invalid_input(
                "WALLET_SECRET_JSON_INVALID",
                format!("credential {variable} is not valid JSON: {error}"),
            )
        })?;
        let private_key = non_empty("private_key", secret.private_key)?;
        let account_id = non_empty(
            "account_id",
            secret
                .account_id
                .unwrap_or_else(|| wallet.signer_address.clone()),
        )?;
        let funder = secret
            .funder
            .or_else(|| Some(wallet.funder_address.clone()));
        Ok(LivePolymarketConfig {
            account_id,
            clob_host: self.clob_host.clone(),
            chain_id: secret.chain_id.unwrap_or(self.chain_id),
            signature_type: signature_scheme(wallet.signature_type)?,
            funder,
            private_key,
            api_key: secret.api_key,
            api_secret: secret.api_secret,
            api_passphrase: secret.api_passphrase,
        })
    }
}

#[derive(Deserialize)]
struct WalletSecret {
    private_key: String,
    account_id: Option<String>,
    funder: Option<String>,
    chain_id: Option<u64>,
    api_key: Option<String>,
    api_secret: Option<String>,
    api_passphrase: Option<String>,
}

fn non_empty(field: &str, value: String) -> Result<String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(AppError::invalid_input(
            "WALLET_SECRET_FIELD_REQUIRED",
            format!("wallet secret {field} must not be empty"),
        ));
    }
    Ok(value)
}

/// Normalize a locator into an environment-safe, deterministic suffix.
/// Separators are collapsed to `_`; an empty or all-separator locator is
/// rejected to prevent accidentally reading a broad variable name.
pub fn normalize_locator(locator: &str) -> Result<String> {
    let mut normalized = String::with_capacity(locator.len());
    let mut previous_separator = false;
    for character in locator.trim().chars() {
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_uppercase());
            previous_separator = false;
        } else if !previous_separator {
            normalized.push('_');
            previous_separator = true;
        }
    }
    while normalized.ends_with('_') {
        normalized.pop();
    }
    if normalized.is_empty() {
        return Err(AppError::invalid_input(
            "WALLET_CREDENTIAL_LOCATOR_INVALID",
            "wallet credential locator must contain an alphanumeric character",
        ));
    }
    Ok(normalized)
}

fn signature_scheme(signature_type: i32) -> Result<PolymarketSignatureScheme> {
    match signature_type {
        0 => Ok(PolymarketSignatureScheme::Eoa),
        1 => Ok(PolymarketSignatureScheme::Proxy),
        2 => Ok(PolymarketSignatureScheme::GnosisSafe),
        3 => Ok(PolymarketSignatureScheme::Poly1271),
        value => Err(AppError::invalid_input(
            "WALLET_SIGNATURE_TYPE_INVALID",
            format!("unsupported wallet signature type {value}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locator_normalization_is_stable_and_collapses_separators() {
        assert_eq!(
            normalize_locator(" trader/main-01 ").ok().as_deref(),
            Some("TRADER_MAIN_01")
        );
        assert!(normalize_locator("---").is_err());
    }

    #[test]
    fn secret_debug_does_not_expose_private_fields() {
        let resolver = WalletSecretResolver::default();
        let debug = format!("{resolver:?}");
        assert!(!debug.contains("private_key"));
    }
}
