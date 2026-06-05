// In-memory `CopyTradeStore` implementation backing tests and the no-database local path.

pub struct InMemoryCopyTradeStore {
    config: RwLock<CopyTradeConfig>,
    wallets: RwLock<Vec<TrackedWallet>>,
    source_trades: RwLock<Vec<SourceTrade>>,
    events: RwLock<Vec<CopyEvent>>,
    control_commands: RwLock<Vec<CopyControlCommand>>,
}

impl InMemoryCopyTradeStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: RwLock::new(CopyTradeConfig::default()),
            wallets: RwLock::new(Vec::new()),
            source_trades: RwLock::new(Vec::new()),
            events: RwLock::new(Vec::new()),
            control_commands: RwLock::new(Vec::new()),
        }
    }
}

#[async_trait]
impl CopyTradeStore for InMemoryCopyTradeStore {
    async fn load_config(&self) -> Result<CopyTradeConfig> {
        Ok(self.config.read().await.clone().normalized())
    }

    async fn save_config(&self, config: &CopyTradeConfig) -> Result<()> {
        *self.config.write().await = config.clone().normalized();
        Ok(())
    }

    async fn enqueue_control_command(&self, command: CopyControlCommand) -> Result<()> {
        let mut commands = self.control_commands.write().await;
        commands.push(command);
        commands.sort_by(|left, right| left.requested_at.cmp(&right.requested_at));
        Ok(())
    }

    async fn claim_next_control_command(
        &self,
        trace_id: &str,
        now: OffsetDateTime,
    ) -> Result<Option<CopyControlCommand>> {
        let mut commands = self.control_commands.write().await;
        let Some(command) = commands
            .iter_mut()
            .find(|command| command.status == CopyControlCommandStatus::Pending)
        else {
            return Ok(None);
        };
        command.status = CopyControlCommandStatus::Running;
        command.started_at = Some(now);
        command.trace_id = Some(trace_id.to_string());
        Ok(Some(command.clone()))
    }

    async fn complete_control_command(
        &self,
        command_id: &str,
        trace_id: &str,
        now: OffsetDateTime,
    ) -> Result<()> {
        let mut commands = self.control_commands.write().await;
        if let Some(command) = commands.iter_mut().find(|command| command.id == command_id) {
            command.status = CopyControlCommandStatus::Completed;
            command.completed_at = Some(now);
            command.trace_id = Some(trace_id.to_string());
        }
        Ok(())
    }

    async fn fail_control_command(
        &self,
        command_id: &str,
        trace_id: &str,
        error: &str,
        now: OffsetDateTime,
    ) -> Result<()> {
        let mut commands = self.control_commands.write().await;
        if let Some(command) = commands.iter_mut().find(|command| command.id == command_id) {
            command.status = CopyControlCommandStatus::Failed;
            command.completed_at = Some(now);
            command.trace_id = Some(trace_id.to_string());
            command.error = Some(error.to_string());
        }
        Ok(())
    }

    async fn list_wallets(&self) -> Result<Vec<TrackedWallet>> {
        let mut wallets = self.wallets.read().await.clone();
        wallets.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(wallets)
    }

    async fn upsert_wallet(&self, wallet: &TrackedWallet) -> Result<()> {
        let mut store = self.wallets.write().await;
        if let Some(existing) = store.iter_mut().find(|w| w.address == wallet.address) {
            *existing = wallet.clone();
        } else {
            store.push(wallet.clone());
        }
        Ok(())
    }

    async fn remove_wallet(&self, address: &str) -> Result<bool> {
        let mut store = self.wallets.write().await;
        let before = store.len();
        store.retain(|w| w.address != address);
        Ok(store.len() < before)
    }

    async fn update_wallet_analysis(
        &self,
        address: &str,
        analysis: &WalletAnalysisStats,
    ) -> Result<()> {
        let mut store = self.wallets.write().await;
        if let Some(wallet) = store.iter_mut().find(|w| w.address == address) {
            wallet.analysis = analysis.clone();
            wallet.updated_at = OffsetDateTime::now_utc();
        }
        Ok(())
    }

    async fn record_source_trades(&self, trades: &[SourceTrade]) -> Result<usize> {
        let mut store = self.source_trades.write().await;
        let existing_ids: std::collections::HashSet<String> =
            store.iter().map(|trade| trade.id.clone()).collect();
        let mut inserted = 0usize;
        for trade in trades {
            if existing_ids.contains(&trade.id) {
                continue;
            }
            store.push(trade.clone());
            inserted += 1;
        }
        store.sort_by(|left, right| right.source_timestamp.cmp(&left.source_timestamp));
        store.truncate(5_000);
        Ok(inserted)
    }

    async fn list_source_trades(&self, limit: u16) -> Result<Vec<SourceTrade>> {
        let mut trades = self.source_trades.read().await.clone();
        trades.sort_by(|left, right| right.source_timestamp.cmp(&left.source_timestamp));
        trades.truncate(usize::from(limit));
        Ok(trades)
    }

    async fn mark_source_trade_processed(&self, trade_id: &str) -> Result<()> {
        let mut store = self.source_trades.write().await;
        if let Some(trade) = store.iter_mut().find(|t| t.id == trade_id) {
            trade.copied = true;
        }
        Ok(())
    }

    async fn list_events(&self, limit: u16) -> Result<Vec<CopyEvent>> {
        let mut events = self.events.read().await.clone();
        events.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        events.truncate(usize::from(limit));
        Ok(events)
    }

    async fn log_event(&self, event: CopyEvent) -> Result<()> {
        let mut events = self.events.write().await;
        events.push(event);
        events.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        events.truncate(1_000);
        Ok(())
    }
}
