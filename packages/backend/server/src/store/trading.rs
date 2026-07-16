use super::*;

type CancellationOrder = (i64, Option<i64>, Option<String>);
type CancellationGroups = HashMap<(i64, i64, i64, i64), HashMap<i64, Vec<CancellationOrder>>>;
impl PostgresStore {
    pub async fn list_orders(
        &self,
        query: &ManualTradingListQuery,
        actor: polyedge_domain::ActorScope,
    ) -> Result<Vec<ManagedOrder>> {
        let (limit, offset) = page_values(query);
        let rows = sqlx::query(
            r#"
            SELECT
              o.managed_order_id, o.owner_user_id, o.wallet_id, o.subscription_id,
              o.market_id, o.strategy_version_id,
              o.quote_slot_id, o.token_id, o.outcome, o.side, o.price,
              o.quantity, o.filled_quantity, o.status, o.external_order_id,
              o.generation, o.created_at, o.updated_at
            FROM managed_orders o
            JOIN strategy_versions v ON v.strategy_version_id = o.strategy_version_id
            WHERE ($1::bigint IS NULL OR o.wallet_id = $1)
              AND ($2::bigint IS NULL OR o.market_id = $2)
              AND ($3::bigint IS NULL OR v.strategy_id = $3)
              AND ($4::text IS NULL OR o.status = $4)
              AND ($5::boolean OR o.owner_user_id = $6)
            ORDER BY o.updated_at DESC, o.managed_order_id DESC
            LIMIT $7 OFFSET $8
            "#,
        )
        .bind(query.wallet_id)
        .bind(query.market_id)
        .bind(query.strategy_id)
        .bind(query.status.as_deref())
        .bind(actor.role == polyedge_domain::UserRole::Admin)
        .bind(actor.user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| managed_order_from_row(&row))
            .collect()
    }

    pub async fn list_positions(
        &self,
        query: &ManualTradingListQuery,
        actor: polyedge_domain::ActorScope,
    ) -> Result<Vec<ManagedPosition>> {
        let (limit, offset) = page_values(query);
        let rows = sqlx::query(
            r#"
            SELECT position_id, owner_user_id, wallet_id, market_id, token_id, outcome,
                   quantity, average_price, realized_pnl, version, updated_at
            FROM positions
            WHERE ($1::bigint IS NULL OR wallet_id = $1)
              AND ($2::bigint IS NULL OR market_id = $2)
              AND quantity > 0
              AND ($3::boolean OR owner_user_id = $4)
            ORDER BY updated_at DESC, position_id DESC
            LIMIT $5 OFFSET $6
            "#,
        )
        .bind(query.wallet_id)
        .bind(query.market_id)
        .bind(actor.role == polyedge_domain::UserRole::Admin)
        .bind(actor.user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| position_from_row(&row))
            .collect()
    }

