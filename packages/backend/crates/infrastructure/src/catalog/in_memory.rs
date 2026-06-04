use super::*;

pub struct InMemoryMarketEventStore {
    markets: RwLock<HashMap<String, MarketView>>,
    events: RwLock<HashMap<String, EventView>>,
    evidences: RwLock<HashMap<String, EvidenceView>>,
    signals: RwLock<HashMap<String, SignalView>>,
    probability_estimates: RwLock<HashMap<String, ProbabilityEstimateView>>,
    signal_transitions: RwLock<Vec<SignalTransitionView>>,
    order_drafts: RwLock<HashMap<String, OrderDraftView>>,
    execution_requests: RwLock<HashMap<String, ExecutionRequestView>>,
    orders: RwLock<HashMap<String, OrderView>>,
    trades: RwLock<HashMap<String, TradeView>>,
    positions: RwLock<HashMap<String, PositionView>>,
    raw_news_events: RwLock<HashMap<String, NewsRawEventView>>,
    raw_news_dedup_keys: RwLock<HashSet<String>>,
    news_source_health: RwLock<HashMap<String, NewsSourceHealthView>>,
    arbitrage_scans: RwLock<HashMap<String, ArbitrageScanView>>,
    market_book_snapshots: RwLock<HashMap<String, MarketBookSnapshotView>>,
    arbitrage_opportunities: RwLock<HashMap<String, ArbitrageOpportunityView>>,
    arbitrage_opportunity_validations: RwLock<HashMap<String, ArbitrageOpportunityValidationView>>,
    arbitrage_analysis_runs: RwLock<HashMap<String, ArbitrageAnalysisRunView>>,
    arbitrage_events: RwLock<Vec<ArbitrageEventView>>,
    arbitrage_event_sequence: RwLock<u64>,
}

impl InMemoryMarketEventStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            markets: RwLock::new(HashMap::new()),
            events: RwLock::new(HashMap::new()),
            evidences: RwLock::new(HashMap::new()),
            signals: RwLock::new(HashMap::new()),
            probability_estimates: RwLock::new(HashMap::new()),
            signal_transitions: RwLock::new(Vec::new()),
            order_drafts: RwLock::new(HashMap::new()),
            execution_requests: RwLock::new(HashMap::new()),
            orders: RwLock::new(HashMap::new()),
            trades: RwLock::new(HashMap::new()),
            positions: RwLock::new(HashMap::new()),
            raw_news_events: RwLock::new(HashMap::new()),
            raw_news_dedup_keys: RwLock::new(HashSet::new()),
            news_source_health: RwLock::new(HashMap::new()),
            arbitrage_scans: RwLock::new(HashMap::new()),
            market_book_snapshots: RwLock::new(HashMap::new()),
            arbitrage_opportunities: RwLock::new(HashMap::new()),
            arbitrage_opportunity_validations: RwLock::new(HashMap::new()),
            arbitrage_analysis_runs: RwLock::new(HashMap::new()),
            arbitrage_events: RwLock::new(Vec::new()),
            arbitrage_event_sequence: RwLock::new(0),
        }
    }

    async fn source_health_adjustment_for_event(
        &self,
        event_id: &str,
    ) -> Option<SourceHealthAdjustment> {
        let source = {
            let events = self.events.read().await;
            events.get(event_id).map(|event| event.source.clone())?
        };
        let health = self.news_source_health.read().await;
        health.get(&source).map(|view| SourceHealthAdjustment {
            source,
            health_score: view.health_score,
        })
    }
}

include!("in_memory/queries.rs");
include!("in_memory/signals.rs");
include!("in_memory/execution_submit.rs");
include!("in_memory/execution_updates.rs");
include!("in_memory/fixtures.rs");

#[async_trait]
impl MarketEventStore for InMemoryMarketEventStore {
    async fn list_markets(&self, filters: &MarketListFilters) -> Result<Vec<MarketView>> {
        self.market_event_list_markets(filters).await
    }

    async fn count_markets(&self, filters: &MarketListFilters) -> Result<i64> {
        self.market_event_count_markets(filters).await
    }

    async fn list_market_categories(&self) -> Result<Vec<MarketCategoryView>> {
        self.market_event_list_market_categories().await
    }

