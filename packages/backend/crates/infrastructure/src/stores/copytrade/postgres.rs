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

    async fn list_orders(&self, limit: u16) -> Result<Vec<CopyOrder>> {
        let rows = sqlx::query(
            r#"
            SELECT id, account_id, wallet_address, source_trade_id, condition_id,
                   token_id, outcome, side, price, size, notional_usd,
                   external_order_id, status, reason, filled_size, realized_pnl,
                   created_at, updated_at, trace_id
            FROM copytrade_orders
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
                format!("failed to query copytrade orders: {error}"),
            )
        })?;

        rows.iter().map(copytrade_order_from_row).collect()
    }

    async fn list_open_orders(&self, account_id: &str) -> Result<Vec<CopyOrder>> {
        let rows = sqlx::query(
            r#"
            SELECT id, account_id, wallet_address, source_trade_id, condition_id,
                   token_id, outcome, side, price, size, notional_usd,
                   external_order_id, status, reason, filled_size, realized_pnl,
                   created_at, updated_at, trace_id
            FROM copytrade_orders
            WHERE account_id = $1
              AND status IN ('planned', 'open')
            ORDER BY updated_at DESC
            "#,
        )
        .bind(account_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query copytrade open orders: {error}"),
            )
        })?;

        rows.iter().map(copytrade_order_from_row).collect()
    }

    async fn list_positions(&self, limit: u16) -> Result<Vec<CopyPosition>> {
        let rows = sqlx::query(
            r#"
            SELECT account_id, wallet_address, condition_id, token_id, outcome,
                   size, avg_price, realized_pnl, updated_at
            FROM copytrade_positions
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
                format!("failed to query copytrade positions: {error}"),
            )
        })?;

        rows.iter().map(copytrade_position_from_row).collect()
    }

    async fn list_account_positions(&self, account_id: &str) -> Result<Vec<CopyPosition>> {
        let rows = sqlx::query(
            r#"
            SELECT account_id, wallet_address, condition_id, token_id, outcome,
                   size, avg_price, realized_pnl, updated_at
            FROM copytrade_positions
            WHERE account_id = $1 AND size <> 0
            "#,
        )
        .bind(account_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query copytrade account positions: {error}"),
            )
        })?;

        rows.iter().map(copytrade_position_from_row).collect()
    }

    async fn load_account_state(&self, config: &CopyTradeConfig) -> Result<CopyAccountState> {
        let existing = sqlx::query(
            r#"
            SELECT account_id, capital_usd, available_usd, reserved_usd,
                   realized_pnl, fees_paid, tick_index, updated_at
            FROM copytrade_account_state
            WHERE account_id = $1
            "#,
        )
        .bind(&config.account_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to query copytrade account state: {error}"),
            )
        })?;

        if let Some(row) = existing {
            return copytrade_account_state_from_row(&row);
        }

        let state = CopyAccountState::fresh(
            &config.account_id,
            config.account_capital_usd,
            OffsetDateTime::now_utc(),
        );
        upsert_copytrade_account_state(&self.pool, &state).await?;
        Ok(state)
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

    async fn apply_copy_tick(
        &self,
        outcome: &CopySimulationOutcome,
        trace_id: &str,
    ) -> Result<()> {
        // Mark processed source trades.
        for trade_id in &outcome.processed_source_trade_ids {
            self.mark_source_trade_processed(trade_id).await?;
        }

        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin copytrade tick transaction: {error}"),
            )
        })?;

        for order in &outcome.orders {
            insert_copytrade_order(&mut transaction, order, trace_id).await?;
        }
        for fill in &outcome.fills {
            insert_copytrade_fill(&mut transaction, fill).await?;
        }
        for position in &outcome.positions {
            upsert_copytrade_position_tx(&mut transaction, position).await?;
        }
        upsert_copytrade_account_state_tx(&mut transaction, &outcome.account).await?;
        for event in &outcome.events {
            insert_copytrade_event_tx(&mut transaction, event).await?;
        }

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit copytrade tick transaction: {error}"),
            )
        })?;
        Ok(())
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
            UPDATE copytrade_orders
            SET status = 'cancelled',
                reason = $1,
                updated_at = $2,
                trace_id = $3
            WHERE status IN ('planned', 'open')
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
                format!("failed to cancel copytrade orders: {error}"),
            )
        })?;
        Ok(result.rows_affected() as usize)
    }

    async fn reset_simulation(
        &self,
        config: &CopyTradeConfig,
        _trace_id: &str,
    ) -> Result<()> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin copytrade reset transaction: {error}"),
            )
        })?;

        for statement in [
            "DELETE FROM copytrade_orders WHERE account_id = $1",
            "DELETE FROM copytrade_positions WHERE account_id = $1",
        ] {
            sqlx::query(statement)
                .bind(&config.account_id)
                .execute(&mut *transaction)
                .await
                .map_err(|error| {
                    db_error(
                        "POSTGRES_DELETE_FAILED",
                        format!("failed to reset copytrade simulation: {error}"),
                    )
                })?;
        }

        let fresh = CopyAccountState::fresh(
            &config.account_id,
            config.account_capital_usd,
            OffsetDateTime::now_utc(),
        );
        upsert_copytrade_account_state_tx(&mut transaction, &fresh).await?;

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit copytrade reset transaction: {error}"),
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

