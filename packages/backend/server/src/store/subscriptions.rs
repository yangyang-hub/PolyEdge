use super::*;
use polyedge_contracts::{
    CreateStrategySubscriptionRequest, StrategySubscriptionData, UpdateStrategySubscriptionRequest,
};
use polyedge_domain::{
    ActorScope, StrategySubscription, StrategySubscriptionKind, StrategySubscriptionStatus,
    StrategySubscriptionWallet,
};

impl PostgresStore {
    pub async fn list_strategy_subscriptions(
        &self,
        query: &ManualTradingListQuery,
        actor: ActorScope,
    ) -> Result<Vec<StrategySubscriptionData>> {
        let (limit, offset) = page_values(query);
        let rows = sqlx::query(
            r#"
            SELECT sub.subscription_id, sub.follower_user_id, sub.source_strategy_id,
              s.name AS source_strategy_name, s.owner_user_id AS source_user_id,
              u.display_name AS source_display_name, sub.subscription_kind, sub.status,
              sub.active_until, LEAST(s.active_until, COALESCE(sub.active_until, s.active_until))
                AS effective_active_until,
              sub.stopped_at, sub.created_at, sub.updated_at
            FROM strategy_subscriptions sub
            JOIN market_strategies s ON s.strategy_id = sub.source_strategy_id
            JOIN users u ON u.user_id = s.owner_user_id
            WHERE ($1 OR sub.follower_user_id = $2)
              AND ($3::bigint IS NULL OR sub.source_strategy_id = $3)
              AND ($4::text IS NULL OR sub.status = $4)
            ORDER BY sub.updated_at DESC, sub.subscription_id DESC
            LIMIT $5 OFFSET $6
            "#,
        )
        .bind(actor.is_admin())
        .bind(actor.user_id)
        .bind(query.strategy_id)
        .bind(query.status.as_deref())
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        self.subscription_rows_to_data(rows).await
    }

