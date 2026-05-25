#[async_trait]
pub trait RewardBotStore: Send + Sync {
    async fn load_config(&self) -> Result<RewardBotConfig>;
    async fn save_config(&self, config: &RewardBotConfig) -> Result<()>;
    async fn upsert_markets(&self, markets: &[RewardMarket]) -> Result<()>;
    async fn save_quote_plans(&self, plans: &[RewardQuotePlan]) -> Result<()>;
    async fn replace_simulated_orders(
        &self,
        account_id: &str,
        orders: &[ManagedRewardOrder],
        trace_id: &str,
    ) -> Result<usize>;
    async fn cancel_open_orders(
        &self,
        account_id: Option<&str>,
        reason: &str,
        trace_id: &str,
    ) -> Result<usize>;
    async fn list_markets(&self, limit: u16) -> Result<Vec<RewardMarket>>;
    async fn list_quote_plans(&self, limit: u16) -> Result<Vec<RewardQuotePlan>>;
    async fn list_orders(&self, limit: u16) -> Result<Vec<ManagedRewardOrder>>;
    async fn list_positions(&self, limit: u16) -> Result<Vec<RewardPosition>>;
    async fn list_events(&self, limit: u16) -> Result<Vec<RewardRiskEvent>>;
    async fn log_event(&self, event: RewardRiskEvent) -> Result<()>;
}

#[derive(Clone)]
pub struct RewardBotService {
    store: Arc<dyn RewardBotStore>,
}

impl RewardBotService {
    #[must_use]
    pub fn new(store: Arc<dyn RewardBotStore>) -> Self {
        Self { store }
    }

    pub async fn read_config(&self) -> Result<RewardBotConfig> {
        self.store
            .load_config()
            .await
            .map(RewardBotConfig::normalized)
    }

    pub async fn update_config(&self, patch: RewardBotConfigPatch) -> Result<RewardBotConfig> {
        let current = self.read_config().await?;
        let next = current.apply_patch(patch);
        self.store.save_config(&next).await?;
        Ok(next)
    }

    pub async fn snapshot(&self) -> Result<RewardBotSnapshot> {
        let config = self.read_config().await?;
        let markets = self.store.list_markets(DEFAULT_LIST_LIMIT).await?;
        let quote_plans = self.store.list_quote_plans(DEFAULT_LIST_LIMIT).await?;
        let orders = self.store.list_orders(200).await?;
        let positions = self.store.list_positions(200).await?;
        let events = self.store.list_events(100).await?;
        let last_scan_at = markets.iter().map(|market| market.updated_at).max();
        let last_run_at = quote_plans.iter().map(|plan| plan.updated_at).max();
        let open_orders = orders
            .iter()
            .filter(|order| order.status.is_open_like())
            .count();
        let error = events
            .iter()
            .find(|event| event.severity == RewardRiskSeverity::Critical)
            .map(|event| event.message.clone());

        Ok(RewardBotSnapshot {
            status: RewardBotStatus {
                enabled: config.enabled,
                running: config.enabled,
                mode: config.mode,
                account_id: config.account_id.clone(),
                markets_tracked: markets.len(),
                eligible_markets: quote_plans.iter().filter(|plan| plan.eligible).count(),
                open_orders,
                positions: positions.len(),
                last_scan_at,
                last_run_at,
                error,
            },
            config,
            markets,
            quote_plans,
            orders,
            positions,
            events,
        })
    }

    pub async fn run_simulation(
        &self,
        markets: Vec<RewardMarket>,
        books: HashMap<String, RewardOrderBook>,
        trace_id: &str,
    ) -> Result<RewardBotRunReport> {
        let config = self.read_config().await?;
        let plans = build_reward_quote_plans(&markets, &books, &config);
        let eligible_plans = plans.iter().filter(|plan| plan.eligible).count();

        self.store.upsert_markets(&markets).await?;
        self.store.save_quote_plans(&plans).await?;

        let mut cancelled_orders = 0;
        let mut simulated_orders = 0;

        if config.enabled {
            if config.mode == RewardBotMode::Live {
                self.store
                    .log_event(new_risk_event(
                        Some(config.account_id.clone()),
                        None,
                        None,
                        "reward_bot_live_unsupported",
                        RewardRiskSeverity::Warning,
                        "Rewards bot live mode is not wired in PolyEdge yet; generated a simulation instead.",
                        json!({ "trace_id": trace_id }),
                    ))
                    .await?;
            }

            let orders = build_simulated_orders(&config, &plans, trace_id);
            cancelled_orders = self
                .store
                .replace_simulated_orders(&config.account_id, &orders, trace_id)
                .await?;
            simulated_orders = orders.len();
        }

        self.store
            .log_event(new_risk_event(
                Some(config.account_id.clone()),
                None,
                None,
                "reward_bot_simulation_run",
                RewardRiskSeverity::Info,
                "Completed rewards quote-plan simulation.",
                json!({
                    "trace_id": trace_id,
                    "markets_scanned": markets.len(),
                    "books_fetched": books.len(),
                    "plans_built": plans.len(),
                    "eligible_plans": eligible_plans,
                    "simulated_orders": simulated_orders,
                    "cancelled_orders": cancelled_orders,
                }),
            ))
            .await?;

        Ok(RewardBotRunReport {
            markets_scanned: markets.len(),
            books_fetched: books.len(),
            plans_built: plans.len(),
            eligible_plans,
            simulated_orders,
            cancelled_orders,
        })
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
            .log_event(new_risk_event(
                account_id.map(str::to_string),
                None,
                None,
                "reward_bot_cancel_all",
                RewardRiskSeverity::Info,
                reason,
                json!({ "trace_id": trace_id, "cancelled_orders": cancelled }),
            ))
            .await?;
        Ok(cancelled)
    }
}
