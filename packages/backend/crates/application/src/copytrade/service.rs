#[async_trait]
pub trait CopyTradeStore: Send + Sync {
    async fn load_config(&self) -> Result<CopyTradeConfig>;
    async fn save_config(&self, config: &CopyTradeConfig) -> Result<()>;
    async fn enqueue_control_command(&self, command: CopyControlCommand) -> Result<()>;
    async fn claim_next_control_command(
        &self,
        trace_id: &str,
        now: OffsetDateTime,
    ) -> Result<Option<CopyControlCommand>>;
    async fn complete_control_command(
        &self,
        command_id: &str,
        trace_id: &str,
        now: OffsetDateTime,
    ) -> Result<()>;
    async fn fail_control_command(
        &self,
        command_id: &str,
        trace_id: &str,
        error: &str,
        now: OffsetDateTime,
    ) -> Result<()>;

    // Wallet management
    async fn list_wallets(&self) -> Result<Vec<TrackedWallet>>;
    async fn upsert_wallet(&self, wallet: &TrackedWallet) -> Result<()>;
    async fn remove_wallet(&self, address: &str) -> Result<bool>;
    async fn update_wallet_analysis(
        &self,
        address: &str,
        analysis: &WalletAnalysisStats,
    ) -> Result<()>;

    // Source trades
    async fn record_source_trades(&self, trades: &[SourceTrade]) -> Result<usize>;
    async fn list_source_trades(&self, limit: u16) -> Result<Vec<SourceTrade>>;
    async fn mark_source_trade_processed(&self, trade_id: &str) -> Result<()>;

    // Orders
    async fn list_orders(&self, limit: u16) -> Result<Vec<CopyOrder>>;
    async fn list_open_orders(&self, account_id: &str) -> Result<Vec<CopyOrder>>;

    // Positions
    async fn list_positions(&self, limit: u16) -> Result<Vec<CopyPosition>>;
    async fn list_account_positions(&self, account_id: &str) -> Result<Vec<CopyPosition>>;

    // Account state (simulation ledger)
    async fn load_account_state(&self, config: &CopyTradeConfig) -> Result<CopyAccountState>;

    // Events
    async fn list_events(&self, limit: u16) -> Result<Vec<CopyEvent>>;
    async fn log_event(&self, event: CopyEvent) -> Result<()>;

    // Atomic tick persistence
    async fn apply_copy_tick(
        &self,
        outcome: &CopySimulationOutcome,
        trace_id: &str,
    ) -> Result<()>;

    // Cancellation and reset
    async fn cancel_open_orders(
        &self,
        account_id: Option<&str>,
        reason: &str,
        trace_id: &str,
    ) -> Result<usize>;
    async fn reset_simulation(
        &self,
        config: &CopyTradeConfig,
        trace_id: &str,
    ) -> Result<()>;
}

#[derive(Clone)]
pub struct CopyTradeService {
    store: Arc<dyn CopyTradeStore>,
}

impl CopyTradeService {
    #[must_use]
    pub fn new(store: Arc<dyn CopyTradeStore>) -> Self {
        Self { store }
    }

    pub async fn read_config(&self) -> Result<CopyTradeConfig> {
        self.store
            .load_config()
            .await
            .map(CopyTradeConfig::normalized)
    }

    pub async fn update_config(&self, patch: CopyTradeConfigPatch) -> Result<CopyTradeConfig> {
        let current = self.read_config().await?;
        let next = current.apply_patch(patch);
        self.store.save_config(&next).await?;
        Ok(next)
    }

    pub async fn enqueue_control_command(
        &self,
        action: CopyControlAction,
        reason: &str,
        trace_id: &str,
    ) -> Result<CopyControlCommand> {
        let config = self.read_config().await?;
        let now = OffsetDateTime::now_utc();
        let command = CopyControlCommand {
            id: copy_control_command_id(trace_id),
            action,
            account_id: Some(config.account_id.clone()),
            reason: reason.to_string(),
            status: CopyControlCommandStatus::Pending,
            requested_at: now,
            started_at: None,
            completed_at: None,
            trace_id: Some(trace_id.to_string()),
            error: None,
        };

        self.store.enqueue_control_command(command.clone()).await?;
        self.store
            .log_event(new_copy_event(
                Some(config.account_id),
                None,
                "copytrade_control_command_queued",
                CopyEventSeverity::Info,
                format!("Queued copytrade control command: {}", action.as_str()),
                json!({
                    "command_id": &command.id,
                    "action": action.as_str(),
                    "reason": reason,
                    "trace_id": trace_id,
                }),
            ))
            .await?;
        Ok(command)
    }

