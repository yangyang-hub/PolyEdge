use super::*;

impl PostgresStore {
    pub async fn list_wallets(
        &self,
        query: &ManualTradingListQuery,
    ) -> Result<Vec<WalletAccountData>> {
        let (limit, offset) = page_values(query);
        let rows = sqlx::query(
            r#"
            SELECT
              w.wallet_id, w.name, w.signer_address, w.funder_address,
              w.signature_type, w.credential_ref_id, w.status, w.trading_enabled,
              w.created_at, w.updated_at,
              c.provider, c.locator, c.key_version,
              c.created_at AS credential_created_at,
              c.updated_at AS credential_updated_at,
              r.max_open_orders, r.max_open_buy_notional,
              r.max_total_position_notional, r.max_market_position_notional,
              r.max_order_notional, r.updated_at AS risk_updated_at,
              s.available_collateral, s.reserved_collateral, s.open_buy_notional,
              s.total_position_notional, s.last_synced_at, s.last_error,
              s.version AS state_version, s.updated_at AS state_updated_at
            FROM wallet_accounts w
            JOIN wallet_credential_refs c ON c.credential_ref_id = w.credential_ref_id
            JOIN wallet_risk_policies r ON r.wallet_id = w.wallet_id
            JOIN wallet_account_state s ON s.wallet_id = w.wallet_id
            WHERE ($1::text IS NULL OR w.status = $1)
            ORDER BY w.updated_at DESC, w.wallet_id DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(query.status.as_deref())
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(wallet_data_from_row).collect()
    }

    pub async fn get_wallet(&self, wallet_id: i64) -> Result<WalletAccountData> {
        let row = sqlx::query(
            r#"
            SELECT
              w.wallet_id, w.name, w.signer_address, w.funder_address,
              w.signature_type, w.credential_ref_id, w.status, w.trading_enabled,
              w.created_at, w.updated_at,
              c.provider, c.locator, c.key_version,
              c.created_at AS credential_created_at,
              c.updated_at AS credential_updated_at,
              r.max_open_orders, r.max_open_buy_notional,
              r.max_total_position_notional, r.max_market_position_notional,
              r.max_order_notional, r.updated_at AS risk_updated_at,
              s.available_collateral, s.reserved_collateral, s.open_buy_notional,
              s.total_position_notional, s.last_synced_at, s.last_error,
              s.version AS state_version, s.updated_at AS state_updated_at
            FROM wallet_accounts w
            JOIN wallet_credential_refs c ON c.credential_ref_id = w.credential_ref_id
            JOIN wallet_risk_policies r ON r.wallet_id = w.wallet_id
            JOIN wallet_account_state s ON s.wallet_id = w.wallet_id
            WHERE w.wallet_id = $1
            "#,
        )
        .bind(wallet_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| ServerError::NotFound(format!("wallet {wallet_id}")))?;
        wallet_data_from_row(row)
    }

    pub async fn create_wallet(
        &self,
        request: &CreateWalletAccountRequest,
        actor_id: &str,
        request_id: &str,
    ) -> Result<WalletAccountData> {
        validate_wallet_request(request)?;
        let name = required_text(&request.name, "name", 120)?;
        let signer_address = required_text(&request.signer_address, "signer_address", 120)?;
        let funder_address = required_text(&request.funder_address, "funder_address", 120)?;
        let locator = required_text(&request.credential_locator, "credential_locator", 500)?;
        let operator_note = optional_note(request.operator_note.as_deref())?;
        let mut tx = self.pool.begin().await?;
        let credential_ref_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO wallet_credential_refs (provider, locator, key_version)
            VALUES ($1, $2, $3)
            ON CONFLICT (provider, locator) DO UPDATE SET
              key_version = EXCLUDED.key_version,
              updated_at = now()
            RETURNING credential_ref_id
            "#,
        )
        .bind(request.credential_provider.as_str())
        .bind(locator)
        .bind(request.credential_key_version.as_deref())
        .fetch_one(&mut *tx)
        .await?;
        let status = if request.trading_enabled {
            "active"
        } else {
            "paused"
        };
        let wallet_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO wallet_accounts (
              name, signer_address, funder_address, signature_type,
              credential_ref_id, status, trading_enabled
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING wallet_id
            "#,
        )
        .bind(name)
        .bind(signer_address)
        .bind(funder_address)
        .bind(request.signature_type)
        .bind(credential_ref_id)
        .bind(status)
        .bind(request.trading_enabled)
        .fetch_one(&mut *tx)
        .await?;
        insert_wallet_risk_policy(&mut tx, wallet_id, &request.risk_policy).await?;
        sqlx::query("INSERT INTO wallet_account_state (wallet_id) VALUES ($1)")
            .bind(wallet_id)
            .execute(&mut *tx)
            .await?;
        insert_audit(
            &mut tx,
            request_id,
            actor_id,
            "wallet.create",
            "wallet",
            &wallet_id.to_string(),
            operator_note.as_deref(),
        )
        .await?;
        tx.commit().await?;
        self.get_wallet(wallet_id).await
    }

