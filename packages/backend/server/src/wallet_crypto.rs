//! Cryptographic boundary for browser wallet imports and encrypted-at-rest credentials.
//!
//! Transport encryption and storage encryption intentionally use different keys:
//!
//! - browser imports use a mounted RSA private key with RSA-OAEP-SHA256 to unwrap an
//!   ephemeral AES-256-GCM key;
//! - database envelopes use a 32-byte AES-256-GCM storage KEK to wrap a fresh DEK for
//!   every credential payload.
//!
//! Secret key material and decrypted payloads are held in `secrecy`/`zeroize` containers.
//! Errors and `Debug` implementations never include PEM, symmetric keys, ciphertext, or
//! plaintext values.

use crate::error::{Result, ServerError};
use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, Payload},
};
use base64::{Engine as _, engine::general_purpose};
use rand::{RngCore, rngs::OsRng};
use rsa::{
    Oaep, RsaPrivateKey, RsaPublicKey, pkcs1::DecodeRsaPrivateKey, pkcs8::DecodePrivateKey,
    traits::PublicKeyParts,
};
use secrecy::{ExposeSecret, ExposeSecretMut, SecretBox, SecretSlice};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::{
    env, fmt, fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use time::OffsetDateTime;
use uuid::Uuid;
use zeroize::Zeroizing;

const AES_KEY_BYTES: usize = 32;
const AES_NONCE_BYTES: usize = 12;
const AES_TAG_BYTES: usize = 16;
const MIN_RSA_BITS: usize = 2_048;
const MAX_PEM_BYTES: u64 = 64 * 1_024;
const MAX_STORAGE_KEY_FILE_BYTES: u64 = 4 * 1_024;
const MAX_IMPORT_CIPHERTEXT_BYTES: usize = 64 * 1_024;
const STORAGE_ENVELOPE_VERSION: i16 = 1;
const IMPORT_AAD_VERSION: &str = "polyedge-wallet-import-v1";
const STORAGE_PAYLOAD_AAD_DOMAIN: &[u8] = b"polyedge-wallet-storage-payload-v1";
const STORAGE_DEK_AAD_DOMAIN: &[u8] = b"polyedge-wallet-storage-dek-v1";

/// Validated wallet cryptography configuration.
#[derive(Clone)]
pub struct WalletCryptoConfig {
    pub transport_key_id: String,
    pub storage_key_id: String,
    pub import_context_ttl: Duration,
    pub max_import_contexts: usize,
    transport_private_key: Arc<RsaPrivateKey>,
    storage_key: Arc<SecretBox<[u8; AES_KEY_BYTES]>>,
    transport_private_key_pem_file: PathBuf,
}

impl fmt::Debug for WalletCryptoConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WalletCryptoConfig")
            .field("transport_key_id", &self.transport_key_id)
            .field("storage_key_id", &self.storage_key_id)
            .field("import_context_ttl", &self.import_context_ttl)
            .field("max_import_contexts", &self.max_import_contexts)
            .field(
                "transport_private_key_pem_file",
                &self.transport_private_key_pem_file,
            )
            .field("transport_private_key", &"[REDACTED]")
            .field("storage_key", &"[REDACTED]")
            .finish()
    }
}

