// In-memory `RewardBotStore` implementation backing tests and the no-database local path.

pub struct InMemoryRewardBotStore {
    config: RwLock<RewardBotConfig>,
    markets: RwLock<HashMap<String, RewardMarket>>,
    quote_plans: RwLock<HashMap<String, RewardQuotePlan>>,
    orders: RwLock<Vec<ManagedRewardOrder>>,
    positions: RwLock<HashMap<(String, String), RewardPosition>>,
    events: RwLock<Vec<RewardRiskEvent>>,
    account_state: RwLock<Option<RewardAccountState>>,
    fills: RwLock<Vec<RewardFill>>,
}

impl InMemoryRewardBotStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: RwLock::new(RewardBotConfig::default()),
            markets: RwLock::new(HashMap::new()),
            quote_plans: RwLock::new(HashMap::new()),
            orders: RwLock::new(Vec::new()),
            positions: RwLock::new(HashMap::new()),
            events: RwLock::new(Vec::new()),
            account_state: RwLock::new(None),
            fills: RwLock::new(Vec::new()),
        }
    }
}

#[async_trait]
impl RewardBotStore for InMemoryRewardBotStore {
    async fn load_config(&self) -> Result<RewardBotConfig> {
        Ok(self.config.read().await.clone().normalized())
    }

    async fn save_config(&self, config: &RewardBotConfig) -> Result<()> {
        *self.config.write().await = config.clone().normalized();
        Ok(())
    }

    async fn upsert_markets(&self, markets: &[RewardMarket]) -> Result<()> {
        let mut store = self.markets.write().await;
        for market in markets {
            store.insert(market.condition_id.clone(), market.clone());
        }
        Ok(())
    }

    async fn save_quote_plans(&self, plans: &[RewardQuotePlan]) -> Result<()> {
        let mut store = self.quote_plans.write().await;
        for plan in plans {
            store.insert(plan.condition_id.clone(), plan.clone());
        }
        Ok(())
    }