    pub async fn update_wallet(
        &self,
        wallet_id: i64,
        request: &UpdateWalletAccountRequest,
        actor_id: &str,
        request_id: &str,
    ) -> Result<WalletAccountData> {
        let existing = self.get_wallet(wallet_id).await?;
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
                "trading_enabled requires wallet status=active".to_string(),
            ));
        }
        let operator_note = optional_note(request.operator_note.as_deref())?;
        let provider = request
            .credential_provider
            .unwrap_or(existing.credential.provider);
        let locator = request
            .credential_locator
            .as_deref()
            .map(|value| required_text(value, "credential_locator", 500))
            .transpose()?
            .unwrap_or(existing.credential.locator.clone());
        let key_version = request
            .credential_key_version
            .as_deref()
            .or(existing.credential.key_version.as_deref());
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            r#"
            UPDATE wallet_credential_refs
            SET provider = $2, locator = $3, key_version = $4, updated_at = now()
            WHERE credential_ref_id = $1
            "#,
        )
        .bind(existing.credential.id)
        .bind(provider.as_str())
        .bind(locator)
        .bind(key_version)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            UPDATE wallet_accounts
            SET name = $2, status = $3, trading_enabled = $4, updated_at = now()
            WHERE wallet_id = $1
            "#,
        )
        .bind(wallet_id)
        .bind(name)
        .bind(status.as_str())
        .bind(trading_enabled)
        .execute(&mut *tx)
        .await?;
        if let Some(policy) = request.risk_policy.as_ref() {
            validate_risk_policy(policy)?;
            insert_wallet_risk_policy(&mut tx, wallet_id, policy).await?;
        }
        insert_audit(
            &mut tx,
            request_id,
            actor_id,
            "wallet.update",
            "wallet",
            &wallet_id.to_string(),
            operator_note.as_deref(),
        )
        .await?;
        tx.commit().await?;
        self.get_wallet(wallet_id).await
    }
}

fn validate_wallet_request(request: &CreateWalletAccountRequest) -> Result<()> {
    if !(0..=2).contains(&request.signature_type) {
        return Err(ServerError::InvalidInput(
            "signature_type must be 0, 1, or 2".to_string(),
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
            "wallet risk policy limits are inconsistent".to_string(),
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
        r#"
        INSERT INTO wallet_risk_policies (
          wallet_id, max_open_orders, max_open_buy_notional,
          max_total_position_notional, max_market_position_notional, max_order_notional
        ) VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (wallet_id) DO UPDATE SET
          max_open_orders = EXCLUDED.max_open_orders,
          max_open_buy_notional = EXCLUDED.max_open_buy_notional,
          max_total_position_notional = EXCLUDED.max_total_position_notional,
          max_market_position_notional = EXCLUDED.max_market_position_notional,
          max_order_notional = EXCLUDED.max_order_notional,
          updated_at = now()
        "#,
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

pub(super) async fn insert_audit(
    tx: &mut Transaction<'_, Postgres>,
    request_id: &str,
    actor_id: &str,
    action: &str,
    resource_type: &str,
    resource_id: &str,
    operator_note: Option<&str>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO audit_logs (
          request_id, actor_id, action, resource_type, resource_id, result, operator_note
        ) VALUES ($1, $2, $3, $4, $5, 'succeeded', $6)
        "#,
    )
    .bind(request_id)
    .bind(actor_id)
    .bind(action)
    .bind(resource_type)
    .bind(resource_id)
    .bind(operator_note)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn wallet_data_from_row(row: sqlx::postgres::PgRow) -> Result<WalletAccountData> {
    let wallet_id: i64 = row.try_get("wallet_id")?;
    Ok(WalletAccountData {
        account: WalletAccount {
            id: wallet_id,
            name: row.try_get("name")?,
            signer_address: row.try_get("signer_address")?,
            funder_address: row.try_get("funder_address")?,
            signature_type: row.try_get("signature_type")?,
            credential_ref_id: row.try_get("credential_ref_id")?,
            status: enum_value(row.try_get("status")?, "wallet status")?,
            trading_enabled: row.try_get("trading_enabled")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        },
        credential: WalletCredentialRef {
            id: row.try_get("credential_ref_id")?,
            provider: enum_value(row.try_get("provider")?, "credential provider")?,
            locator: row.try_get("locator")?,
            key_version: row.try_get("key_version")?,
            created_at: row.try_get("credential_created_at")?,
            updated_at: row.try_get("credential_updated_at")?,
        },
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