impl WalletCryptoConfig {
    pub(crate) fn from_env() -> Result<Self> {
        let transport_private_key_pem_file = PathBuf::from(required_env(
            "POLYEDGE_WALLET_CRYPTO__TRANSPORT_PRIVATE_KEY_PEM_FILE",
        )?);
        let transport_key_id = parse_key_id(
            "POLYEDGE_WALLET_CRYPTO__TRANSPORT_KEY_ID",
            optional_env("POLYEDGE_WALLET_CRYPTO__TRANSPORT_KEY_ID")
                .unwrap_or_else(|| "wallet-import-rsa-v1".to_string()),
        )?;
        let storage_key_id = parse_key_id(
            "POLYEDGE_WALLET_CRYPTO__STORAGE_KEY_ID",
            optional_env("POLYEDGE_WALLET_CRYPTO__STORAGE_KEY_ID")
                .unwrap_or_else(|| "wallet-storage-aes-v1".to_string()),
        )?;
        let import_context_ttl = Duration::from_secs(parse_positive_env(
            "POLYEDGE_WALLET_CRYPTO__IMPORT_CONTEXT_TTL_SECONDS",
            300_u64,
        )?);
        if import_context_ttl > Duration::from_secs(3_600) {
            return Err(ServerError::Configuration(
                "POLYEDGE_WALLET_CRYPTO__IMPORT_CONTEXT_TTL_SECONDS may not exceed 3600"
                    .to_string(),
            ));
        }
        let max_import_contexts =
            parse_positive_env("POLYEDGE_WALLET_CRYPTO__MAX_IMPORT_CONTEXTS", 1_024_usize)?;
        if max_import_contexts > 100_000 {
            return Err(ServerError::Configuration(
                "POLYEDGE_WALLET_CRYPTO__MAX_IMPORT_CONTEXTS may not exceed 100000".to_string(),
            ));
        }
        let storage_key_file =
            PathBuf::from(required_env("POLYEDGE_WALLET_CRYPTO__STORAGE_KEY_FILE")?);
        validate_secret_file(
            &storage_key_file,
            MAX_STORAGE_KEY_FILE_BYTES,
            "wallet storage key",
        )?;
        let storage_key_base64 = fs::read_to_string(&storage_key_file)
            .map(Zeroizing::new)
            .map_err(|_| {
                ServerError::Configuration(
                    "wallet storage key file must contain UTF-8 base64".to_string(),
                )
            })?;
        let storage_key = Arc::new(parse_storage_key(storage_key_base64)?);
        let transport_private_key =
            Arc::new(load_transport_private_key(&transport_private_key_pem_file)?);

        Ok(Self {
            transport_key_id,
            storage_key_id,
            import_context_ttl,
            max_import_contexts,
            transport_private_key,
            storage_key,
            transport_private_key_pem_file,
        })
    }

    #[cfg(test)]
    fn for_test(transport_private_key: RsaPrivateKey, storage_key: [u8; 32]) -> Self {
        Self {
            transport_key_id: "transport-test-v1".to_string(),
            storage_key_id: "storage-test-v1".to_string(),
            import_context_ttl: Duration::from_secs(60),
            max_import_contexts: 8,
            transport_private_key: Arc::new(transport_private_key),
            storage_key: Arc::new(SecretBox::new(Box::new(storage_key))),
            transport_private_key_pem_file: PathBuf::from("[test-key]"),
        }
    }
}

/// Public RSA key returned to a browser as a JSON Web Key.
#[derive(Clone, Debug, Serialize)]
pub struct WalletImportPublicJwk {
    pub kty: &'static str,
    #[serde(rename = "use")]
    pub use_: &'static str,
    pub alg: &'static str,
    pub kid: String,
    pub n: String,
    pub e: String,
}

/// One-time browser import context. The context is consumed before decryption is attempted.
#[derive(Clone, Debug, Serialize)]
pub struct WalletImportContext {
    pub context_id: Uuid,
    pub key_id: String,
    pub algorithm: &'static str,
    pub aad_version: &'static str,
    pub public_key: WalletImportPublicJwk,
    pub expires_at: OffsetDateTime,
}

/// Base64url-without-padding hybrid encryption fields submitted by a browser.
#[derive(Clone, Deserialize)]
pub struct WalletImportCiphertext {
    pub wrapped_key: String,
    pub nonce: String,
    pub ciphertext: String,
}

impl fmt::Debug for WalletImportCiphertext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WalletImportCiphertext")
            .field("wrapped_key", &"[REDACTED]")
            .field("nonce", &"[REDACTED]")
            .field("ciphertext", &"[REDACTED]")
            .finish()
    }
}

/// Database-friendly envelope. All byte vectors map directly to PostgreSQL `BYTEA` columns.
#[derive(Clone, Serialize, Deserialize)]
pub struct WalletSecretEnvelope {
    pub version: i16,
    pub key_id: String,
    pub payload_nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub wrapped_dek_nonce: Vec<u8>,
    pub wrapped_dek: Vec<u8>,
}

