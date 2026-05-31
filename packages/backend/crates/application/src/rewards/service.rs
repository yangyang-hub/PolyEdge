#[async_trait]
pub trait RewardBotStore: Send + Sync {
    async fn load_config(&self) -> Result<RewardBotConfig>;
    async fn save_config(&self, config: &RewardBotConfig) -> Result<()>;
    async fn upsert_markets(&self, markets: &[RewardMarket]) -> Result<()>;
    /// Replace the current rewards quote plan snapshot.
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
    /// List all active markets without a row limit (used by snapshot).
    async fn list_all_active_markets(&self) -> Result<Vec<RewardMarket>>;
    async fn list_quote_plans(&self, limit: u16) -> Result<Vec<RewardQuotePlan>>;
    /// List all quote plans without a row limit (used by snapshot).
    async fn list_all_quote_plans(&self) -> Result<Vec<RewardQuotePlan>>;
    async fn list_orders(&self, limit: u16) -> Result<Vec<ManagedRewardOrder>>;
    async fn list_positions(&self, limit: u16) -> Result<Vec<RewardPosition>>;
    async fn list_events(&self, limit: u16) -> Result<Vec<RewardRiskEvent>>;
    async fn log_event(&self, event: RewardRiskEvent) -> Result<()>;

    /// Load the simulated fund-pool ledger, seeding a fresh one from `config` if absent.
    async fn load_account_state(&self, config: &RewardBotConfig) -> Result<RewardAccountState>;
    /// Currently open-like orders for an account (planned/open/exit_pending).
    async fn list_open_orders(&self, account_id: &str) -> Result<Vec<ManagedRewardOrder>>;
    /// Non-zero inventory for an account.
    async fn list_account_positions(&self, account_id: &str) -> Result<Vec<RewardPosition>>;
    async fn list_fills(&self, limit: u16) -> Result<Vec<RewardFill>>;
    /// Persist a full simulation tick (orders, fills, positions, ledger, events) atomically.
    async fn apply_simulation_tick(
        &self,
        outcome: &RewardSimulationOutcome,
        trace_id: &str,
    ) -> Result<()>;
    /// Reset the simulation: cancel orders, clear fills/positions, reset the ledger to capital.
    async fn reset_simulation(&self, config: &RewardBotConfig, trace_id: &str) -> Result<()>;
}

#[derive(Clone)]
pub struct RewardBotService {
    store: Arc<dyn RewardBotStore>,
    mode_store: Arc<dyn ModeStateStore>,
}

