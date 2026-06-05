// Postgres-backed `CopyTradeStore` implementation.

pub struct PostgresCopyTradeStore {
    pool: PgPool,
}

impl PostgresCopyTradeStore {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CopyTradeStore for PostgresCopyTradeStore {
    async fn load_config(&self) -> Result<CopyTradeConfig> {
        let rows = sqlx::query("SELECT key, value FROM copytrade_config")
            .fetch_all(&self.pool)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_QUERY_FAILED",
                    format!("failed to query copytrade config: {error}"),
                )
            })?;

        let mut config = CopyTradeConfig::default();
        for row in rows {
            let key: String = row.try_get("key").map_err(postgres_decode_error)?;
            let value: String = row.try_get("value").map_err(postgres_decode_error)?;
            apply_copytrade_config_value(&mut config, &key, &value)?;
        }
        Ok(config.normalized())
    }

    async fn save_config(&self, config: &CopyTradeConfig) -> Result<()> {
        let config = config.clone().normalized();
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin copytrade config transaction: {error}"),
            )
        })?;

        for (key, value) in copytrade_config_entries(&config) {
            sqlx::query(
                r#"
                INSERT INTO copytrade_config (key, value, updated_at)
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
                    format!("failed to upsert copytrade config: {error}"),
                )
            })?;
        }

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit copytrade config transaction: {error}"),
            )
        })?;
        Ok(())
    }

    async fn enqueue_control_command(&self, command: CopyControlCommand) -> Result<()> {
        postgres_enqueue_copytrade_control_command(&self.pool, command).await
    }

    async fn claim_next_control_command(
        &self,
        trace_id: &str,
        now: OffsetDateTime,
    ) -> Result<Option<CopyControlCommand>> {
        postgres_claim_next_copytrade_control_command(&self.pool, trace_id, now).await
    }

    async fn complete_control_command(
        &self,
        command_id: &str,
        trace_id: &str,
        now: OffsetDateTime,
    ) -> Result<()> {
        postgres_complete_copytrade_control_command(&self.pool, command_id, trace_id, now).await
    }

    async fn fail_control_command(
        &self,
        command_id: &str,
        trace_id: &str,
        error: &str,
        now: OffsetDateTime,
    ) -> Result<()> {
        postgres_fail_copytrade_control_command(&self.pool, command_id, trace_id, error, now).await
    }

    async fn list_wallets(&self) -> Result<Vec<TrackedWallet>> {
        let rows = sqlx::query(
            r#"
            SELECT address, label, status, sizing_override, max_exposure_override,
                   trades_window, volume_window_usd, realized_pnl_window, win_rate, roi,
                   avg_trade_usd, markets_traded, last_active_at, last_analyzed_at,
                   added_at, updated_at
            FROM copytrade_wallets
            ORDER BY updated_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query copytrade wallets: {error}"),
            )
        })?;

        rows.iter().map(copytrade_wallet_from_row).collect()
    }

    async fn upsert_wallet(&self, wallet: &TrackedWallet) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO copytrade_wallets (
              address, label, status, sizing_override, max_exposure_override,
              trades_window, volume_window_usd, realized_pnl_window, win_rate, roi,
              avg_trade_usd, markets_traded, last_active_at, last_analyzed_at,
              added_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            ON CONFLICT (address) DO UPDATE
            SET label = EXCLUDED.label,
                status = EXCLUDED.status,
                sizing_override = EXCLUDED.sizing_override,
                max_exposure_override = EXCLUDED.max_exposure_override,
                trades_window = EXCLUDED.trades_window,
                volume_window_usd = EXCLUDED.volume_window_usd,
                realized_pnl_window = EXCLUDED.realized_pnl_window,
                win_rate = EXCLUDED.win_rate,
                roi = EXCLUDED.roi,
                avg_trade_usd = EXCLUDED.avg_trade_usd,
                markets_traded = EXCLUDED.markets_traded,
                last_active_at = EXCLUDED.last_active_at,
                last_analyzed_at = EXCLUDED.last_analyzed_at,
                updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(&wallet.address)
        .bind(&wallet.label)
        .bind(wallet.status.as_str())
        .bind(wallet.sizing_override.map(|m| m.as_str().to_string()))
        .bind(wallet.max_exposure_override)
        .bind(wallet.analysis.trades_window)
        .bind(wallet.analysis.volume_window_usd)
        .bind(wallet.analysis.realized_pnl_window)
        .bind(wallet.analysis.win_rate)
        .bind(wallet.analysis.roi)
        .bind(wallet.analysis.avg_trade_usd)
        .bind(wallet.analysis.markets_traded)
        .bind(wallet.analysis.last_active_at)
        .bind(wallet.analysis.last_analyzed_at)
        .bind(wallet.added_at)
        .bind(wallet.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!("failed to upsert copytrade wallet: {error}"),
            )
        })?;
        Ok(())
    }

    async fn remove_wallet(&self, address: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM copytrade_wallets WHERE address = $1")
            .bind(address)
            .execute(&self.pool)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_DELETE_FAILED",
                    format!("failed to delete copytrade wallet: {error}"),
                )
            })?;
        Ok(result.rows_affected() > 0)
    }

    async fn update_wallet_analysis(
        &self,
        address: &str,
        analysis: &WalletAnalysisStats,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE copytrade_wallets
            SET trades_window = $2,
                volume_window_usd = $3,
                realized_pnl_window = $4,
                win_rate = $5,
                roi = $6,
                avg_trade_usd = $7,
                markets_traded = $8,
                last_active_at = $9,
                last_analyzed_at = $10,
                updated_at = now()
            WHERE address = $1
            "#,
        )
        .bind(address)
        .bind(analysis.trades_window)
        .bind(analysis.volume_window_usd)
        .bind(analysis.realized_pnl_window)
        .bind(analysis.win_rate)
        .bind(analysis.roi)
        .bind(analysis.avg_trade_usd)
        .bind(analysis.markets_traded)
        .bind(analysis.last_active_at)
        .bind(analysis.last_analyzed_at)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!("failed to update copytrade wallet analysis: {error}"),
            )
        })?;
        Ok(())
    }

    async fn record_source_trades(&self, trades: &[SourceTrade]) -> Result<usize> {
        let mut inserted = 0usize;
        for trade in trades {
            let result = sqlx::query(
                r#"
                INSERT INTO copytrade_source_trades (
                  id, wallet_address, condition_id, token_id, outcome, side,
                  price, size, usd_size, title, source_tx_hash,
                  source_timestamp, observed_at, copied, decision_reason
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
                ON CONFLICT (id) DO NOTHING
                "#,
            )
            .bind(&trade.id)
            .bind(&trade.wallet_address)
            .bind(&trade.condition_id)
            .bind(&trade.token_id)
            .bind(&trade.outcome)
            .bind(trade.side.as_str())
            .bind(trade.price)
            .bind(trade.size)
            .bind(trade.usd_size)
            .bind(&trade.title)
            .bind(&trade.source_tx_hash)
            .bind(trade.source_timestamp)
            .bind(trade.observed_at)
            .bind(trade.copied)
            .bind(&trade.decision_reason)
            .execute(&self.pool)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_INSERT_FAILED",
                    format!("failed to insert copytrade source trade: {error}"),
                )
            })?;
            if result.rows_affected() > 0 {
                inserted += 1;
            }
        }
        Ok(inserted)
    }

    async fn list_source_trades(&self, limit: u16) -> Result<Vec<SourceTrade>> {
        let rows = sqlx::query(
            r#"
            SELECT id, wallet_address, condition_id, token_id, outcome, side,
                   price, size, usd_size, title, source_tx_hash,
                   source_timestamp, observed_at, copied, decision_reason
            FROM copytrade_source_trades
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
                format!("failed to query copytrade source trades: {error}"),
            )
        })?;

        rows.iter().map(copytrade_source_trade_from_row).collect()
    }

    async fn mark_source_trade_processed(&self, trade_id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE copytrade_source_trades SET copied = true WHERE id = $1",
        )
        .bind(trade_id)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!("failed to mark copytrade source trade processed: {error}"),
            )
        })?;
        Ok(())
    }

    async fn list_events(&self, limit: u16) -> Result<Vec<CopyEvent>> {
        let rows = sqlx::query(
            r#"
            SELECT id, wallet_address, condition_id, event_type, severity,
                   message, metadata_json, created_at
            FROM copytrade_events
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
                format!("failed to query copytrade events: {error}"),
            )
        })?;

        rows.iter().map(copytrade_event_from_row).collect()
    }

    async fn log_event(&self, event: CopyEvent) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO copytrade_events (
              id, wallet_address, condition_id, event_type,
              severity, message, metadata_json, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (id) DO NOTHING
            "#,
        )
        .bind(&event.id)
        .bind(&event.wallet_address)
        .bind(&event.condition_id)
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
                format!("failed to insert copytrade event: {error}"),
            )
        })?;
        Ok(())
    }
}

