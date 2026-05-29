#[async_trait]
pub trait MarketEventStore: Send + Sync {
    async fn list_markets(&self, filters: &MarketListFilters) -> Result<Vec<MarketView>>;

    async fn count_markets(&self, filters: &MarketListFilters) -> Result<i64>;

    async fn list_market_categories(&self) -> Result<Vec<MarketCategoryView>>;

    async fn get_market(&self, market_id: &str) -> Result<Option<MarketView>>;

    async fn get_signal(&self, signal_id: &str) -> Result<Option<SignalView>>;

    async fn list_events(&self, filters: &EventListFilters) -> Result<Vec<EventView>>;

    async fn list_evidences(&self, filters: &EvidenceListFilters) -> Result<Vec<EvidenceView>>;

    async fn list_signals(&self, filters: &SignalListFilters) -> Result<Vec<SignalView>>;

    async fn list_probability_estimates(
        &self,
        filters: &ProbabilityEstimateListFilters,
    ) -> Result<Vec<ProbabilityEstimateView>>;

    async fn list_signal_transitions(
        &self,
        filters: &SignalTransitionListFilters,
    ) -> Result<Vec<SignalTransitionView>>;

    async fn recompute_signal(
        &self,
        command: &RecomputeSignalCommand,
    ) -> Result<RecomputeSignalResult>;

    async fn approve_signal(
        &self,
        signal_id: &str,
        approved_by_user_id: &str,
        approval_reason: &str,
        trace_id: &str,
        expected_version: Option<i64>,
    ) -> Result<SignalView>;

    async fn reject_signal(
        &self,
        signal_id: &str,
        rejected_by_user_id: &str,
        rejection_reason: &str,
        trace_id: &str,
        expected_version: Option<i64>,
    ) -> Result<SignalView>;

    async fn list_order_drafts(
        &self,
        filters: &OrderDraftListFilters,
    ) -> Result<Vec<OrderDraftView>>;

    async fn list_execution_requests(
        &self,
        filters: &ExecutionRequestListFilters,
    ) -> Result<Vec<ExecutionRequestView>>;

    async fn get_order_by_external_ref(
        &self,
        connector_name: &str,
        external_order_id: &str,
    ) -> Result<OrderView>;

    async fn list_orders(&self, filters: &OrderListFilters) -> Result<Vec<OrderView>>;

    async fn list_trades(&self, filters: &TradeListFilters) -> Result<Vec<TradeView>>;

    async fn list_positions(&self, filters: &PositionListFilters) -> Result<Vec<PositionView>>;

    async fn submit_execution_request(
        &self,
        command: &SubmitExecutionStoreCommand,
    ) -> Result<ExecutionSubmissionResult>;

    async fn list_dispatch_candidates(
        &self,
        filters: &DispatchExecutionListFilters,
    ) -> Result<Vec<ExecutionDispatchCandidate>>;

    async fn list_reconciliation_candidates(
        &self,
        filters: &ReconcileExecutionListFilters,
    ) -> Result<Vec<ExecutionReconciliationCandidate>>;

    async fn mark_execution_submitted(
        &self,
        execution_request_id: &str,
        account_id: &str,
        external_order_id: &str,
        trace_id: &str,
    ) -> Result<ExecutionDispatchResult>;

    async fn mark_order_open(&self, order_id: &str, trace_id: &str) -> Result<OrderView>;

    async fn mark_order_canceled(&self, order_id: &str, trace_id: &str) -> Result<OrderView>;

    async fn mark_execution_failed(
        &self,
        execution_request_id: &str,
        failure_code: &str,
        failure_message: &str,
        trace_id: &str,
    ) -> Result<ExecutionDispatchResult>;

    async fn reconcile_execution_fill(
        &self,
        execution_request_id: &str,
        account_id: &str,
        external_trade_id: &str,
        fill_price: Probability,
        filled_quantity: Quantity,
        fee: UsdAmount,
        trace_id: &str,
    ) -> Result<ExecutionFillResult>;

    async fn ingest_fixture_bundle(
        &self,
        bundle: &FixtureBundle,
        trace_id: &str,
    ) -> Result<FixtureIngestionReport>;

    async fn upsert_markets(&self, markets: &[MarketView], trace_id: &str) -> Result<usize>;
}

pub struct MarketEventService {
    store: Arc<dyn MarketEventStore>,
}

