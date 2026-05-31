// Postgres-backed `RewardBotStore` implementation. The `RewardBotStore` trait impl is a
// single indivisible block; row mappers and SQL helpers it calls live in the `stores` module.

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

    async fn list_all_active_markets(&self) -> Result<Vec<RewardMarket>> {
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
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query all reward markets: {error}"),
            )
        })?;

        rows.iter().map(reward_market_from_row).collect()
    }

    async fn list_all_quote_plans(&self) -> Result<Vec<RewardQuotePlan>> {
        let rows = sqlx::query(
            r#"
            SELECT quote_plan_json
            FROM reward_quote_plans
            ORDER BY eligible DESC, score DESC, updated_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query all reward quote plans: {error}"),
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
                   filled_size,
                   reward_earned,
                   last_scored_at,
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

    async fn load_account_state(&self, config: &RewardBotConfig) -> Result<RewardAccountState> {
        let existing = sqlx::query(
            r#"
            SELECT account_id, capital_usd, available_usd, reserved_usd, realized_pnl,
                   reward_earned_usd, fees_paid, tick_index, updated_at
            FROM reward_account_state
            WHERE account_id = $1
            "#,
        )
        .bind(&config.account_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query reward account state: {error}"),
            )
        })?;

        if let Some(row) = existing {
            return reward_account_state_from_row(&row);
        }

        let state = RewardAccountState::fresh(
            &config.account_id,
            config.account_capital_usd,
            OffsetDateTime::now_utc(),
        );
        upsert_reward_account_state(&self.pool, &state).await?;
        Ok(state)
    }

    async fn list_open_orders(&self, account_id: &str) -> Result<Vec<ManagedRewardOrder>> {
        let rows = sqlx::query(
            r#"
            SELECT id, account_id, condition_id, token_id, outcome, side, price, size,
                   external_order_id, status, scoring, reason, filled_size, reward_earned,
                   last_scored_at, created_at, updated_at
            FROM reward_managed_orders
            WHERE account_id = $1
              AND status IN ('planned', 'open', 'exit_pending')
            ORDER BY updated_at DESC
            "#,
        )
        .bind(account_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query open reward orders: {error}"),
            )
        })?;
        rows.iter().map(reward_order_from_row).collect()
    }

    async fn list_account_positions(&self, account_id: &str) -> Result<Vec<RewardPosition>> {
        let rows = sqlx::query(
            r#"
            SELECT account_id, condition_id, token_id, outcome, size, avg_price,
                   realized_pnl, updated_at
            FROM reward_positions
            WHERE account_id = $1 AND size <> 0
            "#,
        )
        .bind(account_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query reward account positions: {error}"),
            )
        })?;
        rows.iter().map(reward_position_from_row).collect()
    }

    async fn list_fills(&self, limit: u16) -> Result<Vec<RewardFill>> {
        let rows = sqlx::query(
            r#"
            SELECT id, order_id, account_id, condition_id, token_id, outcome, side,
                   price, size, notional_usd, role, realized_pnl, reason, trace_id, created_at
            FROM reward_fills
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
                format!("failed to query reward fills: {error}"),
            )
        })?;
        rows.iter().map(reward_fill_from_row).collect()
    }

    async fn apply_simulation_tick(
        &self,
        outcome: &RewardSimulationOutcome,
        trace_id: &str,
    ) -> Result<()> {
        self.upsert_markets(&outcome.markets).await?;
        self.save_quote_plans(&outcome.plans).await?;

        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin reward tick transaction: {error}"),
            )
        })?;

        for order in &outcome.orders {
            insert_reward_order(&mut transaction, order, trace_id).await?;
        }
        for fill in &outcome.fills {
            insert_reward_fill(&mut transaction, fill).await?;
        }
        for position in &outcome.positions {
            upsert_reward_position_tx(&mut transaction, position).await?;
        }
        upsert_reward_account_state_tx(&mut transaction, &outcome.account).await?;
        for event in &outcome.events {
            insert_reward_event_tx(&mut transaction, event).await?;
        }

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit reward tick transaction: {error}"),
            )
        })?;
        Ok(())
    }

    async fn reset_simulation(&self, config: &RewardBotConfig, _trace_id: &str) -> Result<()> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin reward reset transaction: {error}"),
            )
        })?;

        for statement in [
            "DELETE FROM reward_managed_orders WHERE account_id = $1",
            "DELETE FROM reward_positions WHERE account_id = $1",
            "DELETE FROM reward_fills WHERE account_id = $1",
        ] {
            sqlx::query(statement)
                .bind(&config.account_id)
                .execute(&mut *transaction)
                .await
                .map_err(|error| {
                    db_error(
                        "POSTGRES_DELETE_FAILED",
                        format!("failed to reset reward simulation: {error}"),
                    )
                })?;
        }

        let fresh = RewardAccountState::fresh(
            &config.account_id,
            config.account_capital_usd,
            OffsetDateTime::now_utc(),
        );
        upsert_reward_account_state_tx(&mut transaction, &fresh).await?;

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit reward reset transaction: {error}"),
            )
        })?;
        Ok(())
    }
}
