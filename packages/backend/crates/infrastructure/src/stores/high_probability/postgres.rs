pub struct PostgresHighProbabilityStore {
    pool: PgPool,
}

impl PostgresHighProbabilityStore {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl HighProbabilityStore for PostgresHighProbabilityStore {
    async fn load_config(&self) -> Result<HighProbabilityConfig> {
        let rows = sqlx::query("SELECT key, value FROM high_probability_config")
            .fetch_all(&self.pool)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_QUERY_FAILED",
                    format!("failed to query high probability config: {error}"),
                )
            })?;

        let mut config = HighProbabilityConfig::default();
        for row in rows {
            let key: String = row.try_get("key").map_err(postgres_decode_error)?;
            let value: String = row.try_get("value").map_err(postgres_decode_error)?;
            apply_high_probability_config_value(&mut config, &key, &value)?;
        }
        Ok(config.normalized())
    }

    async fn save_config(&self, config: &HighProbabilityConfig) -> Result<()> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin high probability config transaction: {error}"),
            )
        })?;

        for (key, value) in high_probability_config_entries(config) {
            sqlx::query(
                r#"
                INSERT INTO high_probability_config (key, value, updated_at)
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
                    format!("failed to upsert high probability config: {error}"),
                )
            })?;
        }

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit high probability config transaction: {error}"),
            )
        })?;
        Ok(())
    }

    async fn record_samples(&self, samples: &[HighProbabilitySample]) -> Result<usize> {
        let mut inserted = 0usize;
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin high probability samples transaction: {error}"),
            )
        })?;

        for sample in samples {
            let sample = sample.clone().normalized();
            let result = sqlx::query(
                r#"
                INSERT INTO high_probability_samples (
                    condition_id, token_id, side, sampled_at, trigger_kind,
                    executable_price, price_bucket, market_type, time_to_resolution_bucket,
                    liquidity_bucket, spread_bucket, path_features, risk_tags, outcome,
                    settlement_pnl, max_drawdown_cents, hold_seconds, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
                ON CONFLICT (condition_id, token_id, sampled_at, trigger_kind, price_bucket) DO NOTHING
                "#,
            )
            .bind(&sample.condition_id)
            .bind(&sample.token_id)
            .bind(&sample.side)
            .bind(sample.sampled_at)
            .bind(sample.trigger_kind.as_str())
            .bind(sample.executable_price)
            .bind(&sample.price_bucket)
            .bind(&sample.market_type)
            .bind(&sample.time_to_resolution_bucket)
            .bind(&sample.liquidity_bucket)
            .bind(&sample.spread_bucket)
            .bind(Json(sample.path_features))
            .bind(Json(sample.risk_tags))
            .bind(sample.outcome.as_str())
            .bind(sample.settlement_pnl)
            .bind(sample.max_drawdown_cents)
            .bind(sample.hold_seconds)
            .bind(sample.created_at)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_INSERT_FAILED",
                    format!("failed to insert high probability sample: {error}"),
                )
            })?;
            inserted += usize::try_from(result.rows_affected()).unwrap_or(0);
        }

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit high probability samples transaction: {error}"),
            )
        })?;
        Ok(inserted)
    }

    async fn upsert_market_outcome(&self, outcome: &HighProbabilityMarketOutcome) -> Result<()> {
        let outcome = outcome.clone().normalized();
        sqlx::query(
            r#"
            INSERT INTO high_probability_market_outcomes (
                condition_id, status, winning_token_id, resolved_at, market_type,
                risk_tags, label_source, raw, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (condition_id) DO UPDATE
            SET status = EXCLUDED.status,
                winning_token_id = EXCLUDED.winning_token_id,
                resolved_at = EXCLUDED.resolved_at,
                market_type = EXCLUDED.market_type,
                risk_tags = EXCLUDED.risk_tags,
                label_source = EXCLUDED.label_source,
                raw = EXCLUDED.raw,
                updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(&outcome.condition_id)
        .bind(outcome.status.as_str())
        .bind(&outcome.winning_token_id)
        .bind(outcome.resolved_at)
        .bind(&outcome.market_type)
        .bind(Json(outcome.risk_tags))
        .bind(&outcome.label_source)
        .bind(Json(outcome.raw))
        .bind(outcome.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!("failed to upsert high probability market outcome: {error}"),
            )
        })?;
        Ok(())
    }

    async fn list_reward_candle_sample_inputs(
        &self,
        limit: u32,
    ) -> Result<Vec<HighProbabilityRewardCandleSampleInput>> {
        let rows = sqlx::query(
            r#"
            SELECT c.condition_id,
                   c.token_id,
                   c.outcome,
                   c.bucket_start,
                   c.close,
                   c.spread_cents_close,
                   COALESCE(NULLIF(o.market_type, ''), NULLIF(m.category, ''), 'unknown') AS market_type,
                   m.liquidity_usd,
                   o.resolved_at,
                   o.status AS outcome_status,
                   o.winning_token_id,
                   o.risk_tags
            FROM reward_market_candles c
            INNER JOIN high_probability_market_outcomes o
                ON o.condition_id = c.condition_id
            LEFT JOIN markets m
                ON m.polymarket_condition_id = c.condition_id
            WHERE o.status IN ('resolved', 'voided', 'ambiguous')
              AND c.interval_sec = 300
            ORDER BY c.condition_id, c.token_id, c.bucket_start
            LIMIT $1
            "#,
        )
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query high probability reward candle inputs: {error}"),
            )
        })?;

        rows.iter()
            .map(high_probability_reward_candle_sample_input_from_row)
            .collect()
    }

    async fn list_observe_candidates(
        &self,
        limit: u16,
    ) -> Result<Vec<HighProbabilityObserveCandidate>> {
        let rows = sqlx::query(
            r#"
            WITH latest AS (
                SELECT DISTINCT ON (c.token_id)
                       c.condition_id,
                       c.token_id,
                       c.outcome,
                       c.close_observed_at AS observed_at,
                       c.close AS reference_price,
                       c.spread_cents_close AS reference_spread_cents,
                       COALESCE(NULLIF(o.market_type, ''), NULLIF(m.category, ''), 'unknown') AS market_type,
                       m.liquidity_usd,
                       m.end_at,
                       COALESCE(o.risk_tags, '[]'::jsonb) AS risk_tags
                FROM reward_market_candles c
                INNER JOIN reward_markets rm
                    ON rm.condition_id = c.condition_id
                   AND rm.active = TRUE
                LEFT JOIN high_probability_market_outcomes o
                    ON o.condition_id = c.condition_id
                LEFT JOIN markets m
                    ON m.polymarket_condition_id = c.condition_id
                WHERE c.interval_sec = 300
                  AND (o.status IS NULL OR o.status = 'unresolved')
                ORDER BY c.token_id, c.bucket_start DESC
            )
            SELECT condition_id, token_id, outcome, observed_at, reference_price,
                   reference_spread_cents, market_type, liquidity_usd, end_at, risk_tags
            FROM latest
            ORDER BY observed_at DESC
            LIMIT $1
            "#,
        )
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query high probability observe candidates: {error}"),
            )
        })?;

        rows.iter()
            .map(high_probability_observe_candidate_from_row)
            .collect()
    }

    async fn list_samples(
        &self,
        query: HighProbabilitySampleQuery,
    ) -> Result<Vec<HighProbabilitySample>> {
        let outcome = query.outcome.map(|outcome| outcome.as_str().to_string());
        let rows = sqlx::query(
            r#"
            SELECT id, condition_id, token_id, side, sampled_at, trigger_kind,
                   executable_price, price_bucket, market_type, time_to_resolution_bucket,
                   liquidity_bucket, spread_bucket, path_features, risk_tags, outcome,
                   settlement_pnl, max_drawdown_cents, hold_seconds, created_at
            FROM high_probability_samples
            WHERE ($1::text IS NULL OR outcome = $1)
              AND ($2::text IS NULL OR market_type = $2)
            ORDER BY sampled_at DESC
            LIMIT $3
            "#,
        )
        .bind(outcome)
        .bind(query.market_type)
        .bind(i64::from(query.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query high probability samples: {error}"),
            )
        })?;

        rows.iter().map(high_probability_sample_from_row).collect()
    }

    async fn replace_bucket_stats(
        &self,
        model_version: &str,
        stats: &[HighProbabilityBucketStats],
    ) -> Result<usize> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin high probability bucket transaction: {error}"),
            )
        })?;

        sqlx::query("DELETE FROM high_probability_bucket_stats WHERE model_version = $1")
            .bind(model_version)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_DELETE_FAILED",
                    format!("failed to delete old high probability bucket stats: {error}"),
                )
            })?;

        for stats in stats {
            sqlx::query(
                r#"
                INSERT INTO high_probability_bucket_stats (
                    model_version, bucket_key, bucket_dimensions, sample_count, win_count,
                    win_rate, fair_probability, confidence_low, confidence_high, expected_pnl,
                    avg_max_drawdown_cents, break_70_rate, break_60_rate, break_50_rate,
                    avg_hold_seconds, recommended_max_entry_price, computed_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
                "#,
            )
            .bind(&stats.model_version)
            .bind(&stats.bucket_key)
            .bind(Json(stats.bucket_dimensions.clone()))
            .bind(i64::try_from(stats.sample_count).unwrap_or(i64::MAX))
            .bind(i64::try_from(stats.win_count).unwrap_or(i64::MAX))
            .bind(stats.win_rate)
            .bind(stats.fair_probability)
            .bind(stats.confidence_low)
            .bind(stats.confidence_high)
            .bind(stats.expected_pnl)
            .bind(stats.avg_max_drawdown_cents)
            .bind(stats.break_70_rate)
            .bind(stats.break_60_rate)
            .bind(stats.break_50_rate)
            .bind(stats.avg_hold_seconds)
            .bind(stats.recommended_max_entry_price)
            .bind(stats.computed_at)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_INSERT_FAILED",
                    format!("failed to insert high probability bucket stats: {error}"),
                )
            })?;
        }

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit high probability bucket transaction: {error}"),
            )
        })?;
        Ok(stats.len())
    }

    async fn list_bucket_stats(
        &self,
        model_version: Option<&str>,
        limit: u16,
    ) -> Result<Vec<HighProbabilityBucketStats>> {
        let rows = sqlx::query(
            r#"
            SELECT id, model_version, bucket_key, bucket_dimensions, sample_count, win_count,
                   win_rate, fair_probability, confidence_low, confidence_high, expected_pnl,
                   avg_max_drawdown_cents, break_70_rate, break_60_rate, break_50_rate,
                   avg_hold_seconds, recommended_max_entry_price, computed_at
            FROM high_probability_bucket_stats
            WHERE ($1::text IS NULL OR model_version = $1)
            ORDER BY sample_count DESC, computed_at DESC
            LIMIT $2
            "#,
        )
        .bind(model_version)
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query high probability bucket stats: {error}"),
            )
        })?;

        rows.iter()
            .map(high_probability_bucket_stats_from_row)
            .collect()
    }

    async fn record_backtest_result(
        &self,
        result: &HighProbabilityBacktestResult,
    ) -> Result<HighProbabilityBacktestPersistReport> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin high probability backtest transaction: {error}"),
            )
        })?;
        let report = &result.run.report;
        let config_json = serde_json::to_value(&result.config).unwrap_or_else(|_| json!({}));
        let row = sqlx::query(
            r#"
            INSERT INTO high_probability_backtest_runs (
                run_at, model_version, market_scope, sample_limit, train_sample_count,
                test_sample_count, candidate_count, trade_count, skipped_no_bucket_count,
                skipped_no_edge_count, win_trades, loss_trades, win_rate, total_pnl,
                average_pnl, total_entry_cost, roi, max_drawdown, average_entry_price,
                train_start_at, train_end_at, test_start_at, test_end_at, exit_rule_reports,
                notes, config_json, created_at
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15,
                $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, now()
            )
            RETURNING id
            "#,
        )
        .bind(result.run.run_at)
        .bind(&report.model_version)
        .bind(&report.market_scope)
        .bind(i64::from(report.sample_limit))
        .bind(i64::try_from(report.train_sample_count).unwrap_or(i64::MAX))
        .bind(i64::try_from(report.test_sample_count).unwrap_or(i64::MAX))
        .bind(i64::try_from(report.candidate_count).unwrap_or(i64::MAX))
        .bind(i64::try_from(report.trade_count).unwrap_or(i64::MAX))
        .bind(i64::try_from(report.skipped_no_bucket_count).unwrap_or(i64::MAX))
        .bind(i64::try_from(report.skipped_no_edge_count).unwrap_or(i64::MAX))
        .bind(i64::try_from(report.win_trades).unwrap_or(i64::MAX))
        .bind(i64::try_from(report.loss_trades).unwrap_or(i64::MAX))
        .bind(report.win_rate)
        .bind(report.total_pnl)
        .bind(report.average_pnl)
        .bind(report.total_entry_cost)
        .bind(report.roi)
        .bind(report.max_drawdown)
        .bind(report.average_entry_price)
        .bind(report.train_start_at)
        .bind(report.train_end_at)
        .bind(report.test_start_at)
        .bind(report.test_end_at)
        .bind(Json(report.exit_rule_reports.clone()))
        .bind(Json(report.notes.clone()))
        .bind(Json(config_json))
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_INSERT_FAILED",
                format!("failed to insert high probability backtest run: {error}"),
            )
        })?;
        let run_id: i64 = row.try_get("id").map_err(postgres_decode_error)?;
        let mut trades_saved = 0usize;

        for trade in &result.trades {
            sqlx::query(
                r#"
                INSERT INTO high_probability_backtest_trades (
                    run_id, sample_id, condition_id, token_id, sampled_at, bucket_key,
                    executable_price, fair_probability, net_edge, recommended_max_entry_price,
                    outcome, settlement_pnl, cumulative_pnl, drawdown, reasons, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
                "#,
            )
            .bind(run_id)
            .bind(trade.sample_id)
            .bind(&trade.condition_id)
            .bind(&trade.token_id)
            .bind(trade.sampled_at)
            .bind(&trade.bucket_key)
            .bind(trade.executable_price)
            .bind(trade.fair_probability)
            .bind(trade.net_edge)
            .bind(trade.recommended_max_entry_price)
            .bind(trade.outcome.as_str())
            .bind(trade.settlement_pnl)
            .bind(trade.cumulative_pnl)
            .bind(trade.drawdown)
            .bind(Json(trade.reasons.clone()))
            .bind(trade.created_at)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_INSERT_FAILED",
                    format!("failed to insert high probability backtest trade: {error}"),
                )
            })?;
            trades_saved += 1;
        }

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit high probability backtest transaction: {error}"),
            )
        })?;
        Ok(HighProbabilityBacktestPersistReport {
            run_id,
            trades_saved,
        })
    }

    async fn list_backtest_runs(&self, limit: u16) -> Result<Vec<HighProbabilityBacktestRun>> {
        let rows = sqlx::query(
            r#"
            SELECT id, run_at, model_version, market_scope, sample_limit,
                   train_sample_count, test_sample_count, candidate_count, trade_count,
                   skipped_no_bucket_count, skipped_no_edge_count, win_trades, loss_trades,
                   win_rate, total_pnl, average_pnl, total_entry_cost, roi, max_drawdown,
                   average_entry_price, train_start_at, train_end_at, test_start_at,
                   test_end_at, exit_rule_reports, notes
            FROM high_probability_backtest_runs
            ORDER BY run_at DESC, id DESC
            LIMIT $1
            "#,
        )
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query high probability backtest runs: {error}"),
            )
        })?;

        rows.iter()
            .map(high_probability_backtest_run_from_row)
            .collect()
    }

    async fn list_backtest_trades(
        &self,
        run_id: i64,
        limit: u16,
    ) -> Result<Vec<HighProbabilityBacktestTrade>> {
        let rows = sqlx::query(
            r#"
            SELECT id, run_id, sample_id, condition_id, token_id, sampled_at, bucket_key,
                   executable_price, fair_probability, net_edge, recommended_max_entry_price,
                   outcome, settlement_pnl, cumulative_pnl, drawdown, reasons, created_at
            FROM high_probability_backtest_trades
            WHERE run_id = $1
            ORDER BY sampled_at, id
            LIMIT $2
            "#,
        )
        .bind(run_id)
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query high probability backtest trades: {error}"),
            )
        })?;

        rows.iter()
            .map(high_probability_backtest_trade_from_row)
            .collect()
    }

    async fn record_observation(&self, observation: &HighProbabilityObservation) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO high_probability_observations (
                observed_at, condition_id, token_id, mode, executable_price, fair_probability,
                net_edge, recommended_size_usd, decision, reasons, model_version, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            "#,
        )
        .bind(observation.observed_at)
        .bind(&observation.condition_id)
        .bind(&observation.token_id)
        .bind(observation.mode.as_str())
        .bind(observation.executable_price)
        .bind(observation.fair_probability)
        .bind(observation.net_edge)
        .bind(observation.recommended_size_usd)
        .bind(observation.decision.as_str())
        .bind(Json(observation.reasons.clone()))
        .bind(&observation.model_version)
        .bind(observation.created_at)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_INSERT_FAILED",
                format!("failed to insert high probability observation: {error}"),
            )
        })?;
        Ok(())
    }

    async fn list_observations(&self, limit: u16) -> Result<Vec<HighProbabilityObservation>> {
        let rows = sqlx::query(
            r#"
            SELECT id, observed_at, condition_id, token_id, mode, executable_price,
                   fair_probability, net_edge, recommended_size_usd, decision, reasons,
                   model_version, created_at
            FROM high_probability_observations
            ORDER BY observed_at DESC
            LIMIT $1
            "#,
        )
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query high probability observations: {error}"),
            )
        })?;

        rows.iter()
            .map(high_probability_observation_from_row)
            .collect()
    }
}