fn copytrade_wallet_from_row(row: &sqlx::postgres::PgRow) -> Result<TrackedWallet> {
    let status_raw: String = row.try_get("status").map_err(postgres_decode_error)?;
    let sizing_raw: Option<String> = row
        .try_get("sizing_override")
        .map_err(postgres_decode_error)?;
    let sizing_override = sizing_raw
        .and_then(|s| CopySizingMode::from_str(&s).ok());
    Ok(TrackedWallet {
        address: row.try_get("address").map_err(postgres_decode_error)?,
        label: row.try_get("label").map_err(postgres_decode_error)?,
        status: TrackedWalletStatus::from_str(&status_raw)?,
        sizing_override,
        max_exposure_override: row
            .try_get("max_exposure_override")
            .map_err(postgres_decode_error)?,
        added_at: row.try_get("added_at").map_err(postgres_decode_error)?,
        updated_at: row.try_get("updated_at").map_err(postgres_decode_error)?,
        analysis: WalletAnalysisStats {
            trades_window: row.try_get("trades_window").map_err(postgres_decode_error)?,
            volume_window_usd: row
                .try_get("volume_window_usd")
                .map_err(postgres_decode_error)?,
            realized_pnl_window: row
                .try_get("realized_pnl_window")
                .map_err(postgres_decode_error)?,
            win_rate: row.try_get("win_rate").map_err(postgres_decode_error)?,
            roi: row.try_get("roi").map_err(postgres_decode_error)?,
            avg_trade_usd: row
                .try_get("avg_trade_usd")
                .map_err(postgres_decode_error)?,
            markets_traded: row
                .try_get("markets_traded")
                .map_err(postgres_decode_error)?,
            last_active_at: row
                .try_get("last_active_at")
                .map_err(postgres_decode_error)?,
            last_analyzed_at: row
                .try_get("last_analyzed_at")
                .map_err(postgres_decode_error)?,
        },
    })
}

fn copytrade_source_trade_from_row(row: &sqlx::postgres::PgRow) -> Result<SourceTrade> {
    let side_raw: String = row.try_get("side").map_err(postgres_decode_error)?;
    Ok(SourceTrade {
        id: row.try_get("id").map_err(postgres_decode_error)?,
        wallet_address: row
            .try_get("wallet_address")
            .map_err(postgres_decode_error)?,
        condition_id: row
            .try_get("condition_id")
            .map_err(postgres_decode_error)?,
        token_id: row.try_get("token_id").map_err(postgres_decode_error)?,
        outcome: row.try_get("outcome").map_err(postgres_decode_error)?,
        side: CopyOrderSide::from_str(&side_raw)?,
        price: row.try_get("price").map_err(postgres_decode_error)?,
        size: row.try_get("size").map_err(postgres_decode_error)?,
        usd_size: row.try_get("usd_size").map_err(postgres_decode_error)?,
        title: row.try_get("title").map_err(postgres_decode_error)?,
        source_tx_hash: row
            .try_get("source_tx_hash")
            .map_err(postgres_decode_error)?,
        source_timestamp: row
            .try_get("source_timestamp")
            .map_err(postgres_decode_error)?,
        observed_at: row.try_get("observed_at").map_err(postgres_decode_error)?,
        copied: row.try_get("copied").map_err(postgres_decode_error)?,
        decision_reason: row
            .try_get("decision_reason")
            .map_err(postgres_decode_error)?,
    })
}

