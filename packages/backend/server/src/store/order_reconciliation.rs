use super::*;
use polyedge_domain::ManagedOrderStatus;
use serde_json::json;

pub(super) async fn mark_order_unknown(
    store: &PostgresStore,
    job: &WalletExecutionJob,
    order: &ManagedOrder,
    reason: &str,
) -> Result<()> {
    let reason = required_text(reason, "unknown order reason", 160)?;
    let mut tx = store.pool.begin().await?;
    let result = sqlx::query(
        r#"
        UPDATE managed_orders o
        SET status = 'unknown', updated_at = now()
        FROM wallet_execution_jobs j
        JOIN execution_batches b ON b.batch_id = j.batch_id
        WHERE o.managed_order_id = $1
          AND j.job_id = $2 AND j.wallet_id = o.wallet_id
          AND j.owner_user_id = o.owner_user_id
          AND b.subscription_id = o.subscription_id
          AND j.status = 'running' AND j.lease_owner = $3
          AND j.lease_epoch = $4 AND j.lease_expires_at > now()
          AND o.status IN ('planned', 'submitting', 'open', 'partially_filled', 'cancel_pending', 'unknown')
        "#,
    )
    .bind(order.id)
    .bind(job.id)
    .bind(job.lease_owner.as_deref())
    .bind(job.lease_epoch)
    .execute(&mut *tx)
    .await?;
    if result.rows_affected() != 1 {
        return Err(ServerError::Conflict(
            "unknown-order fencing lease is no longer valid".to_string(),
        ));
    }
    sqlx::query(
        r#"
        INSERT INTO order_transitions (
          managed_order_id, from_status, to_status, reason_code
        ) VALUES ($1, $2, 'unknown', $3)
        "#,
    )
    .bind(order.id)
    .bind(order.status.as_str())
    .bind(reason)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

pub(super) async fn reconcile_managed_order(
    store: &PostgresStore,
    job: &WalletExecutionJob,
    order: &ManagedOrder,
    status: ManagedOrderStatus,
    filled_quantity: Decimal,
    reason: &str,
) -> Result<()> {
    let reason = required_text(reason, "order reconciliation reason", 160)?;
    let mut tx = store.pool.begin().await?;
    let row = sqlx::query(
        r#"
        SELECT o.status, o.filled_quantity, o.quantity
        FROM managed_orders o
        JOIN wallet_execution_jobs j
          ON j.job_id = $2 AND j.wallet_id = o.wallet_id
         AND j.owner_user_id = o.owner_user_id
        JOIN execution_batches b
          ON b.batch_id = j.batch_id AND b.subscription_id = o.subscription_id
        WHERE o.managed_order_id = $1
          AND j.status = 'running'
          AND j.lease_owner = $3
          AND j.lease_epoch = $4
          AND j.lease_expires_at > now()
        FOR UPDATE OF o, j
        "#,
    )
    .bind(order.id)
    .bind(job.id)
    .bind(job.lease_owner.as_deref())
    .bind(job.lease_epoch)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| {
        ServerError::Conflict("managed-order reconciliation lease is no longer valid".to_string())
    })?;
    let current_status: ManagedOrderStatus =
        enum_value(row.try_get("status")?, "managed order status")?;
    if is_terminal(current_status) {
        tx.commit().await?;
        return Ok(());
    }

    let current_filled: Decimal = row.try_get("filled_quantity")?;
    let quantity: Decimal = row.try_get("quantity")?;
    if filled_quantity < Decimal::ZERO || filled_quantity > quantity {
        return Err(ServerError::Conflict(format!(
            "venue filled quantity is outside managed order {} bounds",
            order.id
        )));
    }

    sqlx::query(
        r#"
        UPDATE managed_orders
        SET status = $2, filled_quantity = $3,
            last_venue_sync_at = now(), updated_at = now()
        WHERE managed_order_id = $1
        "#,
    )
    .bind(order.id)
    .bind(status.as_str())
    .bind(filled_quantity)
    .execute(&mut *tx)
    .await?;

    if current_status != status || current_filled != filled_quantity {
        sqlx::query(
            r#"
            INSERT INTO order_transitions (
              managed_order_id, from_status, to_status, reason_code, metadata_json
            ) VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(order.id)
        .bind(current_status.as_str())
        .bind(status.as_str())
        .bind(reason)
        .bind(json!({
            "previous_filled_quantity": current_filled.to_string(),
            "filled_quantity": filled_quantity.to_string(),
        }))
        .execute(&mut *tx)
        .await?;
    }
    if filled_quantity > current_filled {
        let fill_delta = filled_quantity - current_filled;
        let external_fill_id = format!(
            "managed:{}:cumulative:{}",
            order.id,
            filled_quantity.normalize()
        );
        sqlx::query(
            r#"INSERT INTO venue_fills (
                 owner_user_id,wallet_id,managed_order_id,market_id,token_id,
                 external_fill_id,side,price,quantity,fee_amount,occurred_at
               ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,0,now())
               ON CONFLICT (wallet_id,external_fill_id) DO NOTHING"#,
        )
        .bind(order.owner_user_id)
        .bind(order.wallet_id)
        .bind(order.id)
        .bind(order.market_id)
        .bind(&order.token_id)
        .bind(external_fill_id)
        .bind(order.side.as_str())
        .bind(order.price)
        .bind(fill_delta)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

fn is_terminal(status: ManagedOrderStatus) -> bool {
    matches!(
        status,
        ManagedOrderStatus::Cancelled
            | ManagedOrderStatus::Filled
            | ManagedOrderStatus::Expired
            | ManagedOrderStatus::Rejected
    )
}
