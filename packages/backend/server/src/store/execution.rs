use super::execution_enqueue::enqueue_active_wallet_reconciles;
use super::*;
use crate::execution::{
    ActionHandle, ActionProposal, ExecutionContext, ExecutionStore, TargetOrder,
    WalletPositionRiskTotals, WalletPositionSnapshot,
};
use async_trait::async_trait;
use serde_json::Value;
use std::time::Duration;

#[async_trait]
impl ExecutionStore for PostgresStore {
    async fn expire_due_strategies(&self, limit: i64) -> polyedge_domain::Result<u64> {
        PostgresStore::expire_due_strategies(self, limit)
            .await
            .map_err(server_to_domain_error)
    }
    async fn enqueue_active_wallet_reconciles(&self) -> polyedge_domain::Result<usize> {
        enqueue_active_wallet_reconciles(self)
            .await
            .map_err(server_to_domain_error)
    }

    async fn claim_next_job(
        &self,
        owner: &str,
        lease_duration: Duration,
    ) -> polyedge_domain::Result<Option<WalletExecutionJob>> {
        claim_next_job(self, owner, lease_duration)
            .await
            .map_err(server_to_domain_error)
    }

    async fn load_execution_context(
        &self,
        job: &WalletExecutionJob,
    ) -> polyedge_domain::Result<ExecutionContext> {
        load_execution_context(self, job)
            .await
            .map_err(server_to_domain_error)
    }

    async fn renew_job_lease(
        &self,
        job: &WalletExecutionJob,
        owner: &str,
        lease_duration: Duration,
    ) -> polyedge_domain::Result<()> {
        renew_job_lease(self, job, owner, lease_duration)
            .await
            .map_err(server_to_domain_error)
    }

    async fn begin_action(
        &self,
        job: &WalletExecutionJob,
        proposal: ActionProposal,
    ) -> polyedge_domain::Result<Option<ActionHandle>> {
        begin_action(self, job, &proposal)
            .await
            .map_err(server_to_domain_error)
    }

    async fn mark_action_succeeded(
        &self,
        action: &ActionHandle,
        result: Value,
    ) -> polyedge_domain::Result<()> {
        mark_action_succeeded(self, action, result)
            .await
            .map_err(server_to_domain_error)
    }

    async fn mark_action_failed(
        &self,
        action: &ActionHandle,
        error_code: &str,
        message: &str,
        unknown: bool,
    ) -> polyedge_domain::Result<()> {
        mark_action_failed(self, action, error_code, message, unknown)
            .await
            .map_err(server_to_domain_error)
    }

    async fn record_order_submitted(
        &self,
        context: &ExecutionContext,
        target: &TargetOrder,
        generation: i64,
        action: &ActionHandle,
        external_order_id: &str,
    ) -> polyedge_domain::Result<()> {
        record_order_submitted(self, context, target, generation, action, external_order_id)
            .await
            .map_err(server_to_domain_error)
    }

    async fn mark_order_cancelled(
        &self,
        order: &ManagedOrder,
        action: &ActionHandle,
    ) -> polyedge_domain::Result<()> {
        mark_order_cancelled(self, order, action)
            .await
            .map_err(server_to_domain_error)
    }

    async fn mark_order_unknown(
        &self,
        job: &WalletExecutionJob,
        order: &ManagedOrder,
        reason: &str,
    ) -> polyedge_domain::Result<()> {
        super::order_reconciliation::mark_order_unknown(self, job, order, reason)
            .await
            .map_err(server_to_domain_error)
    }

    async fn reconcile_managed_order(
        &self,
        job: &WalletExecutionJob,
        order: &ManagedOrder,
        status: polyedge_domain::ManagedOrderStatus,
        filled_quantity: Decimal,
        reason: &str,
    ) -> polyedge_domain::Result<()> {
        super::order_reconciliation::reconcile_managed_order(
            self,
            job,
            order,
            status,
            filled_quantity,
            reason,
        )
        .await
        .map_err(server_to_domain_error)
    }

    async fn finish_job(
        &self,
        job: &WalletExecutionJob,
        success: bool,
        error: Option<&polyedge_domain::AppError>,
    ) -> polyedge_domain::Result<()> {
        finish_job(self, job, success, error)
            .await
            .map_err(server_to_domain_error)
    }

