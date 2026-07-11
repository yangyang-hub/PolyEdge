// In-memory `RewardBotStore` implementation backing tests and the no-database local path.

pub struct InMemoryRewardBotStore {
    config: RwLock<RewardBotConfig>,
    markets: RwLock<HashMap<String, RewardMarket>>,
    event_windows: RwLock<HashMap<(String, String), RewardMarketEventWindow>>,
    quote_plans: RwLock<HashMap<(String, RewardStrategyProfile), RewardQuotePlan>>,
    orders: RwLock<Vec<ManagedRewardOrder>>,
    positions: RwLock<HashMap<(String, String), RewardPosition>>,
    events: RwLock<Vec<RewardRiskEvent>>,
    account_state: RwLock<Option<RewardAccountState>>,
    fills: RwLock<Vec<RewardFill>>,
    merge_intents: RwLock<Vec<RewardMergeIntent>>,
    control_commands: RwLock<Vec<RewardControlCommand>>,
    worker_heartbeats: RwLock<HashMap<String, OffsetDateTime>>,
    advisories: RwLock<Vec<RewardMarketAdvisory>>,
    info_risks: RwLock<Vec<RewardMarketInfoRisk>>,
    llm_calls: RwLock<Vec<RewardLlmCallRecord>>,
    candles: RwLock<HashMap<(String, i32, OffsetDateTime), RewardMarketCandle>>,
    fair_values: RwLock<Vec<RewardFairValueEstimate>>,
    strategy_runs: RwLock<Vec<RewardStrategyRun>>,
    strategy_decisions: RwLock<Vec<RewardStrategyDecision>>,
    strategy_actions: RwLock<Vec<RewardStrategyAction>>,
    strategy_replay_fixtures: RwLock<HashMap<i64, RewardStrategyReplayFixture>>,
    order_transitions: RwLock<Vec<RewardOrderTransition>>,
    next_strategy_run_id: RwLock<i64>,
    next_strategy_action_id: RwLock<i64>,
    next_order_transition_id: RwLock<i64>,
}

impl InMemoryRewardBotStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: RwLock::new(RewardBotConfig::default()),
            markets: RwLock::new(HashMap::new()),
            event_windows: RwLock::new(HashMap::new()),
            quote_plans: RwLock::new(HashMap::new()),
            orders: RwLock::new(Vec::new()),
            positions: RwLock::new(HashMap::new()),
            events: RwLock::new(Vec::new()),
            account_state: RwLock::new(None),
            fills: RwLock::new(Vec::new()),
            merge_intents: RwLock::new(Vec::new()),
            control_commands: RwLock::new(Vec::new()),
            worker_heartbeats: RwLock::new(HashMap::new()),
            advisories: RwLock::new(Vec::new()),
            info_risks: RwLock::new(Vec::new()),
            llm_calls: RwLock::new(Vec::new()),
            candles: RwLock::new(HashMap::new()),
            fair_values: RwLock::new(Vec::new()),
            strategy_runs: RwLock::new(Vec::new()),
            strategy_decisions: RwLock::new(Vec::new()),
            strategy_actions: RwLock::new(Vec::new()),
            strategy_replay_fixtures: RwLock::new(HashMap::new()),
            order_transitions: RwLock::new(Vec::new()),
            next_strategy_run_id: RwLock::new(1),
            next_strategy_action_id: RwLock::new(1),
            next_order_transition_id: RwLock::new(1),
        }
    }
}

fn reward_event_window_source_priority(source: &str) -> u8 {
    match source {
        "manual" => 6,
        "official" | "sports_api" | "economic_calendar" | "earnings_calendar"
        | "governance_calendar" => 5,
        "gamma_reviewed" => 4,
        "gamma" => 3,
        "news" | "rss" => 2,
        "ai_extracted" => 1,
        _ => 0,
    }
}

