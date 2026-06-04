use super::*;

include!("market_event/queries.rs");
include!("market_event/signals.rs");
include!("market_event/execution_submit.rs");
include!("market_event/execution_updates.rs");
include!("market_event/fixtures.rs");
include!("market_event/upsert.rs");

#[async_trait]
impl MarketEventStore for PostgresMarketEventStore {
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
        trace_id: &str,
        expected_version: Option<i64>,
    ) -> Result<SignalView> {
        self.market_event_approve_signal(
            signal_id,
            approved_by_user_id,
            approval_reason,
            trace_id,
            expected_version,
        )
        .await
    }

    async fn reject_signal(
        &self,
        signal_id: &str,
        rejected_by_user_id: &str,
        rejection_reason: &str,
        trace_id: &str,
        expected_version: Option<i64>,
    ) -> Result<SignalView> {
        self.market_event_reject_signal(
            signal_id,
            rejected_by_user_id,
            rejection_reason,
            trace_id,
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
        trace_id: &str,
    ) -> Result<ExecutionDispatchResult> {
        self.market_event_mark_execution_submitted(
            execution_request_id,
            account_id,
            external_order_id,
            trace_id,
        )
        .await
    }

    async fn mark_order_open(&self, order_id: &str, trace_id: &str) -> Result<OrderView> {
        self.market_event_mark_order_open(order_id, trace_id).await
    }

    async fn mark_order_canceled(&self, order_id: &str, trace_id: &str) -> Result<OrderView> {
        self.market_event_mark_order_canceled(order_id, trace_id)
            .await
    }

    async fn mark_execution_failed(
        &self,
        execution_request_id: &str,
        failure_code: &str,
        failure_message: &str,
        trace_id: &str,
    ) -> Result<ExecutionDispatchResult> {
        self.market_event_mark_execution_failed(
            execution_request_id,
            failure_code,
            failure_message,
            trace_id,
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
        trace_id: &str,
    ) -> Result<FixtureIngestionReport> {
        self.market_event_ingest_fixture_bundle(bundle, trace_id)
            .await
    }

    async fn upsert_markets(&self, markets: &[MarketView], trace_id: &str) -> Result<usize> {
        self.market_event_upsert_markets(markets, trace_id).await
    }
}
