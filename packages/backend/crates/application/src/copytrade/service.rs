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

    // Events
    async fn list_events(&self, limit: u16) -> Result<Vec<CopyEvent>>;
    async fn log_event(&self, event: CopyEvent) -> Result<()>;
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
        let wallets = self.store.list_wallets().await?;
        let source_trades = self
            .store
            .list_source_trades(DEFAULT_LIST_LIMIT)
            .await?;
        let events = self.store.list_events(100).await?;

        let active_wallets = wallets
            .iter()
            .filter(|wallet| wallet.status == TrackedWalletStatus::Active)
            .count();
        let last_scan_at = source_trades
            .iter()
            .map(|trade| trade.observed_at)
            .max();
        let error = events
            .iter()
            .find(|event| event.severity == CopyEventSeverity::Critical)
            .map(|event| event.message.clone());

        Ok(CopyTradeSnapshot {
            status: CopyTradeStatus {
                enabled: config.enabled,
                running: config.enabled,
                wallets_tracked: wallets.len(),
                active_wallets,
                source_trades_detected: source_trades.len(),
                last_scan_at,
                error,
            },
            config,
            wallets,
            source_trades,
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

    pub async fn detect_and_record_source_trades(
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
                activity.price,
                activity.size,
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

fn copy_control_command_id(trace_id: &str) -> String {
    format!("copycmd_{}", trace_id.trim_start_matches("trc_"))
}