    async fn update_wallet_balance(
        &self,
        wallet_id: i64,
        available_collateral: Decimal,
    ) -> polyedge_domain::Result<()> {
        sqlx::query(
            r#"
            UPDATE wallet_account_state
            SET available_collateral = $2, last_synced_at = now(),
                last_error = NULL, version = version + 1, updated_at = now()
            WHERE wallet_id = $1
            "#,
        )
        .bind(wallet_id)
        .bind(available_collateral)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            polyedge_domain::AppError::internal(
                "WALLET_BALANCE_UPDATE_FAILED",
                format!("failed to persist wallet balance: {error}"),
            )
        })?;
        Ok(())
    }

    async fn replace_wallet_positions(
        &self,
        wallet_id: i64,
        market_id: i64,
        snapshot: WalletPositionSnapshot,
    ) -> polyedge_domain::Result<WalletPositionRiskTotals> {
        replace_wallet_positions(self, wallet_id, market_id, snapshot)
            .await
            .map_err(server_to_domain_error)
    }
}

async fn claim_next_job(
    store: &PostgresStore,
    owner: &str,
    lease_duration: Duration,
) -> Result<Option<WalletExecutionJob>> {
    let lease_ms = i64::try_from(lease_duration.as_millis()).map_err(|_| {
        ServerError::InvalidInput("execution lease duration is too large".to_string())
    })?;
    let row = sqlx::query(
        r#"
        WITH candidate AS (
          SELECT j.job_id
          FROM wallet_execution_jobs j
          JOIN execution_batches b ON b.batch_id = j.batch_id
          WHERE b.status IN ('pending', 'running')
            AND (
              j.status = 'pending'
              OR (j.status = 'running' AND j.lease_expires_at <= now())
            )
          ORDER BY j.created_at, j.job_id
          FOR UPDATE SKIP LOCKED
          LIMIT 1
        )
        UPDATE wallet_execution_jobs j
        SET status = 'running', attempt_count = j.attempt_count + 1,
            lease_owner = $1, lease_epoch = j.lease_epoch + 1,
            lease_expires_at = now() + ($2::bigint * interval '1 millisecond'),
            started_at = COALESCE(j.started_at, now()), updated_at = now()
        FROM candidate
        WHERE j.job_id = candidate.job_id
        RETURNING j.job_id, j.batch_id, j.owner_user_id, j.wallet_id, j.status, j.attempt_count,
                  j.error_code, j.error_message, j.lease_epoch, j.lease_owner,
                  j.lease_expires_at, j.created_at, j.updated_at
        "#,
    )
    .bind(owner)
    .bind(lease_ms)
    .fetch_optional(&store.pool)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let batch_id: i64 = row.try_get("batch_id")?;
    sqlx::query(
        "UPDATE execution_batches SET status = 'running', started_at = COALESCE(started_at, now()) WHERE batch_id = $1 AND status = 'pending'",
    )
    .bind(batch_id)
    .execute(&store.pool)
    .await?;
    Ok(Some(job_from_row(&row)?))
}

async fn renew_job_lease(
    store: &PostgresStore,
    job: &WalletExecutionJob,
    owner: &str,
    lease_duration: Duration,
) -> Result<()> {
    let lease_ms = i64::try_from(lease_duration.as_millis()).map_err(|_| {
        ServerError::InvalidInput("execution lease duration is too large".to_string())
    })?;
    let result = sqlx::query(
        r#"
        UPDATE wallet_execution_jobs
        SET lease_expires_at = now() + ($4::bigint * interval '1 millisecond'),
            updated_at = now()
        WHERE job_id = $1 AND lease_owner = $2 AND lease_epoch = $3
          AND status = 'running' AND lease_expires_at > now()
        "#,
    )
    .bind(job.id)
    .bind(owner)
    .bind(job.lease_epoch)
    .bind(lease_ms)
    .execute(&store.pool)
    .await?;
    if result.rows_affected() != 1 {
        return Err(ServerError::Conflict(
            "execution job lease could not be renewed".to_string(),
        ));
    }
    Ok(())
}

