pub struct InMemoryRewardBotStore {
    config: RwLock<RewardBotConfig>,
    markets: RwLock<HashMap<String, RewardMarket>>,
    quote_plans: RwLock<HashMap<String, RewardQuotePlan>>,
    orders: RwLock<Vec<ManagedRewardOrder>>,
    positions: RwLock<HashMap<(String, String), RewardPosition>>,
    events: RwLock<Vec<RewardRiskEvent>>,
}

impl InMemoryRewardBotStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: RwLock::new(RewardBotConfig::default()),
            markets: RwLock::new(HashMap::new()),
            quote_plans: RwLock::new(HashMap::new()),
            orders: RwLock::new(Vec::new()),
            positions: RwLock::new(HashMap::new()),
            events: RwLock::new(Vec::new()),
        }
    }
}

#[async_trait]
impl RewardBotStore for InMemoryRewardBotStore {
    async fn load_config(&self) -> Result<RewardBotConfig> {
        Ok(self.config.read().await.clone().normalized())
    }

    async fn save_config(&self, config: &RewardBotConfig) -> Result<()> {
        *self.config.write().await = config.clone().normalized();
        Ok(())
    }

    async fn upsert_markets(&self, markets: &[RewardMarket]) -> Result<()> {
        let mut store = self.markets.write().await;
        for market in markets {
            store.insert(market.condition_id.clone(), market.clone());
        }
        Ok(())
    }

    async fn save_quote_plans(&self, plans: &[RewardQuotePlan]) -> Result<()> {
        let mut store = self.quote_plans.write().await;
        for plan in plans {
            store.insert(plan.condition_id.clone(), plan.clone());
        }
        Ok(())
    }

    async fn replace_simulated_orders(
        &self,
        account_id: &str,
        orders: &[ManagedRewardOrder],
        _trace_id: &str,
    ) -> Result<usize> {
        let now = OffsetDateTime::now_utc();
        let mut store = self.orders.write().await;
        let mut cancelled = 0;
        for order in store.iter_mut() {
            if order.account_id == account_id && order.status.is_open_like() {
                order.status = ManagedRewardOrderStatus::Cancelled;
                order.reason = "replaced by latest rewards simulation".to_string();
                order.updated_at = now;
                cancelled += 1;
            }
        }
        store.extend(orders.iter().cloned());
        store.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(cancelled)
    }

    async fn cancel_open_orders(
        &self,
        account_id: Option<&str>,
        reason: &str,
        _trace_id: &str,
    ) -> Result<usize> {
        let now = OffsetDateTime::now_utc();
        let mut cancelled = 0;
        let mut store = self.orders.write().await;
        for order in store.iter_mut() {
            let account_matches =
                account_id.is_none_or(|account_id| account_id == order.account_id);
            if account_matches && order.status.is_open_like() {
                order.status = ManagedRewardOrderStatus::Cancelled;
                order.reason = reason.to_string();
                order.updated_at = now;
                cancelled += 1;
            }
        }
        Ok(cancelled)
    }

    async fn list_markets(&self, limit: u16) -> Result<Vec<RewardMarket>> {
        let mut markets = self
            .markets
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        markets.sort_by(|left, right| {
            right
                .total_daily_rate
                .cmp(&left.total_daily_rate)
                .then_with(|| right.updated_at.cmp(&left.updated_at))
        });
        markets.truncate(usize::from(limit));
        Ok(markets)
    }

    async fn list_quote_plans(&self, limit: u16) -> Result<Vec<RewardQuotePlan>> {
        let mut plans = self
            .quote_plans
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        plans.sort_by(|left, right| {
            right
                .eligible
                .cmp(&left.eligible)
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| right.updated_at.cmp(&left.updated_at))
        });
        plans.truncate(usize::from(limit));
        Ok(plans)
    }

    async fn list_orders(&self, limit: u16) -> Result<Vec<ManagedRewardOrder>> {
        let mut orders = self.orders.read().await.clone();
        orders.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        orders.truncate(usize::from(limit));
        Ok(orders)
    }

    async fn list_positions(&self, limit: u16) -> Result<Vec<RewardPosition>> {
        let mut positions = self
            .positions
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        positions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        positions.truncate(usize::from(limit));
        Ok(positions)
    }

    async fn list_events(&self, limit: u16) -> Result<Vec<RewardRiskEvent>> {
        let mut events = self.events.read().await.clone();
        events.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        events.truncate(usize::from(limit));
        Ok(events)
    }

    async fn log_event(&self, event: RewardRiskEvent) -> Result<()> {
        let mut events = self.events.write().await;
        events.push(event);
        events.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        events.truncate(1_000);
        Ok(())
    }
}