impl fmt::Debug for WalletSecretEnvelope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WalletSecretEnvelope")
            .field("version", &self.version)
            .field("key_id", &self.key_id)
            .field("payload_nonce", &"[REDACTED]")
            .field("ciphertext", &"[REDACTED]")
            .field("wrapped_dek_nonce", &"[REDACTED]")
            .field("wrapped_dek", &"[REDACTED]")
            .finish()
    }
}

/// Wallet cryptography service. Import-context lifecycle is persisted in PostgreSQL.
pub struct WalletCryptoService {
    transport_key_id: String,
    storage_key_id: String,
    transport_private_key: Arc<RsaPrivateKey>,
    storage_key: Arc<SecretBox<[u8; AES_KEY_BYTES]>>,
    public_key: WalletImportPublicJwk,
    import_context_ttl: Duration,
    max_import_contexts: usize,
}

impl fmt::Debug for WalletCryptoService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WalletCryptoService")
            .field("transport_key_id", &self.transport_key_id)
            .field("storage_key_id", &self.storage_key_id)
            .field("transport_private_key", &"[REDACTED]")
            .field("storage_key", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl WalletCryptoService {
    #[must_use]
    pub fn new(config: &WalletCryptoConfig) -> Self {
        let public_key = public_jwk(
            &config.transport_key_id,
            &RsaPublicKey::from(config.transport_private_key.as_ref()),
        );
        Self {
            transport_key_id: config.transport_key_id.clone(),
            storage_key_id: config.storage_key_id.clone(),
            transport_private_key: Arc::clone(&config.transport_private_key),
            storage_key: Arc::clone(&config.storage_key),
            public_key,
            import_context_ttl: config.import_context_ttl,
            max_import_contexts: config.max_import_contexts,
        }
    }

    /// Issue a context whose lifecycle and capacity are managed by PostgreSQL.
    pub fn create_durable_import_context(&self) -> Result<WalletImportContext> {
        let expires_at = OffsetDateTime::now_utc()
            + time::Duration::try_from(self.import_context_ttl).map_err(|_| {
                ServerError::Configuration(
                    "wallet import context TTL exceeds supported duration".to_string(),
                )
            })?;
        Ok(WalletImportContext {
            context_id: Uuid::new_v4(),
            key_id: self.transport_key_id.clone(),
            algorithm: "RSA-OAEP-256+A256GCM",
            aad_version: IMPORT_AAD_VERSION,
            public_key: self.public_key.clone(),
            expires_at,
        })
    }

    #[must_use]
    pub(crate) fn max_import_contexts(&self) -> usize {
        self.max_import_contexts
    }

    #[must_use]
    pub(crate) fn transport_key_id(&self) -> &str {
        &self.transport_key_id
    }

    /// Decrypt after a durable external context store has atomically validated and consumed
    /// the context. This is used by the PostgreSQL-backed API path so multiple server instances
    /// do not depend on process-local context affinity.
    pub fn decrypt_import_validated(
        &self,
        context_id: Uuid,
        encrypted: &WalletImportCiphertext,
        binding: &[u8],
    ) -> Result<SecretSlice<u8>> {
        let wrapped_key = decode_base64url_field("wrapped_key", &encrypted.wrapped_key)?;
        if wrapped_key.len() != self.transport_private_key.size() {
            return Err(invalid_import_ciphertext());
        }
        let nonce = decode_nonce_base64url("nonce", &encrypted.nonce)?;
        let ciphertext = decode_base64url_field("ciphertext", &encrypted.ciphertext)?;
        if !(AES_TAG_BYTES..=MAX_IMPORT_CIPHERTEXT_BYTES).contains(&ciphertext.len()) {
            return Err(invalid_import_ciphertext());
        }

        let dek = self
            .transport_private_key
            .decrypt(Oaep::new::<Sha256>(), &wrapped_key)
            .map(Zeroizing::new)
            .map_err(|_| invalid_import_ciphertext())?;
        if dek.len() != AES_KEY_BYTES {
            return Err(invalid_import_ciphertext());
        }
        let cipher = aes_cipher(dek.as_slice())?;
        let aad = wallet_import_aad(context_id, binding);
        let plaintext = cipher
            .decrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &ciphertext,
                    aad: &aad,
                },
            )
            .map_err(|_| invalid_import_ciphertext())?;
        Ok(plaintext.into())
    }

    /// Encrypt a wallet credential using a new per-envelope DEK.
    pub fn encrypt_for_storage(
        &self,
        plaintext: &SecretSlice<u8>,
        aad: &[u8],
    ) -> Result<WalletSecretEnvelope> {
        let mut dek = SecretBox::new(Box::new([0_u8; AES_KEY_BYTES]));
        OsRng.fill_bytes(dek.expose_secret_mut());
        let payload_nonce = random_nonce();
        let cipher = aes_cipher(dek.expose_secret())?;
        let payload_aad = storage_payload_aad(STORAGE_ENVELOPE_VERSION, &self.storage_key_id, aad);
        let ciphertext = cipher
            .encrypt(
                Nonce::from_slice(&payload_nonce),
                Payload {
                    msg: plaintext.expose_secret(),
                    aad: &payload_aad,
                },
            )
            .map_err(|_| ServerError::Internal("wallet secret encryption failed".to_string()))?;

        let wrapped_dek_nonce = random_nonce();
        let storage_cipher = aes_cipher(self.storage_key.expose_secret())?;
        let dek_aad = storage_dek_aad(STORAGE_ENVELOPE_VERSION, &self.storage_key_id, aad);
        let wrapped_dek = storage_cipher
            .encrypt(
                Nonce::from_slice(&wrapped_dek_nonce),
                Payload {
                    msg: dek.expose_secret(),
                    aad: &dek_aad,
                },
            )
            .map_err(|_| ServerError::Internal("wallet key wrapping failed".to_string()))?;

        Ok(WalletSecretEnvelope {
            version: STORAGE_ENVELOPE_VERSION,
            key_id: self.storage_key_id.clone(),
            payload_nonce: payload_nonce.to_vec(),
            ciphertext,
            wrapped_dek_nonce: wrapped_dek_nonce.to_vec(),
            wrapped_dek,
        })
    }

    /// Authenticate and decrypt a database envelope. Unknown versions, key ids, malformed
    /// nonces, and authentication failures all fail closed.
    pub fn decrypt_from_storage(
        &self,
        envelope: &WalletSecretEnvelope,
        aad: &[u8],
    ) -> Result<SecretSlice<u8>> {
        if envelope.version != STORAGE_ENVELOPE_VERSION
            || envelope.key_id != self.storage_key_id
            || envelope.payload_nonce.len() != AES_NONCE_BYTES
            || envelope.wrapped_dek_nonce.len() != AES_NONCE_BYTES
            || envelope.ciphertext.len() < AES_TAG_BYTES
            || envelope.wrapped_dek.len() != AES_KEY_BYTES + AES_TAG_BYTES
        {
            return Err(invalid_storage_envelope());
        }

        let storage_cipher = aes_cipher(self.storage_key.expose_secret())?;
        let dek_aad = storage_dek_aad(envelope.version, &envelope.key_id, aad);
        let dek = storage_cipher
            .decrypt(
                Nonce::from_slice(&envelope.wrapped_dek_nonce),
                Payload {
                    msg: &envelope.wrapped_dek,
                    aad: &dek_aad,
                },
            )
            .map(Zeroizing::new)
            .map_err(|_| invalid_storage_envelope())?;
        if dek.len() != AES_KEY_BYTES {
            return Err(invalid_storage_envelope());
        }

        let cipher = aes_cipher(dek.as_slice())?;
        let payload_aad = storage_payload_aad(envelope.version, &envelope.key_id, aad);
        let plaintext = cipher
            .decrypt(
                Nonce::from_slice(&envelope.payload_nonce),
                Payload {
                    msg: &envelope.ciphertext,
                    aad: &payload_aad,
                },
            )
            .map_err(|_| invalid_storage_envelope())?;
        Ok(plaintext.into())
    }
}