    pub async fn claim_next_control_command(
        &self,
        trace_id: &str,
    ) -> Result<Option<CopyControlCommand>> {
        self.store
            .claim_next_control_command(trace_id, OffsetDateTime::now_utc())
            .await
    }

    pub async fn complete_control_command(
        &self,
        command: &CopyControlCommand,
        trace_id: &str,
    ) -> Result<()> {
        self.store
            .complete_control_command(&command.id, trace_id, OffsetDateTime::now_utc())
            .await?;
        self.store
            .log_event(new_copy_event(
                command.account_id.clone(),
                None,
                "copytrade_control_command_completed",
                CopyEventSeverity::Info,
                format!("Completed copytrade control command: {}", command.action.as_str()),
                json!({
                    "command_id": command.id,
                    "action": command.action.as_str(),
                    "trace_id": trace_id,
                }),
            ))
            .await
    }

    pub async fn fail_control_command(
        &self,
        command: &CopyControlCommand,
        trace_id: &str,
        error: &AppError,
    ) -> Result<()> {
        let error_message = error.to_string();
        self.store
            .fail_control_command(
                &command.id,
                trace_id,
                &error_message,
                OffsetDateTime::now_utc(),
            )
            .await?;
        self.store
            .log_event(new_copy_event(
                command.account_id.clone(),
                None,
                "copytrade_control_command_failed",
                CopyEventSeverity::Critical,
                format!(
                    "Failed copytrade control command {}: {error_message}",
                    command.action.as_str()
                ),
                json!({
                    "command_id": command.id,
                    "action": command.action.as_str(),
                    "trace_id": trace_id,
                    "error": error_message,
                }),
            ))
            .await
    }

    pub async fn snapshot(&self) -> Result<CopyTradeSnapshot> {
        let config = self.read_config().await?;
        let account = self.store.load_account_state(&config).await?;
        let wallets = self.store.list_wallets().await?;
        let source_trades = self
            .store
            .list_source_trades(DEFAULT_LIST_LIMIT)
            .await?;
        let orders = self.store.list_orders(200).await?;
        let positions = self.store.list_positions(200).await?;
        let events = self.store.list_events(100).await?;

        let active_wallets = wallets
            .iter()
            .filter(|wallet| wallet.status == TrackedWalletStatus::Active)
            .count();
        let open_orders = orders
            .iter()
            .filter(|order| order.status.is_open_like())
            .count();
        let last_scan_at = source_trades
            .iter()
            .map(|trade| trade.observed_at)
            .max();
        let last_run_at = orders.iter().map(|order| order.updated_at).max();
        let error = events
            .iter()
            .find(|event| event.severity == CopyEventSeverity::Critical)
            .map(|event| event.message.clone());

        Ok(CopyTradeSnapshot {
            status: CopyTradeStatus {
                enabled: config.enabled,
                running: config.enabled,
                mode: config.mode,
                account_id: config.account_id.clone(),
                wallets_tracked: wallets.len(),
                active_wallets,
                open_orders,
                positions: positions.len(),
                source_trades_detected: source_trades.len(),
                last_scan_at,
                last_run_at,
                error,
            },
            config,
            account,
            wallets,
            source_trades,
            orders,
            positions,
            events,
        })
    }

    pub async fn add_wallet(&self, input: &AddTrackedWalletInput) -> Result<TrackedWallet> {
        let address = normalize_address(&input.address).ok_or_else(|| {
            AppError::invalid_input(
                "COPYTRADE_WALLET_ADDRESS_INVALID",
                "wallet address must be a 0x-prefixed 40-hex string",
            )
        })?;

        // Upsert is idempotent; update label / overrides on re-add.
        let now = OffsetDateTime::now_utc();
        let wallet = TrackedWallet {
            address: address.clone(),
            label: if input.label.trim().is_empty() {
                format!("{}…{}", &address[..6], &address[address.len() - 4..])
            } else {
                input.label.clone()
            },
            status: TrackedWalletStatus::Active,
            sizing_override: input.sizing_override,
            max_exposure_override: input.max_exposure_override,
            added_at: now,
            updated_at: now,
            analysis: WalletAnalysisStats::default(),
        };
        self.store.upsert_wallet(&wallet).await?;
        self.store
            .log_event(new_copy_event(
                Some(address.clone()),
                None,
                "wallet_added",
                CopyEventSeverity::Info,
                format!("Started tracking wallet {address}"),
                json!({"label": wallet.label}),
            ))
            .await?;
        Ok(wallet)
    }

