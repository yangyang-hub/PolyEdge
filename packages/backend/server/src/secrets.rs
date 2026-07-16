//! Runtime wallet credential decryption from PostgreSQL envelope storage.

use crate::{store::PostgresStore, wallet_crypto::WalletCryptoService};
use polyedge_connectors::{LivePolymarketConfig, PolymarketSignatureScheme};
use polyedge_domain::{AppError, Result, WalletAccount};
use secrecy::ExposeSecret;
use serde::Deserialize;
use std::{fmt, sync::Arc};
use zeroize::Zeroizing;

#[derive(Clone)]
pub struct WalletSecretResolver {
    chain_id: u64,
    clob_host: String,
    store: PostgresStore,
    crypto: Arc<WalletCryptoService>,
}

impl fmt::Debug for WalletSecretResolver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WalletSecretResolver")
            .field("chain_id", &self.chain_id)
            .field("clob_host", &self.clob_host)
            .field("store", &"[DATABASE]")
            .field("crypto", &"[REDACTED]")
            .finish()
    }
}

impl WalletSecretResolver {
    pub fn new(
        chain_id: u64,
        clob_host: impl Into<String>,
        store: PostgresStore,
        crypto: Arc<WalletCryptoService>,
    ) -> Result<Self> {
        let clob_host = non_empty("clob_host", clob_host.into())?;
        Ok(Self {
            chain_id,
            clob_host,
            store,
            crypto,
        })
    }

    pub async fn resolve(&self, expected_wallet: &WalletAccount) -> Result<LivePolymarketConfig> {
        let (wallet, envelope, secret_version) = self
            .store
            .load_wallet_secret_envelope(expected_wallet.id)
            .await
            .map_err(|_| {
                AppError::dependency_unavailable(
                    "WALLET_SECRET_UNAVAILABLE",
                    format!("wallet {} credential is unavailable", expected_wallet.id),
                )
            })?;
        if wallet.owner_user_id != expected_wallet.owner_user_id
            || wallet.signer_address != expected_wallet.signer_address
        {
            return Err(AppError::conflict(
                "WALLET_SECRET_BINDING_MISMATCH",
                "wallet credential ownership changed",
            ));
        }
        let aad = wallet_storage_aad(
            wallet.id,
            wallet.owner_user_id,
            &wallet.signer_address,
            secret_version,
        );
        let raw = self
            .crypto
            .decrypt_from_storage(&envelope, &aad)
            .map_err(|_| {
                AppError::dependency_unavailable(
                    "WALLET_SECRET_DECRYPT_FAILED",
                    format!("wallet {} credential cannot be decrypted", wallet.id),
                )
            })?;
        let secret = serde_json::from_slice::<WalletSecret>(raw.expose_secret()).map_err(|_| {
            AppError::dependency_unavailable(
                "WALLET_SECRET_JSON_INVALID",
                format!("wallet {} credential payload is invalid", wallet.id),
            )
        })?;
        Ok(LivePolymarketConfig {
            account_id: wallet.signer_address,
            clob_host: self.clob_host.clone(),
            chain_id: self.chain_id,
            signature_type: signature_scheme(wallet.signature_type)?,
            funder: Some(wallet.funder_address),
            private_key: non_empty("private_key", secret.private_key.to_string())?,
            api_key: secret.api_key.map(|value| value.to_string()),
            api_secret: secret.api_secret.map(|value| value.to_string()),
            api_passphrase: secret.api_passphrase.map(|value| value.to_string()),
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WalletSecret {
    private_key: Zeroizing<String>,
    #[serde(default)]
    api_key: Option<Zeroizing<String>>,
    #[serde(default)]
    api_secret: Option<Zeroizing<String>>,
    #[serde(default)]
    api_passphrase: Option<Zeroizing<String>>,
}

fn wallet_storage_aad(wallet_id: i64, owner_user_id: i64, signer: &str, version: i64) -> Vec<u8> {
    format!("wallet={wallet_id};owner={owner_user_id};signer={signer};version={version}")
        .into_bytes()
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
