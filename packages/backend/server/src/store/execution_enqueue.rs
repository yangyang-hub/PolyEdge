use super::*;

type OrderToCancel = (i64, Option<i64>, Option<String>);
type CancelGroups = HashMap<(i64, i64, i64, i64), HashMap<i64, Vec<OrderToCancel>>>;

pub(super) async fn enqueue_active_wallet_reconciles(store: &PostgresStore) -> Result<usize> {
    let mut created = dispatch_strategy_commands(store).await?;
    created += enqueue_invalid_order_cancels(store).await?;
    created += enqueue_periodic_executes(store).await?;
    Ok(created)
}

async fn dispatch_strategy_commands(store: &PostgresStore) -> Result<usize> {
    let mut tx = store.pool.begin().await?;
    let commands = sqlx::query(
        r#"
        SELECT command_id, source_strategy_id, strategy_version_id, command_type
        FROM strategy_commands
        WHERE status = 'pending'
        ORDER BY created_at, command_id
        LIMIT 100
        FOR UPDATE SKIP LOCKED
        "#,
    )
    .fetch_all(&mut *tx)
    .await?;
    let mut created = 0usize;
    for command in commands {
        let command_id: i64 = command.try_get("command_id")?;
        let strategy_id: i64 = command.try_get("source_strategy_id")?;
        let command_type: polyedge_domain::StrategyCommandType =
            enum_value(command.try_get("command_type")?, "strategy command type")?;
        let version_id = match command.try_get::<Option<i64>, _>("strategy_version_id")? {
            Some(version_id) => Some(version_id),
            None => sqlx::query_scalar::<_, i64>(
                "SELECT strategy_version_id FROM strategy_versions WHERE strategy_id = $1 AND status = 'published'",
            )
            .bind(strategy_id)
            .fetch_optional(&mut *tx)
            .await?,
        };
        created += match command_type {
            polyedge_domain::StrategyCommandType::Publish
            | polyedge_domain::StrategyCommandType::Activate
            | polyedge_domain::StrategyCommandType::Resume => {
                if let Some(version_id) = version_id {
                    enqueue_command_executes(&mut tx, command_id, strategy_id, version_id).await?
                } else {
                    0
                }
            }
            polyedge_domain::StrategyCommandType::Pause
            | polyedge_domain::StrategyCommandType::Expire
            | polyedge_domain::StrategyCommandType::Archive
            | polyedge_domain::StrategyCommandType::ForceCancel => {
                enqueue_command_cancels(&mut tx, command_id, strategy_id).await?
            }
        };
        sqlx::query(
            r#"
            UPDATE strategy_commands
            SET status = 'completed', completed_at = now(), updated_at = now(),
                lease_owner = NULL, lease_expires_at = NULL, last_error = NULL
            WHERE command_id = $1 AND status = 'pending'
            "#,
        )
        .bind(command_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(created)
}

async fn enqueue_command_executes(
    tx: &mut Transaction<'_, Postgres>,
    command_id: i64,
    strategy_id: i64,
    version_id: i64,
) -> Result<usize> {
    let rows = sqlx::query(
        r#"
        SELECT sub.follower_user_id, sub.subscription_id, sw.wallet_id
        FROM market_strategies s
        JOIN managed_markets m ON m.market_id = s.market_id
        JOIN users source_user ON source_user.user_id = s.owner_user_id
        JOIN strategy_subscriptions sub ON sub.source_strategy_id = s.strategy_id
        JOIN users follower_user ON follower_user.user_id = sub.follower_user_id
        JOIN strategy_subscription_wallets sw ON sw.subscription_id = sub.subscription_id
        JOIN wallet_accounts w
          ON w.wallet_id = sw.wallet_id AND w.owner_user_id = sub.follower_user_id
        WHERE s.strategy_id = $1 AND s.status = 'active' AND m.status = 'open'
          AND now() >= s.active_from AND now() < s.active_until
          AND sub.status = 'active'
          AND (sub.active_until IS NULL OR now() < sub.active_until)
          AND source_user.status = 'active' AND follower_user.status = 'active'
          AND sw.enabled AND w.status = 'active' AND w.trading_enabled
        ORDER BY sub.subscription_id, sw.wallet_id
        "#,
    )
    .bind(strategy_id)
    .fetch_all(&mut **tx)
    .await?;
    let mut grouped: HashMap<(i64, i64), Vec<i64>> = HashMap::new();
    for row in rows {
        grouped
            .entry((
                row.try_get("follower_user_id")?,
                row.try_get("subscription_id")?,
            ))
            .or_default()
            .push(row.try_get("wallet_id")?);
    }
    let mut created = 0usize;
    for ((owner_user_id, subscription_id), wallet_ids) in grouped {
        let existing: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
              SELECT 1 FROM execution_batches
              WHERE subscription_id = $1 AND strategy_version_id = $2
                AND batch_type = 'execute'
                AND (status IN ('pending', 'running')
                     OR created_at > now() - interval '5 seconds')
            )
            "#,
        )
        .bind(subscription_id)
        .bind(version_id)
        .fetch_one(&mut **tx)
        .await?;
        if existing {
            continue;
        }
        let batch_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO execution_batches (
              subscriber_user_id, subscription_id, source_strategy_id,
              strategy_version_id, strategy_command_id, batch_type, status, request_source
            ) VALUES ($1, $2, $3, $4, $5, 'execute', 'pending', 'strategy_command')
            RETURNING batch_id
            "#,
        )
        .bind(owner_user_id)
        .bind(subscription_id)
        .bind(strategy_id)
        .bind(version_id)
        .bind(command_id)
        .fetch_one(&mut **tx)
        .await?;
        for wallet_id in wallet_ids {
            insert_job(tx, batch_id, owner_user_id, wallet_id).await?;
            created += 1;
        }
    }
    Ok(created)
}