async fn load_execution_context(
    store: &PostgresStore,
    job: &WalletExecutionJob,
) -> Result<ExecutionContext> {
    let row = sqlx::query(
        r#"
        SELECT
          w.wallet_id, w.owner_user_id, w.name, w.signer_address, w.funder_address,
          w.signature_type, w.status AS wallet_status,
          w.trading_enabled AS wallet_trading_enabled,
          w.created_at AS wallet_created_at, w.updated_at AS wallet_updated_at,
          r.max_open_orders, r.max_open_buy_notional,
          r.max_total_position_notional, r.max_market_position_notional,
          r.max_order_notional, r.updated_at AS risk_updated_at,
          a.available_collateral, a.reserved_collateral, a.open_buy_notional,
          a.total_position_notional, a.last_synced_at, a.last_error,
          a.version AS state_version, a.updated_at AS state_updated_at,
          v.strategy_version_id, v.strategy_id, v.version_number,
          v.status AS version_status, v.book_freshness_ms,
          v.reward_minimum_size, v.reward_maximum_spread, v.reward_daily_rate,
          v.downward_reprice_confirm_ms, v.upward_reprice_confirm_ms,
          v.reprice_cooldown_ms, v.max_replaces_per_cycle, v.published_at,
          v.created_at AS version_created_at,
          s.market_id, b.subscription_id, sub.status AS subscription_status,
          COALESCE(sw.enabled, FALSE)
            AND source_user.status='active' AND follower_user.status='active'
            AS subscription_wallet_enabled,
          s.status AS strategy_status, s.active_from AS strategy_active_from,
          LEAST(s.active_until, COALESCE(sub.active_until, s.active_until)) AS effective_active_until,
          m.status AS market_status, sys.trading_enabled AS system_trading_enabled,
          sys.kill_switch_locked,
          COALESCE((
            SELECT SUM(p.quantity * p.average_price)
            FROM positions p
            WHERE p.wallet_id = j.wallet_id AND p.market_id = s.market_id
          ), 0) AS market_position_notional,
          (b.batch_type = 'cancel') AS force_cancel_all
        FROM wallet_execution_jobs j
        JOIN execution_batches b ON b.batch_id = j.batch_id
        JOIN strategy_versions v ON v.strategy_version_id = b.strategy_version_id
        JOIN market_strategies s ON s.strategy_id = v.strategy_id
        JOIN users source_user ON source_user.user_id = s.owner_user_id
        JOIN managed_markets m ON m.market_id = s.market_id
        JOIN strategy_subscriptions sub ON sub.subscription_id = b.subscription_id
        JOIN users follower_user ON follower_user.user_id = sub.follower_user_id
        LEFT JOIN strategy_subscription_wallets sw
          ON sw.subscription_id = sub.subscription_id AND sw.wallet_id = j.wallet_id
        JOIN wallet_accounts w ON w.wallet_id = j.wallet_id
        JOIN wallet_risk_policies r ON r.wallet_id = w.wallet_id
        JOIN wallet_account_state a ON a.wallet_id = w.wallet_id
        JOIN system_runtime_state sys ON sys.singleton = TRUE
        WHERE j.job_id = $1 AND j.lease_owner = $2 AND j.lease_epoch = $3
          AND j.lease_expires_at > now()
        "#,
    )
    .bind(job.id)
    .bind(job.lease_owner.as_deref())
    .bind(job.lease_epoch)
    .fetch_optional(&store.pool)
    .await?
    .ok_or_else(|| ServerError::Conflict("execution job lease is no longer valid".to_string()))?;
    let version_id: i64 = row.try_get("strategy_version_id")?;
    let market_id: i64 = row.try_get("market_id")?;
    let slots = sqlx::query(
        r#"
        SELECT quote_slot_id, strategy_version_id, slot_key, outcome,
               quantity, pricing_mode, fixed_price, book_rank, price_offset,
               minimum_price, maximum_price, post_only, enabled
        FROM strategy_quote_slots
        WHERE strategy_version_id = $1
        ORDER BY quote_slot_id
        "#,
    )
    .bind(version_id)
    .fetch_all(&store.pool)
    .await?
    .into_iter()
    .map(|row| slot_from_row(&row))
    .collect::<Result<Vec<_>>>()?;
    let outcome_rows =
        sqlx::query("SELECT outcome, token_id FROM managed_market_outcomes WHERE market_id = $1")
            .bind(market_id)
            .fetch_all(&store.pool)
            .await?;
    let mut outcomes = HashMap::new();
    for outcome in outcome_rows {
        outcomes.insert(
            enum_value(outcome.try_get("outcome")?, "market outcome")?,
            outcome.try_get("token_id")?,
        );
    }
    let managed_orders = sqlx::query(
        r#"
        SELECT managed_order_id, owner_user_id, wallet_id, subscription_id, market_id, strategy_version_id,
               quote_slot_id, token_id, outcome, side, price, quantity,
               filled_quantity, status, external_order_id, generation,
               created_at, updated_at
        FROM managed_orders
        WHERE wallet_id = $1 AND subscription_id = $2
          AND status IN ('planned', 'submitting', 'open', 'partially_filled', 'cancel_pending', 'unknown')
          AND (
            $3::boolean = FALSE
            OR EXISTS (
              SELECT 1 FROM execution_actions ea
              WHERE ea.job_id = $4 AND ea.managed_order_id = managed_orders.managed_order_id
                AND ea.action_type = 'cancel_order' AND ea.status = 'planned'
            )
          )
        ORDER BY managed_order_id
        "#,
    )
    .bind(job.wallet_id)
    .bind(row.try_get::<i64, _>("subscription_id")?)
    .bind(row.try_get::<bool, _>("force_cancel_all")?)
    .bind(job.id)
    .fetch_all(&store.pool)
    .await?
    .into_iter()
    .map(|row| managed_order_from_row(&row))
    .collect::<Result<Vec<_>>>()?;
    Ok(ExecutionContext {
        job: job.clone(),
        wallet: WalletAccount {
            id: row.try_get("wallet_id")?,
            owner_user_id: row.try_get("owner_user_id")?,
            name: row.try_get("name")?,
            signer_address: row.try_get("signer_address")?,
            funder_address: row.try_get("funder_address")?,
            signature_type: row.try_get("signature_type")?,
            status: enum_value(row.try_get("wallet_status")?, "wallet status")?,
            trading_enabled: row.try_get("wallet_trading_enabled")?,
            created_at: row.try_get("wallet_created_at")?,
            updated_at: row.try_get("wallet_updated_at")?,
        },
        subscription_id: row.try_get("subscription_id")?,
        subscription_status: enum_value(
            row.try_get("subscription_status")?,
            "subscription status",
        )?,
        subscription_wallet_enabled: row.try_get("subscription_wallet_enabled")?,
        strategy_status: enum_value(row.try_get("strategy_status")?, "strategy status")?,
        strategy_active_from: row.try_get("strategy_active_from")?,
        effective_active_until: row.try_get("effective_active_until")?,
        market_status: enum_value(row.try_get("market_status")?, "market status")?,
        strategy_version: StrategyVersion {
            id: version_id,
            strategy_id: row.try_get("strategy_id")?,
            version_number: row.try_get("version_number")?,
            status: enum_value(row.try_get("version_status")?, "version status")?,
            book_freshness_ms: row.try_get("book_freshness_ms")?,
            downward_reprice_confirm_ms: row.try_get("downward_reprice_confirm_ms")?,
            upward_reprice_confirm_ms: row.try_get("upward_reprice_confirm_ms")?,
            reprice_cooldown_ms: row.try_get("reprice_cooldown_ms")?,
            max_replaces_per_cycle: row.try_get("max_replaces_per_cycle")?,
            published_at: row.try_get("published_at")?,
            created_at: row.try_get("version_created_at")?,
        },
        market_id,
        slots,
        outcomes,
        managed_orders,
        risk_policy: WalletRiskPolicy {
            wallet_id: job.wallet_id,
            max_open_orders: row.try_get("max_open_orders")?,
            max_open_buy_notional: row.try_get("max_open_buy_notional")?,
            max_total_position_notional: row.try_get("max_total_position_notional")?,
            max_market_position_notional: row.try_get("max_market_position_notional")?,
            max_order_notional: row.try_get("max_order_notional")?,
            updated_at: row.try_get("risk_updated_at")?,
        },
        account_state: WalletAccountState {
            wallet_id: job.wallet_id,
            available_collateral: row.try_get("available_collateral")?,
            reserved_collateral: row.try_get("reserved_collateral")?,
            open_buy_notional: row.try_get("open_buy_notional")?,
            total_position_notional: row.try_get("total_position_notional")?,
            last_synced_at: row.try_get("last_synced_at")?,
            last_error: row.try_get("last_error")?,
            version: row.try_get("state_version")?,
            updated_at: row.try_get("state_updated_at")?,
        },
        market_position_notional: row.try_get("market_position_notional")?,
        trading_enabled: row.try_get("system_trading_enabled")?,
        kill_switch_locked: row.try_get("kill_switch_locked")?,
        force_cancel_all: row.try_get("force_cancel_all")?,
    })
}

