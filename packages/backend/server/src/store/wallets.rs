use super::*;
use crate::wallet_crypto::{
    WalletCryptoService, WalletImportCiphertext, WalletSecretEnvelope, wallet_storage_aad,
};
use k256::elliptic_curve::sec1::ToEncodedPoint;
use polyedge_domain::{ActorScope, UserRole, WalletSecretMetadata};
use secrecy::ExposeSecret;
use serde::Deserialize;
use sha3::{Digest as _, Keccak256};
use uuid::Uuid;
use zeroize::Zeroizing;

impl PostgresStore {
    pub async fn persist_wallet_import_context(
        &self,
        owner_user_id: i64,
        context_id: Uuid,
        key_id: &str,
        expires_at: OffsetDateTime,
        max_active_contexts: usize,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(731947211)")
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            r#"DELETE FROM wallet_import_contexts
               WHERE expires_at <= now() OR consumed_at < now() - interval '1 hour'"#,
        )
        .execute(&mut *tx)
        .await?;
        let active: i64 = sqlx::query_scalar(
            r#"SELECT count(*)::bigint FROM wallet_import_contexts
               WHERE consumed_at IS NULL AND expires_at > now()"#,
        )
        .fetch_one(&mut *tx)
        .await?;
        let max_active = i64::try_from(max_active_contexts).map_err(|_| {
            ServerError::Configuration("wallet import context capacity is invalid".into())
        })?;
        if active >= max_active {
            return Err(ServerError::Dependency(
                "wallet import context capacity is temporarily exhausted".into(),
            ));
        }
        sqlx::query(
            r#"INSERT INTO wallet_import_contexts (
                 import_context_id,owner_user_id,transport_key_id,expires_at
               ) VALUES ($1,$2,$3,$4)"#,
        )
        .bind(context_id)
        .bind(owner_user_id)
        .bind(key_id)
        .bind(expires_at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn consume_wallet_import_context(
        &self,
        owner_user_id: i64,
        context_id: Uuid,
        key_id: &str,
    ) -> Result<()> {
        let result = sqlx::query(
            r#"UPDATE wallet_import_contexts SET consumed_at=now()
               WHERE import_context_id=$1 AND owner_user_id=$2 AND transport_key_id=$3
                 AND consumed_at IS NULL AND expires_at>now()"#,
        )
        .bind(context_id)
        .bind(owner_user_id)
        .bind(key_id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() != 1 {
            return Err(ServerError::Conflict(
                "wallet import context is invalid, expired, or consumed".into(),
            ));
        }
        Ok(())
    }

    pub async fn list_wallets(
        &self,
        actor: ActorScope,
        query: &ManualTradingListQuery,
    ) -> Result<Vec<WalletAccountData>> {
        let (limit, offset) = page_values(query);
        let rows = sqlx::query(&wallet_select_sql(
            "WHERE ($1::boolean OR w.owner_user_id = $2) AND ($3::text IS NULL OR w.status = $3)\
             ORDER BY w.updated_at DESC, w.wallet_id DESC LIMIT $4 OFFSET $5",
        ))
        .bind(actor.role == UserRole::Admin)
        .bind(actor.user_id)
        .bind(query.status.as_deref())
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(wallet_data_from_row).collect()
    }

    pub async fn get_wallet(&self, actor: ActorScope, wallet_id: i64) -> Result<WalletAccountData> {
        let row = sqlx::query(&wallet_select_sql(
            "WHERE w.wallet_id = $1 AND ($2::boolean OR w.owner_user_id = $3)",
        ))
        .bind(wallet_id)
        .bind(actor.role == UserRole::Admin)
        .bind(actor.user_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| ServerError::NotFound(format!("wallet {wallet_id}")))?;
        wallet_data_from_row(row)
    }

    pub async fn create_wallet(
        &self,
        actor: ActorScope,
        request: &CreateWalletAccountRequest,
        crypto: &WalletCryptoService,
        request_id: &str,
    ) -> Result<WalletAccountData> {
        if actor.role == UserRole::ReadOnly {
            return Err(ServerError::Forbidden);
        }
        validate_wallet_request(request)?;
        let name = required_text(&request.name, "name", 120)?;
        let signer_address = normalize_address(&request.signer_address)?;
        let funder_address = normalize_address(&request.funder_address)?;
        let plaintext =
            decrypt_wallet_import(self, crypto, actor, &request.encrypted_secret).await?;
        validate_wallet_secret(plaintext.expose_secret(), &signer_address)?;
        let operator_note = optional_note(request.operator_note.as_deref())?;
        let mut tx = self.pool.begin().await?;
        let status = if request.trading_enabled {
            "active"
        } else {
            "paused"
        };
        let wallet_id: i64 = sqlx::query_scalar(
            r#"INSERT INTO wallet_accounts (
                 owner_user_id, name, signer_address, funder_address,
                 signature_type, status, trading_enabled
               ) VALUES ($1, $2, $3, $4, $5, $6, $7)
               RETURNING wallet_id"#,
        )
        .bind(actor.user_id)
        .bind(name)
        .bind(&signer_address)
        .bind(funder_address)
        .bind(request.signature_type)
        .bind(status)
        .bind(request.trading_enabled)
        .fetch_one(&mut *tx)
        .await?;
        let envelope = crypto.encrypt_for_storage(
            &plaintext,
            &wallet_storage_aad(wallet_id, actor.user_id, &signer_address, 1),
        )?;
        insert_secret_envelope(&mut tx, wallet_id, actor.user_id, 1, &envelope).await?;
        insert_wallet_risk_policy(&mut tx, wallet_id, &request.risk_policy).await?;
        sqlx::query("INSERT INTO wallet_account_state (wallet_id) VALUES ($1)")
            .bind(wallet_id)
            .execute(&mut *tx)
            .await?;
        insert_audit(
            &mut tx,
            request_id,
            &actor.user_id.to_string(),
            Some(actor.user_id),
            "wallet.create",
            "wallet",
            &wallet_id.to_string(),
            operator_note.as_deref(),
        )
        .await?;
        tx.commit().await?;
        self.get_wallet(actor, wallet_id).await
    }

    pub async fn update_wallet(
        &self,
        actor: ActorScope,
        wallet_id: i64,
        request: &UpdateWalletAccountRequest,
        crypto: &WalletCryptoService,
        request_id: &str,
    ) -> Result<WalletAccountData> {
        if actor.role == UserRole::ReadOnly {
            return Err(ServerError::Forbidden);
        }
        let existing = self.get_wallet(actor, wallet_id).await?;
        if existing.account.owner_user_id != actor.user_id {
            return Err(ServerError::Forbidden);
        }
        let name = request
            .name
            .as_deref()
            .map(|value| required_text(value, "name", 120))
            .transpose()?
            .unwrap_or(existing.account.name.clone());
        let status = request.status.unwrap_or(existing.account.status);
        let trading_enabled = request
            .trading_enabled
            .unwrap_or(existing.account.trading_enabled);
        if trading_enabled && status != WalletAccountStatus::Active {
            return Err(ServerError::InvalidInput(
                "trading_enabled requires wallet status=active".into(),
            ));
        }
        let operator_note = optional_note(request.operator_note.as_deref())?;
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "UPDATE wallet_accounts SET name=$2,status=$3,trading_enabled=$4,updated_at=now() \
             WHERE wallet_id=$1 AND owner_user_id=$5",
        )
        .bind(wallet_id)
        .bind(name)
        .bind(status.as_str())
        .bind(trading_enabled)
        .bind(existing.account.owner_user_id)
        .execute(&mut *tx)
        .await?;
        if let Some(policy) = request.risk_policy.as_ref() {
            validate_risk_policy(policy)?;
            insert_wallet_risk_policy(&mut tx, wallet_id, policy).await?;
        }
        if let Some(encrypted) = request.encrypted_secret.as_ref() {
            let plaintext = decrypt_wallet_import(self, crypto, actor, encrypted).await?;
            validate_wallet_secret(plaintext.expose_secret(), &existing.account.signer_address)?;
            let secret_version = existing.secret.secret_version + 1;
            let envelope = crypto.encrypt_for_storage(
                &plaintext,
                &wallet_storage_aad(
                    wallet_id,
                    existing.account.owner_user_id,
                    &existing.account.signer_address,
                    secret_version,
                ),
            )?;
            update_secret_envelope(&mut tx, wallet_id, secret_version, &envelope).await?;
        }
        insert_audit(
            &mut tx,
            request_id,
            &actor.user_id.to_string(),
            Some(actor.user_id),
            "wallet.update",
            "wallet",
            &wallet_id.to_string(),
            operator_note.as_deref(),
        )
        .await?;
        tx.commit().await?;
        self.get_wallet(actor, wallet_id).await
    }

    pub async fn load_wallet_secret_envelope(
        &self,
        wallet_id: i64,
    ) -> Result<(WalletAccount, WalletSecretEnvelope, i64)> {
        let row = sqlx::query(
            r#"SELECT w.wallet_id,w.owner_user_id,w.name,w.signer_address,w.funder_address,
                      w.signature_type,w.status,w.trading_enabled,w.created_at,w.updated_at,
                      e.ciphertext,e.payload_nonce,e.wrapped_dek,e.wrapped_dek_nonce,
                      e.key_id,e.aad_version,e.secret_version
               FROM wallet_accounts w JOIN wallet_secret_envelopes e ON e.wallet_id=w.wallet_id
               WHERE w.wallet_id=$1"#,
        )
        .bind(wallet_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| ServerError::NotFound(format!("wallet secret {wallet_id}")))?;
        let wallet = wallet_account_from_row(&row)?;
        let version: i64 = row.try_get("secret_version")?;
        Ok((
            wallet,
            WalletSecretEnvelope {
                version: i16::try_from(row.try_get::<i64, _>("aad_version")?).map_err(|_| {
                    ServerError::Internal("wallet envelope version is invalid".into())
                })?,
                key_id: row.try_get("key_id")?,
                payload_nonce: row.try_get("payload_nonce")?,
                ciphertext: row.try_get("ciphertext")?,
                wrapped_dek_nonce: row.try_get("wrapped_dek_nonce")?,
                wrapped_dek: row.try_get("wrapped_dek")?,
            },
            version,
        ))
    }
}