    async fn get_market(&self, market_id: &str) -> Result<Option<MarketView>> {
        self.market_event_get_market(market_id).await
    }

    async fn get_signal(&self, signal_id: &str) -> Result<Option<SignalView>> {
        self.market_event_get_signal(signal_id).await
    }

    async fn list_events(
        &self,
        filters: &EventListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<EventView>> {
        self.market_event_list_events(filters, page).await
    }

    async fn list_evidences(
        &self,
        filters: &EvidenceListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<EvidenceView>> {
        self.market_event_list_evidences(filters, page).await
    }

    async fn list_signals(
        &self,
        filters: &SignalListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<SignalView>> {
        self.market_event_list_signals(filters, page).await
    }

    async fn list_probability_estimates(
        &self,
        filters: &ProbabilityEstimateListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<ProbabilityEstimateView>> {
        self.market_event_list_probability_estimates(filters, page)
            .await
    }

    async fn list_signal_transitions(
        &self,
        filters: &SignalTransitionListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<SignalTransitionView>> {
        self.market_event_list_signal_transitions(filters, page)
            .await
    }

    async fn list_order_drafts(
        &self,
        filters: &OrderDraftListFilters,
    ) -> Result<Vec<OrderDraftView>> {
        self.market_event_list_order_drafts(filters).await
    }

    async fn list_execution_requests(
        &self,
        filters: &ExecutionRequestListFilters,
    ) -> Result<Vec<ExecutionRequestView>> {
        self.market_event_list_execution_requests(filters).await
    }

    async fn get_order_by_external_ref(
        &self,
        connector_name: &str,
        external_order_id: &str,
    ) -> Result<OrderView> {
        self.market_event_get_order_by_external_ref(connector_name, external_order_id)
            .await
    }

    async fn list_orders(&self, filters: &OrderListFilters) -> Result<Vec<OrderView>> {
        self.market_event_list_orders(filters).await
    }

    async fn list_trades(&self, filters: &TradeListFilters) -> Result<Vec<TradeView>> {
        self.market_event_list_trades(filters).await
    }

    async fn list_positions(&self, filters: &PositionListFilters) -> Result<Vec<PositionView>> {
        self.market_event_list_positions(filters).await
    }
    async fn count_order_drafts(&self, filters: &OrderDraftListFilters) -> Result<i64> {
        self.market_event_count_order_drafts(filters).await
    }
    async fn list_order_drafts_paginated(
        &self,
        filters: &OrderDraftListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<OrderDraftView>> {
        self.market_event_list_order_drafts_paginated(filters, page)
            .await
    }
    async fn count_execution_requests(&self, filters: &ExecutionRequestListFilters) -> Result<i64> {
        self.market_event_count_execution_requests(filters).await
    }
    async fn list_execution_requests_paginated(
        &self,
        filters: &ExecutionRequestListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<ExecutionRequestView>> {
        self.market_event_list_execution_requests_paginated(filters, page)
            .await
    }
    async fn count_orders(&self, filters: &OrderListFilters) -> Result<i64> {
        self.market_event_count_orders(filters).await
    }
    async fn list_orders_paginated(
        &self,
        filters: &OrderListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<OrderView>> {
        self.market_event_list_orders_paginated(filters, page).await
    }
    async fn count_trades(&self, filters: &TradeListFilters) -> Result<i64> {
        self.market_event_count_trades(filters).await
    }
    async fn list_trades_paginated(
        &self,
        filters: &TradeListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<TradeView>> {
        self.market_event_list_trades_paginated(filters, page).await
    }
    async fn count_positions(&self, filters: &PositionListFilters) -> Result<i64> {
        self.market_event_count_positions(filters).await
    }
    async fn list_positions_paginated(
        &self,
        filters: &PositionListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<PositionView>> {
        self.market_event_list_positions_paginated(filters, page)
            .await
    }

    async fn recompute_signal(
        &self,
        command: &RecomputeSignalCommand,
    ) -> Result<RecomputeSignalResult> {
        self.market_event_recompute_signal(command).await
    }

    async fn approve_signal(
        &self,
        signal_id: &str,
        approved_by_user_id: &str,
        approval_reason: &str,
        _trace_id: &str,
        expected_version: Option<i64>,
    ) -> Result<SignalView> {
        self.market_event_approve_signal(
            signal_id,
            approved_by_user_id,
            approval_reason,
            _trace_id,
            expected_version,
        )
        .await
    }

    async fn reject_signal(
        &self,
        signal_id: &str,
        rejected_by_user_id: &str,
        rejection_reason: &str,
        _trace_id: &str,
        expected_version: Option<i64>,
    ) -> Result<SignalView> {
        self.market_event_reject_signal(
            signal_id,
            rejected_by_user_id,
            rejection_reason,
            _trace_id,
            expected_version,
        )
        .await
    }

    async fn submit_execution_request(
        &self,
        command: &SubmitExecutionStoreCommand,
    ) -> Result<ExecutionSubmissionResult> {
        self.market_event_submit_execution_request(command).await
    }

    async fn list_dispatch_candidates(
        &self,
        filters: &DispatchExecutionListFilters,
    ) -> Result<Vec<ExecutionDispatchCandidate>> {
        self.market_event_list_dispatch_candidates(filters).await
    }

    async fn list_reconciliation_candidates(
        &self,
        filters: &ReconcileExecutionListFilters,
    ) -> Result<Vec<ExecutionReconciliationCandidate>> {
        self.market_event_list_reconciliation_candidates(filters)
            .await
    }

    async fn mark_execution_submitted(
        &self,
        execution_request_id: &str,
        account_id: &str,
        external_order_id: &str,
        _trace_id: &str,
    ) -> Result<ExecutionDispatchResult> {
        self.market_event_mark_execution_submitted(
            execution_request_id,
            account_id,
            external_order_id,
            _trace_id,
        )
        .await
    }

    async fn mark_order_open(&self, order_id: &str, _trace_id: &str) -> Result<OrderView> {
        self.market_event_mark_order_open(order_id, _trace_id).await
    }

    async fn mark_order_canceled(&self, order_id: &str, _trace_id: &str) -> Result<OrderView> {
        self.market_event_mark_order_canceled(order_id, _trace_id)
            .await
    }

    async fn mark_execution_failed(
        &self,
        execution_request_id: &str,
        failure_code: &str,
        failure_message: &str,
        _trace_id: &str,
    ) -> Result<ExecutionDispatchResult> {
        self.market_event_mark_execution_failed(
            execution_request_id,
            failure_code,
            failure_message,
            _trace_id,
        )
        .await
    }

    async fn reconcile_execution_fill(
        &self,
        execution_request_id: &str,
        account_id: &str,
        external_trade_id: &str,
        fill_price: Probability,
        filled_quantity: Quantity,
        fee: UsdAmount,
        trace_id: &str,
    ) -> Result<ExecutionFillResult> {
        self.market_event_reconcile_execution_fill(MarketEventExecutionFill {
            execution_request_id,
            account_id,
            external_trade_id,
            fill_price,
            filled_quantity,
            fee,
            trace_id,
        })
        .await
    }

    async fn ingest_fixture_bundle(
        &self,
        bundle: &FixtureBundle,
        _trace_id: &str,
    ) -> Result<FixtureIngestionReport> {
        self.market_event_ingest_fixture_bundle(bundle, _trace_id)
            .await
    }

    async fn upsert_markets(&self, markets: &[MarketView], _trace_id: &str) -> Result<usize> {
        let mut store = self.markets.write().await;
        for market in markets {
            store.insert(market.id.clone(), market.clone());
        }
        Ok(markets.len())
    }
}

#[async_trait]
impl NewsIngestionStore for InMemoryMarketEventStore {
    async fn list_news_source_health(
        &self,
        filters: &NewsSourceHealthListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<NewsSourceHealthView>> {
        let health = self.news_source_health.read().await;
        let mut items: Vec<_> = health
            .values()
            .filter(|item| {
                filters
                    .source_type
                    .as_deref()
                    .is_none_or(|source_type| item.source_type == source_type)
            })
            .cloned()
            .collect();
        items.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.source.cmp(&right.source))
        });
        let total = i64::try_from(items.len()).unwrap_or(i64::MAX);
        let offset = page.offset() as usize;
        let page_size = page.validated().1 as usize;
        let paged: Vec<_> = items.into_iter().skip(offset).take(page_size).collect();
        Ok(Paginated::new(paged, page, total))
    }

    async fn list_raw_news_events(
        &self,
        filters: &NewsRawEventListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<NewsRawEventView>> {
        let events = self.raw_news_events.read().await;
        let mut items: Vec<_> = events
            .values()
            .filter(|item| {
                filters
                    .source
                    .as_deref()
                    .is_none_or(|source| item.source == source)
                    && filters
                        .source_type
                        .as_deref()
                        .is_none_or(|source_type| item.source_type == source_type)
            })
            .cloned()
            .collect();
        items.sort_by(|left, right| {
            right
                .event_time
                .cmp(&left.event_time)
                .then_with(|| right.ingested_at.cmp(&left.ingested_at))
                .then_with(|| left.id.cmp(&right.id))
        });
        let total = i64::try_from(items.len()).unwrap_or(i64::MAX);
        let offset = page.offset() as usize;
        let page_size = page.validated().1 as usize;
        let paged: Vec<_> = items.into_iter().skip(offset).take(page_size).collect();
        Ok(Paginated::new(paged, page, total))
    }

    async fn insert_raw_news_event(&self, event: &NewsRawEventInsert) -> Result<bool> {
        let keys = raw_news_dedup_keys(event);
        let mut existing_keys = self.raw_news_dedup_keys.write().await;
        if keys.iter().any(|key| existing_keys.contains(key)) {
            return Ok(false);
        }

        existing_keys.extend(keys);
        self.raw_news_events
            .write()
            .await
            .insert(event.id.clone(), raw_news_event_view_from_insert(event));
        Ok(true)
    }

    async fn record_news_source_success(&self, update: &NewsSourceSuccessUpdate) -> Result<()> {
        let fetched = usize_to_u64(update.fetched)?;
        let inserted = usize_to_u64(update.inserted)?;
        let deduped = usize_to_u64(update.deduped)?;
        let mut health = self.news_source_health.write().await;

        if let Some(existing) = health.get_mut(&update.source) {
            existing.source_type = update.source_type.clone();
            existing.enabled = true;
            existing.reliability = update.reliability;
            existing.last_success_at = Some(update.observed_at);
            existing.consecutive_failures = 0;
            existing.items_fetched = add_news_count(existing.items_fetched, fetched)?;
            existing.items_inserted = add_news_count(existing.items_inserted, inserted)?;
            existing.items_deduped = add_news_count(existing.items_deduped, deduped)?;
            existing.health_score = update.reliability;
            existing.last_error = None;
            existing.updated_at = update.observed_at;
        } else {
            health.insert(
                update.source.clone(),
                NewsSourceHealthView {
                    source: update.source.clone(),
                    source_type: update.source_type.clone(),
                    enabled: true,
                    reliability: update.reliability,
                    last_success_at: Some(update.observed_at),
                    last_error_at: None,
                    consecutive_failures: 0,
                    items_fetched: fetched,
                    items_inserted: inserted,
                    items_deduped: deduped,
                    health_score: update.reliability,
                    last_error: None,
                    updated_at: update.observed_at,
                },
            );
        }

        Ok(())
    }

    async fn record_news_source_failure(&self, update: &NewsSourceFailureUpdate) -> Result<()> {
        let mut health = self.news_source_health.write().await;

        if let Some(existing) = health.get_mut(&update.source) {
            let consecutive_failures = add_news_count(existing.consecutive_failures, 1)?;
            existing.source_type = update.source_type.clone();
            existing.enabled = true;
            existing.reliability = update.reliability;
            existing.last_error_at = Some(update.observed_at);
            existing.consecutive_failures = consecutive_failures;
            existing.health_score =
                degraded_health_score(update.reliability, consecutive_failures)?;
            existing.last_error = Some(clamped_error_message(&update.error_message));
            existing.updated_at = update.observed_at;
        } else {
            health.insert(
                update.source.clone(),
                NewsSourceHealthView {
                    source: update.source.clone(),
                    source_type: update.source_type.clone(),
                    enabled: true,
                    reliability: update.reliability,
                    last_success_at: None,
                    last_error_at: Some(update.observed_at),
                    consecutive_failures: 1,
                    items_fetched: 0,
                    items_inserted: 0,
                    items_deduped: 0,
                    health_score: degraded_health_score(update.reliability, 1)?,
                    last_error: Some(clamped_error_message(&update.error_message)),
                    updated_at: update.observed_at,
                },
            );
        }

        Ok(())
    }
}

#[async_trait]
impl ArbitrageStore for InMemoryMarketEventStore {
    async fn start_arbitrage_scan(&self, scan: &ArbitrageScanView) -> Result<()> {
        self.arbitrage_scans
            .write()
            .await
            .insert(scan.id.clone(), scan.clone());
        Ok(())
    }

    async fn complete_arbitrage_scan(
        &self,
        scan_id: &str,
        finished_at: OffsetDateTime,
        market_count: u32,
        snapshot_count: u32,
        opportunity_count: u32,
    ) -> Result<ArbitrageScanView> {
        let mut scans = self.arbitrage_scans.write().await;
        let scan = scans.get_mut(scan_id).ok_or_else(|| {
            AppError::not_found(
                "ARBITRAGE_SCAN_NOT_FOUND",
                format!("arbitrage scan was not found: {scan_id}"),
            )
        })?;
        scan.finished_at = Some(finished_at);
        scan.market_count = market_count;
        scan.snapshot_count = snapshot_count;
        scan.opportunity_count = opportunity_count;
        Ok(scan.clone())
    }

    async fn record_market_book_snapshot(&self, snapshot: &MarketBookSnapshotView) -> Result<()> {
        self.market_book_snapshots
            .write()
            .await
            .insert(snapshot.id.clone(), snapshot.clone());
        Ok(())
    }

    async fn record_arbitrage_opportunity(
        &self,
        opportunity: &ArbitrageOpportunityView,
    ) -> Result<()> {
        self.arbitrage_opportunities
            .write()
            .await
            .insert(opportunity.id.clone(), opportunity.clone());
        Ok(())
    }

    async fn record_arbitrage_opportunity_validation(
        &self,
        validation: &ArbitrageOpportunityValidationView,
    ) -> Result<()> {
        self.arbitrage_opportunity_validations
            .write()
            .await
            .insert(validation.id.clone(), validation.clone());
        Ok(())
    }

    async fn expire_arbitrage_opportunities(
        &self,
        observed_before: OffsetDateTime,
        trace_id: &str,
    ) -> Result<Vec<ArbitrageOpportunityView>> {
        let mut opportunities = self.arbitrage_opportunities.write().await;
        let mut expired = Vec::new();

        for opportunity in opportunities.values_mut() {
            if opportunity.observed_at < observed_before
                && opportunity.status != ArbitrageOpportunityStatus::Expired
            {
                opportunity.status = ArbitrageOpportunityStatus::Expired;
                opportunity.trace_id = trace_id.to_string();
                expired.push(opportunity.clone());
            }
        }

        Ok(expired)
    }

    async fn list_arbitrage_scans(
        &self,
        _filters: &ArbitrageScanListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<ArbitrageScanView>> {
        let scans = self.arbitrage_scans.read().await;
        let mut items: Vec<_> = scans.values().cloned().collect();
        items.sort_by(|left, right| {
            right
                .started_at
                .cmp(&left.started_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        let total = i64::try_from(items.len()).unwrap_or(i64::MAX);
        let offset = page.offset() as usize;
        let page_size = page.validated().1 as usize;
        let paged: Vec<_> = items.into_iter().skip(offset).take(page_size).collect();
        Ok(Paginated::new(paged, page, total))
    }

    async fn list_arbitrage_opportunities(
        &self,
        filters: &ArbitrageOpportunityListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<ArbitrageOpportunityView>> {
        let opportunities = self.arbitrage_opportunities.read().await;
        let validations = self.arbitrage_opportunity_validations.read().await;
        let mut items: Vec<_> = opportunities
            .values()
            .filter(|opportunity| {
                filters
                    .market_id
                    .as_ref()
                    .is_none_or(|market_id| &opportunity.market_id == market_id)
                    && filters
                        .opportunity_type
                        .is_none_or(|kind| opportunity.opportunity_type == kind)
                    && filters
                        .status
                        .is_none_or(|status| opportunity.status == status)
                    && (!filters.active_only
                        || opportunity.status != ArbitrageOpportunityStatus::Expired)
                    && filters
                        .observed_after
                        .is_none_or(|after| opportunity.observed_at >= after)
            })
            .cloned()
            .map(|mut opportunity| {
                opportunity.validation =
                    latest_validation_for_opportunity(&validations, &opportunity.id);
                opportunity
            })
            .filter(|opportunity| {
                filters.validation_status.is_none_or(|status| {
                    if status == ArbitrageValidationStatus::Unvalidated {
                        opportunity.validation.is_none()
                    } else {
                        opportunity
                            .validation
                            .as_ref()
                            .is_some_and(|validation| validation.status == status)
                    }
                }) && filters.min_net_edge.is_none_or(|min_net_edge| {
                    opportunity
                        .validation
                        .as_ref()
                        .is_some_and(|validation| validation.net_edge >= min_net_edge)
                })
            })
            .collect();
        items.sort_by(|left, right| {
            right
                .observed_at
                .cmp(&left.observed_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        let total = i64::try_from(items.len()).unwrap_or(i64::MAX);
        let offset = page.offset() as usize;
        let page_size = page.validated().1 as usize;
        let paged: Vec<_> = items.into_iter().skip(offset).take(page_size).collect();
        Ok(Paginated::new(paged, page, total))
    }

    async fn record_arbitrage_analysis_run(
        &self,
        analysis: &ArbitrageAnalysisRunView,
    ) -> Result<()> {
        self.arbitrage_analysis_runs
            .write()
            .await
            .insert(analysis.id.clone(), analysis.clone());
        Ok(())
    }

    async fn list_arbitrage_analysis_runs(
        &self,
        _filters: &ArbitrageAnalysisRunListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<ArbitrageAnalysisRunView>> {
        let runs = self.arbitrage_analysis_runs.read().await;
        let mut items: Vec<_> = runs.values().cloned().collect();
        items.sort_by(|left, right| {
            right
                .generated_at
                .cmp(&left.generated_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        let total = i64::try_from(items.len()).unwrap_or(i64::MAX);
        let offset = page.offset() as usize;
        let page_size = page.validated().1 as usize;
        let paged: Vec<_> = items.into_iter().skip(offset).take(page_size).collect();
        Ok(Paginated::new(paged, page, total))
    }

    async fn record_arbitrage_event(
        &self,
        event: &ArbitrageEventView,
    ) -> Result<ArbitrageEventView> {
        let mut sequence = self.arbitrage_event_sequence.write().await;
        *sequence = sequence.saturating_add(1);
        let mut recorded = event.clone();
        recorded.sequence = *sequence;
        self.arbitrage_events.write().await.push(recorded.clone());
        Ok(recorded)
    }

    async fn list_arbitrage_events(
        &self,
        filters: &ArbitrageEventListFilters,
        page: &PageQuery,
    ) -> Result<Paginated<ArbitrageEventView>> {
        let events = self.arbitrage_events.read().await;
        let mut items = events
            .iter()
            .filter(|event| {
                filters
                    .after_sequence
                    .is_none_or(|after_sequence| event.sequence > after_sequence)
            })
            .cloned()
            .collect::<Vec<_>>();
        items.sort_by(|left, right| left.sequence.cmp(&right.sequence));
        let total = i64::try_from(items.len()).unwrap_or(i64::MAX);
        let offset = page.offset() as usize;
        let page_size = page.validated().1 as usize;
        let paged: Vec<_> = items.into_iter().skip(offset).take(page_size).collect();
        Ok(Paginated::new(paged, page, total))
    }

    async fn prune_arbitrage_events(&self, occurred_before: OffsetDateTime) -> Result<u64> {
        let mut events = self.arbitrage_events.write().await;
        let before = events.len();
        events.retain(|event| event.occurred_at >= occurred_before);
        usize_to_u64(before.saturating_sub(events.len()))
    }
}
