#[async_trait]
pub trait RewardBotStore: Send + Sync {
    async fn load_config(&self) -> Result<RewardBotConfig>;
    async fn save_config(&self, config: &RewardBotConfig) -> Result<()>;
    async fn enqueue_control_command(&self, command: RewardControlCommand) -> Result<()>;
    async fn claim_next_control_command(
        &self,
        trace_id: &str,
        now: OffsetDateTime,
    ) -> Result<Option<RewardControlCommand>>;
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
    async fn upsert_markets(&self, markets: &[RewardMarket]) -> Result<()>;
    /// Replace the current rewards quote plan snapshot.
    async fn save_quote_plans(&self, plans: &[RewardQuotePlan]) -> Result<()>;
    async fn list_markets(&self, limit: u16) -> Result<Vec<RewardMarket>>;
    /// List all active markets without a row limit for explicit catalog exports.
    async fn list_all_active_markets(&self) -> Result<Vec<RewardMarket>>;
    /// Count active markets and return their latest update timestamp without loading rows.
    async fn active_market_summary(&self) -> Result<(usize, Option<OffsetDateTime>)>;
    /// List all quote plans without a row limit (used by snapshot).
    async fn list_all_quote_plans(&self) -> Result<Vec<RewardQuotePlan>>;
    async fn list_orders_page(&self, query: &RewardOrderListQuery) -> Result<RewardOrderPage>;
    async fn list_positions(&self, limit: u16) -> Result<Vec<RewardPosition>>;
    async fn list_events(&self, limit: u16) -> Result<Vec<RewardRiskEvent>>;
    async fn log_event(&self, event: RewardRiskEvent) -> Result<()>;

    /// Load the validation fund-pool ledger, seeding a fresh one from `config` if absent.
    async fn load_account_state(&self, config: &RewardBotConfig) -> Result<RewardAccountState>;
    /// Currently open-like orders for an account (planned/open/exit_pending).
    async fn list_open_orders(&self, account_id: &str) -> Result<Vec<ManagedRewardOrder>>;
    /// Lookup a managed rewards order by its external Polymarket order id.
    async fn get_order_by_external_order_id(
        &self,
        external_order_id: &str,
    ) -> Result<Option<ManagedRewardOrder>>;
    /// Non-zero inventory for an account.
    async fn list_account_positions(&self, account_id: &str) -> Result<Vec<RewardPosition>>;
    async fn list_fills(&self, limit: u16) -> Result<Vec<RewardFill>>;
    async fn reward_fill_exists(&self, fill_id: &str) -> Result<bool>;
    /// Persist a full validation/live local-state tick atomically.
    async fn apply_simulation_tick(
        &self,
        outcome: &RewardSimulationOutcome,
        trace_id: &str,
    ) -> Result<()>;
    /// Reset validation state: cancel orders, clear fills/positions, reset the ledger to capital.
    async fn reset_simulation(&self, config: &RewardBotConfig, trace_id: &str) -> Result<()>;
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