fn wallet_select_sql(suffix: &str) -> String {
    format!(
        r#"SELECT
      w.wallet_id,w.owner_user_id,w.name,w.signer_address,w.funder_address,w.signature_type,
      w.status,w.trading_enabled,w.created_at,w.updated_at,
      e.key_id,e.secret_version,e.updated_at AS secret_updated_at,
      r.max_open_orders,r.max_open_buy_notional,r.max_total_position_notional,
      r.max_market_position_notional,r.max_order_notional,r.updated_at AS risk_updated_at,
      s.available_collateral,s.reserved_collateral,s.open_buy_notional,s.total_position_notional,
      s.last_synced_at,s.last_error,s.version AS state_version,s.updated_at AS state_updated_at
      FROM wallet_accounts w JOIN wallet_secret_envelopes e ON e.wallet_id=w.wallet_id
      JOIN wallet_risk_policies r ON r.wallet_id=w.wallet_id
      JOIN wallet_account_state s ON s.wallet_id=w.wallet_id {suffix}"#
    )
}

fn wallet_data_from_row(row: sqlx::postgres::PgRow) -> Result<WalletAccountData> {
    let account = wallet_account_from_row(&row)?;
    let wallet_id = account.id;
    Ok(WalletAccountData {
        secret: WalletSecretMetadata {
            wallet_id,
            key_id: row.try_get("key_id")?,
            secret_version: row.try_get("secret_version")?,
            updated_at: row.try_get("secret_updated_at")?,
        },
        account,
        risk_policy: WalletRiskPolicy {
            wallet_id,
            max_open_orders: row.try_get("max_open_orders")?,
            max_open_buy_notional: row.try_get("max_open_buy_notional")?,
            max_total_position_notional: row.try_get("max_total_position_notional")?,
            max_market_position_notional: row.try_get("max_market_position_notional")?,
            max_order_notional: row.try_get("max_order_notional")?,
            updated_at: row.try_get("risk_updated_at")?,
        },
        state: WalletAccountState {
            wallet_id,
            available_collateral: row.try_get("available_collateral")?,
            reserved_collateral: row.try_get("reserved_collateral")?,
            open_buy_notional: row.try_get("open_buy_notional")?,
            total_position_notional: row.try_get("total_position_notional")?,
            last_synced_at: row.try_get("last_synced_at")?,
            last_error: row.try_get("last_error")?,
            version: row.try_get("state_version")?,
            updated_at: row.try_get("state_updated_at")?,
        },
    })
}