    async fn replace_simulated_orders(
        &self,
        account_id: &str,
        orders: &[ManagedRewardOrder],
        _trace_id: &str,
    ) -> Result<usize> {
        let now = OffsetDateTime::now_utc();
        let mut store = self.orders.write().await;
        let mut cancelled = 0;
        for order in store.iter_mut() {
            if order.account_id == account_id && order.status.is_open_like() {
                order.status = ManagedRewardOrderStatus::Cancelled;
                order.reason = "replaced by latest rewards simulation".to_string();
                order.updated_at = now;
                cancelled += 1;
            }
        }
        store.extend(orders.iter().cloned());
        store.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(cancelled)
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
                order.status = ManagedRewardOrderStatus::Cancelled;
                order.reason = reason.to_string();
                order.updated_at = now;
                cancelled += 1;
            }
        }
        Ok(cancelled)
    }

    async fn list_markets(&self, limit: u16) -> Result<Vec<RewardMarket>> {
        let mut markets = self
            .markets
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        markets.sort_by(|left, right| {
            right
                .total_daily_rate
                .cmp(&left.total_daily_rate)
                .then_with(|| right.updated_at.cmp(&left.updated_at))
        });
        markets.truncate(usize::from(limit));
        Ok(markets)
    }

    async fn list_all_active_markets(&self) -> Result<Vec<RewardMarket>> {
        let mut markets = self
            .markets
            .read()
            .await
            .values()
            .filter(|market| market.active)
            .cloned()
            .collect::<Vec<_>>();
        markets.sort_by(|left, right| {
            right
                .total_daily_rate
                .cmp(&left.total_daily_rate)
                .then_with(|| right.updated_at.cmp(&left.updated_at))
        });
        Ok(markets)
    }

    async fn list_all_quote_plans(&self) -> Result<Vec<RewardQuotePlan>> {
        let mut plans = self
            .quote_plans
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        plans.sort_by(|left, right| {
            right
                .eligible
                .cmp(&left.eligible)
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| right.updated_at.cmp(&left.updated_at))
        });
        Ok(plans)
    }

    async fn list_quote_plans(&self, limit: u16) -> Result<Vec<RewardQuotePlan>> {
        let mut plans = self
            .quote_plans
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        plans.sort_by(|left, right| {
            right
                .eligible
                .cmp(&left.eligible)
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| right.updated_at.cmp(&left.updated_at))
        });
        plans.truncate(usize::from(limit));
        Ok(plans)
    }

    async fn list_orders(&self, limit: u16) -> Result<Vec<ManagedRewardOrder>> {
        let mut orders = self.orders.read().await.clone();
        orders.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        orders.truncate(usize::from(limit));
        Ok(orders)
    }

    async fn list_positions(&self, limit: u16) -> Result<Vec<RewardPosition>> {
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

    async fn list_events(&self, limit: u16) -> Result<Vec<RewardRiskEvent>> {
        let mut events = self.events.read().await.clone();
        events.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        events.truncate(usize::from(limit));
        Ok(events)
    }

    async fn log_event(&self, event: RewardRiskEvent) -> Result<()> {
        let mut events = self.events.write().await;
        events.push(event);
        events.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        events.truncate(1_000);
        Ok(())
    }

    async fn load_account_state(&self, config: &RewardBotConfig) -> Result<RewardAccountState> {
        let mut guard = self.account_state.write().await;
        if let Some(state) = guard.as_ref() {
            return Ok(state.clone());
        }
        let state = RewardAccountState::fresh(
            &config.account_id,
            config.account_capital_usd,
            OffsetDateTime::now_utc(),
        );
        *guard = Some(state.clone());
        Ok(state)
    }

    async fn list_open_orders(&self, account_id: &str) -> Result<Vec<ManagedRewardOrder>> {
        Ok(self
            .orders
            .read()
            .await
            .iter()
            .filter(|order| order.account_id == account_id && order.status.is_open_like())
            .cloned()
            .collect())
    }

    async fn list_account_positions(&self, account_id: &str) -> Result<Vec<RewardPosition>> {
        Ok(self
            .positions
            .read()
            .await
            .values()
            .filter(|position| position.account_id == account_id && position.size != Decimal::ZERO)
            .cloned()
            .collect())
    }

    async fn list_fills(&self, limit: u16) -> Result<Vec<RewardFill>> {
        let mut fills = self.fills.read().await.clone();
        fills.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        fills.truncate(usize::from(limit));
        Ok(fills)
    }

    async fn apply_simulation_tick(
        &self,
        outcome: &RewardSimulationOutcome,
        _trace_id: &str,
    ) -> Result<()> {
        {
            let mut markets = self.markets.write().await;
            for market in &outcome.markets {
                markets.insert(market.condition_id.clone(), market.clone());
            }
        }
        {
            let mut plans = self.quote_plans.write().await;
            for plan in &outcome.plans {
                plans.insert(plan.condition_id.clone(), plan.clone());
            }
        }
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
        {
            let mut positions = self.positions.write().await;
            for position in &outcome.positions {
                positions.insert(
                    (position.account_id.clone(), position.token_id.clone()),
                    position.clone(),
                );
            }
        }
        {
            let mut fills = self.fills.write().await;
            fills.extend(outcome.fills.iter().cloned());
            fills.sort_by(|left, right| right.created_at.cmp(&left.created_at));
            fills.truncate(5_000);
        }
        {
            *self.account_state.write().await = Some(outcome.account.clone());
        }
        {
            let mut events = self.events.write().await;
            events.extend(outcome.events.iter().cloned());
            events.sort_by(|left, right| right.created_at.cmp(&left.created_at));
            events.truncate(1_000);
        }
        Ok(())
    }

    async fn reset_simulation(&self, config: &RewardBotConfig, _trace_id: &str) -> Result<()> {
        self.orders.write().await.clear();
        self.positions.write().await.clear();
        self.fills.write().await.clear();
        *self.account_state.write().await = Some(RewardAccountState::fresh(
            &config.account_id,
            config.account_capital_usd,
            OffsetDateTime::now_utc(),
        ));
        Ok(())
    }
}