fn reward_event_window_precedes(
    candidate: &RewardMarketEventWindow,
    existing: &RewardMarketEventWindow,
) -> bool {
    candidate
        .confidence
        .rank()
        .cmp(&existing.confidence.rank())
        .then_with(|| {
            reward_event_window_source_priority(&candidate.source)
                .cmp(&reward_event_window_source_priority(&existing.source))
        })
        .then_with(|| candidate.updated_at.cmp(&existing.updated_at))
        .then_with(|| candidate.source.cmp(&existing.source).reverse())
        .is_gt()
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

    async fn record_worker_heartbeat(
        &self,
        account_id: &str,
        observed_at: OffsetDateTime,
    ) -> Result<()> {
        self.worker_heartbeats
            .write()
            .await
            .insert(account_id.to_string(), observed_at);
        Ok(())
    }

    async fn latest_worker_heartbeat(
        &self,
        account_id: &str,
    ) -> Result<Option<OffsetDateTime>> {
        Ok(self.worker_heartbeats.read().await.get(account_id).copied())
    }

    async fn prune_history(&self, cutoff: OffsetDateTime) -> Result<RewardHistoryPruneReport> {
        let terminal_orders_deleted = {
            let mut orders = self.orders.write().await;
            let before = orders.len();
            orders.retain(|order| {
                !(order.updated_at < cutoff
                    && matches!(
                        order.status,
                        ManagedRewardOrderStatus::Cancelled
                            | ManagedRewardOrderStatus::Filled
                            | ManagedRewardOrderStatus::Error
                    ))
            });
            (before - orders.len()) as u64
        };

        let risk_events_deleted = {
            let mut events = self.events.write().await;
            let before = events.len();
            events.retain(|event| event.created_at >= cutoff);
            (before - events.len()) as u64
        };

        Ok(RewardHistoryPruneReport {
            terminal_orders_deleted,
            risk_events_deleted,
        })
    }

    async fn enqueue_control_command(&self, command: RewardControlCommand) -> Result<bool> {
        let mut commands = self.control_commands.write().await;
        if commands.iter().any(|existing| {
            existing.action == command.action
                && existing.account_id == command.account_id
                && matches!(
                    existing.status,
                    RewardControlCommandStatus::Pending | RewardControlCommandStatus::Running
                )
        }) {
            return Ok(false);
        }
        commands.push(command);
        commands.sort_by(|left, right| left.requested_at.cmp(&right.requested_at));
        Ok(true)
    }

    async fn claim_next_control_command(
        &self,
        trace_id: &str,
        now: OffsetDateTime,
    ) -> Result<Option<RewardControlCommand>> {
        let mut commands = self.control_commands.write().await;
        let Some(command) = commands
            .iter_mut()
            .find(|command| {
                command.status == RewardControlCommandStatus::Pending
                    || (command.status == RewardControlCommandStatus::Running
                        && command
                            .started_at
                            .is_some_and(|started_at| started_at <= now - REWARD_CONTROL_COMMAND_LEASE))
            })
        else {
            return Ok(None);
        };
        command.status = RewardControlCommandStatus::Running;
        command.started_at = Some(now);
        command.completed_at = None;
        command.trace_id = Some(trace_id.to_string());
        command.error = None;
        Ok(Some(command.clone()))
    }

    async fn complete_control_command(
        &self,
        command_id: &str,
        trace_id: &str,
        now: OffsetDateTime,
    ) -> Result<()> {
        let mut commands = self.control_commands.write().await;
        if let Some(command) = commands.iter_mut().find(|command| {
            command.id == command_id && command.status == RewardControlCommandStatus::Running
        }) {
            command.status = RewardControlCommandStatus::Completed;
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
        if let Some(command) = commands.iter_mut().find(|command| {
            command.id == command_id && command.status == RewardControlCommandStatus::Running
        }) {
            command.status = RewardControlCommandStatus::Failed;
            command.completed_at = Some(now);
            command.trace_id = Some(trace_id.to_string());
            command.error = Some(error.to_string());
        }
        Ok(())
    }

    async fn upsert_markets(&self, markets: &[RewardMarket]) -> Result<()> {
        let mut store = self.markets.write().await;
        for market in markets {
            store.insert(market.condition_id.clone(), market.clone());
        }
        Ok(())
    }

    async fn upsert_market_event_windows(
        &self,
        windows: &[RewardMarketEventWindow],
    ) -> Result<()> {
        if windows.is_empty() {
            return Ok(());
        }
        let mut store = self.event_windows.write().await;
        for window in windows {
            store.insert(
                (window.condition_id.clone(), window.source.clone()),
                window.clone(),
            );
        }
        Ok(())
    }

    async fn list_effective_market_event_windows(
        &self,
        condition_ids: &[String],
    ) -> Result<Vec<RewardMarketEventWindow>> {
        if condition_ids.is_empty() {
            return Ok(Vec::new());
        }
        let condition_ids: HashSet<&str> = condition_ids.iter().map(String::as_str).collect();
        let mut best_by_condition: HashMap<String, RewardMarketEventWindow> = HashMap::new();
        for window in self.event_windows.read().await.values() {
            if !window.active || !condition_ids.contains(window.condition_id.as_str()) {
                continue;
            }
            let replace = match best_by_condition.get(&window.condition_id) {
                Some(existing) => reward_event_window_precedes(window, existing),
                None => true,
            };
            if replace {
                best_by_condition.insert(window.condition_id.clone(), window.clone());
            }
        }
        let mut windows: Vec<_> = best_by_condition.into_values().collect();
        windows.sort_by(|left, right| left.condition_id.cmp(&right.condition_id));
        Ok(windows)
    }

    async fn start_strategy_run(&self, run: &RewardStrategyRunStart) -> Result<i64> {
        let mut next_id = self.next_strategy_run_id.write().await;
        let run_id = *next_id;
        *next_id += 1;
        self.strategy_runs.write().await.push(RewardStrategyRun {
            run_id,
            account_id: run.account_id.clone(),
            trace_id: run.trace_id.clone(),
            trigger_type: run.trigger_type,
            status: RewardStrategyRunStatus::Running,
            config_hash: run.config_hash.clone(),
            config_json: run.config_json.clone(),
            input_summary: run.input_summary.clone(),
            metrics: json!({}),
            started_at: run.started_at,
            completed_at: None,
            error_code: None,
            error_message: None,
        });
        Ok(run_id)
    }

    async fn complete_strategy_run(
        &self,
        run_id: i64,
        metrics: Value,
        completed_at: OffsetDateTime,
    ) -> Result<()> {
        if let Some(run) = self
            .strategy_runs
            .write()
            .await
            .iter_mut()
            .find(|run| run.run_id == run_id)
        {
            run.status = RewardStrategyRunStatus::Completed;
            run.metrics = metrics;
            run.completed_at = Some(completed_at);
            run.error_code = None;
            run.error_message = None;
        }
        Ok(())
    }

    async fn fail_strategy_run(
        &self,
        run_id: i64,
        error_code: &str,
        error_message: &str,
        metrics: Value,
        completed_at: OffsetDateTime,
    ) -> Result<()> {
        if let Some(run) = self
            .strategy_runs
            .write()
            .await
            .iter_mut()
            .find(|run| run.run_id == run_id)
        {
            run.status = RewardStrategyRunStatus::Failed;
            run.metrics = metrics;
            run.completed_at = Some(completed_at);
            run.error_code = Some(error_code.to_string());
            run.error_message = Some(error_message.to_string());
        }
        Ok(())
    }

    async fn record_strategy_decisions(
        &self,
        decisions: &[RewardStrategyDecision],
    ) -> Result<()> {
        if decisions.is_empty() {
            return Ok(());
        }
        let mut store = self.strategy_decisions.write().await;
        for decision in decisions {
            if let Some(existing) = store.iter_mut().find(|stored| {
                stored.run_id == decision.run_id
                    && stored.condition_id == decision.condition_id
                    && stored.strategy_profile == decision.strategy_profile
            }) {
                *existing = decision.clone();
            } else {
                store.push(decision.clone());
            }
        }
        Ok(())
    }

    async fn record_strategy_actions(&self, actions: &[RewardStrategyAction]) -> Result<()> {
        if actions.is_empty() {
            return Ok(());
        }
        let mut store = self.strategy_actions.write().await;
        let mut next_id = self.next_strategy_action_id.write().await;
        for action in actions {
            if let Some(existing) = store
                .iter_mut()
                .find(|stored| stored.idempotency_key == action.idempotency_key)
            {
                let action_id = existing.action_id;
                let mut replacement = action.clone();
                replacement.execution_attempts = replacement
                    .execution_attempts
                    .max(existing.execution_attempts);
                if replacement.status.is_terminal() {
                    replacement.lease_owner = None;
                    replacement.lease_expires_at = None;
                } else {
                    replacement.lease_owner = replacement
                        .lease_owner
                        .or_else(|| existing.lease_owner.clone());
                    replacement.lease_expires_at = replacement
                        .lease_expires_at
                        .or(existing.lease_expires_at);
                }
                *existing = replacement;
                existing.action_id = action_id;
            } else {
                let mut action = action.clone();
                action.action_id = *next_id;
                *next_id += 1;
                store.push(action);
            }
        }
        Ok(())
    }

    async fn claim_strategy_actions(
        &self,
        account_id: &str,
        lease_owner: &str,
        now: OffsetDateTime,
        lease_expires_at: OffsetDateTime,
        limit: u16,
    ) -> Result<Vec<RewardStrategyAction>> {
        let mut store = self.strategy_actions.write().await;
        let mut claimable = store
            .iter()
            .enumerate()
            .filter(|(_, action)| {
                action.account_id == account_id
                    && (action.status == RewardStrategyActionStatus::Planned
                        || (action.status == RewardStrategyActionStatus::Executing
                            && action
                                .lease_expires_at
                                .is_some_and(|expires_at| expires_at <= now)))
            })
            .map(|(index, action)| (index, action.created_at, action.action_id))
            .collect::<Vec<_>>();
        claimable.sort_by_key(|(_, created_at, action_id)| (*created_at, *action_id));

        let mut claimed = Vec::with_capacity(usize::from(limit));
        for (index, _, _) in claimable.into_iter().take(usize::from(limit)) {
            let action = &mut store[index];
            let previous_status = action.status.as_str();
            action.status = RewardStrategyActionStatus::Executing;
            action.lease_owner = Some(lease_owner.to_string());
            action.lease_expires_at = Some(lease_expires_at);
            action.execution_attempts += 1;
            action.updated_at = now;
            if let Value::Object(result) = &mut action.result_json {
                result.insert("status".to_string(), json!("executing"));
                result.insert("lease_owner".to_string(), json!(lease_owner));
                result.insert("lease_expires_at".to_string(), json!(lease_expires_at));
                result.insert(
                    "execution_attempts".to_string(),
                    json!(action.execution_attempts),
                );
                result.insert(
                    "claim_previous_status".to_string(),
                    json!(previous_status),
                );
            }
            claimed.push(action.clone());
        }
        Ok(claimed)
    }

    async fn renew_strategy_action_lease(
        &self,
        action_id: i64,
        lease_owner: &str,
        now: OffsetDateTime,
        lease_expires_at: OffsetDateTime,
    ) -> Result<bool> {
        let mut store = self.strategy_actions.write().await;
        let Some(action) = store.iter_mut().find(|action| {
            action.action_id == action_id
                && action.status == RewardStrategyActionStatus::Executing
                && action.lease_owner.as_deref() == Some(lease_owner)
                && action
                    .lease_expires_at
                    .is_some_and(|expires_at| expires_at > now)
        }) else {
            return Ok(false);
        };
        action.lease_expires_at = Some(lease_expires_at);
        action.updated_at = now;
        if let Value::Object(result) = &mut action.result_json {
            result.insert("lease_expires_at".to_string(), json!(lease_expires_at));
        }
        Ok(true)
    }

    async fn finalize_strategy_action_lease(
        &self,
        action: &RewardStrategyAction,
        lease_owner: &str,
    ) -> Result<bool> {
        if !action.status.is_terminal() || !action.result_json.is_object() {
            return Err(AppError::invalid_input(
                "REWARD_STRATEGY_ACTION_RESOLUTION_INVALID",
                "strategy action resolution requires a terminal status and object result",
            ));
        }
        let mut store = self.strategy_actions.write().await;
        let now = OffsetDateTime::now_utc();
        let Some(existing) = store.iter_mut().find(|existing| {
            existing.action_id == action.action_id
                && existing.status == RewardStrategyActionStatus::Executing
                && existing.lease_owner.as_deref() == Some(lease_owner)
                && existing
                    .lease_expires_at
                    .is_some_and(|expires_at| expires_at > now)
        }) else {
            return Ok(false);
        };
        existing.status = action.status;
        existing.reason_code = action.reason_code.clone();
        existing.reason = action.reason.clone();
        existing.external_order_id = action.external_order_id.clone();
        existing.result_json = action.result_json.clone();
        existing.lease_owner = None;
        existing.lease_expires_at = None;
        existing.updated_at = action.updated_at;
        Ok(true)
    }

    async fn get_strategy_action(
        &self,
        action_id: i64,
    ) -> Result<Option<RewardStrategyAction>> {
        Ok(self
            .strategy_actions
            .read()
            .await
            .iter()
            .find(|action| action.action_id == action_id)
            .cloned())
    }

    async fn release_strategy_action_lease(
        &self,
        action_id: i64,
        lease_owner: &str,
        reason_code: &str,
        reason: &str,
        result: Value,
        now: OffsetDateTime,
    ) -> Result<bool> {
        let Value::Object(result_updates) = result else {
            return Err(AppError::invalid_input(
                "REWARD_STRATEGY_ACTION_RESULT_INVALID",
                "strategy action result must be a JSON object",
            ));
        };
        let mut store = self.strategy_actions.write().await;
        let Some(action) = store.iter_mut().find(|action| {
            action.action_id == action_id
                && action.status == RewardStrategyActionStatus::Executing
                && action.lease_owner.as_deref() == Some(lease_owner)
                && action
                    .lease_expires_at
                    .is_some_and(|expires_at| expires_at > now)
        }) else {
            return Ok(false);
        };
        action.status = RewardStrategyActionStatus::Planned;
        action.reason_code = reason_code.to_string();
        action.reason = reason.to_string();
        if let Value::Object(existing_result) = &mut action.result_json {
            existing_result.extend(result_updates);
            existing_result.insert("status".to_string(), json!("planned"));
        } else {
            action.result_json = json!({ "status": "planned" });
        }
        action.lease_owner = None;
        action.lease_expires_at = None;
        action.updated_at = now;
        Ok(true)
    }

    async fn record_order_transitions(
        &self,
        transitions: &[RewardOrderTransition],
    ) -> Result<()> {
        if transitions.is_empty() {
            return Ok(());
        }
        let mut store = self.order_transitions.write().await;
        let mut next_id = self.next_order_transition_id.write().await;
        for transition in transitions {
            let mut transition = transition.clone();
            transition.transition_id = *next_id;
            *next_id += 1;
            store.push(transition);
        }
        Ok(())
    }

    async fn list_strategy_runs(
        &self,
        query: &RewardStrategyRunListQuery,
    ) -> Result<RewardStrategyRunPage> {
        let mut runs = self
            .strategy_runs
            .read()
            .await
            .iter()
            .filter(|run| {
                query
                    .account_id
                    .as_deref()
                    .is_none_or(|account_id| run.account_id == account_id)
                    && query.status.is_none_or(|status| run.status == status)
            })
            .cloned()
            .collect::<Vec<_>>();
        runs.sort_by(|left, right| {
            right
                .started_at
                .cmp(&left.started_at)
                .then_with(|| right.run_id.cmp(&left.run_id))
        });
        let page = query.page_for_total(runs.len());
        let items = in_memory_page_items(runs, page.page, page.page_size);
        Ok(RewardStrategyRunPage { items, page })
    }

    async fn get_strategy_run(&self, run_id: i64) -> Result<Option<RewardStrategyRun>> {
        Ok(self
            .strategy_runs
            .read()
            .await
            .iter()
            .find(|run| run.run_id == run_id)
            .cloned())
    }

    async fn save_strategy_replay_fixture(
        &self,
        fixture: &RewardStrategyReplayFixture,
    ) -> Result<()> {
        fixture.validate_integrity()?;
        if !self
            .strategy_runs
            .read()
            .await
            .iter()
            .any(|run| run.run_id == fixture.run_id)
        {
            return Err(AppError::not_found(
                "REWARD_STRATEGY_RUN_NOT_FOUND",
                format!("reward strategy run {} does not exist", fixture.run_id),
            ));
        }
        self.strategy_replay_fixtures
            .write()
            .await
            .insert(fixture.run_id, fixture.clone());
        Ok(())
    }

    async fn get_strategy_replay_fixture(
        &self,
        run_id: i64,
    ) -> Result<Option<RewardStrategyReplayFixture>> {
        let fixture = self
            .strategy_replay_fixtures
            .read()
            .await
            .get(&run_id)
            .cloned();
        if let Some(fixture) = &fixture {
            fixture.validate_integrity()?;
        }
        Ok(fixture)
    }

    async fn list_strategy_decisions(
        &self,
        run_id: i64,
        query: &RewardStrategyDecisionListQuery,
    ) -> Result<RewardStrategyDecisionPage> {
        let mut decisions = self
            .strategy_decisions
            .read()
            .await
            .iter()
            .filter(|decision| {
                decision.run_id == run_id
                    && query
                        .eligible
                        .is_none_or(|eligible| decision.eligible == eligible)
                    && query.search.as_deref().is_none_or(|search| {
                        decision.condition_id.to_lowercase().contains(search)
                            || decision.reason.to_lowercase().contains(search)
                            || decision.decision_json.to_string().to_lowercase().contains(search)
                    })
            })
            .cloned()
            .collect::<Vec<_>>();
        decisions.sort_by(|left, right| {
            left.decision_rank
                .cmp(&right.decision_rank)
                .then_with(|| right.selection_score.cmp(&left.selection_score))
        });
        let page = query.page_for_total(decisions.len());
        let items = in_memory_page_items(decisions, page.page, page.page_size);
        Ok(RewardStrategyDecisionPage { items, page })
    }

    async fn list_strategy_actions(
        &self,
        run_id: i64,
        query: &RewardStrategyActionListQuery,
    ) -> Result<RewardStrategyActionPage> {
        let mut actions = self
            .strategy_actions
            .read()
            .await
            .iter()
            .filter(|action| {
                action.run_id == run_id
                    && query.status.is_none_or(|status| action.status == status)
                    && query
                        .action_type
                        .is_none_or(|action_type| action.action_type == action_type)
            })
            .cloned()
            .collect::<Vec<_>>();
        actions.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| right.action_id.cmp(&left.action_id))
        });
        let page = query.page_for_total(actions.len());
        let items = in_memory_page_items(actions, page.page, page.page_size);
        Ok(RewardStrategyActionPage { items, page })
    }

    async fn list_order_transitions(
        &self,
        managed_order_id: &str,
        query: &RewardOrderTransitionListQuery,
    ) -> Result<RewardOrderTransitionPage> {
        let mut transitions = self
            .order_transitions
            .read()
            .await
            .iter()
            .filter(|transition| transition.managed_order_id == managed_order_id)
            .cloned()
            .collect::<Vec<_>>();
        transitions.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| right.transition_id.cmp(&left.transition_id))
        });
        let page = query.page_for_total(transitions.len());
        let items = in_memory_page_items(transitions, page.page, page.page_size);
        Ok(RewardOrderTransitionPage { items, page })
    }

    async fn save_quote_plans(&self, plans: &[RewardQuotePlan]) -> Result<()> {
        let mut store = self.quote_plans.write().await;
        store.clear();
        for plan in plans {
            let mut plan = plan.clone();
            refresh_reward_quote_plan_readiness(&mut plan);
            store.insert((plan.condition_id.clone(), plan.strategy_profile), plan.clone());
        }
        Ok(())
    }

    async fn record_fair_value_estimates(
        &self,
        estimates: &[RewardFairValueEstimate],
    ) -> Result<()> {
        if estimates.is_empty() {
            return Ok(());
        }
        self.fair_values.write().await.extend_from_slice(estimates);
        Ok(())
    }

    async fn record_market_candle_sample(
        &self,
        sample: &RewardMarketCandleSample,
    ) -> Result<()> {
        let markets = self.markets.read().await;
        let Some((condition_id, outcome)) = markets
            .values()
            .filter(|market| market.active)
            .filter_map(|market| {
                market
                    .tokens
                    .iter()
                    .find(|token| token.token_id == sample.token_id)
                    .map(|token| (market.condition_id.clone(), token.outcome.clone()))
            })
            .next()
        else {
            return Ok(());
        };
        drop(markets);

        let key = (
            sample.token_id.clone(),
            sample.interval_sec,
            sample.bucket_start,
        );
        let mut candles = self.candles.write().await;
        match candles.get_mut(&key) {
            Some(existing) if sample.observed_at > existing.close_observed_at => {
                existing.high = Decimal::max(existing.high, sample.midpoint);
                existing.low = Decimal::min(existing.low, sample.midpoint);
                existing.close = sample.midpoint;
                existing.best_bid_close = sample.best_bid;
                existing.best_ask_close = sample.best_ask;
                existing.spread_cents_close = sample.spread_cents;
                existing.sample_count += 1;
                existing.close_observed_at = sample.observed_at;
                existing.updated_at = OffsetDateTime::now_utc();
            }
            Some(existing)
                if sample.observed_at == existing.close_observed_at
                    && (sample.midpoint != existing.close
                        || sample.best_bid != existing.best_bid_close
                        || sample.best_ask != existing.best_ask_close
                        || sample.spread_cents != existing.spread_cents_close) =>
            {
                existing.high = Decimal::max(existing.high, sample.midpoint);
                existing.low = Decimal::min(existing.low, sample.midpoint);
                existing.close = sample.midpoint;
                existing.best_bid_close = sample.best_bid;
                existing.best_ask_close = sample.best_ask;
                existing.spread_cents_close = sample.spread_cents;
                existing.updated_at = OffsetDateTime::now_utc();
            }
            Some(_) => {}
            None => {
                candles.insert(
                    key,
                    RewardMarketCandle {
                        token_id: sample.token_id.clone(),
                        condition_id,
                        outcome,
                        interval_sec: sample.interval_sec,
                        bucket_start: sample.bucket_start,
                        open: sample.midpoint,
                        high: sample.midpoint,
                        low: sample.midpoint,
                        close: sample.midpoint,
                        best_bid_close: sample.best_bid,
                        best_ask_close: sample.best_ask,
                        spread_cents_close: sample.spread_cents,
                        sample_count: 1,
                        close_observed_at: sample.observed_at,
                        updated_at: OffsetDateTime::now_utc(),
                    },
                );
            }
        }
        Ok(())
    }

    async fn list_recent_market_candles(
        &self,
        condition_id: &str,
        interval_sec: i32,
        limit_per_token: u16,
    ) -> Result<Vec<RewardMarketCandle>> {
        let limit = usize::from(limit_per_token.max(1));
        let mut by_token = BTreeMap::<String, Vec<RewardMarketCandle>>::new();
        for candle in self.candles.read().await.values() {
            if candle.condition_id == condition_id && candle.interval_sec == interval_sec {
                by_token
                    .entry(candle.token_id.clone())
                    .or_default()
                    .push(candle.clone());
            }
        }
        let mut output = Vec::new();
        for candles in by_token.values_mut() {
            candles.sort_by_key(|candle| std::cmp::Reverse(candle.bucket_start));
            candles.truncate(limit);
            candles.sort_by_key(|candle| candle.bucket_start);
            output.extend(candles.iter().cloned());
        }
        Ok(output)
    }

    async fn latest_market_advisory(
        &self,
        request: &RewardAiAdvisoryRequest,
        now: OffsetDateTime,
    ) -> Result<Option<RewardMarketAdvisory>> {
        Ok(self
            .advisories
            .read()
            .await
            .iter()
            .filter(|advisory| {
                advisory.condition_id == request.condition_id
                    && advisory.provider == request.provider
                    && advisory.request_format == request.request_format
                    && advisory.model == request.model
                    && advisory.input_hash == request.input_hash
                    && advisory.expires_at > now
            })
            .max_by_key(|advisory| advisory.expires_at)
            .cloned())
    }

    async fn save_market_advisory(&self, advisory: &RewardMarketAdvisory) -> Result<()> {
        self.advisories.write().await.push(advisory.clone());
        Ok(())
    }

    async fn latest_market_info_risk(
        &self,
        request: &RewardInfoRiskAssessmentRequest,
        now: OffsetDateTime,
    ) -> Result<Option<RewardMarketInfoRisk>> {
        Ok(self
            .info_risks
            .read()
            .await
            .iter()
            .filter(|risk| {
                risk.condition_id == request.condition_id
                    && risk.provider == request.provider
                    && risk.request_format == request.request_format
                    && risk.model == request.model
                    && risk.input_hash == request.input_hash
                    && risk.expires_at > now
            })
            .max_by_key(|risk| risk.expires_at)
            .cloned())
    }

    async fn latest_market_info_risks(
        &self,
        condition_ids: &[String],
        now: OffsetDateTime,
    ) -> Result<Vec<RewardMarketInfoRisk>> {
        let wanted = condition_ids.iter().collect::<HashSet<_>>();
        let mut latest = HashMap::<String, RewardMarketInfoRisk>::new();
        for risk in self.info_risks.read().await.iter() {
            if !wanted.contains(&risk.condition_id) || risk.expires_at <= now {
                continue;
            }
            let replace = latest
                .get(&risk.condition_id)
                .is_none_or(|existing| risk.expires_at > existing.expires_at);
            if replace {
                latest.insert(risk.condition_id.clone(), risk.clone());
            }
        }
        Ok(latest.into_values().collect())
    }

    async fn save_market_info_risk(&self, risk: &RewardMarketInfoRisk) -> Result<()> {
        self.info_risks.write().await.push(risk.clone());
        Ok(())
    }

    async fn record_llm_call(&self, call: &RewardLlmCallRecord) -> Result<()> {
        let mut calls = self.llm_calls.write().await;
        if !calls.iter().any(|existing| existing.id == call.id) {
            calls.push(call.clone());
        }
        Ok(())
    }

    async fn list_llm_call_daily_stats(
        &self,
        since: OffsetDateTime,
        limit: u16,
    ) -> Result<Vec<RewardLlmCallDailyStats>> {
        let mut by_day = BTreeMap::<String, RewardLlmCallDailyStats>::new();
        for call in self.llm_calls.read().await.iter() {
            if call.created_at < since || !reward_llm_call_is_tracked(&call.task_type) {
                continue;
            }
            let day = call.created_at.date().to_string();
            let stats = by_day.entry(day.clone()).or_insert_with(|| {
                RewardLlmCallDailyStats {
                    day,
                    ..RewardLlmCallDailyStats::default()
                }
            });
            stats.total_calls += 1;
            stats.provider_calls += 1;
            if call
                .validation_result
                .get("success")
                .and_then(Value::as_bool)
                == Some(false)
            {
                stats.failed_calls += 1;
            }
        }

        Ok(by_day
            .into_values()
            .rev()
            .take(usize::from(limit.max(1)))
            .collect())
    }

    async fn list_markets(&self, limit: u16) -> Result<Vec<RewardMarket>> {
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
        markets.truncate(usize::from(limit));
        Ok(markets)
    }

    async fn list_candidate_markets(
        &self,
        filter: &RewardCandidateFilter,
        safety_limit: u16,
    ) -> Result<Vec<RewardMarket>> {
        let now = OffsetDateTime::now_utc();
        let mut markets: Vec<RewardMarket> = self
            .markets
            .read()
            .await
            .values()
            .filter(|market| {
                market.active
                    && in_memory_reward_tokens_are_binary(market)
                    && market.total_daily_rate >= filter.min_daily_reward
                    && market.rewards_max_spread > rust_decimal::Decimal::ZERO
                    && in_memory_reward_midpoint(market)
                        .is_some_and(|midpoint| in_memory_reward_midpoint_allowed(midpoint, filter))
                    && in_memory_reward_market_activity_allowed(market, filter)
                    && market.market_spread_cents <= filter.max_market_spread_cents
                    && market.ambiguity_level != "high"
                    && market.end_at.is_some_and(|end_at| {
                        end_at >= now + Duration::hours(filter.min_hours_to_end as i64)
                    })
                    && market.market_synced_at.is_some_and(|synced_at| {
                        synced_at
                            >= now
                                - Duration::minutes(
                                    filter.max_market_data_age_minutes as i64,
                                )
                            && synced_at <= now + Duration::minutes(5)
                    })
            })
            .cloned()
            .collect();
        if filter.prefer_sparse_market_ordering {
            markets.sort_by(|left, right| {
                in_memory_reward_sparse_market_density(right)
                    .cmp(&in_memory_reward_sparse_market_density(left))
                    .then_with(|| left.liquidity_usd.cmp(&right.liquidity_usd))
                    .then_with(|| left.volume_24h_usd.cmp(&right.volume_24h_usd))
                    .then_with(|| right.total_daily_rate.cmp(&left.total_daily_rate))
                    .then_with(|| right.updated_at.cmp(&left.updated_at))
            });
        } else {
            markets.sort_by(|left, right| {
                right
                    .liquidity_usd
                    .cmp(&left.liquidity_usd)
                    .then_with(|| right.volume_24h_usd.cmp(&left.volume_24h_usd))
                    .then_with(|| right.end_at.cmp(&left.end_at))
                    .then_with(|| right.total_daily_rate.cmp(&left.total_daily_rate))
                    .then_with(|| right.updated_at.cmp(&left.updated_at))
            });
        }
        markets.truncate(usize::from(safety_limit));
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

    async fn active_market_summary(&self) -> Result<(usize, Option<OffsetDateTime>)> {
        let markets = self.markets.read().await;
        let mut markets_tracked = 0usize;
        let mut last_scan_at = None;

        for market in markets.values().filter(|market| market.active) {
            markets_tracked += 1;
            last_scan_at = last_scan_at.max(Some(market.updated_at));
        }

        Ok((markets_tracked, last_scan_at))
    }

    async fn list_all_quote_plans(&self) -> Result<Vec<RewardQuotePlan>> {
        let mut plans = self
            .quote_plans
            .read()
            .await
            .values()
            .cloned()
            .map(|mut plan| {
                refresh_reward_quote_plan_readiness(&mut plan);
                plan
            })
            .collect::<Vec<_>>();
        plans.sort_by(|left, right| {
            right
                .eligible
                .cmp(&left.eligible)
                .then_with(|| right.selection_score.cmp(&left.selection_score))
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| right.updated_at.cmp(&left.updated_at))
        });
        Ok(plans)
    }

    async fn count_quote_plans(&self) -> Result<RewardQuotePlanCounts> {
        let plans = self.quote_plans.read().await;
        Ok(RewardQuotePlanCounts::from_plans(plans.values()))
    }

    async fn latest_quote_plan_updated_at(&self) -> Result<Option<OffsetDateTime>> {
        let plans = self.quote_plans.read().await;
        Ok(plans.values().map(|plan| plan.updated_at).max())
    }

    async fn list_quote_plans_page(
        &self,
        query: &RewardQuotePlanListQuery,
    ) -> Result<RewardQuotePlanPage> {
        let mut plans = self
            .quote_plans
            .read()
            .await
            .values()
            .filter(|plan| {
                if let Some(eligible) = query.eligible {
                    if plan.eligible != eligible {
                        return false;
                    }
                }
                if let Some(ref search) = query.search {
                    let q: &str = search.as_str();
                    if !plan.question.to_lowercase().contains(q)
                        && !plan.reason.to_lowercase().contains(q)
                        && !plan.market_slug.to_lowercase().contains(q)
                    {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .map(|mut plan| {
                refresh_reward_quote_plan_readiness(&mut plan);
                plan
            })
            .collect::<Vec<_>>();

        plans.sort_by(|a, b| {
            let primary = match query.sort_by {
                RewardQuotePlanSortField::SelectionScore => {
                    a.selection_score.cmp(&b.selection_score)
                }
                RewardQuotePlanSortField::Score => a.score.cmp(&b.score),
                RewardQuotePlanSortField::DailyReward => {
                    a.total_daily_rate.cmp(&b.total_daily_rate)
                }
                RewardQuotePlanSortField::Midpoint => {
                    a.midpoint.cmp(&b.midpoint)
                }
                RewardQuotePlanSortField::Eligible => a.eligible.cmp(&b.eligible),
            };
            let ord = match query.sort_order {
                SortOrder::Asc => primary,
                SortOrder::Desc => primary.reverse(),
            };
            ord.then_with(|| {
                b.eligible
                    .cmp(&a.eligible)
                    .then_with(|| b.updated_at.cmp(&a.updated_at))
            })
        });

        let total_items = plans.len();
        let page = query.page_for_total(total_items);
        let start = (page.page - 1) * page.page_size;
        let end = (start + page.page_size).min(plans.len());
        let items = if start < plans.len() {
            plans[start..end].to_vec()
        } else {
            Vec::new()
        };

        Ok(RewardQuotePlanPage { items, page })
    }

    async fn list_orders_page(&self, query: &RewardOrderListQuery) -> Result<RewardOrderPage> {
        let mut orders = self
            .orders
            .read()
            .await
            .iter()
            .filter(|order| order.account_id == query.account_id && query.matches_order(order))
            .cloned()
            .collect::<Vec<_>>();
        orders.sort_by(|left, right| query.compare_orders(left, right));

        let page = query.page_for_total(orders.len());
        let start = (page.page - 1) * page.page_size;
        let items = orders.into_iter().skip(start).take(page.page_size).collect();
        Ok(RewardOrderPage { items, page })
    }

    async fn list_positions(&self, account_id: &str, limit: u16) -> Result<Vec<RewardPosition>> {
        let mut positions = self
            .positions
            .read()
            .await
            .values()
            .filter(|p| p.account_id == account_id)
            .cloned()
            .collect::<Vec<_>>();
        positions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        positions.truncate(usize::from(limit));
        Ok(positions)
    }

    async fn list_events(&self, account_id: &str, limit: u16) -> Result<Vec<RewardRiskEvent>> {
        let mut events = self
            .events
            .read()
            .await
            .iter()
            .filter(|event| {
                event.account_id.as_deref() == Some(account_id)
                    && event.event_type != "reward_bot_live_plan_built"
            })
            .cloned()
            .collect::<Vec<_>>();
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
            if state.account_id == config.account_id {
                return Ok(state.clone());
            }
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

    async fn count_open_orders(&self, account_id: &str) -> Result<usize> {
        Ok(self
            .orders
            .read()
            .await
            .iter()
            .filter(|order| order.account_id == account_id && order.status.is_open_like())
            .count())
    }

    async fn count_external_open_orders(&self, account_id: &str) -> Result<usize> {
        Ok(self
            .orders
            .read()
            .await
            .iter()
            .filter(|order| {
                order.account_id == account_id && reward_order_counts_as_external_open(order)
            })
            .count())
    }

    async fn get_order_by_external_order_id(
        &self,
        external_order_id: &str,
    ) -> Result<Option<ManagedRewardOrder>> {
        Ok(self
            .orders
            .read()
            .await
            .iter()
            .find(|order| order.external_order_id.as_deref() == Some(external_order_id))
            .cloned())
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

    async fn count_account_positions(&self, account_id: &str) -> Result<usize> {
        Ok(self
            .positions
            .read()
            .await
            .values()
            .filter(|position| position.account_id == account_id && position.size != Decimal::ZERO)
            .count())
    }

    async fn list_fills(&self, account_id: &str, limit: u16) -> Result<Vec<RewardFill>> {
        let mut fills = self
            .fills
            .read()
            .await
            .iter()
            .filter(|f| f.account_id == account_id)
            .cloned()
            .collect::<Vec<_>>();
        fills.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        fills.truncate(usize::from(limit));
        Ok(fills)
    }

    async fn reward_fill_exists(&self, fill_id: &str) -> Result<bool> {
        Ok(self.fills.read().await.iter().any(|fill| fill.id == fill_id))
    }

    async fn latest_fill_at(&self, account_id: &str) -> Result<Option<OffsetDateTime>> {
        Ok(self
            .fills
            .read()
            .await
            .iter()
            .filter(|fill| fill.account_id == account_id)
            .map(|fill| fill.created_at)
            .max())
    }

    async fn active_merge_intent_size(
        &self,
        account_id: &str,
        condition_id: &str,
    ) -> Result<Decimal> {
        Ok(self
            .merge_intents
            .read()
            .await
            .iter()
            .filter(|intent| {
                intent.account_id == account_id
                    && intent.condition_id == condition_id
                    && intent.status.counts_as_active_pair()
            })
            .map(|intent| intent.merge_size)
            .sum())
    }

    async fn create_merge_intent_if_absent(&self, intent: &RewardMergeIntent) -> Result<bool> {
        let mut intents = self.merge_intents.write().await;
        if intents.iter().any(|stored| stored.id == intent.id) {
            return Ok(false);
        }
        intents.push(intent.clone());
        intents.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(true)
    }

    async fn get_merge_intent(&self, intent_id: &str) -> Result<Option<RewardMergeIntent>> {
        Ok(self
            .merge_intents
            .read()
            .await
            .iter()
            .find(|intent| intent.id == intent_id)
            .cloned())
    }

    async fn list_executable_merge_intents(
        &self,
        account_id: &str,
        limit: u16,
    ) -> Result<Vec<RewardMergeIntent>> {
        let mut intents = self
            .merge_intents
            .read()
            .await
            .iter()
            .filter(|intent| {
                intent.account_id == account_id
                    && matches!(
                        intent.status,
                        RewardMergeIntentStatus::Pending | RewardMergeIntentStatus::Unsupported
                    )
                    && intent.tx_hash.is_none()
            })
            .cloned()
            .collect::<Vec<_>>();
        intents.sort_by(|left, right| left.updated_at.cmp(&right.updated_at));
        intents.truncate(usize::from(limit.max(1)));
        Ok(intents)
    }

    async fn mark_merge_intent_submitted(
        &self,
        intent_id: &str,
        tx_hash: &str,
        submitted_at: OffsetDateTime,
        reason: &str,
    ) -> Result<()> {
        if let Some(intent) = self
            .merge_intents
            .write()
            .await
            .iter_mut()
            .find(|intent| {
                intent.id == intent_id
                    && matches!(
                        intent.status,
                        RewardMergeIntentStatus::Pending | RewardMergeIntentStatus::Unsupported
                    )
                    && intent.tx_hash.is_none()
            })
        {
            intent.status = RewardMergeIntentStatus::Submitted;
            intent.tx_hash = Some(tx_hash.to_string());
            intent.submitted_at = Some(submitted_at);
            intent.failed_reason = None;
            intent.reason = reason.to_string();
            intent.updated_at = submitted_at;
        }
        Ok(())
    }

    async fn mark_merge_intent_failed(
        &self,
        intent_id: &str,
        failed_reason: &str,
        failed_at: OffsetDateTime,
    ) -> Result<()> {
        if let Some(intent) = self
            .merge_intents
            .write()
            .await
            .iter_mut()
            .find(|intent| {
                intent.id == intent_id
                    && matches!(
                        intent.status,
                        RewardMergeIntentStatus::Pending | RewardMergeIntentStatus::Unsupported
                    )
                    && intent.tx_hash.is_none()
            })
        {
            intent.status = RewardMergeIntentStatus::Failed;
            intent.failed_reason = Some(failed_reason.to_string());
            intent.retry_count += 1;
            intent.reason = failed_reason.to_string();
            intent.updated_at = failed_at;
        }
        Ok(())
    }

    async fn resolve_merge_intent_transaction(
        &self,
        intent_id: &str,
        tx_hash: &str,
        succeeded: bool,
        reason: &str,
        resolved_at: OffsetDateTime,
    ) -> Result<bool> {
        let mut intents = self.merge_intents.write().await;
        let Some(intent) = intents.iter_mut().find(|intent| {
            intent.id == intent_id
                && intent.status == RewardMergeIntentStatus::Submitted
                && intent.tx_hash.as_deref() == Some(tx_hash)
        }) else {
            return Ok(false);
        };
        intent.status = if succeeded {
            RewardMergeIntentStatus::Completed
        } else {
            RewardMergeIntentStatus::Failed
        };
        intent.confirmed_at = succeeded.then_some(resolved_at);
        intent.failed_reason = (!succeeded).then(|| reason.to_string());
        if !succeeded {
            intent.retry_count += 1;
        }
        intent.reason = reason.to_string();
        intent.updated_at = resolved_at;
        Ok(true)
    }

    async fn apply_tick_outcome(
        &self,
        outcome: &RewardTickOutcome,
        trace_id: &str,
    ) -> Result<()> {
        let run_id = self
            .strategy_runs
            .read()
            .await
            .iter()
            .filter(|run| run.trace_id == trace_id)
            .max_by_key(|run| run.started_at)
            .map(|run| run.run_id);
        let existing_statuses = if run_id.is_some() && !outcome.orders.is_empty() {
            let orders = self.orders.read().await;
            outcome
                .orders
                .iter()
                .filter_map(|order| {
                    orders
                        .iter()
                        .find(|stored| stored.id == order.id)
                        .map(|stored| (order.id.clone(), stored.status))
                })
                .collect::<HashMap<_, _>>()
        } else {
            HashMap::new()
        };

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
            let mut merge_intents = self.merge_intents.write().await;
            for intent in &outcome.merge_intents {
                if let Some(existing) = merge_intents
                    .iter_mut()
                    .find(|stored| stored.id == intent.id)
                {
                    *existing = intent.clone();
                } else {
                    merge_intents.push(intent.clone());
                }
            }
            merge_intents.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
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
        if let Some(run_id) = run_id {
            let now = OffsetDateTime::now_utc();
            let actions = reward_strategy_actions_from_tick_outcome(run_id, outcome, trace_id, now);
            self.record_strategy_actions(&actions).await?;
            let transitions = outcome
                .orders
                .iter()
                .filter_map(|order| {
                    let from_status = existing_statuses.get(&order.id).copied();
                    if from_status == Some(order.status) {
                        return None;
                    }
                    Some(reward_order_transition_from_order_change(
                        Some(run_id),
                        from_status,
                        order,
                        now,
                    ))
                })
                .collect::<Vec<_>>();
            self.record_order_transitions(&transitions).await?;
        }
        Ok(())
    }

    async fn apply_account_sync(
        &self,
        account: &RewardAccountState,
        positions: Option<&[RewardPosition]>,
        _trace_id: &str,
    ) -> Result<()> {
        *self.account_state.write().await = Some(account.clone());
        if let Some(positions) = positions {
            let mut stored = self.positions.write().await;
            stored.retain(|(account_id, _), _| account_id != &account.account_id);
            for position in positions {
                stored.insert(
                    (position.account_id.clone(), position.token_id.clone()),
                    position.clone(),
                );
            }
        }
        Ok(())
    }

    async fn reset_state(&self, config: &RewardBotConfig, _trace_id: &str) -> Result<()> {
        let account_id = &config.account_id;
        self.orders.write().await.retain(|order| order.account_id != *account_id);
        self.positions.write().await.retain(|_, position| position.account_id != *account_id);
        self.fills.write().await.retain(|fill| fill.account_id != *account_id);
        self.merge_intents
            .write()
            .await
            .retain(|intent| intent.account_id != *account_id);
        self.events.write().await.retain(|event| event.account_id.as_deref() != Some(account_id));
        *self.account_state.write().await = Some(RewardAccountState::fresh(
            &config.account_id,
            config.account_capital_usd,
            OffsetDateTime::now_utc(),
        ));
        Ok(())
    }
}

fn in_memory_reward_tokens_are_binary(market: &RewardMarket) -> bool {
    if market.tokens.len() != 2 {
        return false;
    }
    let first = &market.tokens[0];
    let second = &market.tokens[1];
    if first.token_id.trim().is_empty()
        || second.token_id.trim().is_empty()
        || first.token_id == second.token_id
    {
        return false;
    }
    (first.outcome.eq_ignore_ascii_case("yes") && second.outcome.eq_ignore_ascii_case("no"))
        || (first.outcome.eq_ignore_ascii_case("no")
            && second.outcome.eq_ignore_ascii_case("yes"))
}

fn in_memory_reward_midpoint(market: &RewardMarket) -> Option<Decimal> {
    market.tokens.iter().find_map(|token| {
        let price = token.price?;
        if token.outcome.eq_ignore_ascii_case("yes") {
            Some(price)
        } else if token.outcome.eq_ignore_ascii_case("no") {
            Some(Decimal::ONE - price)
        } else {
            None
        }
    })
}

fn in_memory_reward_midpoint_allowed(midpoint: Decimal, filter: &RewardCandidateFilter) -> bool {
    if midpoint >= filter.min_midpoint && midpoint <= filter.max_midpoint {
        return true;
    }
    if !filter.allow_dominant_single_side {
        return false;
    }
    (midpoint >= filter.dominant_min_probability
        && midpoint <= filter.dominant_max_probability)
        || (midpoint >= Decimal::ONE - filter.dominant_max_probability
            && midpoint <= Decimal::ONE - filter.dominant_min_probability)
}

fn in_memory_reward_market_activity_allowed(
    market: &RewardMarket,
    filter: &RewardCandidateFilter,
) -> bool {
    match (
        filter.min_market_liquidity_usd > Decimal::ZERO,
        filter.min_market_volume_24h_usd > Decimal::ZERO,
    ) {
        (true, true) => {
            market.liquidity_usd >= filter.min_market_liquidity_usd
                || market.volume_24h_usd >= filter.min_market_volume_24h_usd
        }
        (true, false) => market.liquidity_usd >= filter.min_market_liquidity_usd,
        (false, true) => market.volume_24h_usd >= filter.min_market_volume_24h_usd,
        (false, false) => true,
    }
}

fn in_memory_reward_sparse_market_density(market: &RewardMarket) -> Decimal {
    let denominator = (market.liquidity_usd + market.volume_24h_usd).max(Decimal::ONE);
    market.total_daily_rate / denominator
}

fn in_memory_page_items<T: Clone>(items: Vec<T>, page: usize, page_size: usize) -> Vec<T> {
    let start = (page.saturating_sub(1)) * page_size;
    items.into_iter().skip(start).take(page_size).collect()
}

fn reward_llm_call_is_tracked(task_type: &str) -> bool {
    matches!(
        task_type,
        "reward_provider" | "reward_ai_advisory" | "reward_info_risk"
    )
}