impl MarketEventService {
    pub fn new(store: Arc<dyn MarketEventStore>) -> Self {
        Self { store }
    }

    pub async fn list_markets(&self, filters: MarketListFilters) -> Result<Vec<MarketView>> {
        self.store.list_markets(&filters).await
    }

    pub async fn count_markets(&self, filters: MarketListFilters) -> Result<i64> {
        self.store.count_markets(&filters).await
    }

    pub async fn list_market_categories(&self) -> Result<Vec<MarketCategoryView>> {
        self.store.list_market_categories().await
    }

    pub async fn get_market(&self, market_id: &str) -> Result<MarketView> {
        if market_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "MARKET_ID_REQUIRED",
                "market id must not be empty",
            ));
        }

        self.store.get_market(market_id).await?.ok_or_else(|| {
            AppError::not_found(
                "MARKET_NOT_FOUND",
                format!("market was not found: {market_id}"),
            )
        })
    }

    pub async fn get_signal(&self, signal_id: &str) -> Result<SignalView> {
        if signal_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "SIGNAL_ID_REQUIRED",
                "signal id must not be empty",
            ));
        }

        self.store.get_signal(signal_id).await?.ok_or_else(|| {
            AppError::not_found(
                "SIGNAL_NOT_FOUND",
                format!("signal was not found: {signal_id}"),
            )
        })
    }

    pub async fn list_events(&self, filters: EventListFilters) -> Result<Vec<EventView>> {
        self.store.list_events(&filters).await
    }

    pub async fn list_evidences(&self, filters: EvidenceListFilters) -> Result<Vec<EvidenceView>> {
        self.store.list_evidences(&filters).await
    }

    pub async fn list_signals(&self, filters: SignalListFilters) -> Result<Vec<SignalView>> {
        self.store.list_signals(&filters).await
    }

    pub async fn list_probability_estimates(
        &self,
        filters: ProbabilityEstimateListFilters,
    ) -> Result<Vec<ProbabilityEstimateView>> {
        self.store.list_probability_estimates(&filters).await
    }

    pub async fn list_signal_transitions(
        &self,
        filters: SignalTransitionListFilters,
    ) -> Result<Vec<SignalTransitionView>> {
        self.store.list_signal_transitions(&filters).await
    }

    pub async fn recompute_signal(
        &self,
        signal_id: impl Into<String>,
        reason: impl Into<String>,
        trace_id: impl Into<String>,
    ) -> Result<RecomputeSignalResult> {
        let signal_id = signal_id.into();
        if signal_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "SIGNAL_ID_REQUIRED",
                "signal id must not be empty",
            ));
        }

        let reason = reason.into();
        if reason.trim().is_empty() {
            return Err(AppError::invalid_input(
                "SIGNAL_RECOMPUTE_REASON_REQUIRED",
                "reason must not be empty",
            ));
        }

        self.store
            .recompute_signal(&RecomputeSignalCommand {
                signal_id: signal_id.trim().to_string(),
                reason: reason.trim().to_string(),
                trace_id: trace_id.into(),
            })
            .await
    }

    pub async fn approve_signal(
        &self,
        signal_id: impl Into<String>,
        approved_by_user_id: impl Into<String>,
        approval_reason: impl Into<String>,
        trace_id: impl Into<String>,
        expected_version: Option<i64>,
    ) -> Result<SignalView> {
        let signal_id = signal_id.into();
        if signal_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "SIGNAL_ID_REQUIRED",
                "signal id must not be empty",
            ));
        }

        let approved_by_user_id = approved_by_user_id.into();
        if approved_by_user_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "SIGNAL_APPROVED_BY_REQUIRED",
                "approved_by_user_id must not be empty",
            ));
        }

        let approval_reason = approval_reason.into();
        if approval_reason.trim().is_empty() {
            return Err(AppError::invalid_input(
                "SIGNAL_APPROVAL_REASON_REQUIRED",
                "approval reason must not be empty",
            ));
        }
        let trace_id = trace_id.into();

        self.store
            .approve_signal(
                signal_id.trim(),
                approved_by_user_id.trim(),
                approval_reason.trim(),
                &trace_id,
                expected_version,
            )
            .await
    }

    pub async fn reject_signal(
        &self,
        signal_id: impl Into<String>,
        rejected_by_user_id: impl Into<String>,
        rejection_reason: impl Into<String>,
        trace_id: impl Into<String>,
        expected_version: Option<i64>,
    ) -> Result<SignalView> {
        let signal_id = signal_id.into();
        if signal_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "SIGNAL_ID_REQUIRED",
                "signal id must not be empty",
            ));
        }

        let rejected_by_user_id = rejected_by_user_id.into();
        if rejected_by_user_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "SIGNAL_REJECTED_BY_REQUIRED",
                "rejected_by_user_id must not be empty",
            ));
        }

        let rejection_reason = rejection_reason.into();
        if rejection_reason.trim().is_empty() {
            return Err(AppError::invalid_input(
                "SIGNAL_REJECTION_REASON_REQUIRED",
                "rejection reason must not be empty",
            ));
        }
        let trace_id = trace_id.into();

        self.store
            .reject_signal(
                signal_id.trim(),
                rejected_by_user_id.trim(),
                rejection_reason.trim(),
                &trace_id,
                expected_version,
            )
            .await
    }

    pub async fn list_order_drafts(
        &self,
        filters: OrderDraftListFilters,
    ) -> Result<Vec<OrderDraftView>> {
        self.store.list_order_drafts(&filters).await
    }

    pub async fn list_execution_requests(
        &self,
        filters: ExecutionRequestListFilters,
    ) -> Result<Vec<ExecutionRequestView>> {
        self.store.list_execution_requests(&filters).await
    }

    pub async fn get_order_by_external_ref(
        &self,
        connector_name: impl Into<String>,
        external_order_id: impl Into<String>,
    ) -> Result<OrderView> {
        let connector_name = connector_name.into();
        if connector_name.trim().is_empty() {
            return Err(AppError::invalid_input(
                "EXECUTION_CONNECTOR_NAME_REQUIRED",
                "connector name must not be empty",
            ));
        }

        let external_order_id = external_order_id.into();
        if external_order_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "EXTERNAL_ORDER_ID_REQUIRED",
                "external order id must not be empty",
            ));
        }

        self.store
            .get_order_by_external_ref(connector_name.trim(), external_order_id.trim())
            .await
    }

    pub async fn list_orders(&self, filters: OrderListFilters) -> Result<Vec<OrderView>> {
        self.store.list_orders(&filters).await
    }

    pub async fn list_trades(&self, filters: TradeListFilters) -> Result<Vec<TradeView>> {
        self.store.list_trades(&filters).await
    }

    pub async fn list_positions(&self, filters: PositionListFilters) -> Result<Vec<PositionView>> {
        self.store.list_positions(&filters).await
    }

    pub async fn submit_execution_request(
        &self,
        command: SubmitExecutionStoreCommand,
    ) -> Result<ExecutionSubmissionResult> {
        if command.signal_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "SIGNAL_ID_REQUIRED",
                "signal id must not be empty",
            ));
        }

        if command.reason.trim().is_empty() {
            return Err(AppError::invalid_input(
                "EXECUTION_REQUEST_REASON_REQUIRED",
                "execution request reason must not be empty",
            ));
        }

        if command.connector_name.trim().is_empty() {
            return Err(AppError::invalid_input(
                "EXECUTION_CONNECTOR_NAME_REQUIRED",
                "connector name must not be empty",
            ));
        }

        self.store.submit_execution_request(&command).await
    }

    pub async fn list_dispatch_candidates(
        &self,
        filters: DispatchExecutionListFilters,
    ) -> Result<Vec<ExecutionDispatchCandidate>> {
        self.store.list_dispatch_candidates(&filters).await
    }

    pub async fn list_reconciliation_candidates(
        &self,
        filters: ReconcileExecutionListFilters,
    ) -> Result<Vec<ExecutionReconciliationCandidate>> {
        self.store.list_reconciliation_candidates(&filters).await
    }

    pub async fn mark_execution_submitted(
        &self,
        execution_request_id: impl Into<String>,
        account_id: impl Into<String>,
        external_order_id: impl Into<String>,
        trace_id: impl Into<String>,
    ) -> Result<ExecutionDispatchResult> {
        let execution_request_id = execution_request_id.into();
        if execution_request_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "EXECUTION_REQUEST_ID_REQUIRED",
                "execution request id must not be empty",
            ));
        }

        let account_id = account_id.into();
        if account_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "EXECUTION_ACCOUNT_ID_REQUIRED",
                "execution account id must not be empty",
            ));
        }

        let external_order_id = external_order_id.into();
        if external_order_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "EXTERNAL_ORDER_ID_REQUIRED",
                "external order id must not be empty",
            ));
        }

        let trace_id = trace_id.into();
        self.store
            .mark_execution_submitted(
                execution_request_id.trim(),
                account_id.trim(),
                external_order_id.trim(),
                &trace_id,
            )
            .await
    }

    pub async fn mark_order_open(
        &self,
        order_id: impl Into<String>,
        trace_id: impl Into<String>,
    ) -> Result<OrderView> {
        let order_id = order_id.into();
        if order_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "ORDER_ID_REQUIRED",
                "order id must not be empty",
            ));
        }

        let trace_id = trace_id.into();
        self.store.mark_order_open(order_id.trim(), &trace_id).await
    }

    pub async fn mark_order_canceled(
        &self,
        order_id: impl Into<String>,
        trace_id: impl Into<String>,
    ) -> Result<OrderView> {
        let order_id = order_id.into();
        if order_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "ORDER_ID_REQUIRED",
                "order id must not be empty",
            ));
        }

        let trace_id = trace_id.into();
        self.store
            .mark_order_canceled(order_id.trim(), &trace_id)
            .await
    }

    pub async fn mark_execution_failed(
        &self,
        execution_request_id: impl Into<String>,
        failure_code: impl Into<String>,
        failure_message: impl Into<String>,
        trace_id: impl Into<String>,
    ) -> Result<ExecutionDispatchResult> {
        let execution_request_id = execution_request_id.into();
        if execution_request_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "EXECUTION_REQUEST_ID_REQUIRED",
                "execution request id must not be empty",
            ));
        }

        let failure_code = failure_code.into();
        if failure_code.trim().is_empty() {
            return Err(AppError::invalid_input(
                "EXECUTION_FAILURE_CODE_REQUIRED",
                "failure code must not be empty",
            ));
        }

        let failure_message = failure_message.into();
        if failure_message.trim().is_empty() {
            return Err(AppError::invalid_input(
                "EXECUTION_FAILURE_MESSAGE_REQUIRED",
                "failure message must not be empty",
            ));
        }

        let trace_id = trace_id.into();
        self.store
            .mark_execution_failed(
                execution_request_id.trim(),
                failure_code.trim(),
                failure_message.trim(),
                &trace_id,
            )
            .await
    }

    pub async fn reconcile_execution_fill(
        &self,
        execution_request_id: impl Into<String>,
        account_id: impl Into<String>,
        external_trade_id: impl Into<String>,
        fill_price: Probability,
        filled_quantity: Quantity,
        fee: UsdAmount,
        trace_id: impl Into<String>,
    ) -> Result<ExecutionFillResult> {
        let execution_request_id = execution_request_id.into();
        if execution_request_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "EXECUTION_REQUEST_ID_REQUIRED",
                "execution request id must not be empty",
            ));
        }

        let account_id = account_id.into();
        if account_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "EXECUTION_ACCOUNT_ID_REQUIRED",
                "account id must not be empty",
            ));
        }

        let external_trade_id = external_trade_id.into();
        if external_trade_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "EXTERNAL_TRADE_ID_REQUIRED",
                "external trade id must not be empty",
            ));
        }

        if filled_quantity.value() <= Decimal::ZERO {
            return Err(AppError::invalid_input(
                "EXECUTION_FILL_QUANTITY_REQUIRED",
                "filled quantity must be greater than zero",
            ));
        }

        let trace_id = trace_id.into();
        self.store
            .reconcile_execution_fill(
                execution_request_id.trim(),
                account_id.trim(),
                external_trade_id.trim(),
                fill_price,
                filled_quantity,
                fee,
                &trace_id,
            )
            .await
    }

    pub async fn ingest_fixture_bundle(
        &self,
        bundle: FixtureBundle,
        trace_id: &str,
    ) -> Result<FixtureIngestionReport> {
        self.store.ingest_fixture_bundle(&bundle, trace_id).await
    }

    pub async fn upsert_markets(&self, markets: &[MarketView], trace_id: &str) -> Result<usize> {
        self.store.upsert_markets(markets, trace_id).await
    }
}