    pub async fn active_token_ids(&self) -> Result<Vec<String>> {
        Ok(sqlx::query_scalar::<_, String>(
            r#"
            SELECT DISTINCT token_id
            FROM (
              SELECT o.token_id
              FROM managed_market_outcomes o
              JOIN market_strategies s ON s.market_id = o.market_id
              JOIN strategy_versions v ON v.strategy_id = s.strategy_id
              JOIN strategy_subscriptions sub ON sub.source_strategy_id = s.strategy_id
              JOIN strategy_subscription_wallets t ON t.subscription_id = sub.subscription_id
              JOIN wallet_accounts w ON w.wallet_id = t.wallet_id
              WHERE s.status = 'active' AND v.status = 'published'
                AND now() >= s.active_from AND now() < s.active_until
                AND sub.status = 'active' AND (sub.active_until IS NULL OR now() < sub.active_until)
                AND t.enabled AND w.status = 'active' AND w.trading_enabled
              UNION
              SELECT token_id FROM managed_orders
              WHERE status IN ('planned', 'submitting', 'open', 'partially_filled', 'cancel_pending', 'unknown')
              UNION
              SELECT token_id FROM positions WHERE quantity > 0
            ) active_tokens
            ORDER BY token_id
            "#,
        )
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn kill_switch_locked(&self) -> Result<bool> {
        Ok(sqlx::query_scalar::<_, bool>(
            "SELECT kill_switch_locked FROM system_runtime_state WHERE singleton = TRUE",
        )
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn system_runtime_state(&self) -> Result<polyedge_contracts::SystemRuntimeStateData> {
        let row = sqlx::query(
            r#"
            SELECT kill_switch_locked, trading_enabled, reason,
                   version,
                   CASE
                     WHEN updated_by_actor = 'user' THEN updated_by_user_id::text
                     ELSE 'system'
                   END AS updated_by,
                   updated_at
            FROM system_runtime_state
            WHERE singleton = TRUE
            "#,
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(polyedge_contracts::SystemRuntimeStateData {
            kill_switch_locked: row.try_get("kill_switch_locked")?,
            trading_enabled: row.try_get("trading_enabled")?,
            reason: row.try_get("reason")?,
            version: row.try_get("version")?,
            updated_by: row.try_get("updated_by")?,
            updated_at: row.try_get("updated_at")?,
        })
    }

    pub async fn update_system_runtime_state(
        &self,
        request: &polyedge_contracts::UpdateSystemRuntimeStateRequest,
        actor: polyedge_domain::ActorScope,
        request_id: &str,
    ) -> Result<polyedge_contracts::SystemRuntimeStateData> {
        if !actor.is_admin() {
            return Err(ServerError::Forbidden);
        }
        if request.kill_switch_locked && request.trading_enabled {
            return Err(ServerError::InvalidInput(
                "trading cannot be enabled while the kill switch is locked".to_string(),
            ));
        }
        let reason = request
            .reason
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let operator_note = optional_note(request.operator_note.as_deref())?;
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            r#"
            UPDATE system_runtime_state
            SET kill_switch_locked = $1,
                trading_enabled = $2,
                reason = $3,
                version = version + 1,
                updated_by_user_id = $4,
                updated_by_actor = 'user',
                updated_at = now()
            WHERE singleton = TRUE
            "#,
        )
        .bind(request.kill_switch_locked)
        .bind(request.trading_enabled)
        .bind(reason)
        .bind(actor.user_id)
        .execute(&mut *tx)
        .await?;
        insert_audit(
            &mut tx,
            request_id,
            &actor.user_id.to_string(),
            None,
            "system.runtime_state.update",
            "system_runtime_state",
            "singleton",
            operator_note.as_deref(),
        )
        .await?;
        tx.commit().await?;
        self.system_runtime_state().await
    }

    pub async fn create_cancellation_batches(
        &self,
        request: &polyedge_contracts::CreateCancellationBatchRequest,
        actor: polyedge_domain::ActorScope,
        request_id: &str,
    ) -> Result<Vec<i64>> {
        let operator_note = optional_note(request.operator_note.as_deref())?;
        let rows = sqlx::query(
            r#"
            SELECT o.managed_order_id, o.owner_user_id, o.subscription_id,
                   v.strategy_id AS source_strategy_id, o.wallet_id, o.strategy_version_id,
                   o.quote_slot_id, o.external_order_id
            FROM managed_orders o
            JOIN managed_markets m ON m.market_id = o.market_id
            JOIN strategy_versions v ON v.strategy_version_id = o.strategy_version_id
            WHERE o.status IN ('open', 'partially_filled', 'unknown')
              AND (cardinality($1::bigint[]) = 0 OR o.wallet_id = ANY($1))
              AND (cardinality($2::text[]) = 0 OR m.condition_id = ANY($2))
              AND ($3::boolean OR o.owner_user_id = $4)
            ORDER BY o.strategy_version_id, o.wallet_id, o.managed_order_id
            "#,
        )
        .bind(&request.wallet_ids)
        .bind(&request.condition_ids)
        .bind(actor.role == polyedge_domain::UserRole::Admin)
        .bind(actor.user_id)
        .fetch_all(&self.pool)
        .await?;
        if rows.is_empty() {
            return Ok(Vec::new());
        }
        let mut grouped = CancellationGroups::new();
        for row in rows {
            grouped
                .entry((
                    row.try_get("owner_user_id")?,
                    row.try_get("subscription_id")?,
                    row.try_get("source_strategy_id")?,
                    row.try_get("strategy_version_id")?,
                ))
                .or_default()
                .entry(row.try_get("wallet_id")?)
                .or_default()
                .push((
                    row.try_get("managed_order_id")?,
                    row.try_get("quote_slot_id")?,
                    row.try_get("external_order_id")?,
                ));
        }
        let mut tx = self.pool.begin().await?;
        let mut batch_ids = Vec::new();
        for ((owner_user_id, subscription_id, source_strategy_id, strategy_version_id), wallets) in
            grouped
        {
            let batch_id: i64 = sqlx::query_scalar(
                r#"
                INSERT INTO execution_batches (
                  subscriber_user_id, subscription_id, source_strategy_id,
                  strategy_version_id, batch_type, status, requested_by_user_id,
                  request_source, operator_note
                ) VALUES ($1, $2, $3, $4, 'cancel', 'pending', $5, 'operator', $6)
                RETURNING batch_id
                "#,
            )
            .bind(owner_user_id)
            .bind(subscription_id)
            .bind(source_strategy_id)
            .bind(strategy_version_id)
            .bind(actor.user_id)
            .bind(operator_note.as_deref())
            .fetch_one(&mut *tx)
            .await?;
            for (wallet_id, orders) in wallets {
                let job_id: i64 = sqlx::query_scalar(
                    "INSERT INTO wallet_execution_jobs (batch_id, owner_user_id, wallet_id, status) VALUES ($1, $2, $3, 'pending') RETURNING job_id",
                )
                .bind(batch_id)
                .bind(owner_user_id)
                .bind(wallet_id)
                .fetch_one(&mut *tx)
                .await?;
                for (order_id, slot_id, external_order_id) in orders {
                    sqlx::query(
                        r#"
                        INSERT INTO execution_actions (
                          job_id, quote_slot_id, managed_order_id, action_type,
                          status, idempotency_key, reason_code, request_json
                        ) VALUES (
                          $1, $2, $3, 'cancel_order', 'planned', $4,
                          'operator_cancel_batch', jsonb_build_object('external_order_id', $5)
                        )
                        ON CONFLICT (idempotency_key) DO NOTHING
                        "#,
                    )
                    .bind(job_id)
                    .bind(slot_id)
                    .bind(order_id)
                    .bind(format!("cancel:{wallet_id}:{order_id}:operator"))
                    .bind(external_order_id)
                    .execute(&mut *tx)
                    .await?;
                }
            }
            insert_audit(
                &mut tx,
                request_id,
                &actor.user_id.to_string(),
                Some(owner_user_id),
                "cancellation_batch.create",
                "execution_batch",
                &batch_id.to_string(),
                operator_note.as_deref(),
            )
            .await?;
            batch_ids.push(batch_id);
        }
        tx.commit().await?;
        Ok(batch_ids)
    }
}

pub(super) fn managed_order_from_row(row: &sqlx::postgres::PgRow) -> Result<ManagedOrder> {
    Ok(ManagedOrder {
        id: row.try_get("managed_order_id")?,
        owner_user_id: row.try_get("owner_user_id")?,
        wallet_id: row.try_get("wallet_id")?,
        subscription_id: row.try_get("subscription_id")?,
        market_id: row.try_get("market_id")?,
        strategy_version_id: row.try_get("strategy_version_id")?,
        quote_slot_id: row.try_get("quote_slot_id")?,
        token_id: row.try_get("token_id")?,
        outcome: enum_value(row.try_get("outcome")?, "order outcome")?,
        side: enum_value(row.try_get("side")?, "order side")?,
        price: row.try_get("price")?,
        quantity: row.try_get("quantity")?,
        filled_quantity: row.try_get("filled_quantity")?,
        status: enum_value(row.try_get("status")?, "order status")?,
        external_order_id: row.try_get("external_order_id")?,
        generation: row.try_get("generation")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn position_from_row(row: &sqlx::postgres::PgRow) -> Result<ManagedPosition> {
    Ok(ManagedPosition {
        id: row.try_get("position_id")?,
        owner_user_id: row.try_get("owner_user_id")?,
        wallet_id: row.try_get("wallet_id")?,
        market_id: row.try_get("market_id")?,
        token_id: row.try_get("token_id")?,
        outcome: enum_value(row.try_get("outcome")?, "position outcome")?,
        quantity: row.try_get("quantity")?,
        average_price: row.try_get("average_price")?,
        realized_pnl: row.try_get("realized_pnl")?,
        version: row.try_get("version")?,
        updated_at: row.try_get("updated_at")?,
    })
}
