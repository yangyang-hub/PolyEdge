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
    control_commands: RwLock<Vec<RewardControlCommand>>,
    worker_heartbeats: RwLock<HashMap<String, OffsetDateTime>>,
    advisories: RwLock<Vec<RewardMarketAdvisory>>,
    info_risks: RwLock<Vec<RewardMarketInfoRisk>>,
    low_competition_observations: RwLock<Vec<RewardLowCompetitionObservation>>,
    candles: RwLock<HashMap<(String, i32, OffsetDateTime), RewardMarketCandle>>,
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
            control_commands: RwLock::new(Vec::new()),
            worker_heartbeats: RwLock::new(HashMap::new()),
            advisories: RwLock::new(Vec::new()),
            info_risks: RwLock::new(Vec::new()),
            low_competition_observations: RwLock::new(Vec::new()),
            candles: RwLock::new(HashMap::new()),
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

        let low_competition_observations_deleted = {
            let mut observations = self.low_competition_observations.write().await;
            let before = observations.len();
            observations.retain(|observation| observation.observed_at >= cutoff);
            (before - observations.len()) as u64
        };

        Ok(RewardHistoryPruneReport {
            terminal_orders_deleted,
            risk_events_deleted,
            low_competition_observations_deleted,
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

    async fn save_quote_plans(&self, plans: &[RewardQuotePlan]) -> Result<()> {
        let mut store = self.quote_plans.write().await;
        store.clear();
        for plan in plans {
            store.insert(plan.condition_id.clone(), plan.clone());
        }
        Ok(())
    }

    async fn record_low_competition_observations(
        &self,
        observations: &[RewardLowCompetitionObservation],
    ) -> Result<()> {
        if observations.is_empty() {
            return Ok(());
        }
        let mut stored = self.low_competition_observations.write().await;
        for observation in observations {
            if !stored.iter().any(|existing| existing.id == observation.id) {
                stored.push(observation.clone());
            }
        }
        stored.sort_by(|left, right| right.observed_at.cmp(&left.observed_at));
        stored.truncate(10_000);
        Ok(())
    }

    async fn list_low_competition_observations(
        &self,
        account_id: &str,
        since: OffsetDateTime,
        limit: u16,
    ) -> Result<Vec<RewardLowCompetitionObservation>> {
        Ok(self
            .low_competition_observations
            .read()
            .await
            .iter()
            .filter(|observation| {
                observation.account_id == account_id && observation.observed_at >= since
            })
            .take(usize::from(limit))
            .cloned()
            .collect())
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
                    && market.liquidity_usd >= filter.min_market_liquidity_usd
                    && market.volume_24h_usd >= filter.min_market_volume_24h_usd
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
        markets.sort_by(|left, right| {
            right
                .liquidity_usd
                .cmp(&left.liquidity_usd)
                .then_with(|| right.volume_24h_usd.cmp(&left.volume_24h_usd))
                .then_with(|| right.end_at.cmp(&left.end_at))
                .then_with(|| right.total_daily_rate.cmp(&left.total_daily_rate))
                .then_with(|| right.updated_at.cmp(&left.updated_at))
        });
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

    async fn count_quote_plans(&self) -> Result<(usize, usize)> {
        let plans = self.quote_plans.read().await;
        let total = plans.len();
        let eligible = plans.values().filter(|p| p.eligible).count();
        Ok((total, eligible))
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
            .collect::<Vec<_>>();

        plans.sort_by(|a, b| {
            let primary = match query.sort_by {
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
                order.account_id == account_id
                    && order.status.is_open_like()
                    && order.external_order_id.is_some()
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

    async fn apply_tick_outcome(
        &self,
        outcome: &RewardTickOutcome,
        _trace_id: &str,
    ) -> Result<()> {
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
