use super::*;
use polyedge_contracts::{CashFlowData, RecordCashFlowRequest};
use polyedge_domain::{ActorScope, UserRole};

impl PostgresStore {
    pub async fn list_cash_flows(
        &self,
        actor: ActorScope,
        query: &ManualTradingListQuery,
    ) -> Result<Vec<CashFlowData>> {
        let (limit, offset) = page_values(query);
        let rows = sqlx::query(
            r#"SELECT cash_flow_id,owner_user_id,wallet_id,flow_type,amount,
          external_reference,note,occurred_at,recorded_by_user_id,created_at
          FROM external_cash_flows WHERE ($1::boolean OR owner_user_id=$2)
          AND ($3::bigint IS NULL OR wallet_id=$3)
          ORDER BY occurred_at DESC,cash_flow_id DESC LIMIT $4 OFFSET $5"#,
        )
        .bind(actor.role == UserRole::Admin)
        .bind(actor.user_id)
        .bind(query.wallet_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(cash_flow_from_row).collect()
    }

    pub async fn record_cash_flow(
        &self,
        actor: ActorScope,
        request: &RecordCashFlowRequest,
        request_id: &str,
    ) -> Result<CashFlowData> {
        if !actor.is_admin() {
            return Err(ServerError::Forbidden);
        }
        if request.amount <= Decimal::ZERO
            || !matches!(
                request.flow_type.as_str(),
                "deposit" | "withdrawal" | "reward" | "fee" | "adjustment"
            )
        {
            return Err(ServerError::InvalidInput(
                "cash flow type or amount is invalid".into(),
            ));
        }
        let wallet =
            sqlx::query("SELECT owner_user_id, created_at FROM wallet_accounts WHERE wallet_id=$1")
                .bind(request.wallet_id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or_else(|| ServerError::NotFound(format!("wallet {}", request.wallet_id)))?;
        let owner_user_id: i64 = wallet.try_get("owner_user_id")?;
        let wallet_created_at: OffsetDateTime = wallet.try_get("created_at")?;
        if request.occurred_at < wallet_created_at {
            return Err(ServerError::InvalidInput(
                "cash flow cannot predate wallet creation".into(),
            ));
        }
        if request.occurred_at > OffsetDateTime::now_utc() + time::Duration::minutes(5) {
            return Err(ServerError::InvalidInput(
                "cash flow cannot be more than 5 minutes in the future".into(),
            ));
        }
        let note = optional_note(request.note.as_deref())?;
        let reference = request
            .external_reference
            .as_deref()
            .map(|v| required_text(v, "external_reference", 256))
            .transpose()?;
        let mut tx = self.pool.begin().await?;
        let id:i64=sqlx::query_scalar(r#"INSERT INTO external_cash_flows(
          owner_user_id,wallet_id,flow_type,amount,external_reference,note,occurred_at,recorded_by_user_id
        ) VALUES($1,$2,$3,$4,$5,$6,$7,$8) RETURNING cash_flow_id"#)
        .bind(owner_user_id).bind(request.wallet_id).bind(&request.flow_type).bind(request.amount)
        .bind(reference).bind(note.as_deref()).bind(request.occurred_at).bind(actor.user_id)
        .fetch_one(&mut *tx).await?;
        insert_audit(
            &mut tx,
            request_id,
            &actor.user_id.to_string(),
            Some(owner_user_id),
            "cash_flow.record",
            "cash_flow",
            &id.to_string(),
            note.as_deref(),
        )
        .await?;
        tx.commit().await?;
        let row = sqlx::query(
            r#"SELECT cash_flow_id,owner_user_id,wallet_id,flow_type,amount,
          external_reference,note,occurred_at,recorded_by_user_id,created_at
          FROM external_cash_flows WHERE cash_flow_id=$1"#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;
        cash_flow_from_row(row)
    }
}

fn cash_flow_from_row(row: sqlx::postgres::PgRow) -> Result<CashFlowData> {
    Ok(CashFlowData {
        id: row.try_get("cash_flow_id")?,
        owner_user_id: row.try_get("owner_user_id")?,
        wallet_id: row.try_get("wallet_id")?,
        flow_type: row.try_get("flow_type")?,
        amount: row.try_get("amount")?,
        external_reference: row.try_get("external_reference")?,
        note: row.try_get("note")?,
        occurred_at: row.try_get("occurred_at")?,
        recorded_by_user_id: row.try_get("recorded_by_user_id")?,
        created_at: row.try_get("created_at")?,
    })
}