pub struct PostgresRewardBotStore {
    pool: PgPool,
}

impl PostgresRewardBotStore {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RewardBotStore for PostgresRewardBotStore {
    async fn load_config(&self) -> Result<RewardBotConfig> {
        let rows = sqlx::query(
            r#"
            SELECT key, value
            FROM reward_bot_config
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query reward bot config: {error}"),
            )
        })?;

        let mut config = RewardBotConfig::default();
        for row in rows {
            let key: String = row.try_get("key").map_err(postgres_decode_error)?;
            let value: String = row.try_get("value").map_err(postgres_decode_error)?;
            apply_reward_config_value(&mut config, &key, &value)?;
        }
        Ok(config.normalized())
    }

    async fn save_config(&self, config: &RewardBotConfig) -> Result<()> {
        let config = config.clone().normalized();
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin reward config transaction: {error}"),
            )
        })?;

        for (key, value) in reward_config_entries(&config) {
            sqlx::query(
                r#"
                INSERT INTO reward_bot_config (key, value, updated_at)
                VALUES ($1, $2, now())
                ON CONFLICT (key) DO UPDATE
                SET value = EXCLUDED.value,
                    updated_at = now()
                "#,
            )
            .bind(key)
            .bind(value)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_UPSERT_FAILED",
                    format!("failed to upsert reward bot config: {error}"),
                )
            })?;
        }

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit reward config transaction: {error}"),
            )
        })?;
        Ok(())
    }

    async fn upsert_markets(&self, markets: &[RewardMarket]) -> Result<()> {
        for market in markets {
            sqlx::query(
                r#"
                INSERT INTO reward_markets (
                  condition_id,
                  question,
                  market_slug,
                  event_slug,
                  image,
                  rewards_max_spread,
                  rewards_min_size,
                  total_daily_rate,
                  tokens_json,
                  active,
                  updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                ON CONFLICT (condition_id) DO UPDATE
                SET question = EXCLUDED.question,
                    market_slug = EXCLUDED.market_slug,
                    event_slug = EXCLUDED.event_slug,
                    image = EXCLUDED.image,
                    rewards_max_spread = EXCLUDED.rewards_max_spread,
                    rewards_min_size = EXCLUDED.rewards_min_size,
                    total_daily_rate = EXCLUDED.total_daily_rate,
                    tokens_json = EXCLUDED.tokens_json,
                    active = EXCLUDED.active,
                    updated_at = EXCLUDED.updated_at
                "#,
            )
            .bind(&market.condition_id)
            .bind(&market.question)
            .bind(&market.market_slug)
            .bind(&market.event_slug)
            .bind(&market.image)
            .bind(market.rewards_max_spread)
            .bind(market.rewards_min_size)
            .bind(market.total_daily_rate)
            .bind(Json(market.tokens.clone()))
            .bind(market.active)
            .bind(market.updated_at)
            .execute(&self.pool)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_UPSERT_FAILED",
                    format!("failed to upsert reward market: {error}"),
                )
            })?;
        }
        Ok(())
    }

    async fn save_quote_plans(&self, plans: &[RewardQuotePlan]) -> Result<()> {
        for plan in plans {
            sqlx::query(
                r#"
                INSERT INTO reward_quote_plans (
                  condition_id,
                  score,
                  eligible,
                  reason,
                  quote_plan_json,
                  updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6)
                ON CONFLICT (condition_id) DO UPDATE
                SET score = EXCLUDED.score,
                    eligible = EXCLUDED.eligible,
                    reason = EXCLUDED.reason,
                    quote_plan_json = EXCLUDED.quote_plan_json,
                    updated_at = EXCLUDED.updated_at
                "#,
            )
            .bind(&plan.condition_id)
            .bind(plan.score)
            .bind(plan.eligible)
            .bind(&plan.reason)
            .bind(Json(plan.clone()))
            .bind(plan.updated_at)
            .execute(&self.pool)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_UPSERT_FAILED",
                    format!("failed to upsert reward quote plan: {error}"),
                )
            })?;
        }
        Ok(())
    }

    async fn replace_simulated_orders(
        &self,
        account_id: &str,
        orders: &[ManagedRewardOrder],
        trace_id: &str,
    ) -> Result<usize> {
        let now = OffsetDateTime::now_utc();
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin reward order transaction: {error}"),
            )
        })?;

        let cancelled = sqlx::query(
            r#"
            UPDATE reward_managed_orders
            SET status = 'cancelled',
                reason = 'replaced by latest rewards simulation',
                updated_at = $1,
                trace_id = $2
            WHERE account_id = $3
              AND status IN ('planned', 'open', 'exit_pending')
            "#,
        )
        .bind(now)
        .bind(trace_id)
        .bind(account_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!("failed to cancel stale reward orders: {error}"),
            )
        })?
        .rows_affected() as usize;

        for order in orders {
            insert_reward_order(&mut transaction, order, trace_id).await?;
        }

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit reward order transaction: {error}"),
            )
        })?;
        Ok(cancelled)
    }

    async fn cancel_open_orders(
        &self,
        account_id: Option<&str>,
        reason: &str,
        trace_id: &str,
    ) -> Result<usize> {
        let now = OffsetDateTime::now_utc();
        let result = sqlx::query(
            r#"
            UPDATE reward_managed_orders
            SET status = 'cancelled',
                reason = $1,
                updated_at = $2,
                trace_id = $3
            WHERE status IN ('planned', 'open', 'exit_pending')
              AND ($4::text IS NULL OR account_id = $4)
            "#,
        )
        .bind(reason)
        .bind(now)
        .bind(trace_id)
        .bind(account_id)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!("failed to cancel reward orders: {error}"),
            )
        })?;
        Ok(result.rows_affected() as usize)
    }

    async fn list_markets(&self, limit: u16) -> Result<Vec<RewardMarket>> {
        let rows = sqlx::query(
            r#"
            SELECT condition_id,
                   question,
                   market_slug,
                   event_slug,
                   image,
                   rewards_max_spread,
                   rewards_min_size,
                   total_daily_rate,
                   tokens_json,
                   active,
                   updated_at
            FROM reward_markets
            WHERE active = true
            ORDER BY total_daily_rate DESC, updated_at DESC
            LIMIT $1
            "#,
        )
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query reward markets: {error}"),
            )
        })?;

        rows.iter().map(reward_market_from_row).collect()
    }

    async fn list_quote_plans(&self, limit: u16) -> Result<Vec<RewardQuotePlan>> {
        let rows = sqlx::query(
            r#"
            SELECT quote_plan_json
            FROM reward_quote_plans
            ORDER BY eligible DESC, score DESC, updated_at DESC
            LIMIT $1
            "#,
        )
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query reward quote plans: {error}"),
            )
        })?;

        rows.iter()
            .map(|row| {
                let plan: Json<RewardQuotePlan> = row
                    .try_get("quote_plan_json")
                    .map_err(postgres_decode_error)?;
                Ok(plan.0)
            })
            .collect()
    }

    async fn list_orders(&self, limit: u16) -> Result<Vec<ManagedRewardOrder>> {
        let rows = sqlx::query(
            r#"
            SELECT id,
                   account_id,
                   condition_id,
                   token_id,
                   outcome,
                   side,
                   price,
                   size,
                   external_order_id,
                   status,
                   scoring,
                   reason,
                   created_at,
                   updated_at
            FROM reward_managed_orders
            ORDER BY updated_at DESC
            LIMIT $1
            "#,
        )
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query reward managed orders: {error}"),
            )
        })?;

        rows.iter().map(reward_order_from_row).collect()
    }

    async fn list_positions(&self, limit: u16) -> Result<Vec<RewardPosition>> {
        let rows = sqlx::query(
            r#"
            SELECT account_id,
                   condition_id,
                   token_id,
                   outcome,
                   size,
                   avg_price,
                   realized_pnl,
                   updated_at
            FROM reward_positions
            WHERE size <> 0
            ORDER BY updated_at DESC
            LIMIT $1
            "#,
        )
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query reward positions: {error}"),
            )
        })?;

        rows.iter().map(reward_position_from_row).collect()
    }

    async fn list_events(&self, limit: u16) -> Result<Vec<RewardRiskEvent>> {
        let rows = sqlx::query(
            r#"
            SELECT id,
                   account_id,
                   condition_id,
                   external_order_id,
                   event_type,
                   severity,
                   message,
                   metadata_json,
                   created_at
            FROM reward_risk_events
            ORDER BY created_at DESC
            LIMIT $1
            "#,
        )
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query reward risk events: {error}"),
            )
        })?;

        rows.iter().map(reward_event_from_row).collect()
    }

    async fn log_event(&self, event: RewardRiskEvent) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO reward_risk_events (
              id,
              account_id,
              condition_id,
              external_order_id,
              event_type,
              severity,
              message,
              metadata_json,
              created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(&event.id)
        .bind(&event.account_id)
        .bind(&event.condition_id)
        .bind(&event.external_order_id)
        .bind(&event.event_type)
        .bind(event.severity.as_str())
        .bind(&event.message)
        .bind(Json(event.metadata.clone()))
        .bind(event.created_at)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_INSERT_FAILED",
                format!("failed to insert reward risk event: {error}"),
            )
        })?;
        Ok(())
    }
}