async fn enqueue_command_cancels(
    tx: &mut Transaction<'_, Postgres>,
    command_id: i64,
    strategy_id: i64,
) -> Result<usize> {
    let rows = sqlx::query(
        r#"
        SELECT o.managed_order_id, o.owner_user_id, o.subscription_id,
               o.strategy_version_id, o.wallet_id, o.quote_slot_id, o.external_order_id
        FROM managed_orders o
        JOIN strategy_versions v ON v.strategy_version_id = o.strategy_version_id
        WHERE v.strategy_id = $1
          AND o.status IN ('planned','submitting','open','partially_filled','cancel_pending','unknown')
        ORDER BY o.subscription_id, o.strategy_version_id, o.wallet_id, o.managed_order_id
        "#,
    )
    .bind(strategy_id)
    .fetch_all(&mut **tx)
    .await?;
    let groups = cancellation_groups(rows, strategy_id)?;
    enqueue_cancel_groups(tx, groups, Some(command_id), "strategy_command").await
}

async fn enqueue_periodic_executes(store: &PostgresStore) -> Result<usize> {
    let rows = sqlx::query(
        r#"
        SELECT sub.follower_user_id, sub.subscription_id, s.strategy_id,
               v.strategy_version_id, sw.wallet_id
        FROM strategy_versions v
        JOIN market_strategies s ON s.strategy_id = v.strategy_id
        JOIN managed_markets m ON m.market_id = s.market_id
        JOIN users source_user ON source_user.user_id = s.owner_user_id
        JOIN strategy_subscriptions sub ON sub.source_strategy_id = s.strategy_id
        JOIN users follower_user ON follower_user.user_id = sub.follower_user_id
        JOIN strategy_subscription_wallets sw ON sw.subscription_id = sub.subscription_id
        JOIN wallet_accounts w
          ON w.wallet_id = sw.wallet_id AND w.owner_user_id = sub.follower_user_id
        WHERE v.status = 'published' AND s.status = 'active' AND m.status = 'open'
          AND now() >= s.active_from AND now() < s.active_until
          AND sub.status = 'active' AND (sub.active_until IS NULL OR now() < sub.active_until)
          AND source_user.status = 'active' AND follower_user.status = 'active'
          AND sw.enabled AND w.status = 'active' AND w.trading_enabled
        ORDER BY v.strategy_version_id, sw.wallet_id
        "#,
    )
    .fetch_all(&store.pool)
    .await?;
    let mut grouped: HashMap<(i64, i64, i64, i64), Vec<i64>> = HashMap::new();
    for row in rows {
        grouped
            .entry((
                row.try_get("follower_user_id")?,
                row.try_get("subscription_id")?,
                row.try_get("strategy_id")?,
                row.try_get("strategy_version_id")?,
            ))
            .or_default()
            .push(row.try_get("wallet_id")?);
    }
    let mut created = 0usize;
    for ((owner_user_id, subscription_id, strategy_id, version_id), wallet_ids) in grouped {
        let mut tx = store.pool.begin().await?;
        let existing: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
              SELECT 1 FROM execution_batches
              WHERE subscription_id = $1 AND strategy_version_id = $2
                AND batch_type = 'execute'
                AND (status IN ('pending', 'running')
                     OR created_at > now() - interval '5 seconds')
            )
            "#,
        )
        .bind(subscription_id)
        .bind(version_id)
        .fetch_one(&mut *tx)
        .await?;
        if existing {
            tx.rollback().await?;
            continue;
        }
        let batch_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO execution_batches (
              subscriber_user_id, subscription_id, source_strategy_id,
              strategy_version_id, batch_type, status, request_source
            ) VALUES ($1, $2, $3, $4, 'execute', 'pending', 'runtime')
            RETURNING batch_id
            "#,
        )
        .bind(owner_user_id)
        .bind(subscription_id)
        .bind(strategy_id)
        .bind(version_id)
        .fetch_one(&mut *tx)
        .await?;
        for wallet_id in wallet_ids {
            insert_job(&mut tx, batch_id, owner_user_id, wallet_id).await?;
            created += 1;
        }
        tx.commit().await?;
    }
    Ok(created)
}