    pub async fn create_strategy_subscription(
        &self,
        request: &CreateStrategySubscriptionRequest,
        actor: ActorScope,
        request_id: &str,
    ) -> Result<StrategySubscriptionData> {
        require_subscription_writer(actor)?;
        if request.wallet_ids.is_empty() {
            return Err(ServerError::InvalidInput(
                "a follower subscription requires at least one wallet".to_string(),
            ));
        }
        validate_subscription_expiry(request.active_until)?;
        let operator_note = optional_note(request.operator_note.as_deref())?;
        let mut tx = self.pool.begin().await?;
        let source = sqlx::query(
            r#"
            SELECT owner_user_id, visibility, status, active_until
            FROM market_strategies WHERE strategy_id = $1 FOR SHARE
            "#,
        )
        .bind(request.source_strategy_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ServerError::NotFound(format!("strategy {}", request.source_strategy_id)))?;
        let source_owner_user_id: i64 = source.try_get("owner_user_id")?;
        let visibility: String = source.try_get("visibility")?;
        let status: String = source.try_get("status")?;
        let source_active_until: OffsetDateTime = source.try_get("active_until")?;
        if source_owner_user_id == actor.user_id {
            return Err(ServerError::Conflict(
                "strategy owners already have an owner subscription".to_string(),
            ));
        }
        if visibility != "followable"
            || matches!(status.as_str(), "expired" | "archived")
            || source_active_until <= OffsetDateTime::now_utc()
        {
            return Err(ServerError::NotFound(format!(
                "strategy {} is not followable",
                request.source_strategy_id
            )));
        }
        let subscription_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO strategy_subscriptions (
              follower_user_id, source_strategy_id, subscription_kind, status, active_until
            ) VALUES ($1, $2, 'follower', 'active', $3)
            RETURNING subscription_id
            "#,
        )
        .bind(actor.user_id)
        .bind(request.source_strategy_id)
        .bind(request.active_until)
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| {
            if is_unique_violation(&error) {
                ServerError::Conflict("the strategy is already followed by this user".to_string())
            } else {
                error.into()
            }
        })?;
        super::strategies::replace_subscription_wallets(
            &mut tx,
            subscription_id,
            actor.user_id,
            &request.wallet_ids,
        )
        .await?;
        super::strategies::insert_strategy_audit(
            &mut tx,
            request_id,
            actor,
            actor.user_id,
            "strategy_subscription.create",
            "strategy_subscription",
            subscription_id,
            operator_note.as_deref(),
        )
        .await?;
        tx.commit().await?;
        self.get_strategy_subscription(subscription_id, actor).await
    }

    pub async fn update_strategy_subscription(
        &self,
        subscription_id: i64,
        request: &UpdateStrategySubscriptionRequest,
        actor: ActorScope,
        request_id: &str,
    ) -> Result<StrategySubscriptionData> {
        require_subscription_writer(actor)?;
        validate_subscription_expiry(request.active_until)?;
        let operator_note = optional_note(request.operator_note.as_deref())?;
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            r#"
            SELECT follower_user_id, subscription_kind, status, active_until
            FROM strategy_subscriptions WHERE subscription_id = $1 FOR UPDATE
            "#,
        )
        .bind(subscription_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ServerError::NotFound(format!("subscription {subscription_id}")))?;
        let follower_user_id: i64 = row.try_get("follower_user_id")?;
        if !actor.is_admin() && follower_user_id != actor.user_id {
            return Err(ServerError::NotFound(format!(
                "subscription {subscription_id}"
            )));
        }
        let kind: StrategySubscriptionKind =
            enum_value(row.try_get("subscription_kind")?, "subscription kind")?;
        if kind == StrategySubscriptionKind::Owner {
            return Err(ServerError::Conflict(
                "owner subscriptions are managed through the strategy resource".to_string(),
            ));
        }
        let existing_status: StrategySubscriptionStatus =
            enum_value(row.try_get("status")?, "subscription status")?;
        if matches!(
            existing_status,
            StrategySubscriptionStatus::Stopped | StrategySubscriptionStatus::Expired
        ) {
            return Err(ServerError::Conflict(
                "stopped or expired subscriptions cannot be restarted".to_string(),
            ));
        }
        let mut status = request.status.unwrap_or(existing_status);
        if status == StrategySubscriptionStatus::Expired {
            return Err(ServerError::InvalidInput(
                "expired is managed by the subscription lifetime".to_string(),
            ));
        }
        let active_until = request
            .active_until
            .or(row.try_get::<Option<OffsetDateTime>, _>("active_until")?);
        if active_until.is_some_and(|until| until <= OffsetDateTime::now_utc()) {
            status = StrategySubscriptionStatus::Expired;
        }
        let stopped = matches!(
            status,
            StrategySubscriptionStatus::Stopped | StrategySubscriptionStatus::Expired
        );
        sqlx::query(
            r#"
            UPDATE strategy_subscriptions
            SET status = $2, active_until = $3,
                stopped_at = CASE WHEN $4 THEN COALESCE(stopped_at, now()) ELSE NULL END,
                updated_at = now()
            WHERE subscription_id = $1
            "#,
        )
        .bind(subscription_id)
        .bind(status.as_str())
        .bind(active_until)
        .bind(stopped)
        .execute(&mut *tx)
        .await?;
        if let Some(wallet_ids) = request.wallet_ids.as_ref() {
            if status == StrategySubscriptionStatus::Active && wallet_ids.is_empty() {
                return Err(ServerError::InvalidInput(
                    "an active follower subscription requires at least one wallet".to_string(),
                ));
            }
            super::strategies::replace_subscription_wallets(
                &mut tx,
                subscription_id,
                follower_user_id,
                wallet_ids,
            )
            .await?;
        }
        super::strategies::insert_strategy_audit(
            &mut tx,
            request_id,
            actor,
            follower_user_id,
            "strategy_subscription.update",
            "strategy_subscription",
            subscription_id,
            operator_note.as_deref(),
        )
        .await?;
        tx.commit().await?;
        self.get_strategy_subscription(subscription_id, actor).await
    }

    pub(super) async fn load_subscription_data_for_strategies(
        &self,
        strategy_ids: &[i64],
        follower_user_id: i64,
    ) -> Result<HashMap<i64, StrategySubscriptionData>> {
        if strategy_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let rows = sqlx::query(
            r#"
            SELECT sub.subscription_id, sub.follower_user_id, sub.source_strategy_id,
              s.name AS source_strategy_name, s.owner_user_id AS source_user_id,
              u.display_name AS source_display_name, sub.subscription_kind, sub.status,
              sub.active_until, LEAST(s.active_until, COALESCE(sub.active_until, s.active_until))
                AS effective_active_until,
              sub.stopped_at, sub.created_at, sub.updated_at
            FROM strategy_subscriptions sub
            JOIN market_strategies s ON s.strategy_id = sub.source_strategy_id
            JOIN users u ON u.user_id = s.owner_user_id
            WHERE sub.follower_user_id = $1 AND sub.source_strategy_id = ANY($2)
            "#,
        )
        .bind(follower_user_id)
        .bind(strategy_ids)
        .fetch_all(&self.pool)
        .await?;
        let data = self.subscription_rows_to_data(rows).await?;
        Ok(data
            .into_iter()
            .map(|item| (item.subscription.source_strategy_id, item))
            .collect())
    }

    async fn get_strategy_subscription(
        &self,
        subscription_id: i64,
        actor: ActorScope,
    ) -> Result<StrategySubscriptionData> {
        let rows = sqlx::query(
            r#"
            SELECT sub.subscription_id, sub.follower_user_id, sub.source_strategy_id,
              s.name AS source_strategy_name, s.owner_user_id AS source_user_id,
              u.display_name AS source_display_name, sub.subscription_kind, sub.status,
              sub.active_until, LEAST(s.active_until, COALESCE(sub.active_until, s.active_until))
                AS effective_active_until,
              sub.stopped_at, sub.created_at, sub.updated_at
            FROM strategy_subscriptions sub
            JOIN market_strategies s ON s.strategy_id = sub.source_strategy_id
            JOIN users u ON u.user_id = s.owner_user_id
            WHERE sub.subscription_id = $1 AND ($2 OR sub.follower_user_id = $3)
            "#,
        )
        .bind(subscription_id)
        .bind(actor.is_admin())
        .bind(actor.user_id)
        .fetch_all(&self.pool)
        .await?;
        self.subscription_rows_to_data(rows)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| ServerError::NotFound(format!("subscription {subscription_id}")))
    }

    async fn subscription_rows_to_data(
        &self,
        rows: Vec<sqlx::postgres::PgRow>,
    ) -> Result<Vec<StrategySubscriptionData>> {
        let subscriptions = rows
            .into_iter()
            .map(subscription_from_row)
            .collect::<Result<Vec<_>>>()?;
        let ids = subscriptions.iter().map(|item| item.id).collect::<Vec<_>>();
        let wallet_rows = if ids.is_empty() {
            Vec::new()
        } else {
            sqlx::query(
                r#"
                SELECT subscription_id, follower_user_id, wallet_id, enabled, created_at
                FROM strategy_subscription_wallets
                WHERE subscription_id = ANY($1)
                ORDER BY subscription_id, wallet_id
                "#,
            )
            .bind(&ids)
            .fetch_all(&self.pool)
            .await?
        };
        let mut wallets: HashMap<i64, Vec<StrategySubscriptionWallet>> = HashMap::new();
        for row in wallet_rows {
            let subscription_id: i64 = row.try_get("subscription_id")?;
            wallets
                .entry(subscription_id)
                .or_default()
                .push(StrategySubscriptionWallet {
                    subscription_id,
                    follower_user_id: row.try_get("follower_user_id")?,
                    wallet_id: row.try_get("wallet_id")?,
                    enabled: row.try_get("enabled")?,
                    created_at: row.try_get("created_at")?,
                });
        }
        Ok(subscriptions
            .into_iter()
            .map(|subscription| StrategySubscriptionData {
                wallets: wallets.remove(&subscription.id).unwrap_or_default(),
                subscription,
            })
            .collect())
    }
}