    pub async fn enqueue_control_command(
        &self,
        action: RewardControlAction,
        reason: &str,
        trace_id: &str,
    ) -> Result<RewardControlCommand> {
        let config = self.read_config().await?;
        let now = OffsetDateTime::now_utc();
        let command = RewardControlCommand {
            id: reward_control_command_id(trace_id),
            action,
            account_id: Some(config.account_id.clone()),
            reason: reason.to_string(),
            status: RewardControlCommandStatus::Pending,
            requested_at: now,
            started_at: None,
            completed_at: None,
            trace_id: Some(trace_id.to_string()),
            error: None,
        };

        self.store.enqueue_control_command(command.clone()).await?;
        self.store
            .log_event(new_risk_event(
                Some(config.account_id),
                None,
                None,
                "reward_control_command_queued",
                RewardRiskSeverity::Info,
                format!("Queued rewards control command: {}", action.as_str()),
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
    ) -> Result<Option<RewardControlCommand>> {
        self.store
            .claim_next_control_command(trace_id, OffsetDateTime::now_utc())
            .await
    }

    pub async fn complete_control_command(
        &self,
        command: &RewardControlCommand,
        trace_id: &str,
    ) -> Result<()> {
        self.store
            .complete_control_command(&command.id, trace_id, OffsetDateTime::now_utc())
            .await?;
        self.store
            .log_event(new_risk_event(
                command.account_id.clone(),
                None,
                None,
                "reward_control_command_completed",
                RewardRiskSeverity::Info,
                format!("Completed rewards control command: {}", command.action.as_str()),
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
        command: &RewardControlCommand,
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
            .log_event(new_risk_event(
                command.account_id.clone(),
                None,
                None,
                "reward_control_command_failed",
                RewardRiskSeverity::Critical,
                format!(
                    "Failed rewards control command {}: {error_message}",
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

    /// Persist reward markets fetched by the sync worker.
    pub async fn upsert_reward_markets(&self, markets: &[RewardMarket]) -> Result<()> {
        self.store.upsert_markets(markets).await
    }

    /// List all active reward markets from the database.
    pub async fn list_active_reward_markets(&self) -> Result<Vec<RewardMarket>> {
        self.store.list_all_active_markets().await
    }

    /// List a bounded candidate pool for one rewards strategy tick.
    pub async fn list_reward_run_candidate_markets(&self) -> Result<Vec<RewardMarket>> {
        let config = self.read_config().await?;
        let markets = self
            .store
            .list_markets(reward_run_market_limit(&config))
            .await?;
        Ok(select_reward_quote_candidate_markets(&markets, &config))
    }

    /// Return distinct token IDs from markets where the reward bot currently has
    /// open-like orders or non-zero positions. This is a much smaller set than
    /// `list_reward_run_candidate_markets` and is used by the orderbook stream to
    /// subscribe only to relevant orderbook channels.
    pub async fn list_active_reward_book_token_ids(&self) -> Result<Vec<String>> {
        let config = self.read_config().await?;
        let account_id = &config.account_id;

        let open_orders = self.store.list_open_orders(account_id).await?;
        let positions = self.store.list_account_positions(account_id).await?;

        let mut seen = HashSet::new();
        let mut token_ids = Vec::new();

        for order in &open_orders {
            if order.token_id.trim().is_empty() || !seen.insert(order.token_id.clone()) {
                continue;
            }
            token_ids.push(order.token_id.clone());
        }
        for position in &positions {
            if position.token_id.trim().is_empty() || !seen.insert(position.token_id.clone()) {
                continue;
            }
            token_ids.push(position.token_id.clone());
        }

        Ok(token_ids)
    }

    /// Return distinct token IDs from **all** reward candidate markets, regardless
    /// of whether the bot currently has orders or positions. This breaks the cold
    /// start loop: the orderbook stream can subscribe to reward market tokens even
    /// before the bot has placed its first order.
    pub async fn list_all_reward_candidate_token_ids(&self) -> Result<Vec<String>> {
        let config = self.read_config().await?;
        let limit = reward_run_market_limit(&config);
        let markets = self.store.list_markets(limit).await?;
        let candidates = select_reward_quote_candidate_markets(&markets, &config);
        Ok(select_reward_book_token_ids(&candidates))
    }

    pub async fn snapshot(&self) -> Result<RewardBotSnapshot> {
        self.snapshot_with_order_query(&RewardOrderListQuery::default())
            .await
    }

    pub async fn snapshot_with_order_query(
        &self,
        order_query: &RewardOrderListQuery,
    ) -> Result<RewardBotSnapshot> {
        let config = self.read_config().await?;
        let account = self.store.load_account_state(&config).await?;
        let (markets_tracked, last_scan_at) = self.store.active_market_summary().await?;
        let quote_plans = self.store.list_all_quote_plans().await?;
        let orders = self.store.list_orders_page(order_query).await?;
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
                markets_tracked,
                eligible_markets: quote_plans.iter().filter(|plan| plan.eligible).count(),
                open_orders: open_order_count,
                positions: position_count,
                last_scan_at,
                last_run_at,
                error,
            },
            config,
            account,
            markets: Vec::new(),
            quote_plans,
            orders: orders.items,
            orders_page: orders.page,
            positions,
            fills,
            events,
        })
    }

    pub async fn prepare_live_cycle(
        &self,
        markets: Vec<RewardMarket>,
        books: HashMap<String, RewardOrderBook>,
        trace_id: &str,
        force_orders: bool,
    ) -> Result<RewardLiveCycle> {
        let config = self.read_config().await?;
        let plans = build_reward_quote_plans(&markets, &books, &config);
        // NOTE: Do NOT call upsert_markets() here.  The full reward-market catalog is
        // synced by the orderbook service (every 5 min).  Calling upsert_markets() with
        // only the filtered candidate subset would deactivate all other active markets
        // and collapse markets_tracked from ~10k down to the candidate count.
        self.store.save_quote_plans(&plans).await?;

        let account = self.store.load_account_state(&config).await?;
        let open_orders = self.store.list_open_orders(&account.account_id).await?;
        let positions = self
            .store
            .list_account_positions(&account.account_id)
            .await?;
        let should_execute = config.enabled || force_orders;

        self.store
            .log_event(new_risk_event(
                Some(config.account_id.clone()),
                None,
                None,
                "reward_bot_live_plan_built",
                RewardRiskSeverity::Info,
                "Prepared rewards live order plan.",
                json!({
                    "trace_id": trace_id,
                    "execution_mode": config.execution_mode.as_str(),
                    "markets_scanned": markets.len(),
                    "books_fetched": books.len(),
                    "plans_built": plans.len(),
                    "eligible_plans": plans.iter().filter(|plan| plan.eligible).count(),
                    "should_execute": should_execute,
                }),
            ))
            .await?;

        Ok(RewardLiveCycle {
            config,
            account,
            markets,
            plans,
            open_orders,
            positions,
            should_execute,
        })
    }

    /// Load mutable live state for sync/cancel/reconcile paths without scanning
    /// the full reward market catalog. Full ticks pass candidate markets through
    /// `prepare_live_cycle`.
    pub async fn current_live_cycle_state(&self) -> Result<RewardLiveCycle> {
        let config = self.read_config().await?;
        let account = self.store.load_account_state(&config).await?;
        let open_orders = self.store.list_open_orders(&account.account_id).await?;
        let positions = self
            .store
            .list_account_positions(&account.account_id)
            .await?;
        let plans = self.store.list_all_quote_plans().await?;
        Ok(RewardLiveCycle {
            should_execute: config.enabled,
            config,
            account,
            markets: Vec::new(),
            plans,
            open_orders,
            positions,
        })
    }

    pub async fn get_managed_order_by_external_order_id(
        &self,
        external_order_id: &str,
    ) -> Result<Option<ManagedRewardOrder>> {
        self.store
            .get_order_by_external_order_id(external_order_id)
            .await
    }

    pub async fn reward_fill_exists(&self, fill_id: &str) -> Result<bool> {
        self.store.reward_fill_exists(fill_id).await
    }

    pub async fn record_live_reset_cancel_all(&self, trace_id: &str) -> Result<()> {
        let config = self.read_config().await?;
        self.store
            .log_event(new_risk_event(
                Some(config.account_id.clone()),
                None,
                None,
                "reward_bot_live_reset_cancel_all",
                RewardRiskSeverity::Info,
                "Reset rewards live bot by cancelling managed live orders without clearing local ledger.",
                json!({ "trace_id": trace_id }),
            ))
            .await
    }

    pub async fn apply_live_tick_outcome(
        &self,
        outcome: &RewardSimulationOutcome,
        trace_id: &str,
    ) -> Result<()> {
        self.store.apply_simulation_tick(outcome, trace_id).await
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
                "Reset rewards validation account, orders, positions and fills.",
                json!({ "trace_id": trace_id, "capital_usd": config.account_capital_usd }),
            ))
            .await
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

    market_limit.max(order_limit).max(DEFAULT_LIST_LIMIT)
}

fn reward_control_command_id(trace_id: &str) -> String {
    format!("rewcmd_{}", trace_id.trim_start_matches("trc_"))
}