async fn enqueue_invalid_order_cancels(store: &PostgresStore) -> Result<usize> {
    let rows = sqlx::query(
        r#"
        SELECT o.managed_order_id, o.owner_user_id, o.subscription_id,
               v.strategy_id, o.strategy_version_id, o.wallet_id,
               o.quote_slot_id, o.external_order_id
        FROM managed_orders o
        JOIN strategy_versions v ON v.strategy_version_id = o.strategy_version_id
        JOIN market_strategies s ON s.strategy_id = v.strategy_id
        JOIN managed_markets m ON m.market_id = s.market_id
        JOIN users source_user ON source_user.user_id = s.owner_user_id
        JOIN strategy_subscriptions sub ON sub.subscription_id = o.subscription_id
        JOIN users follower_user ON follower_user.user_id = sub.follower_user_id
        JOIN wallet_accounts w
          ON w.wallet_id = o.wallet_id AND w.owner_user_id = o.owner_user_id
        LEFT JOIN strategy_subscription_wallets sw
          ON sw.subscription_id = sub.subscription_id AND sw.wallet_id = o.wallet_id
        WHERE o.status IN ('planned','submitting','open','partially_filled','cancel_pending','unknown')
          AND (v.status <> 'published' OR s.status <> 'active' OR m.status <> 'open'
               OR w.status <> 'active' OR w.trading_enabled = FALSE
               OR now() < s.active_from OR now() >= s.active_until
               OR sub.status <> 'active'
               OR source_user.status <> 'active' OR follower_user.status <> 'active'
               OR (sub.active_until IS NOT NULL AND now() >= sub.active_until)
               OR COALESCE(sw.enabled, FALSE) = FALSE)
        ORDER BY o.subscription_id, o.strategy_version_id, o.wallet_id, o.managed_order_id
        "#,
    )
    .fetch_all(&store.pool)
    .await?;
    let groups = cancellation_groups(rows, 0)?;
    let mut tx = store.pool.begin().await?;
    let created = enqueue_cancel_groups(&mut tx, groups, None, "expiry_supervisor").await?;
    tx.commit().await?;
    Ok(created)
}