impl RewardBotService {
    #[must_use]
    pub fn new(store: Arc<dyn RewardBotStore>, mode_store: Arc<dyn ModeStateStore>) -> Self {
        Self { store, mode_store }
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

    /// Persist reward markets fetched by the sync worker.
    pub async fn upsert_reward_markets(&self, markets: &[RewardMarket]) -> Result<()> {
        self.store.upsert_markets(markets).await
    }

    /// List all active reward markets from the database.
    pub async fn list_active_reward_markets(&self) -> Result<Vec<RewardMarket>> {
        self.store.list_all_active_markets().await
    }

    /// List a bounded candidate pool for one rewards simulation tick.
    pub async fn list_reward_run_candidate_markets(&self) -> Result<Vec<RewardMarket>> {
        let config = self.read_config().await?;
        let markets = self
            .store
            .list_markets(reward_run_market_limit(&config))
            .await?;
        Ok(select_reward_quote_candidate_markets(&markets, &config))
    }

    pub async fn snapshot(&self) -> Result<RewardBotSnapshot> {
        let config = self.read_config().await?;
        let account = self.store.load_account_state(&config).await?;
        let markets = self.store.list_all_active_markets().await?;
        let quote_plans = self.store.list_all_quote_plans().await?;
        let orders = self.store.list_orders(200).await?;
        let positions = self.store.list_positions(200).await?;
        let open_order_count = self
            .store
            .list_open_orders(&account.account_id)
            .await?
            .len();
        let position_count = self
            .store
            .list_account_positions(&account.account_id)
            .await?
            .len();
        let fills = self.store.list_fills(200).await?;
        let events = self.store.list_events(100).await?;
        let last_scan_at = markets.iter().map(|market| market.updated_at).max();
        let last_run_at = quote_plans.iter().map(|plan| plan.updated_at).max();
        let error = events
            .iter()
            .find(|event| event.severity == RewardRiskSeverity::Critical)
            .map(|event| event.message.clone());

        Ok(RewardBotSnapshot {
            status: RewardBotStatus {
                enabled: config.enabled,
                running: config.enabled,
                account_id: config.account_id.clone(),
                markets_tracked: markets.len(),
                eligible_markets: quote_plans.iter().filter(|plan| plan.eligible).count(),
                open_orders: open_order_count,
                positions: position_count,
                last_scan_at,
                last_run_at,
                error,
            },
            config,
            account,
            markets,
            quote_plans,
            orders,
            positions,
            fills,
            events,
        })
    }

    pub async fn run_simulation(
        &self,
        markets: Vec<RewardMarket>,
        books: HashMap<String, RewardOrderBook>,
        trace_id: &str,
    ) -> Result<RewardBotRunReport> {
        self.run_simulation_inner(markets, books, trace_id, false)
            .await
    }

    pub async fn run_simulation_forced(
        &self,
        markets: Vec<RewardMarket>,
        books: HashMap<String, RewardOrderBook>,
        trace_id: &str,
    ) -> Result<RewardBotRunReport> {
        self.run_simulation_inner(markets, books, trace_id, true)
            .await
    }

    async fn run_simulation_inner(
        &self,
        markets: Vec<RewardMarket>,
        books: HashMap<String, RewardOrderBook>,
        trace_id: &str,
        force_orders: bool,
    ) -> Result<RewardBotRunReport> {
        let config = self.read_config().await?;

        // When the bot is neither enabled nor manually triggered, just refresh
        // the market scan + quote plans without touching orders or the ledger.
        if !config.enabled && !force_orders {
            let plans = build_reward_quote_plans(&markets, &books, &config);
            let eligible_plans = plans.iter().filter(|plan| plan.eligible).count();
            self.store.upsert_markets(&markets).await?;
            self.store.save_quote_plans(&plans).await?;
            self.log_run_summary(
                &config,
                trace_id,
                markets.len(),
                books.len(),
                &plans,
                0,
                0,
                0,
            )
            .await?;
            return Ok(RewardBotRunReport {
                markets_scanned: markets.len(),
                books_fetched: books.len(),
                plans_built: plans.len(),
                eligible_plans,
                simulated_orders: 0,
                cancelled_orders: 0,
                filled_orders: 0,
                reward_accrued: Decimal::ZERO,
            });
        }

        // Check global system mode — live trading is not wired yet.
        let system_mode = self.mode_store.current().await?.mode;
        if system_mode == SystemMode::LiveAuto {
            self.store
                .log_event(new_risk_event(
                    Some(config.account_id.clone()),
                    None,
                    None,
                    "reward_bot_live_auto_unsupported",
                    RewardRiskSeverity::Warning,
                    "Global mode is live_auto but rewards bot live trading is not wired yet; running simulation.",
                    json!({ "trace_id": trace_id, "system_mode": system_mode.as_str() }),
                ))
                .await?;
        }

        let account = self.store.load_account_state(&config).await?;
        let open_orders = self.store.list_open_orders(&account.account_id).await?;
        let positions = self
            .store
            .list_account_positions(&account.account_id)
            .await?;
        let elapsed_seconds = (OffsetDateTime::now_utc() - account.updated_at).whole_seconds();

        let outcome = run_reward_simulation_tick(
            &config,
            account,
            open_orders,
            positions,
            &markets,
            &books,
            elapsed_seconds,
            trace_id,
        );
        let report = outcome.report.clone();
        self.store.apply_simulation_tick(&outcome, trace_id).await?;

        self.log_run_summary(
            &config,
            trace_id,
            markets.len(),
            books.len(),
            &outcome.plans,
            report.simulated_orders,
            report.cancelled_orders,
            report.filled_orders,
        )
        .await?;

        Ok(report)
    }

    #[allow(clippy::too_many_arguments)]
    async fn log_run_summary(
        &self,
        config: &RewardBotConfig,
        trace_id: &str,
        markets_scanned: usize,
        books_fetched: usize,
        plans: &[RewardQuotePlan],
        placed: usize,
        cancelled: usize,
        filled: usize,
    ) -> Result<()> {
        self.store
            .log_event(new_risk_event(
                Some(config.account_id.clone()),
                None,
                None,
                "reward_bot_simulation_run",
                RewardRiskSeverity::Info,
                "Completed rewards simulation tick.",
                json!({
                    "trace_id": trace_id,
                    "markets_scanned": markets_scanned,
                    "books_fetched": books_fetched,
                    "plans_built": plans.len(),
                    "eligible_plans": plans.iter().filter(|plan| plan.eligible).count(),
                    "placed_orders": placed,
                    "cancelled_orders": cancelled,
                    "filled_orders": filled,
                }),
            ))
            .await
    }

    pub async fn reset_simulation(&self, trace_id: &str) -> Result<()> {
        let config = self.read_config().await?;
        self.store.reset_simulation(&config, trace_id).await?;
        self.store
            .log_event(new_risk_event(
                Some(config.account_id.clone()),
                None,
                None,
                "reward_bot_reset",
                RewardRiskSeverity::Info,
                "Reset rewards simulation account, orders, positions and fills.",
                json!({ "trace_id": trace_id, "capital_usd": config.account_capital_usd }),
            ))
            .await
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

fn reward_run_market_limit(config: &RewardBotConfig) -> u16 {
    let market_limit = if config.max_markets == 0 {
        DEFAULT_LIST_LIMIT
    } else {
        config.max_markets.saturating_mul(20)
    };
    let order_limit = if config.max_open_orders == 0 {
        DEFAULT_LIST_LIMIT
    } else {
        config.max_open_orders.saturating_mul(10)
    };

    market_limit
        .max(order_limit)
        .max(DEFAULT_LIST_LIMIT)
        .min(MAX_LIST_LIMIT)
}