fn wallet_account_from_row(row: &sqlx::postgres::PgRow) -> Result<WalletAccount> {
    Ok(WalletAccount {
        id: row.try_get("wallet_id")?,
        owner_user_id: row.try_get("owner_user_id")?,
        name: row.try_get("name")?,
        signer_address: row.try_get("signer_address")?,
        funder_address: row.try_get("funder_address")?,
        signature_type: row.try_get("signature_type")?,
        status: enum_value(row.try_get("status")?, "wallet status")?,
        trading_enabled: row.try_get("trading_enabled")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

async fn decrypt_wallet_import(
    store: &PostgresStore,
    crypto: &WalletCryptoService,
    actor: ActorScope,
    encrypted: &polyedge_contracts::EncryptedWalletSecretInput,
) -> Result<secrecy::SecretSlice<u8>> {
    if encrypted.algorithm != "RSA-OAEP-256+A256GCM" {
        return Err(ServerError::InvalidInput(
            "unsupported wallet import algorithm".into(),
        ));
    }
    if encrypted.key_id != crypto.transport_key_id() {
        return Err(ServerError::InvalidInput(
            "wallet import transport key id is invalid".into(),
        ));
    }
    let context_id = Uuid::parse_str(&encrypted.context_id)
        .map_err(|_| ServerError::InvalidInput("wallet import context id is invalid".into()))?;
    store
        .consume_wallet_import_context(actor.user_id, context_id, &encrypted.key_id)
        .await?;
    crypto.decrypt_import_validated(
        context_id,
        &WalletImportCiphertext {
            wrapped_key: encrypted.wrapped_key.clone(),
            nonce: encrypted.nonce.clone(),
            ciphertext: encrypted.ciphertext.clone(),
        },
        actor.user_id.to_string().as_bytes(),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WalletSecretPayload {
    private_key: Zeroizing<String>,
    #[serde(default)]
    api_key: Option<Zeroizing<String>>,
    #[serde(default)]
    api_secret: Option<Zeroizing<String>>,
    #[serde(default)]
    api_passphrase: Option<Zeroizing<String>>,
}

fn validate_wallet_secret(raw: &[u8], expected_signer: &str) -> Result<()> {
    let payload: WalletSecretPayload = serde_json::from_slice(raw)
        .map_err(|_| ServerError::InvalidInput("wallet credential payload is invalid".into()))?;
    if payload
        .api_key
        .as_ref()
        .is_some_and(|value| value.is_empty())
        || payload
            .api_secret
            .as_ref()
            .is_some_and(|value| value.is_empty())
        || payload
            .api_passphrase
            .as_ref()
            .is_some_and(|value| value.is_empty())
    {
        return Err(ServerError::InvalidInput(
            "wallet API credentials cannot be empty".into(),
        ));
    }
    let actual = address_from_private_key(&payload.private_key)?;
    if actual != expected_signer.to_ascii_lowercase() {
        return Err(ServerError::InvalidInput(
            "private key does not match signer address".into(),
        ));
    }
    Ok(())
}

fn address_from_private_key(value: &str) -> Result<String> {
    let bytes = hex::decode(value.trim().strip_prefix("0x").unwrap_or(value.trim()))
        .map_err(|_| ServerError::InvalidInput("private key must be hex encoded".into()))?;
    let secret = k256::SecretKey::from_slice(&bytes)
        .map_err(|_| ServerError::InvalidInput("private key is invalid".into()))?;
    let encoded = secret.public_key().to_encoded_point(false);
    let digest = Keccak256::digest(&encoded.as_bytes()[1..]);
    Ok(format!("0x{}", hex::encode(&digest[12..])))
}

fn normalize_address(value: &str) -> Result<String> {
    let value = value.trim().to_ascii_lowercase();
    if value.len() != 42 || !value.starts_with("0x") || hex::decode(&value[2..]).is_err() {
        return Err(ServerError::InvalidInput(
            "wallet address must be a 20-byte 0x hex address".into(),
        ));
    }
    Ok(value)
}

async fn insert_secret_envelope(
    tx: &mut Transaction<'_, Postgres>,
    wallet_id: i64,
    owner_user_id: i64,
    secret_version: i64,
    envelope: &WalletSecretEnvelope,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO wallet_secret_envelopes (
      wallet_id,owner_user_id,ciphertext,payload_nonce,wrapped_dek,wrapped_dek_nonce,
      key_id,algorithm,aad_version,secret_version
    ) VALUES ($1,$2,$3,$4,$5,$6,$7,'aes-256-gcm+wrapped-dek',$8,$9)"#,
    )
    .bind(wallet_id)
    .bind(owner_user_id)
    .bind(&envelope.ciphertext)
    .bind(&envelope.payload_nonce)
    .bind(&envelope.wrapped_dek)
    .bind(&envelope.wrapped_dek_nonce)
    .bind(&envelope.key_id)
    .bind(i64::from(envelope.version))
    .bind(secret_version)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn update_secret_envelope(
    tx: &mut Transaction<'_, Postgres>,
    wallet_id: i64,
    secret_version: i64,
    envelope: &WalletSecretEnvelope,
) -> Result<()> {
    sqlx::query(r#"UPDATE wallet_secret_envelopes SET ciphertext=$2,payload_nonce=$3,
      wrapped_dek=$4,wrapped_dek_nonce=$5,key_id=$6,aad_version=$7,secret_version=$8,updated_at=now()
      WHERE wallet_id=$1"#).bind(wallet_id).bind(&envelope.ciphertext).bind(&envelope.payload_nonce)
    .bind(&envelope.wrapped_dek).bind(&envelope.wrapped_dek_nonce).bind(&envelope.key_id)
    .bind(i64::from(envelope.version)).bind(secret_version).execute(&mut **tx).await?;
    Ok(())
}

fn validate_wallet_request(request: &CreateWalletAccountRequest) -> Result<()> {
    if !(0..=2).contains(&request.signature_type) {
        return Err(ServerError::InvalidInput(
            "signature_type must be 0, 1, or 2".into(),
        ));
    }
    validate_risk_policy(&request.risk_policy)
}

fn validate_risk_policy(policy: &polyedge_contracts::WalletRiskPolicyInput) -> Result<()> {
    if policy.max_open_orders <= 0
        || policy.max_open_buy_notional < Decimal::ZERO
        || policy.max_total_position_notional < Decimal::ZERO
        || policy.max_market_position_notional < Decimal::ZERO
        || policy.max_order_notional <= Decimal::ZERO
        || policy.max_market_position_notional > policy.max_total_position_notional
        || policy.max_order_notional > policy.max_open_buy_notional
    {
        return Err(ServerError::InvalidInput(
            "wallet risk policy limits are inconsistent".into(),
        ));
    }
    Ok(())
}

async fn insert_wallet_risk_policy(
    tx: &mut Transaction<'_, Postgres>,
    wallet_id: i64,
    policy: &polyedge_contracts::WalletRiskPolicyInput,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO wallet_risk_policies (
      wallet_id,max_open_orders,max_open_buy_notional,max_total_position_notional,
      max_market_position_notional,max_order_notional
    ) VALUES ($1,$2,$3,$4,$5,$6) ON CONFLICT(wallet_id) DO UPDATE SET
      max_open_orders=EXCLUDED.max_open_orders,max_open_buy_notional=EXCLUDED.max_open_buy_notional,
      max_total_position_notional=EXCLUDED.max_total_position_notional,
      max_market_position_notional=EXCLUDED.max_market_position_notional,
      max_order_notional=EXCLUDED.max_order_notional,updated_at=now()"#,
    )
    .bind(wallet_id)
    .bind(policy.max_open_orders)
    .bind(policy.max_open_buy_notional)
    .bind(policy.max_total_position_notional)
    .bind(policy.max_market_position_notional)
    .bind(policy.max_order_notional)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn insert_audit(
    tx: &mut Transaction<'_, Postgres>,
    request_id: &str,
    actor_id: &str,
    resource_owner_user_id: Option<i64>,
    action: &str,
    resource_type: &str,
    resource_id: &str,
    operator_note: Option<&str>,
) -> Result<()> {
    let actor_user_id = actor_id.parse::<i64>().ok();
    sqlx::query(
        r#"INSERT INTO audit_logs (
      request_id,actor_type,actor_user_id,action,resource_owner_user_id,
      resource_type,resource_id,result,operator_note
    ) VALUES ($1,$2,$3,$4,$5,$6,$7,'succeeded',$8)"#,
    )
    .bind(request_id)
    .bind(if actor_user_id.is_some() {
        "user"
    } else {
        "system"
    })
    .bind(actor_user_id)
    .bind(action)
    .bind(resource_owner_user_id)
    .bind(resource_type)
    .bind(resource_id)
    .bind(operator_note)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRIVATE_KEY_ONE: &str =
        "0000000000000000000000000000000000000000000000000000000000000001";
    const PRIVATE_KEY_ONE_ADDRESS: &str = "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf";

    #[test]
    fn wallet_secret_accepts_the_canonical_payload() -> Result<()> {
        let payload = serde_json::json!({
            "private_key": PRIVATE_KEY_ONE,
            "api_key": "key",
            "api_secret": "secret",
            "api_passphrase": "passphrase",
        });
        validate_wallet_secret(payload.to_string().as_bytes(), PRIVATE_KEY_ONE_ADDRESS)
    }

    #[test]
    fn wallet_secret_rejects_hidden_account_overrides() {
        let payload = serde_json::json!({
            "private_key": PRIVATE_KEY_ONE,
            "funder": "0x0000000000000000000000000000000000000000",
            "chain_id": 1,
        });
        assert!(
            validate_wallet_secret(payload.to_string().as_bytes(), PRIVATE_KEY_ONE_ADDRESS)
                .is_err()
        );
    }
}