// ── Row mappers ─────────────────────────────────────────────────────────────

fn apply_copytrade_config_value(
    config: &mut CopyTradeConfig,
    key: &str,
    value: &str,
) -> Result<()> {
    match key {
        "enabled" => config.enabled = parse_bool_config(key, value)?,
        "mode" => config.mode = CopyTradeMode::from_str(value)?,
        "account_id" => config.account_id = value.to_string(),
        "account_capital_usd" => config.account_capital_usd = parse_decimal_config(key, value)?,
        "sizing_mode" => config.sizing_mode = CopySizingMode::from_str(value)?,
        "fixed_usd_per_trade" => config.fixed_usd_per_trade = parse_decimal_config(key, value)?,
        "proportional_factor" => config.proportional_factor = parse_decimal_config(key, value)?,
        "capital_ratio" => config.capital_ratio = parse_decimal_config(key, value)?,
        "min_source_trade_usd" => {
            config.min_source_trade_usd = parse_decimal_config(key, value)?;
        }
        "max_price" => config.max_price = parse_decimal_config(key, value)?,
        "min_price" => config.min_price = parse_decimal_config(key, value)?,
        "copy_sells" => config.copy_sells = parse_bool_config(key, value)?,
        "max_position_per_market_usd" => {
            config.max_position_per_market_usd = parse_decimal_config(key, value)?;
        }
        "per_wallet_max_exposure_usd" => {
            config.per_wallet_max_exposure_usd = parse_decimal_config(key, value)?;
        }
        "max_total_exposure_usd" => {
            config.max_total_exposure_usd = parse_decimal_config(key, value)?;
        }
        "max_open_copy_orders" => {
            config.max_open_copy_orders = parse_u16_config(key, value)?;
        }
        "daily_loss_limit_usd" => {
            config.daily_loss_limit_usd = parse_decimal_config(key, value)?;
        }
        "cooldown_secs" => config.cooldown_secs = parse_u64_config(key, value)?,
        "max_slippage_cents" => config.max_slippage_cents = parse_decimal_config(key, value)?,
        "fill_rate_per_tick" => config.fill_rate_per_tick = parse_decimal_config(key, value)?,
        "max_fill_ratio" => config.max_fill_ratio = parse_decimal_config(key, value)?,
        _ => {}
    }
    Ok(())
}