fn copytrade_order_from_row(row: &sqlx::postgres::PgRow) -> Result<CopyOrder> {
    let side_raw: String = row.try_get("side").map_err(postgres_decode_error)?;
    let status_raw: String = row.try_get("status").map_err(postgres_decode_error)?;
    Ok(CopyOrder {
        id: row.try_get("id").map_err(postgres_decode_error)?,
        account_id: row.try_get("account_id").map_err(postgres_decode_error)?,
        wallet_address: row
            .try_get("wallet_address")
            .map_err(postgres_decode_error)?,
        source_trade_id: row
            .try_get("source_trade_id")
            .map_err(postgres_decode_error)?,
        condition_id: row
            .try_get("condition_id")
            .map_err(postgres_decode_error)?,
        token_id: row.try_get("token_id").map_err(postgres_decode_error)?,
        outcome: row.try_get("outcome").map_err(postgres_decode_error)?,
        side: CopyOrderSide::from_str(&side_raw)?,
        price: row.try_get("price").map_err(postgres_decode_error)?,
        size: row.try_get("size").map_err(postgres_decode_error)?,
        notional_usd: row.try_get("notional_usd").map_err(postgres_decode_error)?,
        external_order_id: row
            .try_get("external_order_id")
            .map_err(postgres_decode_error)?,
        status: CopyOrderStatus::from_str(&status_raw)?,
        reason: row.try_get("reason").map_err(postgres_decode_error)?,
        filled_size: row.try_get("filled_size").map_err(postgres_decode_error)?,
        realized_pnl: row.try_get("realized_pnl").map_err(postgres_decode_error)?,
        created_at: row.try_get("created_at").map_err(postgres_decode_error)?,
        updated_at: row.try_get("updated_at").map_err(postgres_decode_error)?,
    })
}

fn copytrade_position_from_row(row: &sqlx::postgres::PgRow) -> Result<CopyPosition> {
    Ok(CopyPosition {
        account_id: row.try_get("account_id").map_err(postgres_decode_error)?,
        wallet_address: row
            .try_get("wallet_address")
            .map_err(postgres_decode_error)?,
        condition_id: row
            .try_get("condition_id")
            .map_err(postgres_decode_error)?,
        token_id: row.try_get("token_id").map_err(postgres_decode_error)?,
        outcome: row.try_get("outcome").map_err(postgres_decode_error)?,
        size: row.try_get("size").map_err(postgres_decode_error)?,
        avg_price: row.try_get("avg_price").map_err(postgres_decode_error)?,
        realized_pnl: row.try_get("realized_pnl").map_err(postgres_decode_error)?,
        updated_at: row.try_get("updated_at").map_err(postgres_decode_error)?,
    })
}

fn copytrade_account_state_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<CopyAccountState> {
    Ok(CopyAccountState {
        account_id: row.try_get("account_id").map_err(postgres_decode_error)?,
        capital_usd: row.try_get("capital_usd").map_err(postgres_decode_error)?,
        available_usd: row.try_get("available_usd").map_err(postgres_decode_error)?,
        reserved_usd: row.try_get("reserved_usd").map_err(postgres_decode_error)?,
        realized_pnl: row.try_get("realized_pnl").map_err(postgres_decode_error)?,
        fees_paid: row.try_get("fees_paid").map_err(postgres_decode_error)?,
        tick_index: row.try_get("tick_index").map_err(postgres_decode_error)?,
        updated_at: row.try_get("updated_at").map_err(postgres_decode_error)?,
    })
}

fn copytrade_event_from_row(row: &sqlx::postgres::PgRow) -> Result<CopyEvent> {
    let severity_raw: String = row.try_get("severity").map_err(postgres_decode_error)?;
    let metadata: Json<Value> = row
        .try_get("metadata_json")
        .map_err(postgres_decode_error)?;
    Ok(CopyEvent {
        id: row.try_get("id").map_err(postgres_decode_error)?,
        wallet_address: row
            .try_get("wallet_address")
            .map_err(postgres_decode_error)?,
        condition_id: row
            .try_get("condition_id")
            .map_err(postgres_decode_error)?,
        event_type: row.try_get("event_type").map_err(postgres_decode_error)?,
        severity: CopyEventSeverity::from_str(&severity_raw)?,
        message: row.try_get("message").map_err(postgres_decode_error)?,
        metadata: metadata.0,
        created_at: row.try_get("created_at").map_err(postgres_decode_error)?,
    })
}

// ── SQL helpers ─────────────────────────────────────────────────────────────

const COPYTRADE_ACCOUNT_STATE_UPSERT: &str = r#"
    INSERT INTO copytrade_account_state (
      account_id, capital_usd, available_usd, reserved_usd, realized_pnl,
      fees_paid, tick_index, updated_at
    )
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
    ON CONFLICT (account_id) DO UPDATE
    SET capital_usd = EXCLUDED.capital_usd,
        available_usd = EXCLUDED.available_usd,
        reserved_usd = EXCLUDED.reserved_usd,
        realized_pnl = EXCLUDED.realized_pnl,
        fees_paid = EXCLUDED.fees_paid,
        tick_index = EXCLUDED.tick_index,
        updated_at = EXCLUDED.updated_at
"#;

fn bind_copytrade_account_state<'q>(
    query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    state: &'q CopyAccountState,
) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
    query
        .bind(&state.account_id)
        .bind(state.capital_usd)
        .bind(state.available_usd)
        .bind(state.reserved_usd)
        .bind(state.realized_pnl)
        .bind(state.fees_paid)
        .bind(state.tick_index)
        .bind(state.updated_at)
}