/// Canonical AAD that the browser and server must both use for an import payload.
#[must_use]
pub fn wallet_import_aad(context_id: Uuid, binding: &[u8]) -> Vec<u8> {
    domain_aad(
        IMPORT_AAD_VERSION.as_bytes(),
        context_id.as_bytes(),
        binding,
    )
}

/// Canonical database-envelope binding for a wallet secret.
#[must_use]
pub(crate) fn wallet_storage_aad(
    wallet_id: i64,
    owner_user_id: i64,
    signer: &str,
    version: i64,
) -> Vec<u8> {
    format!("wallet={wallet_id};owner={owner_user_id};signer={signer};version={version}")
        .into_bytes()
}

fn storage_payload_aad(version: i16, key_id: &str, aad: &[u8]) -> Vec<u8> {
    storage_aad(STORAGE_PAYLOAD_AAD_DOMAIN, version, key_id, aad)
}

fn storage_dek_aad(version: i16, key_id: &str, aad: &[u8]) -> Vec<u8> {
    storage_aad(STORAGE_DEK_AAD_DOMAIN, version, key_id, aad)
}

fn storage_aad(domain: &[u8], version: i16, key_id: &str, aad: &[u8]) -> Vec<u8> {
    let mut metadata = Vec::with_capacity(2 + key_id.len());
    metadata.extend_from_slice(&version.to_be_bytes());
    metadata.extend_from_slice(key_id.as_bytes());
    domain_aad(domain, &metadata, aad)
}

