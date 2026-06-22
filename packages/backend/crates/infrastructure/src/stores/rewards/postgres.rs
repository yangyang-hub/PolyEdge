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

    async fn record_worker_heartbeat(
        &self,
        account_id: &str,
        observed_at: OffsetDateTime,
    ) -> Result<()> {
        postgres_record_reward_worker_heartbeat(&self.pool, account_id, observed_at).await
    }

    async fn latest_worker_heartbeat(
        &self,
        account_id: &str,
    ) -> Result<Option<OffsetDateTime>> {
        postgres_latest_reward_worker_heartbeat(&self.pool, account_id).await
    }

    async fn prune_history(&self, cutoff: OffsetDateTime) -> Result<RewardHistoryPruneReport> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin reward history prune transaction: {error}"),
            )
        })?;

        let terminal_orders_deleted = sqlx::query(
            r#"
            DELETE FROM reward_managed_orders
            WHERE updated_at < $1
              AND status IN ('cancelled', 'filled', 'error')
            "#,
        )
        .bind(cutoff)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_DELETE_FAILED",
                format!("failed to prune terminal reward orders: {error}"),
            )
        })?
        .rows_affected();

        let risk_events_deleted = sqlx::query(
            r#"
            DELETE FROM reward_risk_events
            WHERE created_at < $1
            "#,
        )
        .bind(cutoff)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_DELETE_FAILED",
                format!("failed to prune reward risk events: {error}"),
            )
        })?
        .rows_affected();

        let low_competition_observations_deleted = sqlx::query(
            r#"
            DELETE FROM reward_low_competition_observations
            WHERE observed_at < $1
            "#,
        )
        .bind(cutoff)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_DELETE_FAILED",
                format!("failed to prune reward low-competition observations: {error}"),
            )
        })?
        .rows_affected();

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit reward history prune transaction: {error}"),
            )
        })?;

        Ok(RewardHistoryPruneReport {
            terminal_orders_deleted,
            risk_events_deleted,
            low_competition_observations_deleted,
        })
    }

    async fn enqueue_control_command(&self, command: RewardControlCommand) -> Result<bool> {
        postgres_enqueue_reward_control_command(&self.pool, command).await
    }

    async fn claim_next_control_command(
        &self,
        trace_id: &str,
        now: OffsetDateTime,
    ) -> Result<Option<RewardControlCommand>> {
        postgres_claim_next_reward_control_command(&self.pool, trace_id, now).await
    }

    async fn complete_control_command(
        &self,
        command_id: &str,
        trace_id: &str,
        now: OffsetDateTime,
    ) -> Result<()> {
        postgres_complete_reward_control_command(&self.pool, command_id, trace_id, now).await
    }

    async fn fail_control_command(
        &self,
        command_id: &str,
        trace_id: &str,
        error: &str,
        now: OffsetDateTime,
    ) -> Result<()> {
        postgres_fail_reward_control_command(&self.pool, command_id, trace_id, error, now).await
    }

    async fn upsert_markets(&self, markets: &[RewardMarket]) -> Result<()> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin reward market transaction: {error}"),
            )
        })?;
        upsert_reward_markets_tx(&mut transaction, markets).await?;
        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit reward market transaction: {error}"),
            )
        })?;
        Ok(())
    }

    async fn save_quote_plans(&self, plans: &[RewardQuotePlan]) -> Result<()> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin reward quote plan transaction: {error}"),
            )
        })?;
        replace_reward_quote_plans_tx(&mut transaction, plans).await?;
        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit reward quote plan transaction: {error}"),
            )
        })?;
        Ok(())
    }

    async fn record_low_competition_observations(
        &self,
        observations: &[RewardLowCompetitionObservation],
    ) -> Result<()> {
        postgres_record_low_competition_observations(&self.pool, observations).await
    }

    async fn list_low_competition_observations(
        &self,
        account_id: &str,
        since: OffsetDateTime,
        limit: u16,
    ) -> Result<Vec<RewardLowCompetitionObservation>> {
        postgres_list_low_competition_observations(&self.pool, account_id, since, limit).await
    }

    async fn record_market_candle_sample(
        &self,
        sample: &RewardMarketCandleSample,
    ) -> Result<()> {
        postgres_record_reward_market_candle_sample(&self.pool, sample).await
    }

    async fn list_recent_market_candles(
        &self,
        condition_id: &str,
        interval_sec: i32,
        limit_per_token: u16,
    ) -> Result<Vec<RewardMarketCandle>> {
        postgres_list_recent_reward_market_candles(
            &self.pool,
            condition_id,
            interval_sec,
            limit_per_token,
        )
        .await
    }

    async fn latest_market_advisory(
        &self,
        request: &RewardAiAdvisoryRequest,
        now: OffsetDateTime,
    ) -> Result<Option<RewardMarketAdvisory>> {
        let row = sqlx::query(
            r#"
            SELECT condition_id,
                   provider,
                   request_format,
                   model,
                   input_hash,
                   suitability,
                   quote_mode,
                   exit_policy,
                   confidence,
                   reasons_json,
                   metrics_json,
                   created_at,
                   expires_at
            FROM reward_market_advisories
            WHERE condition_id = $1
              AND provider = $2
              AND request_format = $3
              AND model = $4
              AND input_hash = $5
              AND expires_at > $6
            ORDER BY expires_at DESC
            LIMIT 1
            "#,
        )
        .bind(&request.condition_id)
        .bind(request.provider.as_str())
        .bind(request.request_format.as_str())
        .bind(&request.model)
        .bind(&request.input_hash)
        .bind(now)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query reward market advisory: {error}"),
            )
        })?;

        row.as_ref().map(reward_market_advisory_from_row).transpose()
    }

    async fn save_market_advisory(&self, advisory: &RewardMarketAdvisory) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO reward_market_advisories (
              condition_id,
              provider,
              request_format,
              model,
              input_hash,
              suitability,
              quote_mode,
              exit_policy,
              confidence,
              reasons_json,
              metrics_json,
              created_at,
              expires_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#,
        )
        .bind(&advisory.condition_id)
        .bind(advisory.provider.as_str())
        .bind(advisory.request_format.as_str())
        .bind(&advisory.model)
        .bind(&advisory.input_hash)
        .bind(advisory.suitability.as_str())
        .bind(advisory.quote_mode.as_str())
        .bind(advisory.exit_policy.as_str())
        .bind(advisory.confidence)
        .bind(Json(json!(advisory.reasons)))
        .bind(Json(advisory.metrics.clone()))
        .bind(advisory.created_at)
        .bind(advisory.expires_at)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_INSERT_FAILED",
                format!("failed to insert reward market advisory: {error}"),
            )
        })?;
        Ok(())
    }

    async fn latest_market_info_risk(
        &self,
        request: &RewardInfoRiskAssessmentRequest,
        now: OffsetDateTime,
    ) -> Result<Option<RewardMarketInfoRisk>> {
        postgres_latest_market_info_risk(&self.pool, request, now).await
    }

    async fn latest_market_info_risks(
        &self,
        condition_ids: &[String],
        now: OffsetDateTime,
    ) -> Result<Vec<RewardMarketInfoRisk>> {
        postgres_latest_market_info_risks(&self.pool, condition_ids, now).await
    }

    async fn save_market_info_risk(&self, risk: &RewardMarketInfoRisk) -> Result<()> {
        postgres_save_market_info_risk(&self.pool, risk).await
    }

    async fn list_markets(&self, limit: u16) -> Result<Vec<RewardMarket>> {
        postgres_list_reward_markets(self, limit).await
    }

    async fn list_candidate_markets(
        &self,
        filter: &RewardCandidateFilter,
        safety_limit: u16,
    ) -> Result<Vec<RewardMarket>> {
        postgres_list_reward_candidate_markets(self, filter, safety_limit).await
    }

    async fn list_all_active_markets(&self) -> Result<Vec<RewardMarket>> {
        postgres_list_all_active_reward_markets(self).await
    }
    async fn active_market_summary(&self) -> Result<(usize, Option<OffsetDateTime>)> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) AS markets_tracked,
                   MAX(updated_at) AS last_scan_at
            FROM reward_markets
            WHERE active = true
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to summarize active reward markets: {error}"),
            )
        })?;

        let count: i64 = row.try_get("markets_tracked").map_err(postgres_decode_error)?;
        let markets_tracked = usize::try_from(count).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode reward market count: {error}"),
            )
        })?;
        let last_scan_at = row.try_get("last_scan_at").map_err(postgres_decode_error)?;

        Ok((markets_tracked, last_scan_at))
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

    async fn count_quote_plans(&self) -> Result<(usize, usize)> {
        postgres_count_quote_plans(&self.pool).await
    }

    async fn latest_quote_plan_updated_at(&self) -> Result<Option<OffsetDateTime>> {
        postgres_latest_quote_plan_updated_at(&self.pool).await
    }

    async fn list_quote_plans_page(
        &self,
        query: &RewardQuotePlanListQuery,
    ) -> Result<RewardQuotePlanPage> {
        postgres_list_quote_plans_page(&self.pool, query).await
    }

    async fn list_orders_page(&self, query: &RewardOrderListQuery) -> Result<RewardOrderPage> {
        postgres_list_reward_orders_page(&self.pool, query).await
    }

    async fn list_positions(&self, account_id: &str, limit: u16) -> Result<Vec<RewardPosition>> {
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
            WHERE account_id = $1 AND size <> 0
            ORDER BY updated_at DESC
            LIMIT $2
            "#,
        )
        .bind(account_id)
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

    async fn list_events(&self, account_id: &str, limit: u16) -> Result<Vec<RewardRiskEvent>> {
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
            WHERE account_id = $1
              AND event_type <> 'reward_bot_live_plan_built'
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(account_id)
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
            SELECT account_id, wallet_address, capital_usd, available_usd, external_buy_notional,
                   reserved_usd, realized_pnl,
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
                   strategy_bucket,
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

    async fn count_open_orders(&self, account_id: &str) -> Result<usize> {
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM reward_managed_orders
            WHERE account_id = $1
              AND status IN ('planned', 'open', 'exit_pending')
            "#,
        )
        .bind(account_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to count open reward orders: {error}"),
            )
        })?;
        Ok(count.max(0) as usize)
    }

    async fn count_external_open_orders(&self, account_id: &str) -> Result<usize> {
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM reward_managed_orders
            WHERE account_id = $1
              AND status IN ('planned', 'open', 'exit_pending')
              AND external_order_id IS NOT NULL
            "#,
        )
        .bind(account_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to count external open reward orders: {error}"),
            )
        })?;
        Ok(count.max(0) as usize)
    }

    async fn get_order_by_external_order_id(
        &self,
        external_order_id: &str,
    ) -> Result<Option<ManagedRewardOrder>> {
        let row = sqlx::query(
            r#"
            SELECT id, account_id, condition_id, token_id, outcome, side, price, size,
                   strategy_bucket,
                   external_order_id, status, scoring, reason, filled_size, reward_earned,
                   last_scored_at, created_at, updated_at
            FROM reward_managed_orders
            WHERE external_order_id = $1
            "#,
        )
        .bind(external_order_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query reward order by external id: {error}"),
            )
        })?;

        row.as_ref().map(reward_order_from_row).transpose()
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

    async fn count_account_positions(&self, account_id: &str) -> Result<usize> {
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM reward_positions
            WHERE account_id = $1 AND size <> 0
            "#,
        )
        .bind(account_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to count reward account positions: {error}"),
            )
        })?;
        Ok(count.max(0) as usize)
    }

    async fn list_fills(&self, account_id: &str, limit: u16) -> Result<Vec<RewardFill>> {
        let rows = sqlx::query(
            r#"
            SELECT id, order_id, account_id, condition_id, token_id, outcome, side,
                   price, size, notional_usd, role, realized_pnl, reason, trace_id, created_at
            FROM reward_fills
            WHERE account_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(account_id)
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

    async fn reward_fill_exists(&self, fill_id: &str) -> Result<bool> {
        let exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
              SELECT 1
              FROM reward_fills
              WHERE id = $1
            )
            "#,
        )
        .bind(fill_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query reward fill existence: {error}"),
            )
        })?;
        Ok(exists)
    }

    async fn latest_fill_at(&self, account_id: &str) -> Result<Option<OffsetDateTime>> {
        sqlx::query_scalar(
            r#"
            SELECT MAX(created_at)
            FROM reward_fills
            WHERE account_id = $1
            "#,
        )
        .bind(account_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query latest reward fill timestamp: {error}"),
            )
        })
    }

    async fn apply_tick_outcome(
        &self,
        outcome: &RewardTickOutcome,
        trace_id: &str,
    ) -> Result<()> {
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

    async fn apply_account_sync(
        &self,
        account: &RewardAccountState,
        positions: Option<&[RewardPosition]>,
        _trace_id: &str,
    ) -> Result<()> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin reward sync transaction: {error}"),
            )
        })?;

        upsert_reward_account_state_tx(&mut transaction, account).await?;
        if let Some(positions) = positions {
            sqlx::query("DELETE FROM reward_positions WHERE account_id = $1")
                .bind(&account.account_id)
                .execute(&mut *transaction)
                .await
                .map_err(|error| {
                    db_error(
                        "POSTGRES_DELETE_FAILED",
                        format!("failed to replace externally synced reward positions: {error}"),
                    )
                })?;
            for position in positions {
                upsert_reward_position_tx(&mut transaction, position).await?;
            }
        }

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit reward sync transaction: {error}"),
            )
        })?;
        Ok(())
    }

    async fn reset_state(&self, config: &RewardBotConfig, _trace_id: &str) -> Result<()> {
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
            "DELETE FROM reward_risk_events WHERE account_id = $1",
        ] {
            sqlx::query(statement)
                .bind(&config.account_id)
                .execute(&mut *transaction)
                .await
                .map_err(|error| {
                    db_error(
                        "POSTGRES_DELETE_FAILED",
                        format!("failed to reset reward state: {error}"),
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