fn copytrade_config_entries(config: &CopyTradeConfig) -> Vec<(&'static str, String)> {
    vec![
        ("enabled", config.enabled.to_string()),
        ("mode", config.mode.as_str().to_string()),
        ("account_id", config.account_id.clone()),
        ("account_capital_usd", config.account_capital_usd.to_string()),
        ("sizing_mode", config.sizing_mode.as_str().to_string()),
        ("fixed_usd_per_trade", config.fixed_usd_per_trade.to_string()),
        ("proportional_factor", config.proportional_factor.to_string()),
        ("capital_ratio", config.capital_ratio.to_string()),
        ("min_source_trade_usd", config.min_source_trade_usd.to_string()),
        ("max_price", config.max_price.to_string()),
        ("min_price", config.min_price.to_string()),
        ("copy_sells", config.copy_sells.to_string()),
        (
            "max_position_per_market_usd",
            config.max_position_per_market_usd.to_string(),
        ),
        (
            "per_wallet_max_exposure_usd",
            config.per_wallet_max_exposure_usd.to_string(),
        ),
        (
            "max_total_exposure_usd",
            config.max_total_exposure_usd.to_string(),
        ),
        ("max_open_copy_orders", config.max_open_copy_orders.to_string()),
        ("daily_loss_limit_usd", config.daily_loss_limit_usd.to_string()),
        ("cooldown_secs", config.cooldown_secs.to_string()),
        ("max_slippage_cents", config.max_slippage_cents.to_string()),
        ("fill_rate_per_tick", config.fill_rate_per_tick.to_string()),
        ("max_fill_ratio", config.max_fill_ratio.to_string()),
    ]
}