    pub async fn remove_wallet(&self, address: &str) -> Result<bool> {
        let address = normalize_address(address).ok_or_else(|| {
            AppError::invalid_input(
                "COPYTRADE_WALLET_ADDRESS_INVALID",
                "wallet address must be a 0x-prefixed 40-hex string",
            )
        })?;
        let removed = self.store.remove_wallet(&address).await?;
        if removed {
            self.store
                .log_event(new_copy_event(
                    Some(address.clone()),
                    None,
                    "wallet_removed",
                    CopyEventSeverity::Info,
                    format!("Stopped tracking wallet {address}"),
                    json!(null),
                ))
                .await?;
        }
        Ok(removed)
    }

    pub async fn set_wallet_status(
        &self,
        address: &str,
        status: TrackedWalletStatus,
    ) -> Result<()> {
        let address = normalize_address(address).ok_or_else(|| {
            AppError::invalid_input(
                "COPYTRADE_WALLET_ADDRESS_INVALID",
                "wallet address must be a 0x-prefixed 40-hex string",
            )
        })?;
        let mut wallets = self.store.list_wallets().await?;
        let wallet = wallets.iter_mut().find(|w| w.address == address).ok_or_else(|| {
            AppError::not_found("COPYTRADE_WALLET_NOT_FOUND", "wallet not tracked")
        })?;
        wallet.status = status;
        wallet.updated_at = OffsetDateTime::now_utc();
        self.store.upsert_wallet(wallet).await
    }