fn domain_aad(domain: &[u8], metadata: &[u8], binding: &[u8]) -> Vec<u8> {
    let mut aad = Vec::with_capacity(domain.len() + metadata.len() + binding.len() + 16);
    append_length_prefixed(&mut aad, domain);
    append_length_prefixed(&mut aad, metadata);
    append_length_prefixed(&mut aad, binding);
    aad
}

fn append_length_prefixed(target: &mut Vec<u8>, value: &[u8]) {
    let length = u64::try_from(value.len()).unwrap_or(u64::MAX);
    target.extend_from_slice(&length.to_be_bytes());
    target.extend_from_slice(value);
}

fn random_nonce() -> [u8; AES_NONCE_BYTES] {
    let mut nonce = [0_u8; AES_NONCE_BYTES];
    OsRng.fill_bytes(&mut nonce);
    nonce
}

fn aes_cipher(key: &[u8]) -> Result<Aes256Gcm> {
    <Aes256Gcm as aes_gcm::KeyInit>::new_from_slice(key)
        .map_err(|_| ServerError::Internal("wallet encryption key is invalid".to_string()))
}

fn public_jwk(key_id: &str, public_key: &RsaPublicKey) -> WalletImportPublicJwk {
    WalletImportPublicJwk {
        kty: "RSA",
        use_: "enc",
        alg: "RSA-OAEP-256",
        kid: key_id.to_string(),
        n: general_purpose::URL_SAFE_NO_PAD.encode(public_key.n().to_bytes_be()),
        e: general_purpose::URL_SAFE_NO_PAD.encode(public_key.e().to_bytes_be()),
    }
}

fn load_transport_private_key(path: &Path) -> Result<RsaPrivateKey> {
    let metadata = fs::metadata(path).map_err(|_| {
        ServerError::Configuration(format!(
            "wallet transport private key file {} is not readable",
            path.display()
        ))
    })?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_PEM_BYTES {
        return Err(ServerError::Configuration(format!(
            "wallet transport private key file {} must be a non-empty PEM file no larger than {MAX_PEM_BYTES} bytes",
            path.display()
        )));
    }
    validate_secret_permissions(path, &metadata)?;
    let pem = fs::read_to_string(path).map(Zeroizing::new).map_err(|_| {
        ServerError::Configuration(format!(
            "wallet transport private key file {} is not valid UTF-8 PEM",
            path.display()
        ))
    })?;
    parse_transport_private_key(&pem)
}

fn validate_secret_file(path: &Path, max_bytes: u64, label: &str) -> Result<()> {
    let metadata = fs::metadata(path).map_err(|_| {
        ServerError::Configuration(format!("{label} file {} is not readable", path.display()))
    })?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > max_bytes {
        return Err(ServerError::Configuration(format!(
            "{label} file {} must be non-empty and no larger than {max_bytes} bytes",
            path.display()
        )));
    }
    validate_secret_permissions(path, &metadata)
}

fn validate_secret_permissions(path: &Path, metadata: &fs::Metadata) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(ServerError::Configuration(format!(
                "secret file {} must not be group/world accessible (chmod 600)",
                path.display()
            )));
        }
    }
    Ok(())
}