async fn upsert_copytrade_account_state(
    pool: &PgPool,
    state: &CopyAccountState,
) -> Result<()> {
    bind_copytrade_account_state(sqlx::query(COPYTRADE_ACCOUNT_STATE_UPSERT), state)
        .execute(pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!("failed to upsert copytrade account state: {error}"),
            )
        })?;
    Ok(())
}

async fn upsert_copytrade_account_state_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    state: &CopyAccountState,
) -> Result<()> {
    bind_copytrade_account_state(sqlx::query(COPYTRADE_ACCOUNT_STATE_UPSERT), state)
        .execute(&mut **transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!("failed to upsert copytrade account state: {error}"),
            )
        })?;
    Ok(())
}

async fn upsert_copytrade_position_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    position: &CopyPosition,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO copytrade_positions (
          account_id, wallet_address, condition_id, token_id, outcome,
          size, avg_price, realized_pnl, updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        ON CONFLICT (account_id, token_id) DO UPDATE
        SET wallet_address = EXCLUDED.wallet_address,
            condition_id = EXCLUDED.condition_id,
            outcome = EXCLUDED.outcome,
            size = EXCLUDED.size,
            avg_price = EXCLUDED.avg_price,
            realized_pnl = EXCLUDED.realized_pnl,
            updated_at = EXCLUDED.updated_at
        "#,
    )
    .bind(&position.account_id)
    .bind(&position.wallet_address)
    .bind(&position.condition_id)
    .bind(&position.token_id)
    .bind(&position.outcome)
    .bind(position.size)
    .bind(position.avg_price)
    .bind(position.realized_pnl)
    .bind(position.updated_at)
    .execute(&mut **transaction)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_UPSERT_FAILED",
            format!("failed to upsert copytrade position: {error}"),
        )
    })?;
    Ok(())
}

async fn insert_copytrade_order(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    order: &CopyOrder,
    trace_id: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO copytrade_orders (
          id, account_id, wallet_address, source_trade_id, condition_id,
          token_id, outcome, side, price, size, notional_usd,
          external_order_id, status, reason, filled_size, realized_pnl,
          created_at, updated_at, trace_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)
        ON CONFLICT (id) DO UPDATE
        SET external_order_id = EXCLUDED.external_order_id,
            status = EXCLUDED.status,
            reason = EXCLUDED.reason,
            filled_size = EXCLUDED.filled_size,
            realized_pnl = EXCLUDED.realized_pnl,
            updated_at = EXCLUDED.updated_at,
            trace_id = EXCLUDED.trace_id
        "#,
    )
    .bind(&order.id)
    .bind(&order.account_id)
    .bind(&order.wallet_address)
    .bind(&order.source_trade_id)
    .bind(&order.condition_id)
    .bind(&order.token_id)
    .bind(&order.outcome)
    .bind(order.side.as_str())
    .bind(order.price)
    .bind(order.size)
    .bind(order.notional_usd)
    .bind(&order.external_order_id)
    .bind(order.status.as_str())
    .bind(&order.reason)
    .bind(order.filled_size)
    .bind(order.realized_pnl)
    .bind(order.created_at)
    .bind(order.updated_at)
    .bind(trace_id)
    .execute(&mut **transaction)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_INSERT_FAILED",
            format!("failed to insert copytrade order: {error}"),
        )
    })?;
    Ok(())
}

async fn insert_copytrade_fill(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    fill: &CopyFill,
) -> Result<()> {
    // Fills are persisted as events (not a separate table in this schema —
    // the `copytrade_events` table captures them as `copytrade_fill` event_type).
    // We record them here for completeness; the engine already stores fill
    // data on the order's `filled_size`/`realized_pnl` fields.
    insert_copytrade_event_tx(
        transaction,
        &CopyEvent {
            id: fill.id.clone(),
            wallet_address: Some(fill.wallet_address.clone()),
            condition_id: Some(fill.condition_id.clone()),
            event_type: "copytrade_fill".to_string(),
            severity: CopyEventSeverity::Info,
            message: format!(
                "Filled {} {} @ {} ({})",
                fill.size, fill.token_id, fill.price, fill.reason
            ),
            metadata: serde_json::json!({
                "order_id": fill.order_id,
                "token_id": fill.token_id,
                "side": fill.side.as_str(),
                "price": fill.price,
                "size": fill.size,
                "notional_usd": fill.notional_usd,
                "realized_pnl": fill.realized_pnl,
                "reason": fill.reason,
            }),
            created_at: fill.created_at,
        },
    )
    .await
}

async fn insert_copytrade_event_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event: &CopyEvent,
) -> Result<()> {
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
    .execute(&mut **transaction)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_INSERT_FAILED",
            format!("failed to insert copytrade event: {error}"),
        )
    })?;
    Ok(())
}
