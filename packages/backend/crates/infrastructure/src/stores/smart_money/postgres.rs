pub struct PostgresSmartMoneyStore {
    pool: PgPool,
}

impl PostgresSmartMoneyStore {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SmartMoneyStore for PostgresSmartMoneyStore {
    async fn load_config(&self) -> Result<SmartMoneyConfig> {
        let rows = sqlx::query("SELECT key, value FROM smart_money_config")
            .fetch_all(&self.pool)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_QUERY_FAILED",
                    format!("failed to query smart money config: {error}"),
                )
            })?;

        let mut config = SmartMoneyConfig::default();
        for row in rows {
            let key: String = row.try_get("key").map_err(postgres_decode_error)?;
            let value: String = row.try_get("value").map_err(postgres_decode_error)?;
            apply_smart_money_config_value(&mut config, &key, &value)?;
        }
        Ok(config.normalized())
    }

    async fn save_config(&self, config: &SmartMoneyConfig) -> Result<()> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin smart money config transaction: {error}"),
            )
        })?;

        for (key, value) in smart_money_config_entries(&config.clone().normalized()) {
            sqlx::query(
                r#"
                INSERT INTO smart_money_config (key, value, updated_at)
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
                    format!("failed to upsert smart money config: {error}"),
                )
            })?;
        }

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit smart money config transaction: {error}"),
            )
        })?;
        Ok(())
    }

    async fn upsert_candidate(&self, candidate: &SmartWalletCandidate) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO smart_wallet_candidates (
                wallet_address, source, status, first_seen_at, last_seen_at,
                last_analyzed_at, promoted_at, rejected_at, reason, raw
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (wallet_address, source) DO UPDATE
            SET last_seen_at = EXCLUDED.last_seen_at,
                last_analyzed_at = COALESCE(EXCLUDED.last_analyzed_at, smart_wallet_candidates.last_analyzed_at),
                reason = EXCLUDED.reason,
                raw = EXCLUDED.raw
            "#,
        )
        .bind(&candidate.wallet_address)
        .bind(&candidate.source)
        .bind(candidate.status.as_str())
        .bind(candidate.first_seen_at)
        .bind(candidate.last_seen_at)
        .bind(candidate.last_analyzed_at)
        .bind(candidate.promoted_at)
        .bind(candidate.rejected_at)
        .bind(&candidate.reason)
        .bind(Json(candidate.raw.clone()))
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!("failed to upsert smart wallet candidate: {error}"),
            )
        })?;
        Ok(())
    }

    async fn update_candidate_status(
        &self,
        wallet_address: &str,
        source: Option<&str>,
        status: SmartWalletCandidateStatus,
        reason: Option<&str>,
        now: OffsetDateTime,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE smart_wallet_candidates
            SET status = $3,
                last_seen_at = $5,
                promoted_at = CASE
                    WHEN $3 IN ('watch', 'tracked') THEN $5
                    ELSE promoted_at
                END,
                rejected_at = CASE
                    WHEN $3 IN ('blocked', 'rejected') THEN $5
                    ELSE rejected_at
                END,
                reason = COALESCE($4, reason)
            WHERE wallet_address = $1
              AND ($2::text IS NULL OR source = $2)
            "#,
        )
        .bind(wallet_address)
        .bind(source)
        .bind(status.as_str())
        .bind(reason)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!("failed to update smart wallet candidate status: {error}"),
            )
        })?;
        Ok(result.rows_affected())
    }

    async fn list_candidates(
        &self,
        status: Option<SmartWalletCandidateStatus>,
        limit: u16,
    ) -> Result<Vec<SmartWalletCandidate>> {
        let status = status.map(|status| status.as_str().to_string());
        let rows = sqlx::query(
            r#"
            SELECT id, wallet_address, source, status, first_seen_at, last_seen_at,
                   last_analyzed_at, promoted_at, rejected_at, reason, raw
            FROM smart_wallet_candidates
            WHERE ($1::text IS NULL OR status = $1)
            ORDER BY last_seen_at DESC
            LIMIT $2
            "#,
        )
        .bind(status)
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query smart wallet candidates: {error}"),
            )
        })?;

        rows.iter().map(smart_wallet_candidate_from_row).collect()
    }

    async fn upsert_profile(&self, profile: &SmartWalletProfile) -> Result<()> {
        let profile = profile.clone().normalized();
        sqlx::query(
            r#"
            INSERT INTO smart_wallet_profiles (
                wallet_address, trade_count, settled_trade_count, total_volume_usd,
                realized_pnl_usd, roi, win_rate, max_drawdown_usd, avg_trade_usd,
                median_trade_usd, avg_hold_secs, active_days, markets_traded,
                category_concentration_score, market_concentration_score,
                low_liquidity_trade_ratio, stale_copy_window_ratio, last_trade_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)
            ON CONFLICT (wallet_address) DO UPDATE
            SET trade_count = EXCLUDED.trade_count,
                settled_trade_count = EXCLUDED.settled_trade_count,
                total_volume_usd = EXCLUDED.total_volume_usd,
                realized_pnl_usd = EXCLUDED.realized_pnl_usd,
                roi = EXCLUDED.roi,
                win_rate = EXCLUDED.win_rate,
                max_drawdown_usd = EXCLUDED.max_drawdown_usd,
                avg_trade_usd = EXCLUDED.avg_trade_usd,
                median_trade_usd = EXCLUDED.median_trade_usd,
                avg_hold_secs = EXCLUDED.avg_hold_secs,
                active_days = EXCLUDED.active_days,
                markets_traded = EXCLUDED.markets_traded,
                category_concentration_score = EXCLUDED.category_concentration_score,
                market_concentration_score = EXCLUDED.market_concentration_score,
                low_liquidity_trade_ratio = EXCLUDED.low_liquidity_trade_ratio,
                stale_copy_window_ratio = EXCLUDED.stale_copy_window_ratio,
                last_trade_at = EXCLUDED.last_trade_at,
                updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(&profile.wallet_address)
        .bind(profile.trade_count)
        .bind(profile.settled_trade_count)
        .bind(profile.total_volume_usd)
        .bind(profile.realized_pnl_usd)
        .bind(profile.roi)
        .bind(profile.win_rate)
        .bind(profile.max_drawdown_usd)
        .bind(profile.avg_trade_usd)
        .bind(profile.median_trade_usd)
        .bind(profile.avg_hold_secs)
        .bind(profile.active_days)
        .bind(profile.markets_traded)
        .bind(profile.category_concentration_score)
        .bind(profile.market_concentration_score)
        .bind(profile.low_liquidity_trade_ratio)
        .bind(profile.stale_copy_window_ratio)
        .bind(profile.last_trade_at)
        .bind(profile.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!("failed to upsert smart wallet profile: {error}"),
            )
        })?;
        Ok(())
    }

    async fn list_profiles(&self, limit: u16) -> Result<Vec<SmartWalletProfile>> {
        let rows = sqlx::query(
            r#"
            SELECT wallet_address, trade_count, settled_trade_count, total_volume_usd,
                   realized_pnl_usd, roi, win_rate, max_drawdown_usd, avg_trade_usd,
                   median_trade_usd, avg_hold_secs, active_days, markets_traded,
                   category_concentration_score, market_concentration_score,
                   low_liquidity_trade_ratio, stale_copy_window_ratio, last_trade_at, updated_at
            FROM smart_wallet_profiles
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
                format!("failed to query smart wallet profiles: {error}"),
            )
        })?;
        rows.iter().map(smart_wallet_profile_from_row).collect()
    }

    async fn upsert_score(&self, score: &SmartWalletScore) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO smart_wallet_scores (
                wallet_address, total_score, profit_score, consistency_score,
                risk_score, liquidity_score, recency_score, copyability_score,
                tier, explanation, scoring_version, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (wallet_address) DO UPDATE
            SET total_score = EXCLUDED.total_score,
                profit_score = EXCLUDED.profit_score,
                consistency_score = EXCLUDED.consistency_score,
                risk_score = EXCLUDED.risk_score,
                liquidity_score = EXCLUDED.liquidity_score,
                recency_score = EXCLUDED.recency_score,
                copyability_score = EXCLUDED.copyability_score,
                tier = EXCLUDED.tier,
                explanation = EXCLUDED.explanation,
                scoring_version = EXCLUDED.scoring_version,
                updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(&score.wallet_address)
        .bind(score.total_score)
        .bind(score.profit_score)
        .bind(score.consistency_score)
        .bind(score.risk_score)
        .bind(score.liquidity_score)
        .bind(score.recency_score)
        .bind(score.copyability_score)
        .bind(score.tier.as_str())
        .bind(Json(score.explanation.clone()))
        .bind(&score.scoring_version)
        .bind(score.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!("failed to upsert smart wallet score: {error}"),
            )
        })?;
        Ok(())
    }

    async fn list_scores(
        &self,
        tier: Option<SmartWalletTier>,
        limit: u16,
    ) -> Result<Vec<SmartWalletScore>> {
        let tier = tier.map(|tier| tier.as_str().to_string());
        let rows = sqlx::query(
            r#"
            SELECT wallet_address, total_score, profit_score, consistency_score,
                   risk_score, liquidity_score, recency_score, copyability_score,
                   tier, explanation, scoring_version, updated_at
            FROM smart_wallet_scores
            WHERE ($1::text IS NULL OR tier = $1)
            ORDER BY total_score DESC
            LIMIT $2
            "#,
        )
        .bind(tier)
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query smart wallet scores: {error}"),
            )
        })?;
        rows.iter().map(smart_wallet_score_from_row).collect()
    }

    async fn record_trades(&self, trades: &[SmartWalletTrade]) -> Result<usize> {
        let mut inserted = 0usize;
        for trade in trades {
            let result = sqlx::query(
                r#"
                INSERT INTO smart_wallet_trades (
                    id, wallet_address, source, condition_id, token_id, side, outcome,
                    price, size, notional_usd, tx_hash, source_timestamp, discovered_at, raw
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                ON CONFLICT (id) DO NOTHING
                "#,
            )
            .bind(&trade.id)
            .bind(&trade.wallet_address)
            .bind(&trade.source)
            .bind(&trade.condition_id)
            .bind(&trade.token_id)
            .bind(trade.side.as_str())
            .bind(&trade.outcome)
            .bind(trade.price)
            .bind(trade.size)
            .bind(trade.notional_usd)
            .bind(&trade.tx_hash)
            .bind(trade.source_timestamp)
            .bind(trade.discovered_at)
            .bind(Json(trade.raw.clone()))
            .execute(&self.pool)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_INSERT_FAILED",
                    format!("failed to insert smart wallet trade: {error}"),
                )
            })?;
            if result.rows_affected() > 0 {
                inserted += 1;
            }
        }
        Ok(inserted)
    }

    async fn list_trades(&self, limit: u16) -> Result<Vec<SmartWalletTrade>> {
        let rows = sqlx::query(
            r#"
            SELECT id, wallet_address, source, condition_id, token_id, side, outcome,
                   price, size, notional_usd, tx_hash, source_timestamp, discovered_at, raw
            FROM smart_wallet_trades
            ORDER BY source_timestamp DESC
            LIMIT $1
            "#,
        )
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query smart wallet trades: {error}"),
            )
        })?;
        rows.iter().map(smart_wallet_trade_from_row).collect()
    }

    async fn list_unprocessed_signal_trades(&self, limit: u16) -> Result<Vec<SmartWalletTrade>> {
        let rows = sqlx::query(
            r#"
            SELECT t.id, t.wallet_address, t.source, t.condition_id, t.token_id, t.side,
                   t.outcome, t.price, t.size, t.notional_usd, t.tx_hash,
                   t.source_timestamp, t.discovered_at, t.raw
            FROM smart_wallet_trades t
            WHERE NOT EXISTS (
                SELECT 1
                FROM smart_signals s
                WHERE s.source_trade_id = t.id
            )
            ORDER BY t.source_timestamp DESC
            LIMIT $1
            "#,
        )
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query unprocessed smart wallet trades: {error}"),
            )
        })?;
        rows.iter().map(smart_wallet_trade_from_row).collect()
    }

    async fn record_signals(&self, signals: &[SmartSignal]) -> Result<usize> {
        let mut inserted = 0usize;
        for signal in signals {
            let result = sqlx::query(
                r#"
                INSERT INTO smart_signals (
                    source_trade_id, wallet_address, condition_id, token_id, side,
                    source_price, current_price, price_slippage_cents, latency_ms,
                    source_notional_usd, consensus_wallet_count, score, status, reason,
                    created_at, updated_at
                )
                SELECT $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16
                WHERE NOT EXISTS (
                    SELECT 1
                    FROM smart_signals
                    WHERE source_trade_id = $1
                )
                "#,
            )
            .bind(&signal.source_trade_id)
            .bind(&signal.wallet_address)
            .bind(&signal.condition_id)
            .bind(&signal.token_id)
            .bind(signal.side.as_str())
            .bind(signal.source_price)
            .bind(signal.current_price)
            .bind(signal.price_slippage_cents)
            .bind(signal.latency_ms)
            .bind(signal.source_notional_usd)
            .bind(signal.consensus_wallet_count)
            .bind(signal.score)
            .bind(signal.status.as_str())
            .bind(&signal.reason)
            .bind(signal.created_at)
            .bind(signal.updated_at)
            .execute(&self.pool)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_INSERT_FAILED",
                    format!("failed to insert smart signal: {error}"),
                )
            })?;
            if result.rows_affected() > 0 {
                inserted += 1;
            }
        }
        Ok(inserted)
    }

    async fn list_signals_by_source_trade_ids(
        &self,
        source_trade_ids: &[String],
    ) -> Result<Vec<SmartSignal>> {
        if source_trade_ids.is_empty() {
            return Ok(Vec::new());
        }

        let rows = sqlx::query(
            r#"
            SELECT id, source_trade_id, wallet_address, condition_id, token_id, side,
                   source_price, current_price, price_slippage_cents, latency_ms,
                   source_notional_usd, consensus_wallet_count, score, status, reason,
                   created_at, updated_at
            FROM smart_signals
            WHERE source_trade_id = ANY($1)
            ORDER BY created_at DESC
            "#,
        )
        .bind(source_trade_ids)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query smart signals by source trade ids: {error}"),
            )
        })?;
        rows.iter().map(smart_signal_from_row).collect()
    }

    async fn list_signals(&self, limit: u16) -> Result<Vec<SmartSignal>> {
        let rows = sqlx::query(
            r#"
            SELECT id, source_trade_id, wallet_address, condition_id, token_id, side,
                   source_price, current_price, price_slippage_cents, latency_ms,
                   source_notional_usd, consensus_wallet_count, score, status, reason,
                   created_at, updated_at
            FROM smart_signals
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
                format!("failed to query smart signals: {error}"),
            )
        })?;
        rows.iter().map(smart_signal_from_row).collect()
    }

    async fn record_signal_decisions(&self, decisions: &[SmartSignalDecision]) -> Result<usize> {
        let mut inserted = 0usize;
        for decision in decisions {
            let result = sqlx::query(
                r#"
                INSERT INTO smart_signal_decisions (
                    signal_id, decision, stage, mode, rejection_reason, risk_checks, decided_at
                )
                SELECT $1, $2, $3, $4, $5, $6, $7
                WHERE NOT EXISTS (
                    SELECT 1
                    FROM smart_signal_decisions
                    WHERE signal_id = $1
                      AND stage = $3
                )
                "#,
            )
            .bind(decision.signal_id)
            .bind(decision.decision.as_str())
            .bind(&decision.stage)
            .bind(decision.mode.as_str())
            .bind(&decision.rejection_reason)
            .bind(Json(decision.risk_checks.clone()))
            .bind(decision.decided_at)
            .execute(&self.pool)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_INSERT_FAILED",
                    format!("failed to insert smart signal decision: {error}"),
                )
            })?;
            if result.rows_affected() > 0 {
                inserted += 1;
            }
        }
        Ok(inserted)
    }

    async fn list_signal_decisions(&self, limit: u16) -> Result<Vec<SmartSignalDecision>> {
        let rows = sqlx::query(
            r#"
            SELECT id, signal_id, decision, stage, mode, rejection_reason, risk_checks, decided_at
            FROM smart_signal_decisions
            ORDER BY decided_at DESC
            LIMIT $1
            "#,
        )
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query smart signal decisions: {error}"),
            )
        })?;
        rows.iter().map(smart_signal_decision_from_row).collect()
    }

    async fn latest_signal_advisory(
        &self,
        lookup: &SmartSignalAdvisoryLookup,
        now: OffsetDateTime,
    ) -> Result<Option<SmartSignalAdvisory>> {
        let row = sqlx::query(
            r#"
            SELECT id, signal_id, provider, request_format, model, input_hash,
                   recommendation, confidence, risk_tags, summary, reasons, raw_output,
                   expires_at, created_at
            FROM smart_signal_advisories
            WHERE signal_id = $1
              AND provider = $2
              AND request_format = $3
              AND model = $4
              AND input_hash = $5
              AND expires_at > $6
            ORDER BY expires_at DESC, created_at DESC
            LIMIT 1
            "#,
        )
        .bind(lookup.signal_id)
        .bind(&lookup.provider)
        .bind(&lookup.request_format)
        .bind(&lookup.model)
        .bind(&lookup.input_hash)
        .bind(now)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query latest smart signal advisory: {error}"),
            )
        })?;

        row.as_ref().map(smart_signal_advisory_from_row).transpose()
    }

    async fn save_signal_advisory(&self, advisory: &SmartSignalAdvisory) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO smart_signal_advisories (
                signal_id, provider, request_format, model, input_hash, recommendation,
                confidence, risk_tags, summary, reasons, raw_output, expires_at, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            ON CONFLICT (signal_id, provider, request_format, model, input_hash) DO UPDATE
            SET recommendation = EXCLUDED.recommendation,
                confidence = EXCLUDED.confidence,
                risk_tags = EXCLUDED.risk_tags,
                summary = EXCLUDED.summary,
                reasons = EXCLUDED.reasons,
                raw_output = EXCLUDED.raw_output,
                expires_at = EXCLUDED.expires_at,
                created_at = EXCLUDED.created_at
            "#,
        )
        .bind(advisory.signal_id)
        .bind(&advisory.provider)
        .bind(&advisory.request_format)
        .bind(&advisory.model)
        .bind(&advisory.input_hash)
        .bind(advisory.recommendation.as_str())
        .bind(advisory.confidence)
        .bind(Json(advisory.risk_tags.clone()))
        .bind(&advisory.summary)
        .bind(Json(advisory.reasons.clone()))
        .bind(Json(advisory.raw_output.clone()))
        .bind(advisory.expires_at)
        .bind(advisory.created_at)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!("failed to upsert smart signal advisory: {error}"),
            )
        })?;
        Ok(())
    }

    async fn list_signal_advisories(&self, limit: u16) -> Result<Vec<SmartSignalAdvisory>> {
        let rows = sqlx::query(
            r#"
            SELECT id, signal_id, provider, request_format, model, input_hash,
                   recommendation, confidence, risk_tags, summary, reasons, raw_output,
                   expires_at, created_at
            FROM smart_signal_advisories
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
                format!("failed to query smart signal advisories: {error}"),
            )
        })?;
        rows.iter().map(smart_signal_advisory_from_row).collect()
    }
}