async fn begin_action(
    store: &PostgresStore,
    job: &WalletExecutionJob,
    proposal: &ActionProposal,
) -> Result<Option<ActionHandle>> {
    let row = sqlx::query(
        r#"
        INSERT INTO execution_actions (
          job_id, quote_slot_id, managed_order_id, action_type, status,
          idempotency_key, reason_code, request_json, attempt_count,
          lease_owner, lease_epoch, lease_expires_at
        ) VALUES (
          $1, $2, $3, $4, 'executing', $5, $6, $7, 1,
          $8, $9, $10
        )
        ON CONFLICT (idempotency_key) DO NOTHING
        RETURNING action_id, idempotency_key
        "#,
    )
    .bind(job.id)
    .bind(proposal.slot_id)
    .bind(proposal.managed_order_id)
    .bind(proposal.action.as_str())
    .bind(&proposal.idempotency_key)
    .bind(&proposal.reason)
    .bind(&proposal.request)
    .bind(job.lease_owner.as_deref())
    .bind(job.lease_epoch)
    .bind(job.lease_expires_at)
    .fetch_optional(&store.pool)
    .await?;
    Ok(row.map(|row| ActionHandle {
        action_id: row.get("action_id"),
        idempotency_key: row.get("idempotency_key"),
    }))
}