fn parse_transport_private_key(pem: &str) -> Result<RsaPrivateKey> {
    let private_key = RsaPrivateKey::from_pkcs8_pem(pem)
        .or_else(|_| RsaPrivateKey::from_pkcs1_pem(pem))
        .map_err(|_| {
            ServerError::Configuration(
                "wallet transport private key must be PKCS#8 or PKCS#1 RSA PEM".to_string(),
            )
        })?;
    if private_key.n().bits() < MIN_RSA_BITS {
        return Err(ServerError::Configuration(format!(
            "wallet transport RSA key must contain at least {MIN_RSA_BITS} bits"
        )));
    }
    private_key.validate().map_err(|_| {
        ServerError::Configuration("wallet transport RSA private key is invalid".to_string())
    })?;
    Ok(private_key)
}

fn parse_storage_key(encoded: Zeroizing<String>) -> Result<SecretBox<[u8; AES_KEY_BYTES]>> {
    let decoded = general_purpose::STANDARD
        .decode(encoded.trim())
        .map(Zeroizing::new)
        .map_err(|_| {
            ServerError::Configuration(
                "wallet storage key file must contain valid Base64".to_string(),
            )
        })?;
    if decoded.len() != AES_KEY_BYTES {
        return Err(ServerError::Configuration(format!(
            "wallet storage key must decode to exactly {AES_KEY_BYTES} bytes"
        )));
    }
    let mut key = SecretBox::new(Box::new([0_u8; AES_KEY_BYTES]));
    key.expose_secret_mut().copy_from_slice(decoded.as_slice());
    Ok(key)
}

fn decode_base64url_field(field: &str, encoded: &str) -> Result<Vec<u8>> {
    general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| {
            ServerError::InvalidInput(format!("wallet import {field} must be unpadded base64url"))
        })
}

fn decode_nonce_base64url(field: &str, encoded: &str) -> Result<[u8; AES_NONCE_BYTES]> {
    let decoded = decode_base64url_field(field, encoded)?;
    decoded.try_into().map_err(|_| invalid_import_ciphertext())
}

fn invalid_import_ciphertext() -> ServerError {
    ServerError::InvalidInput("wallet import ciphertext authentication failed".to_string())
}

fn invalid_storage_envelope() -> ServerError {
    ServerError::Internal("wallet secret envelope authentication failed".to_string())
}

fn parse_key_id(name: &str, value: String) -> Result<String> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(ServerError::Configuration(format!(
            "{name} must contain 1-128 ASCII letters, digits, '.', '_' or '-'"
        )));
    }
    Ok(value)
}

fn parse_positive_env<T>(name: &str, default: T) -> Result<T>
where
    T: std::str::FromStr + PartialEq + Default,
    T::Err: fmt::Display,
{
    let value = match optional_env(name) {
        Some(raw) => raw.parse().map_err(|error| {
            ServerError::Configuration(format!("invalid {name} value: {error}"))
        })?,
        None => default,
    };
    if value == T::default() {
        return Err(ServerError::Configuration(format!(
            "{name} must be greater than zero"
        )));
    }
    Ok(value)
}

fn required_env(name: &str) -> Result<String> {
    optional_env(name).ok_or_else(|| ServerError::Configuration(format!("{name} is required")))
}