    /// Run one full copy-trading cycle (called by the worker).
    ///
    /// `wallet_feeds` is per-wallet Data API results (pre-fetched by the worker
    /// so the service stays connector-agnostic). `books` is order books keyed
    /// by token_id.
    pub async fn run_copy_cycle(
        &self,
        wallet_feeds: Vec<WalletFeedInput>,
        books: HashMap<String, CopyOrderBook>,
        trace_id: &str,
    ) -> Result<CopyTradeRunReport> {
        let config = self.read_config().await?;

        if !config.enabled {
            // Even when disabled, still detect + record source trades so the
            // source_trades view stays populated.
            let detected = self
                .detect_and_record_source_trades(&config, &wallet_feeds, trace_id)
                .await?;
            return Ok(CopyTradeRunReport {
                wallets_scanned: wallet_feeds.len(),
                trades_detected: detected,
                orders_placed: 0,
                orders_filled: 0,
                orders_skipped: 0,
            });
        }

        if config.mode == CopyTradeMode::Live {
            self.store
                .log_event(new_copy_event(
                    Some(config.account_id.clone()),
                    None,
                    "copytrade_live_unsupported",
                    CopyEventSeverity::Warning,
                    "Copy trading live mode is not wired yet; falling back to simulation.",
                    json!({"trace_id": trace_id}),
                ))
                .await?;
        }

        // 1. Detect new source trades from wallet activity feeds.
        let detected = self
            .detect_and_record_source_trades(&config, &wallet_feeds, trace_id)
            .await?;

        // 2. Load unprocessed source trades and build copy decisions.
        let unprocessed = self
            .store
            .list_source_trades(DEFAULT_LIST_LIMIT)
            .await?
            .into_iter()
            .filter(|trade| !trade.copied)
            .collect::<Vec<_>>();

        let account = self.store.load_account_state(&config).await?;
        let open_orders = self
            .store
            .list_open_orders(&account.account_id)
            .await?;
        let positions = self
            .store
            .list_account_positions(&account.account_id)
            .await?;

        // Build lookup indexes for risk gating.
        let wallet_exposure = build_wallet_exposure_map(&positions, &open_orders);
        let total_exposure = compute_total_exposure(&positions, &open_orders);

        let mut decisions = Vec::new();
        for source_trade in &unprocessed {
            let wallet = wallet_feeds.iter().find(|f| f.address == source_trade.wallet_address);

            let skip = check_skip_reasons(
                &config,
                source_trade,
                &account,
                &positions,
                &open_orders,
                wallet_exposure.get(&source_trade.wallet_address).copied().unwrap_or(Decimal::ZERO),
                total_exposure,
            );
            if let Some(reason) = skip {
                decisions.push((source_trade.id.clone(), false, reason.as_str().to_string(), Decimal::ZERO, Decimal::ZERO));
                continue;
            }

            let _position_key = format!("{}:{}", account.account_id, source_trade.token_id);
            let current_position = positions.iter().find(|p| p.token_id == source_trade.token_id);
            let source_position = wallet.and_then(|f| {
                f.positions.iter().find(|p| p.asset == source_trade.token_id)
            });

            let decision = compute_copy_size(
                &config,
                source_trade,
                source_position,
                &account,
                current_position,
            );

            let price = if decision.copy && decision.size > Decimal::ZERO {
                apply_slippage(source_trade.price, source_trade.side, &config)
            } else {
                Decimal::ZERO
            };

            decisions.push((
                source_trade.id.clone(),
                decision.copy,
                decision.reason.clone(),
                decision.size,
                price,
            ));
        }

        // 3. Build copy orders from positive decisions.
        let order_now = OffsetDateTime::now_utc();
        let mut new_orders = Vec::new();
        let mut processed_ids = Vec::new();
        let mut skipped = 0usize;
        let mut placed = 0usize;

        for (trade_id, should_copy, reason, size, price) in &decisions {
            processed_ids.push(trade_id.clone());

            if !*should_copy || *size <= Decimal::ZERO {
                skipped += 1;
                continue;
            }

            let source = unprocessed.iter().find(|t| &t.id == trade_id).unwrap();
            let notional = *size * *price;

            new_orders.push(CopyOrder {
                id: new_copy_order_id(),
                account_id: config.account_id.clone(),
                wallet_address: source.wallet_address.clone(),
                source_trade_id: trade_id.clone(),
                condition_id: source.condition_id.clone(),
                token_id: source.token_id.clone(),
                outcome: source.outcome.clone(),
                side: source.side,
                price: *price,
                size: *size,
                notional_usd: notional,
                external_order_id: None,
                status: CopyOrderStatus::Planned,
                reason: reason.clone(),
                filled_size: Decimal::ZERO,
                realized_pnl: Decimal::ZERO,
                created_at: order_now,
                updated_at: order_now,
            });
            placed += 1;
        }

        // 4. Run the simulation engine on the new + existing orders.
        let elapsed_seconds = (OffsetDateTime::now_utc() - account.updated_at).whole_seconds();
        let outcome = run_copy_simulation_tick(
            &config,
            account,
            open_orders,
            positions,
            new_orders,
            &books,
            &processed_ids,
            elapsed_seconds,
            trace_id,
        );

        let report = CopyTradeRunReport {
            wallets_scanned: wallet_feeds.len(),
            trades_detected: detected,
            orders_placed: placed,
            orders_filled: outcome.fills.len(),
            orders_skipped: skipped,
        };

        let mut full_outcome = outcome;
        full_outcome.report = report;

        self.store.apply_copy_tick(&full_outcome, trace_id).await?;

        self.store
            .log_event(new_copy_event(
                Some(config.account_id.clone()),
                None,
                "copytrade_simulation_run",
                CopyEventSeverity::Info,
                "Completed copy-trading simulation tick.",
                json!({
                    "trace_id": trace_id,
                    "wallets_scanned": wallet_feeds.len(),
                    "trades_detected": detected,
                    "orders_placed": placed,
                    "orders_filled": full_outcome.fills.len(),
                    "orders_skipped": skipped,
                }),
            ))
            .await?;

        Ok(full_outcome.report)
    }

    pub async fn analyze_wallets(
        &self,
        wallet_feeds: Vec<WalletFeedInput>,
    ) -> Result<usize> {
        let wallets = self.store.list_wallets().await?;
        let mut analyzed = 0usize;

        for feed in &wallet_feeds {
            if !wallets.iter().any(|w| w.address == feed.address) {
                continue;
            }
            let stats = build_wallet_analysis(&feed.activities, &feed.positions);
            self.store
                .update_wallet_analysis(&feed.address, &stats)
                .await?;
            analyzed += 1;
        }

        Ok(analyzed)
    }