async fn mark_action_succeeded(
    store: &PostgresStore,
    action: &ActionHandle,
    result: Value,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE execution_actions
        SET status = 'succeeded', result_json = $2, completed_at = now(),
            updated_at = now(), lease_owner = NULL, lease_expires_at = NULL
        WHERE action_id = $1 AND status = 'executing'
        "#,
    )
    .bind(action.action_id)
    .bind(result)
    .execute(&store.pool)
    .await?;
    Ok(())
}

async fn mark_action_failed(
    store: &PostgresStore,
    action: &ActionHandle,
    error_code: &str,
    message: &str,
    unknown: bool,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE execution_actions
        SET status = $2, last_error = $3, result_json = jsonb_build_object('error_code', $4),
            completed_at = now(), updated_at = now(),
            lease_owner = NULL, lease_expires_at = NULL
        WHERE action_id = $1 AND status = 'executing'
        "#,
    )
    .bind(action.action_id)
    .bind(if unknown { "unknown" } else { "failed" })
    .bind(message)
    .bind(error_code)
    .execute(&store.pool)
    .await?;
    Ok(())
}

async fn record_order_submitted(
    store: &PostgresStore,
    context: &ExecutionContext,
    target: &TargetOrder,
    generation: i64,
    action: &ActionHandle,
    external_order_id: &str,
) -> Result<()> {
    let mut tx = store.pool.begin().await?;
    let managed_order_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO managed_orders (
          owner_user_id, wallet_id, subscription_id, market_id, strategy_version_id, quote_slot_id,
          token_id, outcome, side, price, quantity, status,
          external_order_id, client_order_key, generation, last_venue_sync_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'buy', $9, $10, 'open', $11, $12, $13, now())
        RETURNING managed_order_id
        "#,
    )
    .bind(context.wallet.owner_user_id)
    .bind(context.wallet.id)
    .bind(context.subscription_id)
    .bind(context.market_id)
    .bind(context.strategy_version.id)
    .bind(target.slot.id)
    .bind(&target.token_id)
    .bind(target.slot.outcome.as_str())
    .bind(target.price)
    .bind(target.quantity)
    .bind(external_order_id)
    .bind(&action.idempotency_key)
    .bind(generation)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO order_transitions (
          managed_order_id, action_id, from_status, to_status, reason_code
        ) VALUES ($1, $2, NULL, 'open', 'venue_accepted')
        "#,
    )
    .bind(managed_order_id)
    .bind(action.action_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

async fn mark_order_cancelled(
    store: &PostgresStore,
    order: &ManagedOrder,
    action: &ActionHandle,
) -> Result<()> {
    let mut tx = store.pool.begin().await?;
    let result = sqlx::query(
        r#"
        UPDATE managed_orders
        SET status = 'cancelled', updated_at = now(), last_venue_sync_at = now()
        WHERE managed_order_id = $1
          AND status IN ('open', 'partially_filled', 'cancel_pending', 'unknown')
        "#,
    )
    .bind(order.id)
    .execute(&mut *tx)
    .await?;
    if result.rows_affected() == 1 {
        sqlx::query(
            r#"
            INSERT INTO order_transitions (
              managed_order_id, action_id, from_status, to_status, reason_code
            ) VALUES ($1, $2, $3, 'cancelled', 'venue_cancel_confirmed')
            "#,
        )
        .bind(order.id)
        .bind(action.action_id)
        .bind(order.status.as_str())
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn finish_job(
    store: &PostgresStore,
    job: &WalletExecutionJob,
    success: bool,
    error: Option<&polyedge_domain::AppError>,
) -> Result<()> {
    if success {
        sqlx::query(
            r#"
            UPDATE execution_actions
            SET status = 'succeeded', result_json = '{"marker":true}'::jsonb,
                completed_at = now(), updated_at = now()
            WHERE job_id = $1 AND status = 'planned'
              AND action_type = 'cancel_order'
            "#,
        )
        .bind(job.id)
        .execute(&store.pool)
        .await?;
    }
    let status = if success { "succeeded" } else { "failed" };
    let result = sqlx::query(
        r#"
        UPDATE wallet_execution_jobs
        SET status = $2, error_code = $3, error_message = $4,
            completed_at = now(), updated_at = now(),
            lease_owner = NULL, lease_expires_at = NULL
        WHERE job_id = $1 AND lease_owner = $5 AND lease_epoch = $6
        "#,
    )
    .bind(job.id)
    .bind(status)
    .bind(error.map(|error| error.code()))
    .bind(error.map(|error| error.message()))
    .bind(job.lease_owner.as_deref())
    .bind(job.lease_epoch)
    .execute(&store.pool)
    .await?;
    if result.rows_affected() != 1 {
        return Err(ServerError::Conflict(
            "execution job lease was lost before completion".to_string(),
        ));
    }
    sqlx::query(
        r#"
        UPDATE execution_batches b
        SET status = summary.status,
            completed_at = CASE WHEN summary.status IN ('succeeded', 'failed', 'partially_succeeded') THEN now() ELSE b.completed_at END
        FROM (
          SELECT batch_id,
            CASE
              WHEN bool_and(status = 'succeeded') THEN 'succeeded'
              WHEN bool_and(status = 'failed') THEN 'failed'
              WHEN bool_and(status IN ('succeeded', 'failed', 'cancelled')) THEN 'partially_succeeded'
              ELSE 'running'
            END AS status
          FROM wallet_execution_jobs
          WHERE batch_id = $1
          GROUP BY batch_id
        ) summary
        WHERE b.batch_id = summary.batch_id
        "#,
    )
    .bind(job.batch_id)
    .execute(&store.pool)
    .await?;
    Ok(())
}

fn server_to_domain_error(error: ServerError) -> polyedge_domain::AppError {
    tracing::error!(error = ?error, "execution store operation failed");
    match error {
        ServerError::InvalidInput(message) => {
            polyedge_domain::AppError::invalid_input("EXECUTION_STORE_INVALID", message)
        }
        ServerError::NotFound(message) => {
            polyedge_domain::AppError::not_found("EXECUTION_STORE_NOT_FOUND", message)
        }
        ServerError::Conflict(message) => {
            polyedge_domain::AppError::conflict("EXECUTION_STORE_CONFLICT", message)
        }
        ServerError::Dependency(message) => {
            polyedge_domain::AppError::dependency_unavailable("EXECUTION_STORE_DEPENDENCY", message)
        }
        other => polyedge_domain::AppError::internal("EXECUTION_STORE_FAILED", other.to_string()),
    }
}
