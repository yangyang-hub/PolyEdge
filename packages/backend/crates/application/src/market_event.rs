use crate::execution::{
    DispatchExecutionListFilters, ExecutionDispatchCandidate, ExecutionDispatchResult,
    ExecutionFillResult, ExecutionReconciliationCandidate, ExecutionRequestListFilters,
    ExecutionRequestView, ExecutionSubmissionResult, OrderDraftListFilters, OrderDraftView,
    OrderListFilters, OrderView, PositionListFilters, PositionView, ReconcileExecutionListFilters,
    SubmitExecutionStoreCommand, TradeListFilters, TradeView,
};
use async_trait::async_trait;
use polyedge_domain::{
    AmbiguityLevel, AppError, Edge, EventStatus, EvidenceDirection, EvidenceStatus, MarketStatus,
    Probability, Quantity, Result, SignalAction, SignalLifecycleState, SignalSide, TimeHorizon,
    TradabilityStatus, UsdAmount,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{str::FromStr, sync::Arc};
use time::OffsetDateTime;

const DEFAULT_LIST_LIMIT: u16 = 100;
const MAX_LIST_LIMIT: u16 = 200;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketView {
    pub id: String,
    pub question: String,
    pub category: String,
    pub status: MarketStatus,
    pub best_bid: Probability,
    pub best_ask: Probability,
    pub mid_price: Probability,
    pub volume_24h: UsdAmount,
    pub ambiguity_level: AmbiguityLevel,
    pub tradability_status: TradabilityStatus,
    pub resolution_source: String,
    pub edge_case_notes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polymarket_condition_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polymarket_yes_asset_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polymarket_no_asset_id: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

impl MarketView {
    #[must_use]
    pub fn polymarket_asset_id_for_side(&self, side: SignalSide) -> Option<&str> {
        match side {
            SignalSide::Yes => self.polymarket_yes_asset_id.as_deref(),
            SignalSide::No => self.polymarket_no_asset_id.as_deref(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventView {
    pub id: String,
    pub source: String,
    pub summary: String,
    pub relevance_score: Probability,
    pub confidence: Probability,
    pub status: EventStatus,
    pub related_market_ids: Vec<String>,
    pub reason_trace: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceView {
    pub id: String,
    pub market_id: String,
    pub event_id: String,
    pub direction: EvidenceDirection,
    pub strength: Probability,
    pub source_reliability: Probability,
    pub novelty: Probability,
    pub resolution_relevance: Probability,
    pub status: EvidenceStatus,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalView {
    pub id: String,
    pub market_id: String,
    pub event_id: String,
    pub action: SignalAction,
    pub side: SignalSide,
    pub market_price: Probability,
    pub fair_price: Probability,
    pub edge: Edge,
    pub confidence: Probability,
    pub lifecycle_state: SignalLifecycleState,
    pub reason: String,
    pub risk_decision: String,
    pub evidence_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_by_user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub approved_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejected_by_user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub rejected_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbabilityEstimateView {
    pub id: String,
    pub market_id: String,
    pub event_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal_id: Option<String>,
    pub prior_price: Probability,
    pub posterior_price: Probability,
    pub fair_price: Probability,
    pub market_price: Probability,
    pub edge: Edge,
    pub confidence: Probability,
    pub time_horizon: TimeHorizon,
    pub model_version: String,
    pub reason_codes: Vec<String>,
    pub evidence_count: u32,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalTransitionView {
    pub id: String,
    pub signal_id: String,
    pub from_state: SignalLifecycleState,
    pub to_state: SignalLifecycleState,
    pub trigger_type: String,
    pub trigger_payload: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct MarketListFilters {
    pub status: Option<MarketStatus>,
    pub tradability_status: Option<TradabilityStatus>,
    pub limit: u16,
}

impl MarketListFilters {
    pub fn new(
        status: Option<MarketStatus>,
        tradability_status: Option<TradabilityStatus>,
        limit: Option<u16>,
    ) -> Result<Self> {
        Ok(Self {
            status,
            tradability_status,
            limit: validate_limit(limit)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct EventListFilters {
    pub status: Option<EventStatus>,
    pub limit: u16,
}

impl EventListFilters {
    pub fn new(status: Option<EventStatus>, limit: Option<u16>) -> Result<Self> {
        Ok(Self {
            status,
            limit: validate_limit(limit)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct EvidenceListFilters {
    pub market_id: Option<String>,
    pub event_id: Option<String>,
    pub status: Option<EvidenceStatus>,
    pub limit: u16,
}

impl EvidenceListFilters {
    pub fn new(
        market_id: Option<String>,
        event_id: Option<String>,
        status: Option<EvidenceStatus>,
        limit: Option<u16>,
    ) -> Result<Self> {
        Ok(Self {
            market_id: validate_optional_id("market_id", market_id)?,
            event_id: validate_optional_id("event_id", event_id)?,
            status,
            limit: validate_limit(limit)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct SignalListFilters {
    pub market_id: Option<String>,
    pub event_id: Option<String>,
    pub lifecycle_state: Option<SignalLifecycleState>,
    pub limit: u16,
}

impl SignalListFilters {
    pub fn new(
        market_id: Option<String>,
        event_id: Option<String>,
        lifecycle_state: Option<SignalLifecycleState>,
        limit: Option<u16>,
    ) -> Result<Self> {
        Ok(Self {
            market_id: validate_optional_id("market_id", market_id)?,
            event_id: validate_optional_id("event_id", event_id)?,
            lifecycle_state,
            limit: validate_limit(limit)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ProbabilityEstimateListFilters {
    pub market_id: Option<String>,
    pub event_id: Option<String>,
    pub signal_id: Option<String>,
    pub limit: u16,
}

impl ProbabilityEstimateListFilters {
    pub fn new(
        market_id: Option<String>,
        event_id: Option<String>,
        signal_id: Option<String>,
        limit: Option<u16>,
    ) -> Result<Self> {
        Ok(Self {
            market_id: validate_optional_id("market_id", market_id)?,
            event_id: validate_optional_id("event_id", event_id)?,
            signal_id: validate_optional_id("signal_id", signal_id)?,
            limit: validate_limit(limit)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct SignalTransitionListFilters {
    pub signal_id: String,
    pub limit: u16,
}

impl SignalTransitionListFilters {
    pub fn new(signal_id: impl Into<String>, limit: Option<u16>) -> Result<Self> {
        let signal_id = signal_id.into();
        if signal_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "SIGNAL_ID_REQUIRED",
                "signal id must not be empty",
            ));
        }

        Ok(Self {
            signal_id: signal_id.trim().to_string(),
            limit: validate_limit(limit)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureMarketRecord {
    pub id: String,
    pub question: String,
    pub category: String,
    pub status: MarketStatus,
    pub best_bid: Probability,
    pub best_ask: Probability,
    pub mid_price: Probability,
    pub volume_24h: UsdAmount,
    pub ambiguity_level: AmbiguityLevel,
    pub tradability_status: TradabilityStatus,
    pub resolution_source: String,
    pub edge_case_notes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polymarket_condition_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polymarket_yes_asset_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polymarket_no_asset_id: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureEventRecord {
    pub id: String,
    pub source: String,
    pub summary: String,
    pub relevance_score: Probability,
    pub confidence: Probability,
    pub status: EventStatus,
    pub related_market_ids: Vec<String>,
    pub reason_trace: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureEvidenceRecord {
    pub id: String,
    pub market_id: String,
    pub event_id: String,
    pub direction: EvidenceDirection,
    pub strength: Probability,
    pub source_reliability: Probability,
    pub novelty: Probability,
    pub resolution_relevance: Probability,
    pub status: EvidenceStatus,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureSignalRecord {
    pub id: String,
    pub market_id: String,
    pub event_id: String,
    pub action: SignalAction,
    pub side: SignalSide,
    pub market_price: Probability,
    pub fair_price: Probability,
    pub edge: Edge,
    pub confidence: Probability,
    pub lifecycle_state: SignalLifecycleState,
    pub reason: String,
    pub risk_decision: String,
    pub evidence_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_by_user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub approved_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejected_by_user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub rejected_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FixtureBundle {
    pub markets: Vec<FixtureMarketRecord>,
    pub events: Vec<FixtureEventRecord>,
    pub evidences: Vec<FixtureEvidenceRecord>,
    pub signals: Vec<FixtureSignalRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureIngestionReport {
    pub markets_upserted: usize,
    pub events_upserted: usize,
    pub evidences_upserted: usize,
    pub signals_upserted: usize,
}

#[derive(Debug, Clone)]
pub struct RecomputeSignalCommand {
    pub signal_id: String,
    pub reason: String,
    pub trace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecomputeSignalResult {
    pub signal: SignalView,
    pub estimate: ProbabilityEstimateView,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition: Option<SignalTransitionView>,
}

#[derive(Debug, Clone)]
pub struct RecomputeSignalDraft {
    pub next_signal: SignalView,
    pub estimate: ProbabilityEstimateView,
    pub transition: Option<SignalTransitionDraft>,
}

#[derive(Debug, Clone)]
pub struct SourceHealthAdjustment {
    pub source: String,
    pub health_score: Probability,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalTransitionDraft {
    pub from_state: SignalLifecycleState,
    pub to_state: SignalLifecycleState,
    pub trigger_type: String,
    pub trigger_payload: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[async_trait]
pub trait MarketEventStore: Send + Sync {
    async fn list_markets(&self, filters: &MarketListFilters) -> Result<Vec<MarketView>>;

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
}

pub fn build_recompute_signal_draft(
    signal: &SignalView,
    market: &MarketView,
    evidences: &[EvidenceView],
    recompute_reason: &str,
    estimate_id: impl Into<String>,
) -> Result<RecomputeSignalDraft> {
    build_recompute_signal_draft_with_source_health(
        signal,
        market,
        evidences,
        recompute_reason,
        None,
        estimate_id,
    )
}

pub fn build_recompute_signal_draft_with_source_health(
    signal: &SignalView,
    market: &MarketView,
    evidences: &[EvidenceView],
    recompute_reason: &str,
    source_health: Option<&SourceHealthAdjustment>,
    estimate_id: impl Into<String>,
) -> Result<RecomputeSignalDraft> {
    let active_evidences: Vec<_> = evidences
        .iter()
        .filter(|evidence| evidence.status == EvidenceStatus::Active)
        .cloned()
        .collect();

    let reference_time = active_evidences
        .iter()
        .map(|evidence| evidence.updated_at)
        .fold(max_time(signal.updated_at, market.updated_at), max_time);

    let evidence_count = active_evidences.len();
    let prior_price = market.mid_price;
    let market_price = market.mid_price;

    let source_health_score = source_health
        .map(|adjustment| adjustment.health_score.value())
        .unwrap_or(Decimal::ONE);
    let (weighted_delta, avg_signal_quality) =
        compute_evidence_signal(&active_evidences, reference_time, source_health_score);
    let ambiguity_factor = match market.ambiguity_level {
        AmbiguityLevel::Low => dec("1.00"),
        AmbiguityLevel::Medium => dec("0.85"),
        AmbiguityLevel::High => dec("0.70"),
    };
    let posterior_raw =
        clamp_zero_one(prior_price.value() + weighted_delta * ambiguity_factor * dec("0.25"));
    let posterior_price = probability_from_decimal(posterior_raw)?;
    let fair_price = posterior_price;
    let edge = Edge::new(fair_price.value() - market_price.value())?;

    let ambiguity_penalty = match market.ambiguity_level {
        AmbiguityLevel::Low => Decimal::ZERO,
        AmbiguityLevel::Medium => dec("0.05"),
        AmbiguityLevel::High => dec("0.10"),
    };
    let tradability_penalty = match market.tradability_status {
        TradabilityStatus::Tradable => Decimal::ZERO,
        TradabilityStatus::ManualReview => dec("0.04"),
        TradabilityStatus::ObserveOnly => dec("0.08"),
        TradabilityStatus::Blocked => dec("0.12"),
    };
    let confidence_raw = clamp_zero_one(
        dec("0.35") + avg_signal_quality * dec("0.40") - ambiguity_penalty - tradability_penalty,
    );
    let confidence = probability_from_decimal(confidence_raw)?;

    let time_horizon = derive_time_horizon(&active_evidences, reference_time);
    let reason_codes = derive_reason_codes(
        market,
        &active_evidences,
        edge,
        confidence,
        evidence_count,
        source_health,
    );
    let next_side = if edge.value() >= Decimal::ZERO {
        SignalSide::Yes
    } else {
        SignalSide::No
    };
    let directional_edge = if next_side == SignalSide::Yes {
        edge.value()
    } else {
        -edge.value()
    };
    let next_state = derive_signal_lifecycle_state(
        directional_edge,
        confidence,
        evidence_count,
        market.tradability_status,
    );
    let risk_decision = derive_risk_decision(market.tradability_status, next_state);
    let reason = format!(
        "posterior recomputed from {} active evidence(s): {}",
        evidence_count,
        reason_codes.join(", ")
    );

    let estimate = ProbabilityEstimateView {
        id: estimate_id.into(),
        market_id: signal.market_id.clone(),
        event_id: signal.event_id.clone(),
        signal_id: Some(signal.id.clone()),
        prior_price,
        posterior_price,
        fair_price,
        market_price,
        edge,
        confidence,
        time_horizon,
        model_version: "v1_evidence_weighted".to_string(),
        reason_codes: reason_codes.clone(),
        evidence_count: u32::try_from(evidence_count).unwrap_or(u32::MAX),
        created_at: reference_time,
    };

    let next_signal = SignalView {
        id: signal.id.clone(),
        market_id: signal.market_id.clone(),
        event_id: signal.event_id.clone(),
        action: SignalAction::Buy,
        side: next_side,
        market_price,
        fair_price,
        edge,
        confidence,
        lifecycle_state: next_state,
        reason,
        risk_decision,
        evidence_ids: active_evidences
            .into_iter()
            .map(|evidence| evidence.id)
            .collect(),
        approved_by_user_id: None,
        approved_at: None,
        rejected_by_user_id: None,
        rejected_at: None,
        updated_at: reference_time,
        version: signal.version + 1,
    };

    let transition = if signal.lifecycle_state != next_state {
        Some(SignalTransitionDraft {
            from_state: signal.lifecycle_state,
            to_state: next_state,
            trigger_type: "recompute".to_string(),
            trigger_payload: json!({
                "reason": recompute_reason,
                "estimate_id": estimate.id,
                "reason_codes": reason_codes,
                "prior_price": estimate.prior_price,
                "posterior_price": estimate.posterior_price,
                "market_tradability_status": market.tradability_status,
                "source_health": source_health.map(|adjustment| json!({
                    "source": adjustment.source,
                    "health_score": adjustment.health_score,
                })),
            }),
            created_at: reference_time,
        })
    } else {
        None
    };

    Ok(RecomputeSignalDraft {
        next_signal,
        estimate,
        transition,
    })
}

#[must_use]
pub fn demo_fixture_bundle() -> FixtureBundle {
    FixtureBundle {
        markets: vec![
            fixture_market(
                "mkt_120",
                "Will BTC close above 95k on Apr 30?",
                "Crypto",
                MarketStatus::Open,
                "0.51",
                "0.53",
                "0.52",
                "125000.00",
                AmbiguityLevel::Low,
                TradabilityStatus::Tradable,
                "Polymarket BTC settlement reference close.",
                &[
                    "Low ambiguity market.",
                    "Resolution uses published close methodology.",
                ],
                Some("0x0000000000000000000000000000000000000000000000000000000000000120"),
                Some("120001"),
                Some("120002"),
                "2026-04-16T14:30:00Z",
                12,
            ),
            fixture_market(
                "mkt_121",
                "Will SEC approve ETH staking ETF by Q2?",
                "Regulation",
                MarketStatus::Open,
                "0.40",
                "0.42",
                "0.41",
                "98400.00",
                AmbiguityLevel::Medium,
                TradabilityStatus::ManualReview,
                "Official SEC filing or public approval announcement.",
                &[
                    "Delayed filing language may not equal approval.",
                    "Conditional launch wording requires operator review.",
                    "Partial scope approval should be escalated.",
                ],
                Some("0x0000000000000000000000000000000000000000000000000000000000000121"),
                Some("121001"),
                Some("121002"),
                "2026-04-16T14:30:00Z",
                9,
            ),
            fixture_market(
                "mkt_122",
                "Will the Fed cut rates in June?",
                "Macro",
                MarketStatus::Open,
                "0.62",
                "0.64",
                "0.63",
                "141200.00",
                AmbiguityLevel::High,
                TradabilityStatus::ObserveOnly,
                "FOMC target rate decision.",
                &[
                    "Interpretation risk around corridor adjustments.",
                    "Observe only until macro source stabilizes.",
                ],
                Some("0x0000000000000000000000000000000000000000000000000000000000000122"),
                Some("122001"),
                Some("122002"),
                "2026-04-16T14:30:00Z",
                7,
            ),
            fixture_market(
                "mkt_123",
                "Will the White House publish a formal AI executive order by May 31?",
                "Policy",
                MarketStatus::Open,
                "0.36",
                "0.39",
                "0.38",
                "54200.00",
                AmbiguityLevel::High,
                TradabilityStatus::Blocked,
                "Official White House release log and executive order registry.",
                &[
                    "Draft memos do not satisfy settlement unless a formal executive order is published.",
                    "Block automation until the official publication source is stable again.",
                ],
                Some("0x0000000000000000000000000000000000000000000000000000000000000123"),
                Some("123001"),
                Some("123002"),
                "2026-04-16T14:30:00Z",
                5,
            ),
        ],
        events: vec![
            fixture_event(
                "evt_9001",
                "reuters",
                "Senior SEC staff signals concerns over ETH staking disclosures.",
                "0.81",
                "0.78",
                EventStatus::Active,
                &["mkt_121"],
                "Official language changes settlement path relevance and supports a lower approval probability.",
                "2026-04-16T13:42:00Z",
                "2026-04-16T14:30:00Z",
                3,
            ),
            fixture_event(
                "evt_9002",
                "fomc_calendar",
                "Fed speakers reinforce patience narrative ahead of June meeting.",
                "0.74",
                "0.69",
                EventStatus::Superseded,
                &["mkt_122"],
                "Original macro take was superseded by newer desk notes after rate path wording changed.",
                "2026-04-16T13:27:00Z",
                "2026-04-16T14:30:00Z",
                4,
            ),
            fixture_event(
                "evt_9003",
                "x_whitelist",
                "Market influencers push BTC breakout narrative after ETF inflows.",
                "0.46",
                "0.44",
                EventStatus::Expired,
                &["mkt_120"],
                "Social chatter is directionally aligned, but evidence quality is too weak for autonomous weighting.",
                "2026-04-16T13:08:00Z",
                "2026-04-16T14:30:00Z",
                2,
            ),
            fixture_event(
                "evt_9004",
                "official_gov_feed",
                "Draft policy memo was retracted after publication metadata proved incorrect.",
                "0.67",
                "0.72",
                EventStatus::Invalidated,
                &["mkt_123"],
                "Upstream source inconsistency invalidates the settlement path assumption and blocks automation.",
                "2026-04-16T12:51:00Z",
                "2026-04-16T14:30:00Z",
                2,
            ),
        ],
        evidences: vec![
            fixture_evidence(
                "evd_5001",
                "mkt_121",
                "evt_9001",
                EvidenceDirection::SupportsNo,
                "0.34",
                "0.90",
                "0.80",
                "0.91",
                EvidenceStatus::Active,
                "2026-04-16T18:30:00Z",
                "2026-04-16T13:43:00Z",
                "2026-04-16T14:30:00Z",
                2,
            ),
            fixture_evidence(
                "evd_5002",
                "mkt_121",
                "evt_9001",
                EvidenceDirection::Background,
                "0.18",
                "0.42",
                "0.55",
                "0.30",
                EvidenceStatus::Active,
                "2026-04-16T16:00:00Z",
                "2026-04-16T13:44:00Z",
                "2026-04-16T14:30:00Z",
                1,
            ),
            fixture_evidence(
                "evd_5003",
                "mkt_122",
                "evt_9002",
                EvidenceDirection::SupportsNo,
                "0.27",
                "0.86",
                "0.52",
                "0.88",
                EvidenceStatus::Active,
                "2026-04-16T20:00:00Z",
                "2026-04-16T13:30:00Z",
                "2026-04-16T14:30:00Z",
                2,
            ),
            fixture_evidence(
                "evd_5004",
                "mkt_120",
                "evt_9003",
                EvidenceDirection::SupportsYes,
                "0.12",
                "0.35",
                "0.41",
                "0.25",
                EvidenceStatus::Active,
                "2026-04-16T15:00:00Z",
                "2026-04-16T13:10:00Z",
                "2026-04-16T14:30:00Z",
                1,
            ),
        ],
        signals: vec![
            fixture_signal(
                "sig_2411",
                "mkt_120",
                "evt_9003",
                SignalAction::Buy,
                SignalSide::Yes,
                "0.52",
                "0.58",
                "0.06",
                "0.88",
                SignalLifecycleState::Active,
                "ETF inflow narrative still supports underpriced upside participation.",
                "Eligible for automated execution under current bucket limits.",
                &["evd_5004"],
                "2026-04-16T14:30:00Z",
                9,
            ),
            fixture_signal(
                "sig_2412",
                "mkt_121",
                "evt_9001",
                SignalAction::Buy,
                SignalSide::No,
                "0.41",
                "0.35",
                "-0.06",
                "0.62",
                SignalLifecycleState::New,
                "Official update increases review-delay odds more than current price reflects.",
                "Signal is queued for manual review because settlement ambiguity is medium and theme exposure is elevated.",
                &["evd_5001", "evd_5002"],
                "2026-04-16T14:30:00Z",
                9,
            ),
            fixture_signal(
                "sig_2413",
                "mkt_120",
                "evt_9003",
                SignalAction::Buy,
                SignalSide::Yes,
                "0.28",
                "0.30",
                "0.02",
                "0.44",
                SignalLifecycleState::Weakened,
                "Momentum evidence remains directionally positive but confidence decayed after contradictory flow.",
                "Watch only until confidence recovers above activation threshold.",
                &["evd_5004"],
                "2026-04-16T14:30:00Z",
                4,
            ),
            fixture_signal(
                "sig_2414",
                "mkt_122",
                "evt_9002",
                SignalAction::Buy,
                SignalSide::No,
                "0.63",
                "0.57",
                "-0.06",
                "0.53",
                SignalLifecycleState::Reversed,
                "Macro drift remains negative for cuts, but live macro feed instability invalidates autonomous posture.",
                "Reversed to manual monitoring because upstream data quality is degraded.",
                &["evd_5003"],
                "2026-04-16T14:30:00Z",
                6,
            ),
        ],
    }
}

fn validate_limit(limit: Option<u16>) -> Result<u16> {
    let limit = limit.unwrap_or(DEFAULT_LIST_LIMIT);
    if limit == 0 {
        return Err(AppError::invalid_input(
            "LIST_LIMIT_INVALID",
            "list limit must be greater than zero",
        ));
    }

    if limit > MAX_LIST_LIMIT {
        return Err(AppError::invalid_input(
            "LIST_LIMIT_TOO_LARGE",
            format!("list limit must be at most {MAX_LIST_LIMIT}"),
        ));
    }

    Ok(limit)
}

fn validate_optional_id(field: &'static str, value: Option<String>) -> Result<Option<String>> {
    match value {
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Err(AppError::invalid_input(
                    "LIST_FILTER_INVALID",
                    format!("{field} must not be empty when provided"),
                ));
            }

            Ok(Some(trimmed.to_string()))
        }
        None => Ok(None),
    }
}

fn compute_evidence_signal(
    evidences: &[EvidenceView],
    reference_time: OffsetDateTime,
    source_health_score: Decimal,
) -> (Decimal, Decimal) {
    if evidences.is_empty() {
        return (Decimal::ZERO, Decimal::ZERO);
    }

    let mut weighted_delta = Decimal::ZERO;
    let mut quality_sum = Decimal::ZERO;

    for evidence in evidences {
        let total_window_secs = (evidence.expires_at - evidence.created_at)
            .whole_seconds()
            .max(1);
        let remaining_secs = (evidence.expires_at - reference_time).whole_seconds();
        let clamped_remaining = remaining_secs.clamp(0, total_window_secs);
        let freshness_decay = Decimal::from(clamped_remaining) / Decimal::from(total_window_secs);
        let effective_source_reliability =
            evidence.source_reliability.value() * source_health_score;
        let weight = evidence.strength.value()
            * effective_source_reliability
            * evidence.novelty.value()
            * evidence.resolution_relevance.value()
            * freshness_decay;

        let direction_multiplier = match evidence.direction {
            EvidenceDirection::SupportsYes => Decimal::ONE,
            EvidenceDirection::SupportsNo => -Decimal::ONE,
            EvidenceDirection::Background => Decimal::ZERO,
        };

        weighted_delta += weight * direction_multiplier;
        quality_sum += ((effective_source_reliability
            + evidence.novelty.value()
            + evidence.resolution_relevance.value())
            / dec("3"))
            * freshness_decay;
    }

    let avg_quality = quality_sum / Decimal::from(evidences.len() as i64);
    (weighted_delta, avg_quality)
}

fn derive_time_horizon(evidences: &[EvidenceView], reference_time: OffsetDateTime) -> TimeHorizon {
    let Some(min_remaining_secs) = evidences
        .iter()
        .map(|evidence| {
            (evidence.expires_at - reference_time)
                .whole_seconds()
                .max(0)
        })
        .min()
    else {
        return TimeHorizon::Short;
    };

    if min_remaining_secs <= 6 * 60 * 60 {
        TimeHorizon::Short
    } else if min_remaining_secs <= 24 * 60 * 60 {
        TimeHorizon::Medium
    } else {
        TimeHorizon::Long
    }
}

fn derive_reason_codes(
    market: &MarketView,
    evidences: &[EvidenceView],
    edge: Edge,
    confidence: Probability,
    evidence_count: usize,
    source_health: Option<&SourceHealthAdjustment>,
) -> Vec<String> {
    let mut reason_codes = Vec::new();
    let source_health_score = source_health
        .map(|adjustment| adjustment.health_score.value())
        .unwrap_or(Decimal::ONE);

    if evidences
        .iter()
        .any(|evidence| evidence.source_reliability.value() * source_health_score >= dec("0.85"))
    {
        reason_codes.push("official_source".to_string());
    }

    if source_health.is_some_and(|adjustment| adjustment.health_score.value() < dec("0.75")) {
        reason_codes.push("source_health_degraded".to_string());
    }

    if evidence_count >= 2 {
        reason_codes.push("corroborated".to_string());
    }

    if edge.value().abs() >= dec("0.05") {
        reason_codes.push("material_update".to_string());
    }

    if confidence.value() < dec("0.45") {
        reason_codes.push("low_confidence".to_string());
    }

    if market.ambiguity_level == AmbiguityLevel::High {
        reason_codes.push("high_ambiguity".to_string());
    }

    if market.tradability_status == TradabilityStatus::Blocked {
        reason_codes.push("blocked_market".to_string());
    }

    if evidences.is_empty() {
        reason_codes.push("no_active_evidence".to_string());
    }

    if reason_codes.is_empty() {
        reason_codes.push("steady_state".to_string());
    }

    reason_codes
}

fn derive_signal_lifecycle_state(
    directional_edge: Decimal,
    confidence: Probability,
    evidence_count: usize,
    tradability_status: TradabilityStatus,
) -> SignalLifecycleState {
    if evidence_count == 0 {
        return SignalLifecycleState::Expired;
    }

    if tradability_status == TradabilityStatus::Blocked {
        return SignalLifecycleState::Invalidated;
    }

    let edge_abs = directional_edge.abs();
    if confidence.value() < dec("0.35") {
        SignalLifecycleState::Invalidated
    } else if edge_abs >= dec("0.05") && confidence.value() >= dec("0.55") {
        SignalLifecycleState::Active
    } else if edge_abs >= dec("0.02") && confidence.value() >= dec("0.45") {
        SignalLifecycleState::New
    } else {
        SignalLifecycleState::Weakened
    }
}

fn derive_risk_decision(
    tradability_status: TradabilityStatus,
    lifecycle_state: SignalLifecycleState,
) -> String {
    match tradability_status {
        TradabilityStatus::Blocked => {
            "Blocked by tradability status; do not release to execution.".to_string()
        }
        TradabilityStatus::ObserveOnly => {
            "Observe only until posterior stabilizes and tradability restrictions are lifted."
                .to_string()
        }
        TradabilityStatus::ManualReview => {
            "Manual review required before downstream risk evaluation.".to_string()
        }
        TradabilityStatus::Tradable => match lifecycle_state {
            SignalLifecycleState::Active => {
                "Eligible for downstream risk evaluation under current tradability settings."
                    .to_string()
            }
            SignalLifecycleState::New => {
                "Queue for risk evaluation after next posterior refresh.".to_string()
            }
            _ => "Watch only until posterior strengthens.".to_string(),
        },
    }
}

fn probability_from_decimal(value: Decimal) -> Result<Probability> {
    Probability::new(clamp_zero_one(value))
}

fn clamp_zero_one(value: Decimal) -> Decimal {
    value.clamp(Decimal::ZERO, Decimal::ONE)
}

fn max_time(left: OffsetDateTime, right: OffsetDateTime) -> OffsetDateTime {
    if left >= right { left } else { right }
}

fn dec(raw: &str) -> Decimal {
    Decimal::from_str(raw).expect("static decimal must be valid")
}

fn fixture_market(
    id: &str,
    question: &str,
    category: &str,
    status: MarketStatus,
    best_bid: &str,
    best_ask: &str,
    mid_price: &str,
    volume_24h: &str,
    ambiguity_level: AmbiguityLevel,
    tradability_status: TradabilityStatus,
    resolution_source: &str,
    edge_case_notes: &[&str],
    polymarket_condition_id: Option<&str>,
    polymarket_yes_asset_id: Option<&str>,
    polymarket_no_asset_id: Option<&str>,
    updated_at: &str,
    version: i64,
) -> FixtureMarketRecord {
    FixtureMarketRecord {
        id: id.to_string(),
        question: question.to_string(),
        category: category.to_string(),
        status,
        best_bid: probability(best_bid),
        best_ask: probability(best_ask),
        mid_price: probability(mid_price),
        volume_24h: usd_amount(volume_24h),
        ambiguity_level,
        tradability_status,
        resolution_source: resolution_source.to_string(),
        edge_case_notes: edge_case_notes.iter().map(ToString::to_string).collect(),
        polymarket_condition_id: polymarket_condition_id.map(ToString::to_string),
        polymarket_yes_asset_id: polymarket_yes_asset_id.map(ToString::to_string),
        polymarket_no_asset_id: polymarket_no_asset_id.map(ToString::to_string),
        updated_at: timestamp(updated_at),
        version,
    }
}

fn fixture_event(
    id: &str,
    source: &str,
    summary: &str,
    relevance_score: &str,
    confidence: &str,
    status: EventStatus,
    related_market_ids: &[&str],
    reason_trace: &str,
    created_at: &str,
    updated_at: &str,
    version: i64,
) -> FixtureEventRecord {
    FixtureEventRecord {
        id: id.to_string(),
        source: source.to_string(),
        summary: summary.to_string(),
        relevance_score: probability(relevance_score),
        confidence: probability(confidence),
        status,
        related_market_ids: related_market_ids.iter().map(ToString::to_string).collect(),
        reason_trace: reason_trace.to_string(),
        created_at: timestamp(created_at),
        updated_at: timestamp(updated_at),
        version,
    }
}

fn fixture_evidence(
    id: &str,
    market_id: &str,
    event_id: &str,
    direction: EvidenceDirection,
    strength: &str,
    source_reliability: &str,
    novelty: &str,
    resolution_relevance: &str,
    status: EvidenceStatus,
    expires_at: &str,
    created_at: &str,
    updated_at: &str,
    version: i64,
) -> FixtureEvidenceRecord {
    FixtureEvidenceRecord {
        id: id.to_string(),
        market_id: market_id.to_string(),
        event_id: event_id.to_string(),
        direction,
        strength: probability(strength),
        source_reliability: probability(source_reliability),
        novelty: probability(novelty),
        resolution_relevance: probability(resolution_relevance),
        status,
        expires_at: timestamp(expires_at),
        created_at: timestamp(created_at),
        updated_at: timestamp(updated_at),
        version,
    }
}

fn fixture_signal(
    id: &str,
    market_id: &str,
    event_id: &str,
    action: SignalAction,
    side: SignalSide,
    market_price: &str,
    fair_price: &str,
    edge_value: &str,
    confidence: &str,
    lifecycle_state: SignalLifecycleState,
    reason: &str,
    risk_decision: &str,
    evidence_ids: &[&str],
    updated_at: &str,
    version: i64,
) -> FixtureSignalRecord {
    FixtureSignalRecord {
        id: id.to_string(),
        market_id: market_id.to_string(),
        event_id: event_id.to_string(),
        action,
        side,
        market_price: probability(market_price),
        fair_price: probability(fair_price),
        edge: edge(edge_value),
        confidence: probability(confidence),
        lifecycle_state,
        reason: reason.to_string(),
        risk_decision: risk_decision.to_string(),
        evidence_ids: evidence_ids.iter().map(ToString::to_string).collect(),
        approved_by_user_id: None,
        approved_at: None,
        rejected_by_user_id: None,
        rejected_at: None,
        updated_at: timestamp(updated_at),
        version,
    }
}

fn probability(raw: &str) -> Probability {
    Probability::new(Decimal::from_str(raw).expect("fixture decimal must be valid"))
        .expect("fixture probability must be valid")
}

fn edge(raw: &str) -> Edge {
    Edge::new(Decimal::from_str(raw).expect("fixture decimal must be valid"))
        .expect("fixture edge must be valid")
}

fn usd_amount(raw: &str) -> UsdAmount {
    UsdAmount::new(Decimal::from_str(raw).expect("fixture decimal must be valid"))
        .expect("fixture usd amount must be valid")
}

fn timestamp(raw: &str) -> OffsetDateTime {
    OffsetDateTime::parse(raw, &time::format_description::well_known::Rfc3339)
        .expect("fixture timestamp must be valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signal_view(record: &FixtureSignalRecord) -> SignalView {
        SignalView {
            id: record.id.clone(),
            market_id: record.market_id.clone(),
            event_id: record.event_id.clone(),
            action: record.action,
            side: record.side,
            market_price: record.market_price,
            fair_price: record.fair_price,
            edge: record.edge,
            confidence: record.confidence,
            lifecycle_state: record.lifecycle_state,
            reason: record.reason.clone(),
            risk_decision: record.risk_decision.clone(),
            evidence_ids: record.evidence_ids.clone(),
            approved_by_user_id: record.approved_by_user_id.clone(),
            approved_at: record.approved_at,
            rejected_by_user_id: record.rejected_by_user_id.clone(),
            rejected_at: record.rejected_at,
            updated_at: record.updated_at,
            version: record.version,
        }
    }

    fn market_view(record: &FixtureMarketRecord) -> MarketView {
        MarketView {
            id: record.id.clone(),
            question: record.question.clone(),
            category: record.category.clone(),
            status: record.status,
            best_bid: record.best_bid,
            best_ask: record.best_ask,
            mid_price: record.mid_price,
            volume_24h: record.volume_24h,
            ambiguity_level: record.ambiguity_level,
            tradability_status: record.tradability_status,
            resolution_source: record.resolution_source.clone(),
            edge_case_notes: record.edge_case_notes.clone(),
            polymarket_condition_id: record.polymarket_condition_id.clone(),
            polymarket_yes_asset_id: record.polymarket_yes_asset_id.clone(),
            polymarket_no_asset_id: record.polymarket_no_asset_id.clone(),
            updated_at: record.updated_at,
            version: record.version,
        }
    }

    fn evidence_view(record: &FixtureEvidenceRecord) -> EvidenceView {
        EvidenceView {
            id: record.id.clone(),
            market_id: record.market_id.clone(),
            event_id: record.event_id.clone(),
            direction: record.direction,
            strength: record.strength,
            source_reliability: record.source_reliability,
            novelty: record.novelty,
            resolution_relevance: record.resolution_relevance,
            status: record.status,
            expires_at: record.expires_at,
            created_at: record.created_at,
            updated_at: record.updated_at,
            version: record.version,
        }
    }

    #[test]
    fn market_filters_reject_zero_limit() {
        let result = MarketListFilters::new(None, None, Some(0));
        assert!(result.is_err());
    }

    #[test]
    fn signal_transition_filters_require_signal_id() {
        let result = SignalTransitionListFilters::new("   ", None);
        assert!(result.is_err());
    }

    #[test]
    fn demo_fixture_bundle_contains_full_chain_records() {
        let bundle = demo_fixture_bundle();
        assert_eq!(bundle.markets.len(), 4);
        assert_eq!(bundle.events.len(), 4);
        assert_eq!(bundle.evidences.len(), 4);
        assert_eq!(bundle.signals.len(), 4);
    }

    #[test]
    fn recompute_draft_keeps_negative_manual_review_signal_in_new_state() {
        let bundle = demo_fixture_bundle();
        let signal = bundle
            .signals
            .iter()
            .find(|signal| signal.id == "sig_2412")
            .expect("fixture signal");
        let market = bundle
            .markets
            .iter()
            .find(|market| market.id == signal.market_id)
            .expect("fixture market");
        let evidences: Vec<_> = bundle
            .evidences
            .iter()
            .filter(|evidence| {
                evidence.market_id == signal.market_id && evidence.event_id == signal.event_id
            })
            .map(evidence_view)
            .collect();
        let draft = build_recompute_signal_draft(
            &signal_view(signal),
            &market_view(market),
            &evidences,
            "manual refresh",
            "est_test",
        )
        .expect("recompute draft");

        assert_eq!(draft.next_signal.side, SignalSide::No);
        assert_eq!(draft.next_signal.lifecycle_state, SignalLifecycleState::New);
        assert!(draft.transition.is_none());
    }

    #[test]
    fn recompute_draft_discounts_degraded_event_source_health() {
        let bundle = demo_fixture_bundle();
        let signal = bundle
            .signals
            .iter()
            .find(|signal| signal.id == "sig_2412")
            .expect("fixture signal");
        let market = bundle
            .markets
            .iter()
            .find(|market| market.id == signal.market_id)
            .expect("fixture market");
        let evidences: Vec<_> = bundle
            .evidences
            .iter()
            .filter(|evidence| {
                evidence.market_id == signal.market_id && evidence.event_id == signal.event_id
            })
            .map(evidence_view)
            .collect();
        let signal = signal_view(signal);
        let market = market_view(market);

        let baseline = build_recompute_signal_draft(
            &signal,
            &market,
            &evidences,
            "baseline recompute",
            "est_baseline",
        )
        .expect("baseline recompute draft");
        let degraded = build_recompute_signal_draft_with_source_health(
            &signal,
            &market,
            &evidences,
            "degraded recompute",
            Some(&SourceHealthAdjustment {
                source: "reuters".to_string(),
                health_score: probability("0.20"),
            }),
            "est_degraded",
        )
        .expect("degraded recompute draft");

        assert!(degraded.estimate.edge.value().abs() < baseline.estimate.edge.value().abs());
        assert!(degraded.estimate.confidence < baseline.estimate.confidence);
        assert!(
            degraded
                .estimate
                .reason_codes
                .contains(&"source_health_degraded".to_string())
        );
        assert!(
            !degraded
                .estimate
                .reason_codes
                .contains(&"official_source".to_string())
        );
    }
}