fn require_subscription_writer(actor: ActorScope) -> Result<()> {
    if actor.can_write_markets() {
        Ok(())
    } else {
        Err(ServerError::Forbidden)
    }
}

fn validate_subscription_expiry(active_until: Option<OffsetDateTime>) -> Result<()> {
    if active_until.is_some_and(|until| until <= OffsetDateTime::now_utc()) {
        return Err(ServerError::InvalidInput(
            "subscription active_until must be in the future".to_string(),
        ));
    }
    Ok(())
}

fn subscription_from_row(row: sqlx::postgres::PgRow) -> Result<StrategySubscription> {
    Ok(StrategySubscription {
        id: row.try_get("subscription_id")?,
        follower_user_id: row.try_get("follower_user_id")?,
        source_strategy_id: row.try_get("source_strategy_id")?,
        source_strategy_name: row.try_get("source_strategy_name")?,
        source_user_id: row.try_get("source_user_id")?,
        source_display_name: row.try_get("source_display_name")?,
        kind: enum_value(row.try_get("subscription_kind")?, "subscription kind")?,
        status: enum_value(row.try_get("status")?, "subscription status")?,
        active_until: row.try_get("active_until")?,
        effective_active_until: row.try_get("effective_active_until")?,
        stopped_at: row.try_get("stopped_at")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .is_some_and(|database| database.code().as_deref() == Some("23505"))
}