    pub async fn cancel_all_orders(
        &self,
        account_id: Option<&str>,
        reason: &str,
        trace_id: &str,
    ) -> Result<usize> {
        let cancelled = self
            .store
            .cancel_open_orders(account_id, reason, trace_id)
            .await?;
        self.store
            .log_event(new_copy_event(
                account_id.map(str::to_string),
                None,
                "copytrade_cancel_all",
                CopyEventSeverity::Info,
                reason,
                json!({"trace_id": trace_id, "cancelled_orders": cancelled}),
            ))
            .await?;
        Ok(cancelled)
    }

    pub async fn reset_simulation(&self, trace_id: &str) -> Result<()> {
        let config = self.read_config().await?;
        self.store.reset_simulation(&config, trace_id).await?;
        self.store
            .log_event(new_copy_event(
                Some(config.account_id.clone()),
                None,
                "copytrade_reset",
                CopyEventSeverity::Info,
                "Reset copy-trading simulation account, orders, positions.",
                json!({"trace_id": trace_id, "capital_usd": config.account_capital_usd}),
            ))
            .await
    }

    async fn detect_and_record_source_trades(
        &self,
        config: &CopyTradeConfig,
        wallet_feeds: &[WalletFeedInput],
        _trace_id: &str,
    ) -> Result<usize> {
        let all_trades = wallets_to_source_trades(config, wallet_feeds);
        self.store.record_source_trades(&all_trades).await
    }
}

/// Convert raw Data API activity items for all tracked wallets into domain
/// `SourceTrade` objects (only TRADE-type items, active wallets only).
fn wallets_to_source_trades(
    _config: &CopyTradeConfig,
    wallet_feeds: &[WalletFeedInput],
) -> Vec<SourceTrade> {
    let now = OffsetDateTime::now_utc();
    let mut trades = Vec::new();

    for feed in wallet_feeds {
        for activity in &feed.activities {
            if activity.kind.to_uppercase() != "TRADE" {
                continue;
            }

            let side = match activity.side.to_uppercase().as_str() {
                "BUY" => CopyOrderSide::Buy,
                "SELL" => CopyOrderSide::Sell,
                _ => continue,
            };

            let timestamp_secs = activity.timestamp.unix_timestamp();
            let id = source_trade_id(
                &feed.address,
                &activity.transaction_hash,
                &activity.asset,
                activity.side.to_uppercase().as_str(),
                timestamp_secs,
            );

            trades.push(SourceTrade {
                id,
                wallet_address: feed.address.clone(),
                condition_id: activity.condition_id.clone(),
                token_id: activity.asset.clone(),
                outcome: activity.outcome.clone(),
                side,
                price: activity.price,
                size: activity.size,
                usd_size: activity.usdc_size,
                title: activity.title.clone(),
                source_tx_hash: activity.transaction_hash.clone(),
                source_timestamp: activity.timestamp,
                observed_at: now,
                copied: false,
                decision_reason: String::new(),
            });
        }
    }

    trades
}

fn build_wallet_exposure_map(
    positions: &[CopyPosition],
    open_orders: &[CopyOrder],
) -> HashMap<String, Decimal> {
    let mut map: HashMap<String, Decimal> = HashMap::new();
    for position in positions {
        let notional = position.size * position.avg_price;
        *map.entry(position.wallet_address.clone())
            .or_insert(Decimal::ZERO) += notional;
    }
    for order in open_orders {
        let notional = order.remaining_size() * order.price;
        *map.entry(order.wallet_address.clone())
            .or_insert(Decimal::ZERO) += notional;
    }
    map
}

fn compute_total_exposure(
    positions: &[CopyPosition],
    open_orders: &[CopyOrder],
) -> Decimal {
    let position_exposure: Decimal = positions.iter().map(|p| p.size * p.avg_price).sum();
    let order_exposure: Decimal = open_orders
        .iter()
        .filter(|o| o.status.is_open_like())
        .map(|o| o.remaining_size() * o.price)
        .sum();
    position_exposure + order_exposure
}

fn apply_slippage(
    source_price: Decimal,
    side: CopyOrderSide,
    config: &CopyTradeConfig,
) -> Decimal {
    let slippage = config.max_slippage_cents / decimal("100");
    match side {
        CopyOrderSide::Buy => (source_price + slippage).min(config.max_price),
        CopyOrderSide::Sell => (source_price - slippage).max(config.min_price),
    }
}

fn copy_control_command_id(trace_id: &str) -> String {
    format!("copycmd_{}", trace_id.trim_start_matches("trc_"))
}
