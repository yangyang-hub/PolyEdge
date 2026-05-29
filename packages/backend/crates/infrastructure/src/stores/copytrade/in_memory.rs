// In-memory `CopyTradeStore` implementation backing tests and the no-database local path.

pub struct InMemoryCopyTradeStore {
    config: RwLock<CopyTradeConfig>,
    wallets: RwLock<Vec<TrackedWallet>>,
    source_trades: RwLock<Vec<SourceTrade>>,
    orders: RwLock<Vec<CopyOrder>>,
    positions: RwLock<HashMap<(String, String), CopyPosition>>,
    events: RwLock<Vec<CopyEvent>>,
    account_state: RwLock<Option<CopyAccountState>>,
}

impl InMemoryCopyTradeStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: RwLock::new(CopyTradeConfig::default()),
            wallets: RwLock::new(Vec::new()),
            source_trades: RwLock::new(Vec::new()),
            orders: RwLock::new(Vec::new()),
            positions: RwLock::new(HashMap::new()),
            events: RwLock::new(Vec::new()),
            account_state: RwLock::new(None),
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

    async fn list_orders(&self, limit: u16) -> Result<Vec<CopyOrder>> {
        let mut orders = self.orders.read().await.clone();
        orders.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        orders.truncate(usize::from(limit));
        Ok(orders)
    }

    async fn list_open_orders(&self, account_id: &str) -> Result<Vec<CopyOrder>> {
        Ok(self
            .orders
            .read()
            .await
            .iter()
            .filter(|order| order.account_id == account_id && order.status.is_open_like())
            .cloned()
            .collect())
    }

    async fn list_positions(&self, limit: u16) -> Result<Vec<CopyPosition>> {
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

    async fn list_account_positions(&self, account_id: &str) -> Result<Vec<CopyPosition>> {
        Ok(self
            .positions
            .read()
            .await
            .values()
            .filter(|position| position.account_id == account_id && position.size != Decimal::ZERO)
            .cloned()
            .collect())
    }

    async fn load_account_state(&self, config: &CopyTradeConfig) -> Result<CopyAccountState> {
        let mut guard = self.account_state.write().await;
        if let Some(state) = guard.as_ref() {
            return Ok(state.clone());
        }
        let state = CopyAccountState::fresh(
            &config.account_id,
            config.account_capital_usd,
            OffsetDateTime::now_utc(),
        );
        *guard = Some(state.clone());
        Ok(state)
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

    async fn apply_copy_tick(
        &self,
        outcome: &CopySimulationOutcome,
        _trace_id: &str,
    ) -> Result<()> {
        // Mark processed source trades.
        {
            let mut trades = self.source_trades.write().await;
            for id in &outcome.processed_source_trade_ids {
                if let Some(trade) = trades.iter_mut().find(|t| &t.id == id) {
                    trade.copied = true;
                }
            }
        }
        // Upsert orders.
        {
            let mut orders = self.orders.write().await;
            for order in &outcome.orders {
                if let Some(existing) = orders.iter_mut().find(|stored| stored.id == order.id) {
                    *existing = order.clone();
                } else {
                    orders.push(order.clone());
                }
            }
            orders.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        }
        // Upsert positions.
        {
            let mut positions = self.positions.write().await;
            for position in &outcome.positions {
                positions.insert(
                    (position.account_id.clone(), position.token_id.clone()),
                    position.clone(),
                );
            }
        }
        // Update account state.
        {
            *self.account_state.write().await = Some(outcome.account.clone());
        }
        // Append events.
        {
            let mut events = self.events.write().await;
            events.extend(outcome.events.iter().cloned());
            events.sort_by(|left, right| right.created_at.cmp(&left.created_at));
            events.truncate(1_000);
        }
        Ok(())
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
                order.status = CopyOrderStatus::Cancelled;
                order.reason = reason.to_string();
                order.updated_at = now;
                cancelled += 1;
            }
        }
        Ok(cancelled)
    }

    async fn reset_simulation(
        &self,
        config: &CopyTradeConfig,
        _trace_id: &str,
    ) -> Result<()> {
        self.orders.write().await.clear();
        self.positions.write().await.clear();
        *self.account_state.write().await = Some(CopyAccountState::fresh(
            &config.account_id,
            config.account_capital_usd,
            OffsetDateTime::now_utc(),
        ));
        Ok(())
    }
}