fn cancellation_groups(rows: Vec<sqlx::postgres::PgRow>, strategy_id: i64) -> Result<CancelGroups> {
    let mut groups: CancelGroups = HashMap::new();
    for row in rows {
        let source_strategy_id = if strategy_id > 0 {
            strategy_id
        } else {
            row.try_get("strategy_id")?
        };
        groups
            .entry((
                row.try_get("owner_user_id")?,
                row.try_get("subscription_id")?,
                source_strategy_id,
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
    Ok(groups)
}

async fn enqueue_cancel_groups(
    tx: &mut Transaction<'_, Postgres>,
    groups: CancelGroups,
    command_id: Option<i64>,
    request_source: &str,
) -> Result<usize> {
    let mut created = 0usize;
    for ((owner_user_id, subscription_id, strategy_id, version_id), wallets) in groups {
        let existing: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
              SELECT 1 FROM execution_batches
              WHERE subscription_id = $1 AND strategy_version_id = $2
                AND batch_type = 'cancel'
                AND (status IN ('pending','running')
                     OR created_at > now() - interval '5 seconds')
            )
            "#,
        )
        .bind(subscription_id)
        .bind(version_id)
        .fetch_one(&mut **tx)
        .await?;
        if existing {
            continue;
        }
        let batch_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO execution_batches (
              subscriber_user_id, subscription_id, source_strategy_id,
              strategy_version_id, strategy_command_id, batch_type, status, request_source
            ) VALUES ($1, $2, $3, $4, $5, 'cancel', 'pending', $6)
            RETURNING batch_id
            "#,
        )
        .bind(owner_user_id)
        .bind(subscription_id)
        .bind(strategy_id)
        .bind(version_id)
        .bind(command_id)
        .bind(request_source)
        .fetch_one(&mut **tx)
        .await?;
        for (wallet_id, orders) in wallets {
            let job_id = insert_job(tx, batch_id, owner_user_id, wallet_id).await?;
            for (order_id, slot_id, external_order_id) in orders {
                sqlx::query(
                    r#"
                    INSERT INTO execution_actions (
                      job_id, quote_slot_id, managed_order_id, action_type,
                      status, idempotency_key, reason_code, request_json
                    ) VALUES (
                      $1, $2, $3, 'cancel_order', 'planned', $4,
                      'desired_state_inactive', jsonb_build_object('external_order_id', $5)
                    )
                    ON CONFLICT (idempotency_key) DO NOTHING
                    "#,
                )
                .bind(job_id)
                .bind(slot_id)
                .bind(order_id)
                .bind(format!("cancel:{wallet_id}:{order_id}:batch:{batch_id}"))
                .bind(external_order_id)
                .execute(&mut **tx)
                .await?;
            }
            created += 1;
        }
    }
    Ok(created)
}

async fn insert_job(
    tx: &mut Transaction<'_, Postgres>,
    batch_id: i64,
    owner_user_id: i64,
    wallet_id: i64,
) -> Result<i64> {
    Ok(sqlx::query_scalar(
        r#"
        INSERT INTO wallet_execution_jobs (batch_id, owner_user_id, wallet_id, status)
        VALUES ($1, $2, $3, 'pending')
        RETURNING job_id
        "#,
    )
    .bind(batch_id)
    .bind(owner_user_id)
    .bind(wallet_id)
    .fetch_one(&mut **tx)
    .await?)
}