fn optional_env(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsa::RsaPublicKey;

    fn test_service() -> Result<(WalletCryptoService, RsaPublicKey)> {
        let private_key = RsaPrivateKey::new(&mut OsRng, 2_048)
            .map_err(|_| ServerError::Internal("test RSA key generation failed".to_string()))?;
        let public_key = RsaPublicKey::from(&private_key);
        let config = WalletCryptoConfig::for_test(private_key, [7_u8; 32]);
        Ok((WalletCryptoService::new(&config), public_key))
    }

    #[test]
    fn storage_envelope_round_trips_and_binds_aad() -> Result<()> {
        let (service, _) = test_service()?;
        let secret: SecretSlice<u8> = b"private wallet material".to_vec().into();
        let envelope = service.encrypt_for_storage(&secret, b"user-1/wallet-9")?;
        let decrypted = service.decrypt_from_storage(&envelope, b"user-1/wallet-9")?;
        assert_eq!(decrypted.expose_secret(), secret.expose_secret());
        assert!(
            service
                .decrypt_from_storage(&envelope, b"user-2/wallet-9")
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn storage_envelope_detects_ciphertext_tampering() -> Result<()> {
        let (service, _) = test_service()?;
        let secret: SecretSlice<u8> = b"secret".to_vec().into();
        let mut envelope = service.encrypt_for_storage(&secret, b"binding")?;
        let Some(first) = envelope.ciphertext.first_mut() else {
            return Err(ServerError::Internal(
                "test ciphertext unexpectedly empty".to_string(),
            ));
        };
        *first ^= 1;
        assert!(service.decrypt_from_storage(&envelope, b"binding").is_err());
        Ok(())
    }

    #[test]
    fn browser_import_ciphertext_round_trips_with_durable_context() -> Result<()> {
        let (service, public_key) = test_service()?;
        let context = service.create_durable_import_context()?;
        let dek = [11_u8; AES_KEY_BYTES];
        let nonce = [13_u8; AES_NONCE_BYTES];
        let aad = wallet_import_aad(context.context_id, b"user-1/new-wallet");
        let cipher = aes_cipher(&dek)?;
        let ciphertext = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: b"browser wallet secret",
                    aad: &aad,
                },
            )
            .map_err(|_| ServerError::Internal("test encryption failed".to_string()))?;
        let wrapped_key = public_key
            .encrypt(&mut OsRng, Oaep::new::<Sha256>(), &dek)
            .map_err(|_| ServerError::Internal("test key wrapping failed".to_string()))?;
        let encrypted = WalletImportCiphertext {
            wrapped_key: general_purpose::URL_SAFE_NO_PAD.encode(wrapped_key),
            nonce: general_purpose::URL_SAFE_NO_PAD.encode(nonce),
            ciphertext: general_purpose::URL_SAFE_NO_PAD.encode(ciphertext),
        };

        let decrypted = service.decrypt_import_validated(
            context.context_id,
            &encrypted,
            b"user-1/new-wallet",
        )?;
        assert_eq!(decrypted.expose_secret(), b"browser wallet secret");
        assert!(
            service
                .decrypt_import_validated(context.context_id, &encrypted, b"user-2/new-wallet")
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn storage_key_validation_requires_exactly_32_bytes() {
        let valid = general_purpose::STANDARD.encode([3_u8; AES_KEY_BYTES]);
        assert!(parse_storage_key(Zeroizing::new(valid)).is_ok());
        let short = general_purpose::STANDARD.encode([3_u8; AES_KEY_BYTES - 1]);
        assert!(parse_storage_key(Zeroizing::new(short)).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn secret_files_reject_group_or_world_permissions() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let path = std::env::temp_dir().join(format!(
            "polyedge-wallet-secret-permissions-{}",
            Uuid::now_v7()
        ));
        fs::write(&path, b"secret")
            .map_err(|error| ServerError::Internal(format!("test file write failed: {error}")))?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644))
            .map_err(|error| ServerError::Internal(format!("test chmod failed: {error}")))?;
        let rejected = validate_secret_file(&path, 32, "test secret").is_err();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .map_err(|error| ServerError::Internal(format!("test chmod failed: {error}")))?;
        let accepted = validate_secret_file(&path, 32, "test secret").is_ok();
        let _ = fs::remove_file(&path);
        assert!(rejected);
        assert!(accepted);
        Ok(())
    }

    #[test]
    fn debug_output_redacts_all_sensitive_fields() -> Result<()> {
        let (service, _) = test_service()?;
        let private_key = RsaPrivateKey::new(&mut OsRng, 2_048)
            .map_err(|_| ServerError::Internal("test RSA key generation failed".to_string()))?;
        let config = WalletCryptoConfig::for_test(private_key, [99_u8; 32]);
        let debug = format!("{config:?} {service:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("99, 99"));

        let encrypted = WalletImportCiphertext {
            wrapped_key: "sensitive-wrapped-key".to_string(),
            nonce: "sensitive-nonce".to_string(),
            ciphertext: "sensitive-ciphertext".to_string(),
        };
        let debug = format!("{encrypted:?}");
        assert!(!debug.contains("sensitive"));
        Ok(())
    }
}
