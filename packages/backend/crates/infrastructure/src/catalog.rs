use async_trait::async_trait;
use polyedge_application::{
    ArbitrageAnalysisRunListFilters, ArbitrageAnalysisRunView, ArbitrageEventListFilters,
    ArbitrageEventType, ArbitrageEventView, ArbitrageOpportunityListFilters,
    ArbitrageOpportunityStatus, ArbitrageOpportunityType, ArbitrageOpportunityValidationView,
    ArbitrageOpportunityView, ArbitrageScanListFilters, ArbitrageScanView, ArbitrageStore,
    ArbitrageValidationStatus, DispatchExecutionListFilters, EventListFilters, EventView,
    EvidenceListFilters, EvidenceView, ExecutionDispatchCandidate, ExecutionDispatchResult,
    ExecutionFillResult, ExecutionReconciliationCandidate, ExecutionRequestListFilters,
    ExecutionRequestView, ExecutionSubmissionResult, FixtureBundle, FixtureIngestionReport,
    MarketBookSnapshotView, MarketEventStore, MarketListFilters, MarketView, NewsIngestionStore,
    NewsRawEventInsert, NewsRawEventListFilters, NewsRawEventView, NewsSourceFailureUpdate,
    NewsSourceHealthListFilters, NewsSourceHealthView, NewsSourceSuccessUpdate,
    OrderDraftListFilters, OrderDraftView, OrderListFilters, OrderView, PositionListFilters,
    PositionView, ProbabilityEstimateListFilters, ProbabilityEstimateView, RecomputeSignalCommand,
    RecomputeSignalResult, ReconcileExecutionListFilters, SignalListFilters,
    SignalTransitionListFilters, SignalTransitionView, SignalView, SourceHealthAdjustment,
    SubmitExecutionStoreCommand, TradeListFilters, TradeView,
    build_recompute_signal_draft_with_source_health, degraded_health_score,
};
use polyedge_domain::{
    AmbiguityLevel, AppError, Edge, EventStatus, EvidenceDirection, EvidenceStatus,
    ExecutionRequestStatus, MarketStatus, OrderDraftStatus, OrderStatus, Probability, Quantity,
    Result, SignalAction, SignalLifecycleState, SignalSide, SignedUsdAmount, TimeHorizon,
    TradabilityStatus, UsdAmount,
};
use rust_decimal::{Decimal, RoundingStrategy};
use serde_json::{Value, json};
use sqlx::{PgPool, Row, types::Json};
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};
use time::OffsetDateTime;
use tokio::sync::RwLock;
use uuid::Uuid;

fn db_error(code: &'static str, context: impl Into<String>) -> AppError {
    AppError::dependency_unavailable(code, context.into())
}

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

#[async_trait]
impl MarketEventStore for InMemoryMarketEventStore {
    async fn list_markets(&self, filters: &MarketListFilters) -> Result<Vec<MarketView>> {
        let markets = self.markets.read().await;
        let mut items: Vec<_> = markets
            .values()
            .filter(|market| {
                filters.status.is_none_or(|status| market.status == status)
                    && filters
                        .tradability_status
                        .is_none_or(|status| market.tradability_status == status)
            })
            .cloned()
            .collect();
        items.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        items.truncate(usize::from(filters.limit));
        Ok(items)
    }

    async fn get_market(&self, market_id: &str) -> Result<Option<MarketView>> {
        Ok(self.markets.read().await.get(market_id).cloned())
    }

    async fn get_signal(&self, signal_id: &str) -> Result<Option<SignalView>> {
        Ok(self.signals.read().await.get(signal_id).cloned())
    }

    async fn list_events(&self, filters: &EventListFilters) -> Result<Vec<EventView>> {
        let events = self.events.read().await;
        let mut items: Vec<_> = events
            .values()
            .filter(|event| filters.status.is_none_or(|status| event.status == status))
            .cloned()
            .collect();
        items.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        items.truncate(usize::from(filters.limit));
        Ok(items)
    }

    async fn list_evidences(&self, filters: &EvidenceListFilters) -> Result<Vec<EvidenceView>> {
        let evidences = self.evidences.read().await;
        let mut items: Vec<_> = evidences
            .values()
            .filter(|evidence| {
                filters
                    .market_id
                    .as_ref()
                    .is_none_or(|market_id| &evidence.market_id == market_id)
                    && filters
                        .event_id
                        .as_ref()
                        .is_none_or(|event_id| &evidence.event_id == event_id)
                    && filters
                        .status
                        .is_none_or(|status| evidence.status == status)
            })
            .cloned()
            .collect();
        items.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        items.truncate(usize::from(filters.limit));
        Ok(items)
    }

    async fn list_signals(&self, filters: &SignalListFilters) -> Result<Vec<SignalView>> {
        let signals = self.signals.read().await;
        let mut items: Vec<_> = signals
            .values()
            .filter(|signal| {
                filters
                    .market_id
                    .as_ref()
                    .is_none_or(|market_id| &signal.market_id == market_id)
                    && filters
                        .event_id
                        .as_ref()
                        .is_none_or(|event_id| &signal.event_id == event_id)
                    && filters
                        .lifecycle_state
                        .is_none_or(|state| signal.lifecycle_state == state)
            })
            .cloned()
            .collect();
        items.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        items.truncate(usize::from(filters.limit));
        Ok(items)
    }

    async fn list_probability_estimates(
        &self,
        filters: &ProbabilityEstimateListFilters,
    ) -> Result<Vec<ProbabilityEstimateView>> {
        let estimates = self.probability_estimates.read().await;
        let mut items: Vec<_> = estimates
            .values()
            .filter(|estimate| {
                filters
                    .market_id
                    .as_ref()
                    .is_none_or(|market_id| &estimate.market_id == market_id)
                    && filters
                        .event_id
                        .as_ref()
                        .is_none_or(|event_id| &estimate.event_id == event_id)
                    && filters
                        .signal_id
                        .as_ref()
                        .is_none_or(|signal_id| estimate.signal_id.as_ref() == Some(signal_id))
            })
            .cloned()
            .collect();
        items.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        items.truncate(usize::from(filters.limit));
        Ok(items)
    }

    async fn list_signal_transitions(
        &self,
        filters: &SignalTransitionListFilters,
    ) -> Result<Vec<SignalTransitionView>> {
        let transitions = self.signal_transitions.read().await;
        let mut items: Vec<_> = transitions
            .iter()
            .filter(|transition| transition.signal_id == filters.signal_id)
            .cloned()
            .collect();
        items.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        items.truncate(usize::from(filters.limit));
        Ok(items)
    }

    async fn list_order_drafts(
        &self,
        filters: &OrderDraftListFilters,
    ) -> Result<Vec<OrderDraftView>> {
        let order_drafts = self.order_drafts.read().await;
        let mut items: Vec<_> = order_drafts
            .values()
            .filter(|draft| {
                filters
                    .signal_id
                    .as_ref()
                    .is_none_or(|signal_id| &draft.signal_id == signal_id)
                    && filters
                        .connector_name
                        .as_ref()
                        .is_none_or(|connector_name| &draft.connector_name == connector_name)
                    && filters.status.is_none_or(|status| draft.status == status)
            })
            .cloned()
            .collect();
        items.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        items.truncate(usize::from(filters.limit));
        Ok(items)
    }

    async fn list_execution_requests(
        &self,
        filters: &ExecutionRequestListFilters,
    ) -> Result<Vec<ExecutionRequestView>> {
        let execution_requests = self.execution_requests.read().await;
        let mut items: Vec<_> = execution_requests
            .values()
            .filter(|request| {
                filters
                    .signal_id
                    .as_ref()
                    .is_none_or(|signal_id| &request.signal_id == signal_id)
                    && filters
                        .connector_name
                        .as_ref()
                        .is_none_or(|connector_name| &request.connector_name == connector_name)
                    && filters.status.is_none_or(|status| request.status == status)
            })
            .cloned()
            .collect();
        items.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        items.truncate(usize::from(filters.limit));
        Ok(items)
    }

    async fn get_order_by_external_ref(
        &self,
        connector_name: &str,
        external_order_id: &str,
    ) -> Result<OrderView> {
        let orders = self.orders.read().await;
        orders
            .values()
            .find(|order| {
                order.connector_name == connector_name
                    && order.external_order_id == external_order_id
            })
            .cloned()
            .ok_or_else(|| {
                AppError::not_found(
                    "ORDER_NOT_FOUND",
                    format!(
                        "order was not found for connector={} external_order_id={}",
                        connector_name, external_order_id
                    ),
                )
            })
    }

    async fn list_orders(&self, filters: &OrderListFilters) -> Result<Vec<OrderView>> {
        let orders = self.orders.read().await;
        let mut items: Vec<_> = orders
            .values()
            .filter(|order| {
                filters
                    .signal_id
                    .as_ref()
                    .is_none_or(|signal_id| &order.signal_id == signal_id)
                    && filters
                        .market_id
                        .as_ref()
                        .is_none_or(|market_id| &order.market_id == market_id)
                    && filters
                        .connector_name
                        .as_ref()
                        .is_none_or(|connector_name| &order.connector_name == connector_name)
                    && filters.status.is_none_or(|status| order.status == status)
            })
            .cloned()
            .collect();
        items.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        items.truncate(usize::from(filters.limit));
        Ok(items)
    }

    async fn list_trades(&self, filters: &TradeListFilters) -> Result<Vec<TradeView>> {
        let trades = self.trades.read().await;
        let mut items: Vec<_> = trades
            .values()
            .filter(|trade| {
                filters
                    .order_id
                    .as_ref()
                    .is_none_or(|order_id| &trade.order_id == order_id)
                    && filters
                        .signal_id
                        .as_ref()
                        .is_none_or(|signal_id| &trade.signal_id == signal_id)
                    && filters
                        .market_id
                        .as_ref()
                        .is_none_or(|market_id| &trade.market_id == market_id)
                    && filters
                        .connector_name
                        .as_ref()
                        .is_none_or(|connector_name| &trade.connector_name == connector_name)
            })
            .cloned()
            .collect();
        items.sort_by(|left, right| {
            right
                .executed_at
                .cmp(&left.executed_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        items.truncate(usize::from(filters.limit));
        Ok(items)
    }

    async fn list_positions(&self, filters: &PositionListFilters) -> Result<Vec<PositionView>> {
        let positions = self.positions.read().await;
        let mut items: Vec<_> = positions
            .values()
            .filter(|position| {
                filters
                    .market_id
                    .as_ref()
                    .is_none_or(|market_id| &position.market_id == market_id)
                    && filters
                        .connector_name
                        .as_ref()
                        .is_none_or(|connector_name| &position.connector_name == connector_name)
                    && filters.side.is_none_or(|side| position.side == side)
            })
            .cloned()
            .collect();
        items.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        items.truncate(usize::from(filters.limit));
        Ok(items)
    }

    async fn recompute_signal(
        &self,
        command: &RecomputeSignalCommand,
    ) -> Result<RecomputeSignalResult> {
        let signal = {
            let signals = self.signals.read().await;
            signals.get(&command.signal_id).cloned().ok_or_else(|| {
                AppError::not_found(
                    "SIGNAL_NOT_FOUND",
                    format!("signal was not found: {}", command.signal_id),
                )
            })?
        };
        let market = {
            let markets = self.markets.read().await;
            markets.get(&signal.market_id).cloned().ok_or_else(|| {
                AppError::not_found(
                    "MARKET_NOT_FOUND",
                    format!("market was not found: {}", signal.market_id),
                )
            })?
        };
        let evidences: Vec<_> = {
            let evidences = self.evidences.read().await;
            evidences
                .values()
                .filter(|evidence| {
                    evidence.market_id == signal.market_id && evidence.event_id == signal.event_id
                })
                .cloned()
                .collect()
        };
        let source_health = self
            .source_health_adjustment_for_event(&signal.event_id)
            .await;

        let estimate_id = format!("est_{}", Uuid::now_v7());
        let draft = build_recompute_signal_draft_with_source_health(
            &signal,
            &market,
            &evidences,
            &command.reason,
            source_health.as_ref(),
            &estimate_id,
        )?;

        {
            let mut estimates = self.probability_estimates.write().await;
            estimates.insert(draft.estimate.id.clone(), draft.estimate.clone());
        }

        {
            let mut signals = self.signals.write().await;
            signals.insert(draft.next_signal.id.clone(), draft.next_signal.clone());
        }

        let transition = if let Some(transition) = draft.transition {
            let view = SignalTransitionView {
                id: format!("sgt_{}", Uuid::now_v7()),
                signal_id: draft.next_signal.id.clone(),
                from_state: transition.from_state,
                to_state: transition.to_state,
                trigger_type: transition.trigger_type,
                trigger_payload: transition.trigger_payload,
                created_at: transition.created_at,
            };
            self.signal_transitions.write().await.push(view.clone());
            Some(view)
        } else {
            None
        };

        Ok(RecomputeSignalResult {
            signal: draft.next_signal,
            estimate: draft.estimate,
            transition,
        })
    }

    async fn approve_signal(
        &self,
        signal_id: &str,
        approved_by_user_id: &str,
        approval_reason: &str,
        _trace_id: &str,
        expected_version: Option<i64>,
    ) -> Result<SignalView> {
        let mut signals = self.signals.write().await;
        let current = signals.get(signal_id).cloned().ok_or_else(|| {
            AppError::not_found(
                "SIGNAL_NOT_FOUND",
                format!("signal was not found: {signal_id}"),
            )
        })?;

        if let Some(expected_version) = expected_version {
            if current.version != expected_version {
                return Err(AppError::conflict(
                    "STATE_VERSION_MISMATCH",
                    "signal version does not match the expected_version",
                ));
            }
        }

        if current.approved_by_user_id.is_some() {
            return Err(AppError::conflict(
                "STATE_SIGNAL_ALREADY_APPROVED",
                "signal has already been approved",
            ));
        }

        if current.rejected_by_user_id.is_some() {
            return Err(AppError::conflict(
                "STATE_SIGNAL_ALREADY_REJECTED",
                "signal has already been rejected for the current version",
            ));
        }

        let approved_at = OffsetDateTime::now_utc();
        let approved_signal = SignalView {
            id: current.id.clone(),
            market_id: current.market_id.clone(),
            event_id: current.event_id.clone(),
            action: current.action,
            side: current.side,
            market_price: current.market_price,
            fair_price: current.fair_price,
            edge: current.edge,
            confidence: current.confidence,
            lifecycle_state: current.lifecycle_state,
            reason: current.reason.clone(),
            risk_decision: approval_reason.to_string(),
            evidence_ids: current.evidence_ids.clone(),
            approved_by_user_id: Some(approved_by_user_id.to_string()),
            approved_at: Some(approved_at),
            rejected_by_user_id: None,
            rejected_at: None,
            updated_at: approved_at,
            version: current.version + 1,
        };

        signals.insert(signal_id.to_string(), approved_signal.clone());
        Ok(approved_signal)
    }

    async fn reject_signal(
        &self,
        signal_id: &str,
        rejected_by_user_id: &str,
        rejection_reason: &str,
        _trace_id: &str,
        expected_version: Option<i64>,
    ) -> Result<SignalView> {
        let mut signals = self.signals.write().await;
        let current = signals.get(signal_id).cloned().ok_or_else(|| {
            AppError::not_found(
                "SIGNAL_NOT_FOUND",
                format!("signal was not found: {signal_id}"),
            )
        })?;

        if let Some(expected_version) = expected_version {
            if current.version != expected_version {
                return Err(AppError::conflict(
                    "STATE_VERSION_MISMATCH",
                    "signal version does not match the expected_version",
                ));
            }
        }

        if current.approved_by_user_id.is_some() {
            return Err(AppError::conflict(
                "STATE_SIGNAL_ALREADY_APPROVED",
                "approved signals cannot be rejected for the current version",
            ));
        }

        if current.rejected_by_user_id.is_some() {
            return Err(AppError::conflict(
                "STATE_SIGNAL_ALREADY_REJECTED",
                "signal has already been rejected for the current version",
            ));
        }

        let rejected_at = OffsetDateTime::now_utc();
        let rejected_signal = SignalView {
            id: current.id.clone(),
            market_id: current.market_id.clone(),
            event_id: current.event_id.clone(),
            action: current.action,
            side: current.side,
            market_price: current.market_price,
            fair_price: current.fair_price,
            edge: current.edge,
            confidence: current.confidence,
            lifecycle_state: current.lifecycle_state,
            reason: current.reason.clone(),
            risk_decision: rejection_reason.to_string(),
            evidence_ids: current.evidence_ids.clone(),
            approved_by_user_id: None,
            approved_at: None,
            rejected_by_user_id: Some(rejected_by_user_id.to_string()),
            rejected_at: Some(rejected_at),
            updated_at: rejected_at,
            version: current.version + 1,
        };

        signals.insert(signal_id.to_string(), rejected_signal.clone());
        Ok(rejected_signal)
    }

    async fn submit_execution_request(
        &self,
        command: &SubmitExecutionStoreCommand,
    ) -> Result<ExecutionSubmissionResult> {
        let signal = {
            let signals = self.signals.read().await;
            signals.get(&command.signal_id).cloned().ok_or_else(|| {
                AppError::not_found(
                    "SIGNAL_NOT_FOUND",
                    format!("signal was not found: {}", command.signal_id),
                )
            })?
        };

        if let Some(expected_signal_version) = command.expected_signal_version {
            if signal.version != expected_signal_version {
                return Err(AppError::conflict(
                    "STATE_VERSION_MISMATCH",
                    "signal version does not match the expected_signal_version",
                ));
            }
        }

        validate_signal_for_execution(&signal, command.mode)?;

        {
            let execution_requests = self.execution_requests.read().await;
            if execution_requests.values().any(|request| {
                request.signal_id == signal.id && request.signal_version == signal.version
            }) {
                return Err(AppError::conflict(
                    "STATE_EXECUTION_REQUEST_ALREADY_EXISTS",
                    "an execution request already exists for the current signal version",
                ));
            }
        }

        let now = OffsetDateTime::now_utc();
        let order_draft = OrderDraftView {
            id: format!("odr_{}", Uuid::now_v7()),
            signal_id: signal.id.clone(),
            signal_version: signal.version,
            market_id: signal.market_id.clone(),
            connector_name: command.connector_name.clone(),
            side: signal.side,
            limit_price: command.limit_price,
            quantity: command.quantity,
            notional: compute_order_notional(command.limit_price, command.quantity)?,
            status: OrderDraftStatus::Queued,
            created_by_user_id: command.requested_by_user_id.clone(),
            external_order_id: None,
            submitted_at: None,
            failure_code: None,
            failure_message: None,
            created_at: now,
            updated_at: now,
            version: 1,
        };
        let execution_request = ExecutionRequestView {
            id: format!("exr_{}", Uuid::now_v7()),
            signal_id: signal.id,
            signal_version: signal.version,
            order_draft_id: order_draft.id.clone(),
            connector_name: command.connector_name.clone(),
            mode: command.mode,
            requested_by_user_id: command.requested_by_user_id.clone(),
            status: ExecutionRequestStatus::Queued,
            reason: command.reason.clone(),
            external_order_id: None,
            submitted_at: None,
            failure_code: None,
            failure_message: None,
            created_at: now,
            updated_at: now,
            version: 1,
        };

        self.order_drafts
            .write()
            .await
            .insert(order_draft.id.clone(), order_draft.clone());
        self.execution_requests
            .write()
            .await
            .insert(execution_request.id.clone(), execution_request.clone());

        Ok(ExecutionSubmissionResult {
            order_draft,
            execution_request,
        })
    }

    async fn list_dispatch_candidates(
        &self,
        filters: &DispatchExecutionListFilters,
    ) -> Result<Vec<ExecutionDispatchCandidate>> {
        let execution_requests = self.execution_requests.read().await;
        let order_drafts = self.order_drafts.read().await;
        let mut items: Vec<_> = execution_requests
            .values()
            .filter(|request| {
                request.status == ExecutionRequestStatus::Queued
                    && filters
                        .connector_name
                        .as_ref()
                        .is_none_or(|connector_name| &request.connector_name == connector_name)
            })
            .filter_map(|request| {
                let order_draft = order_drafts.get(&request.order_draft_id)?;
                (order_draft.status == OrderDraftStatus::Queued).then(|| {
                    ExecutionDispatchCandidate {
                        order_draft: order_draft.clone(),
                        execution_request: request.clone(),
                    }
                })
            })
            .collect();
        items.sort_by(|left, right| {
            left.execution_request
                .created_at
                .cmp(&right.execution_request.created_at)
                .then_with(|| left.execution_request.id.cmp(&right.execution_request.id))
        });
        items.truncate(usize::from(filters.limit));
        Ok(items)
    }

    async fn list_reconciliation_candidates(
        &self,
        filters: &ReconcileExecutionListFilters,
    ) -> Result<Vec<ExecutionReconciliationCandidate>> {
        let execution_requests = self.execution_requests.read().await;
        let order_drafts = self.order_drafts.read().await;
        let orders = self.orders.read().await;
        let mut items: Vec<_> = execution_requests
            .values()
            .filter(|request| {
                request.status == ExecutionRequestStatus::Submitted
                    && filters
                        .connector_name
                        .as_ref()
                        .is_none_or(|connector_name| &request.connector_name == connector_name)
            })
            .filter_map(|request| {
                let order_draft = order_drafts.get(&request.order_draft_id)?;
                let order = orders
                    .values()
                    .find(|order| order.execution_request_id == request.id)
                    .cloned();
                let is_reconcilable = order.as_ref().is_none_or(|order| {
                    matches!(
                        order.status,
                        OrderStatus::Submitted | OrderStatus::Open | OrderStatus::PartiallyFilled
                    ) && order.filled_quantity.value() < order.quantity.value()
                });
                (order_draft.status == OrderDraftStatus::Submitted && is_reconcilable).then(|| {
                    ExecutionReconciliationCandidate {
                        order_draft: order_draft.clone(),
                        execution_request: request.clone(),
                        order,
                    }
                })
            })
            .collect();
        items.sort_by(|left, right| {
            left.execution_request
                .updated_at
                .cmp(&right.execution_request.updated_at)
                .then_with(|| left.execution_request.id.cmp(&right.execution_request.id))
        });
        items.truncate(usize::from(filters.limit));
        Ok(items)
    }

    async fn mark_execution_submitted(
        &self,
        execution_request_id: &str,
        account_id: &str,
        external_order_id: &str,
        _trace_id: &str,
    ) -> Result<ExecutionDispatchResult> {
        let mut execution_requests = self.execution_requests.write().await;
        let mut order_drafts = self.order_drafts.write().await;
        let mut orders = self.orders.write().await;
        let request = execution_requests
            .get(execution_request_id)
            .cloned()
            .ok_or_else(|| {
                AppError::not_found(
                    "EXECUTION_REQUEST_NOT_FOUND",
                    format!("execution request was not found: {execution_request_id}"),
                )
            })?;

        if request.status != ExecutionRequestStatus::Queued {
            return Err(AppError::conflict(
                "STATE_EXECUTION_REQUEST_NOT_DISPATCHABLE",
                "execution request is no longer queued",
            ));
        }

        let order_draft = order_drafts
            .get(&request.order_draft_id)
            .cloned()
            .ok_or_else(|| {
                AppError::not_found(
                    "ORDER_DRAFT_NOT_FOUND",
                    format!("order draft was not found: {}", request.order_draft_id),
                )
            })?;

        if order_draft.status != OrderDraftStatus::Queued {
            return Err(AppError::conflict(
                "STATE_ORDER_DRAFT_NOT_DISPATCHABLE",
                "order draft is no longer queued",
            ));
        }

        let submitted_at = OffsetDateTime::now_utc();
        let next_order_draft = OrderDraftView {
            status: OrderDraftStatus::Submitted,
            external_order_id: Some(external_order_id.to_string()),
            submitted_at: Some(submitted_at),
            failure_code: None,
            failure_message: None,
            updated_at: submitted_at,
            version: order_draft.version + 1,
            ..order_draft
        };
        let next_request = ExecutionRequestView {
            status: ExecutionRequestStatus::Submitted,
            external_order_id: Some(external_order_id.to_string()),
            submitted_at: Some(submitted_at),
            failure_code: None,
            failure_message: None,
            updated_at: submitted_at,
            version: request.version + 1,
            ..request
        };
        let submitted_order = OrderView {
            id: format!("ord_{}", Uuid::now_v7()),
            signal_id: next_request.signal_id.clone(),
            execution_request_id: next_request.id.clone(),
            order_draft_id: next_order_draft.id.clone(),
            market_id: next_order_draft.market_id.clone(),
            connector_name: next_request.connector_name.clone(),
            account_id: account_id.to_string(),
            external_order_id: external_order_id.to_string(),
            side: next_order_draft.side,
            limit_price: next_order_draft.limit_price,
            quantity: next_order_draft.quantity,
            filled_quantity: Quantity::new(Decimal::ZERO)?,
            avg_fill_price: Probability::new(Decimal::ZERO)?,
            status: OrderStatus::Submitted,
            submitted_at,
            updated_at: submitted_at,
            version: 1,
        };

        order_drafts.insert(next_order_draft.id.clone(), next_order_draft.clone());
        execution_requests.insert(next_request.id.clone(), next_request.clone());
        orders.insert(submitted_order.id.clone(), submitted_order);

        Ok(ExecutionDispatchResult {
            order_draft: next_order_draft,
            execution_request: next_request,
        })
    }

    async fn mark_order_open(&self, order_id: &str, _trace_id: &str) -> Result<OrderView> {
        let mut orders = self.orders.write().await;
        let order = orders.get(order_id).cloned().ok_or_else(|| {
            AppError::not_found(
                "ORDER_NOT_FOUND",
                format!("order was not found: {order_id}"),
            )
        })?;

        let next_order = match order.status {
            OrderStatus::Submitted => OrderView {
                status: OrderStatus::Open,
                updated_at: OffsetDateTime::now_utc(),
                version: order.version + 1,
                ..order
            },
            OrderStatus::Open => order,
            _ => {
                return Err(AppError::conflict(
                    "STATE_ORDER_NOT_POLLABLE",
                    "only submitted/open orders can be polled as open",
                ));
            }
        };

        orders.insert(next_order.id.clone(), next_order.clone());
        Ok(next_order)
    }

    async fn mark_order_canceled(&self, order_id: &str, _trace_id: &str) -> Result<OrderView> {
        let mut orders = self.orders.write().await;
        let mut execution_requests = self.execution_requests.write().await;
        let order = orders.get(order_id).cloned().ok_or_else(|| {
            AppError::not_found(
                "ORDER_NOT_FOUND",
                format!("order was not found: {order_id}"),
            )
        })?;

        let next_order = match order.status {
            OrderStatus::Submitted | OrderStatus::Open | OrderStatus::PartiallyFilled => {
                OrderView {
                    status: OrderStatus::Canceled,
                    updated_at: OffsetDateTime::now_utc(),
                    version: order.version + 1,
                    ..order
                }
            }
            OrderStatus::Canceled => order,
            _ => {
                return Err(AppError::conflict(
                    "STATE_ORDER_NOT_CANCELABLE",
                    "only submitted/open/partially_filled orders can be canceled",
                ));
            }
        };

        if let Some(request) = execution_requests
            .get(&next_order.execution_request_id)
            .cloned()
            .filter(|request| request.status == ExecutionRequestStatus::Submitted)
        {
            execution_requests.insert(
                request.id.clone(),
                ExecutionRequestView {
                    status: ExecutionRequestStatus::Canceled,
                    updated_at: next_order.updated_at,
                    version: request.version + 1,
                    ..request
                },
            );
        }

        orders.insert(next_order.id.clone(), next_order.clone());
        Ok(next_order)
    }

    async fn mark_execution_failed(
        &self,
        execution_request_id: &str,
        failure_code: &str,
        failure_message: &str,
        _trace_id: &str,
    ) -> Result<ExecutionDispatchResult> {
        let mut execution_requests = self.execution_requests.write().await;
        let mut order_drafts = self.order_drafts.write().await;
        let request = execution_requests
            .get(execution_request_id)
            .cloned()
            .ok_or_else(|| {
                AppError::not_found(
                    "EXECUTION_REQUEST_NOT_FOUND",
                    format!("execution request was not found: {execution_request_id}"),
                )
            })?;

        if request.status != ExecutionRequestStatus::Queued {
            return Err(AppError::conflict(
                "STATE_EXECUTION_REQUEST_NOT_DISPATCHABLE",
                "execution request is no longer queued",
            ));
        }

        let order_draft = order_drafts
            .get(&request.order_draft_id)
            .cloned()
            .ok_or_else(|| {
                AppError::not_found(
                    "ORDER_DRAFT_NOT_FOUND",
                    format!("order draft was not found: {}", request.order_draft_id),
                )
            })?;

        if order_draft.status != OrderDraftStatus::Queued {
            return Err(AppError::conflict(
                "STATE_ORDER_DRAFT_NOT_DISPATCHABLE",
                "order draft is no longer queued",
            ));
        }

        let failed_at = OffsetDateTime::now_utc();
        let next_order_draft = OrderDraftView {
            status: OrderDraftStatus::Rejected,
            external_order_id: None,
            submitted_at: None,
            failure_code: Some(failure_code.to_string()),
            failure_message: Some(failure_message.to_string()),
            updated_at: failed_at,
            version: order_draft.version + 1,
            ..order_draft
        };
        let next_request = ExecutionRequestView {
            status: ExecutionRequestStatus::Failed,
            external_order_id: None,
            submitted_at: None,
            failure_code: Some(failure_code.to_string()),
            failure_message: Some(failure_message.to_string()),
            updated_at: failed_at,
            version: request.version + 1,
            ..request
        };

        order_drafts.insert(next_order_draft.id.clone(), next_order_draft.clone());
        execution_requests.insert(next_request.id.clone(), next_request.clone());

        Ok(ExecutionDispatchResult {
            order_draft: next_order_draft,
            execution_request: next_request,
        })
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
        let execution_requests = self.execution_requests.read().await;
        let order_drafts = self.order_drafts.read().await;
        let mut orders = self.orders.write().await;
        let mut trades = self.trades.write().await;
        let mut positions = self.positions.write().await;
        let mut signals = self.signals.write().await;
        let mut transitions = self.signal_transitions.write().await;

        let request = execution_requests
            .get(execution_request_id)
            .cloned()
            .ok_or_else(|| {
                AppError::not_found(
                    "EXECUTION_REQUEST_NOT_FOUND",
                    format!("execution request was not found: {execution_request_id}"),
                )
            })?;

        if request.status != ExecutionRequestStatus::Submitted {
            return Err(AppError::conflict(
                "STATE_EXECUTION_REQUEST_NOT_RECONCILABLE",
                "execution request is not in submitted state",
            ));
        }

        if trades.values().any(|trade| {
            trade.connector_name == request.connector_name
                && trade.external_trade_id == external_trade_id
        }) {
            return Err(AppError::conflict(
                "STATE_TRADE_ALREADY_RECORDED",
                "external trade id has already been recorded",
            ));
        }

        let order_draft = order_drafts
            .get(&request.order_draft_id)
            .cloned()
            .ok_or_else(|| {
                AppError::not_found(
                    "ORDER_DRAFT_NOT_FOUND",
                    format!("order draft was not found: {}", request.order_draft_id),
                )
            })?;

        if order_draft.status != OrderDraftStatus::Submitted {
            return Err(AppError::conflict(
                "STATE_ORDER_DRAFT_NOT_RECONCILABLE",
                "order draft is not in submitted state",
            ));
        }

        let external_order_id = request
            .external_order_id
            .clone()
            .or_else(|| order_draft.external_order_id.clone())
            .ok_or_else(|| {
                AppError::conflict(
                    "STATE_EXTERNAL_ORDER_ID_MISSING",
                    "submitted execution request is missing external_order_id",
                )
            })?;

        let now = OffsetDateTime::now_utc();
        let submitted_at = request
            .submitted_at
            .or(order_draft.submitted_at)
            .unwrap_or(now);
        let order = if let Some(current) = orders
            .values()
            .find(|order| order.execution_request_id == request.id)
            .cloned()
        {
            if !matches!(
                current.status,
                OrderStatus::Submitted | OrderStatus::Open | OrderStatus::PartiallyFilled
            ) {
                return Err(AppError::conflict(
                    "STATE_ORDER_NOT_RECONCILABLE",
                    "existing order is not in a reconcilable state",
                ));
            }

            let next_filled_quantity_value =
                current.filled_quantity.value() + filled_quantity.value();
            if next_filled_quantity_value > current.quantity.value() {
                return Err(AppError::conflict(
                    "STATE_FILL_QUANTITY_EXCEEDS_ORDER",
                    "filled quantity exceeds order quantity",
                ));
            }

            let next_filled_quantity = Quantity::new(next_filled_quantity_value)?;
            let next_avg_fill_price = weighted_fill_price(
                current.avg_fill_price,
                current.filled_quantity,
                fill_price,
                filled_quantity,
            )?;
            let next_status = if next_filled_quantity.value() == current.quantity.value() {
                OrderStatus::Filled
            } else {
                OrderStatus::PartiallyFilled
            };

            OrderView {
                filled_quantity: next_filled_quantity,
                avg_fill_price: next_avg_fill_price,
                status: next_status,
                updated_at: now,
                version: current.version + 1,
                ..current
            }
        } else {
            let next_status = if filled_quantity.value() == order_draft.quantity.value() {
                OrderStatus::Filled
            } else {
                OrderStatus::PartiallyFilled
            };
            OrderView {
                id: format!("ord_{}", Uuid::now_v7()),
                signal_id: request.signal_id.clone(),
                execution_request_id: request.id.clone(),
                order_draft_id: order_draft.id.clone(),
                market_id: order_draft.market_id.clone(),
                connector_name: request.connector_name.clone(),
                account_id: account_id.to_string(),
                external_order_id,
                side: order_draft.side,
                limit_price: order_draft.limit_price,
                quantity: order_draft.quantity,
                filled_quantity,
                avg_fill_price: fill_price,
                status: next_status,
                submitted_at,
                updated_at: now,
                version: 1,
            }
        };
        let trade = TradeView {
            id: format!("trd_{}", Uuid::now_v7()),
            order_id: order.id.clone(),
            signal_id: order.signal_id.clone(),
            market_id: order.market_id.clone(),
            connector_name: order.connector_name.clone(),
            external_trade_id: external_trade_id.to_string(),
            side: order.side,
            price: fill_price,
            quantity: filled_quantity,
            fee,
            executed_at: now,
        };

        let position_key = in_memory_position_key(
            &order.connector_name,
            account_id,
            &order.market_id,
            order.side,
        );
        let position = if let Some(current) = positions.get(&position_key).cloned() {
            build_next_position(current, filled_quantity, fill_price, trace_id)?
        } else {
            PositionView {
                id: position_key.clone(),
                market_id: order.market_id.clone(),
                connector_name: order.connector_name.clone(),
                account_id: account_id.to_string(),
                side: order.side,
                net_quantity: filled_quantity,
                avg_cost: fill_price,
                mark_price: fill_price,
                unrealized_pnl: SignedUsdAmount::new(Decimal::ZERO)?,
                realized_pnl: SignedUsdAmount::new(Decimal::ZERO)?,
                updated_at: now,
                version: 1,
            }
        };

        let current_signal = signals.get(&order.signal_id).cloned().ok_or_else(|| {
            AppError::not_found(
                "SIGNAL_NOT_FOUND",
                format!("signal was not found: {}", order.signal_id),
            )
        })?;

        orders.insert(order.id.clone(), order.clone());
        trades.insert(trade.id.clone(), trade.clone());
        positions.insert(position.id.clone(), position.clone());
        if current_signal.lifecycle_state != SignalLifecycleState::Executed {
            let next_signal = SignalView {
                lifecycle_state: SignalLifecycleState::Executed,
                updated_at: now,
                version: current_signal.version + 1,
                ..current_signal.clone()
            };
            signals.insert(next_signal.id.clone(), next_signal.clone());
            transitions.push(SignalTransitionView {
                id: format!("sgt_{}", Uuid::now_v7()),
                signal_id: next_signal.id.clone(),
                from_state: current_signal.lifecycle_state,
                to_state: SignalLifecycleState::Executed,
                trigger_type: "execution_fill_reconciled".to_string(),
                trigger_payload: json!({
                    "execution_request_id": execution_request_id,
                    "order_id": order.id,
                    "trade_id": trade.id,
                    "trace_id": trace_id,
                }),
                created_at: now,
            });
        }

        Ok(ExecutionFillResult {
            order,
            trade,
            position,
        })
    }

    async fn ingest_fixture_bundle(
        &self,
        bundle: &FixtureBundle,
        _trace_id: &str,
    ) -> Result<FixtureIngestionReport> {
        {
            let mut markets = self.markets.write().await;
            for market in &bundle.markets {
                markets.insert(
                    market.id.clone(),
                    MarketView {
                        id: market.id.clone(),
                        question: market.question.clone(),
                        category: market.category.clone(),
                        status: market.status,
                        best_bid: market.best_bid,
                        best_ask: market.best_ask,
                        mid_price: market.mid_price,
                        volume_24h: market.volume_24h,
                        ambiguity_level: market.ambiguity_level,
                        tradability_status: market.tradability_status,
                        resolution_source: market.resolution_source.clone(),
                        edge_case_notes: market.edge_case_notes.clone(),
                        polymarket_condition_id: market.polymarket_condition_id.clone(),
                        polymarket_yes_asset_id: market.polymarket_yes_asset_id.clone(),
                        polymarket_no_asset_id: market.polymarket_no_asset_id.clone(),
                        updated_at: market.updated_at,
                        version: market.version,
                    },
                );
            }
        }

        {
            let mut events = self.events.write().await;
            for event in &bundle.events {
                events.insert(
                    event.id.clone(),
                    EventView {
                        id: event.id.clone(),
                        source: event.source.clone(),
                        summary: event.summary.clone(),
                        relevance_score: event.relevance_score,
                        confidence: event.confidence,
                        status: event.status,
                        related_market_ids: event.related_market_ids.clone(),
                        reason_trace: event.reason_trace.clone(),
                        created_at: event.created_at,
                        updated_at: event.updated_at,
                        version: event.version,
                    },
                );
            }
        }

        {
            let mut evidences = self.evidences.write().await;
            for evidence in &bundle.evidences {
                evidences.insert(
                    evidence.id.clone(),
                    EvidenceView {
                        id: evidence.id.clone(),
                        market_id: evidence.market_id.clone(),
                        event_id: evidence.event_id.clone(),
                        direction: evidence.direction,
                        strength: evidence.strength,
                        source_reliability: evidence.source_reliability,
                        novelty: evidence.novelty,
                        resolution_relevance: evidence.resolution_relevance,
                        status: evidence.status,
                        expires_at: evidence.expires_at,
                        created_at: evidence.created_at,
                        updated_at: evidence.updated_at,
                        version: evidence.version,
                    },
                );
            }
        }

        {
            let mut signals = self.signals.write().await;
            for signal in &bundle.signals {
                signals.insert(
                    signal.id.clone(),
                    SignalView {
                        id: signal.id.clone(),
                        market_id: signal.market_id.clone(),
                        event_id: signal.event_id.clone(),
                        action: signal.action,
                        side: signal.side,
                        market_price: signal.market_price,
                        fair_price: signal.fair_price,
                        edge: signal.edge,
                        confidence: signal.confidence,
                        lifecycle_state: signal.lifecycle_state,
                        reason: signal.reason.clone(),
                        risk_decision: signal.risk_decision.clone(),
                        evidence_ids: signal.evidence_ids.clone(),
                        approved_by_user_id: signal.approved_by_user_id.clone(),
                        approved_at: signal.approved_at,
                        rejected_by_user_id: signal.rejected_by_user_id.clone(),
                        rejected_at: signal.rejected_at,
                        updated_at: signal.updated_at,
                        version: signal.version,
                    },
                );
            }
        }

        Ok(FixtureIngestionReport {
            markets_upserted: bundle.markets.len(),
            events_upserted: bundle.events.len(),
            evidences_upserted: bundle.evidences.len(),
            signals_upserted: bundle.signals.len(),
        })
    }
}

#[async_trait]
impl NewsIngestionStore for InMemoryMarketEventStore {
    async fn list_news_source_health(
        &self,
        filters: &NewsSourceHealthListFilters,
    ) -> Result<Vec<NewsSourceHealthView>> {
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
        items.truncate(usize::from(filters.limit));
        Ok(items)
    }

    async fn list_raw_news_events(
        &self,
        filters: &NewsRawEventListFilters,
    ) -> Result<Vec<NewsRawEventView>> {
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
        items.truncate(usize::from(filters.limit));
        Ok(items)
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
        filters: &ArbitrageScanListFilters,
    ) -> Result<Vec<ArbitrageScanView>> {
        let scans = self.arbitrage_scans.read().await;
        let mut items: Vec<_> = scans.values().cloned().collect();
        items.sort_by(|left, right| {
            right
                .started_at
                .cmp(&left.started_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        items.truncate(usize::from(filters.limit));
        Ok(items)
    }

    async fn list_arbitrage_opportunities(
        &self,
        filters: &ArbitrageOpportunityListFilters,
    ) -> Result<Vec<ArbitrageOpportunityView>> {
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
        items.truncate(usize::from(filters.limit));
        Ok(items)
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
        filters: &ArbitrageAnalysisRunListFilters,
    ) -> Result<Vec<ArbitrageAnalysisRunView>> {
        let runs = self.arbitrage_analysis_runs.read().await;
        let mut items: Vec<_> = runs.values().cloned().collect();
        items.sort_by(|left, right| {
            right
                .generated_at
                .cmp(&left.generated_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        items.truncate(usize::from(filters.limit));
        Ok(items)
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
    ) -> Result<Vec<ArbitrageEventView>> {
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
        items.truncate(usize::from(filters.limit));
        Ok(items)
    }

    async fn prune_arbitrage_events(&self, occurred_before: OffsetDateTime) -> Result<u64> {
        let mut events = self.arbitrage_events.write().await;
        let before = events.len();
        events.retain(|event| event.occurred_at >= occurred_before);
        usize_to_u64(before.saturating_sub(events.len()))
    }
}

pub struct PostgresMarketEventStore {
    pool: PgPool,
}

impl PostgresMarketEventStore {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl NewsIngestionStore for PostgresMarketEventStore {
    async fn list_news_source_health(
        &self,
        filters: &NewsSourceHealthListFilters,
    ) -> Result<Vec<NewsSourceHealthView>> {
        let rows = sqlx::query(
            r#"
            SELECT
              source,
              source_type,
              enabled,
              reliability,
              last_success_at,
              last_error_at,
              consecutive_failures,
              items_fetched,
              items_inserted,
              items_deduped,
              health_score,
              last_error,
              updated_at
            FROM news_source_health
            WHERE ($1::TEXT IS NULL OR source_type = $1)
            ORDER BY updated_at DESC, source ASC
            LIMIT $2
            "#,
        )
        .bind(filters.source_type.as_deref())
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list news source health: {error}"),
            )
        })?;

        rows.iter().map(parse_news_source_health_row).collect()
    }

    async fn list_raw_news_events(
        &self,
        filters: &NewsRawEventListFilters,
    ) -> Result<Vec<NewsRawEventView>> {
        let rows = sqlx::query(
            r#"
            SELECT
              id,
              source,
              source_type,
              external_id,
              title,
              url,
              author,
              published_at,
              event_time,
              hash,
              raw_payload,
              ingested_at,
              trace_id
            FROM raw_events
            WHERE source_type IS NOT NULL
              AND title IS NOT NULL
              AND event_time IS NOT NULL
              AND ($1::TEXT IS NULL OR source = $1)
              AND ($2::TEXT IS NULL OR source_type = $2)
            ORDER BY event_time DESC, ingested_at DESC, id ASC
            LIMIT $3
            "#,
        )
        .bind(filters.source.as_deref())
        .bind(filters.source_type.as_deref())
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list raw news events: {error}"),
            )
        })?;

        rows.iter().map(parse_news_raw_event_row).collect()
    }

    async fn insert_raw_news_event(&self, event: &NewsRawEventInsert) -> Result<bool> {
        let result = sqlx::query(
            r#"
            INSERT INTO raw_events (
              id,
              source,
              source_type,
              external_id,
              title,
              url,
              author,
              published_at,
              event_time,
              hash,
              raw_payload,
              ingested_at,
              trace_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(&event.id)
        .bind(&event.source)
        .bind(&event.source_type)
        .bind(&event.external_id)
        .bind(&event.title)
        .bind(&event.url)
        .bind(&event.author)
        .bind(event.published_at)
        .bind(event.event_time)
        .bind(&event.hash)
        .bind(Json(event.raw_payload.clone()))
        .bind(event.ingested_at)
        .bind(&event.trace_id)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_INSERT_FAILED",
                format!("failed to insert raw news event {}: {error}", event.id),
            )
        })?;

        Ok(result.rows_affected() > 0)
    }

    async fn record_news_source_success(&self, update: &NewsSourceSuccessUpdate) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO news_source_health (
              source,
              source_type,
              enabled,
              reliability,
              last_success_at,
              last_error_at,
              consecutive_failures,
              items_fetched,
              items_inserted,
              items_deduped,
              health_score,
              last_error,
              updated_at,
              trace_id
            )
            VALUES ($1, $2, TRUE, $3, $4, NULL, 0, $5, $6, $7, $3, NULL, $4, $8)
            ON CONFLICT (source) DO UPDATE
            SET
              source_type = EXCLUDED.source_type,
              enabled = TRUE,
              reliability = EXCLUDED.reliability,
              last_success_at = EXCLUDED.last_success_at,
              consecutive_failures = 0,
              items_fetched = news_source_health.items_fetched + EXCLUDED.items_fetched,
              items_inserted = news_source_health.items_inserted + EXCLUDED.items_inserted,
              items_deduped = news_source_health.items_deduped + EXCLUDED.items_deduped,
              health_score = EXCLUDED.health_score,
              last_error = NULL,
              updated_at = EXCLUDED.updated_at,
              trace_id = EXCLUDED.trace_id
            "#,
        )
        .bind(&update.source)
        .bind(&update.source_type)
        .bind(update.reliability.value())
        .bind(update.observed_at)
        .bind(usize_to_i64(update.fetched)?)
        .bind(usize_to_i64(update.inserted)?)
        .bind(usize_to_i64(update.deduped)?)
        .bind(&update.trace_id)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!(
                    "failed to record news source success {}: {error}",
                    update.source
                ),
            )
        })?;

        Ok(())
    }

    async fn record_news_source_failure(&self, update: &NewsSourceFailureUpdate) -> Result<()> {
        let last_error = clamped_error_message(&update.error_message);
        sqlx::query(
            r#"
            INSERT INTO news_source_health (
              source,
              source_type,
              enabled,
              reliability,
              last_success_at,
              last_error_at,
              consecutive_failures,
              items_fetched,
              items_inserted,
              items_deduped,
              health_score,
              last_error,
              updated_at,
              trace_id
            )
            VALUES (
              $1,
              $2,
              TRUE,
              $3,
              NULL,
              $4,
              1,
              0,
              0,
              0,
              GREATEST(0::numeric, $3 - (1::numeric / 5::numeric)),
              $5,
              $4,
              $6
            )
            ON CONFLICT (source) DO UPDATE
            SET
              source_type = EXCLUDED.source_type,
              enabled = TRUE,
              reliability = EXCLUDED.reliability,
              last_error_at = EXCLUDED.last_error_at,
              consecutive_failures = news_source_health.consecutive_failures + 1,
              health_score = GREATEST(
                0::numeric,
                EXCLUDED.reliability
                  - (LEAST(news_source_health.consecutive_failures + 1, 5)::numeric / 5::numeric)
              ),
              last_error = EXCLUDED.last_error,
              updated_at = EXCLUDED.updated_at,
              trace_id = EXCLUDED.trace_id
            "#,
        )
        .bind(&update.source)
        .bind(&update.source_type)
        .bind(update.reliability.value())
        .bind(update.observed_at)
        .bind(last_error)
        .bind(&update.trace_id)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!(
                    "failed to record news source failure {}: {error}",
                    update.source
                ),
            )
        })?;

        Ok(())
    }
}

#[async_trait]
impl ArbitrageStore for PostgresMarketEventStore {
    async fn start_arbitrage_scan(&self, scan: &ArbitrageScanView) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO arbitrage_scans (
              id,
              started_at,
              finished_at,
              market_count,
              snapshot_count,
              opportunity_count,
              scanner_version,
              metadata_json,
              trace_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(&scan.id)
        .bind(scan.started_at)
        .bind(scan.finished_at)
        .bind(i64::from(scan.market_count))
        .bind(i64::from(scan.snapshot_count))
        .bind(i64::from(scan.opportunity_count))
        .bind(&scan.scanner_version)
        .bind(Json(scan.metadata.clone()))
        .bind(&scan.trace_id)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_INSERT_FAILED",
                format!("failed to insert arbitrage scan {}: {error}", scan.id),
            )
        })?;

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
        let row = sqlx::query(
            r#"
            UPDATE arbitrage_scans
            SET
              finished_at = $2,
              market_count = $3,
              snapshot_count = $4,
              opportunity_count = $5
            WHERE id = $1
            RETURNING
              id,
              started_at,
              finished_at,
              market_count,
              snapshot_count,
              opportunity_count,
              scanner_version,
              metadata_json,
              trace_id
            "#,
        )
        .bind(scan_id)
        .bind(finished_at)
        .bind(i64::from(market_count))
        .bind(i64::from(snapshot_count))
        .bind(i64::from(opportunity_count))
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!("failed to complete arbitrage scan {scan_id}: {error}"),
            )
        })?;

        row.as_ref()
            .map(parse_arbitrage_scan_row)
            .transpose()?
            .ok_or_else(|| {
                AppError::not_found(
                    "ARBITRAGE_SCAN_NOT_FOUND",
                    format!("arbitrage scan was not found: {scan_id}"),
                )
            })
    }

    async fn record_market_book_snapshot(&self, snapshot: &MarketBookSnapshotView) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO market_book_snapshots (
              id,
              scan_id,
              connector_name,
              market_id,
              yes_asset_id,
              no_asset_id,
              yes_bid,
              yes_ask,
              yes_bid_size,
              yes_ask_size,
              no_bid,
              no_ask,
              no_bid_size,
              no_ask_size,
              observed_at,
              raw_payload_json,
              trace_id
            )
            VALUES (
              $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
              $11, $12, $13, $14, $15, $16, $17
            )
            ON CONFLICT (id) DO UPDATE
            SET
              connector_name = EXCLUDED.connector_name,
              market_id = EXCLUDED.market_id,
              yes_asset_id = EXCLUDED.yes_asset_id,
              no_asset_id = EXCLUDED.no_asset_id,
              yes_bid = EXCLUDED.yes_bid,
              yes_ask = EXCLUDED.yes_ask,
              yes_bid_size = EXCLUDED.yes_bid_size,
              yes_ask_size = EXCLUDED.yes_ask_size,
              no_bid = EXCLUDED.no_bid,
              no_ask = EXCLUDED.no_ask,
              no_bid_size = EXCLUDED.no_bid_size,
              no_ask_size = EXCLUDED.no_ask_size,
              observed_at = EXCLUDED.observed_at,
              raw_payload_json = EXCLUDED.raw_payload_json,
              trace_id = EXCLUDED.trace_id
            "#,
        )
        .bind(&snapshot.id)
        .bind(&snapshot.scan_id)
        .bind(&snapshot.connector_name)
        .bind(&snapshot.market_id)
        .bind(&snapshot.yes_asset_id)
        .bind(&snapshot.no_asset_id)
        .bind(snapshot.yes_bid.map(Probability::value))
        .bind(snapshot.yes_ask.map(Probability::value))
        .bind(snapshot.yes_bid_size.value())
        .bind(snapshot.yes_ask_size.value())
        .bind(snapshot.no_bid.map(Probability::value))
        .bind(snapshot.no_ask.map(Probability::value))
        .bind(snapshot.no_bid_size.value())
        .bind(snapshot.no_ask_size.value())
        .bind(snapshot.observed_at)
        .bind(Json(snapshot.raw_payload.clone()))
        .bind(&snapshot.trace_id)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!(
                    "failed to record market book snapshot {}: {error}",
                    snapshot.id
                ),
            )
        })?;

        Ok(())
    }

    async fn record_arbitrage_opportunity(
        &self,
        opportunity: &ArbitrageOpportunityView,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO arbitrage_opportunities (
              id,
              scan_id,
              market_id,
              opportunity_type,
              status,
              gross_edge,
              price_sum,
              capacity,
              yes_price,
              no_price,
              yes_size,
              no_size,
              observed_at,
              reason_codes_json,
              analysis_payload_json,
              trace_id
            )
            VALUES (
              $1, $2, $3, $4, $5, $6, $7, $8,
              $9, $10, $11, $12, $13, $14, $15, $16
            )
            ON CONFLICT (id) DO UPDATE
            SET
              status = EXCLUDED.status,
              gross_edge = EXCLUDED.gross_edge,
              price_sum = EXCLUDED.price_sum,
              capacity = EXCLUDED.capacity,
              yes_price = EXCLUDED.yes_price,
              no_price = EXCLUDED.no_price,
              yes_size = EXCLUDED.yes_size,
              no_size = EXCLUDED.no_size,
              observed_at = EXCLUDED.observed_at,
              reason_codes_json = EXCLUDED.reason_codes_json,
              analysis_payload_json = EXCLUDED.analysis_payload_json,
              trace_id = EXCLUDED.trace_id
            "#,
        )
        .bind(&opportunity.id)
        .bind(&opportunity.scan_id)
        .bind(&opportunity.market_id)
        .bind(opportunity.opportunity_type.as_str())
        .bind(opportunity.status.as_str())
        .bind(opportunity.gross_edge.value())
        .bind(opportunity.price_sum)
        .bind(opportunity.capacity.value())
        .bind(opportunity.yes_price.value())
        .bind(opportunity.no_price.value())
        .bind(opportunity.yes_size.value())
        .bind(opportunity.no_size.value())
        .bind(opportunity.observed_at)
        .bind(Json(opportunity.reason_codes.clone()))
        .bind(Json(opportunity.analysis_payload.clone()))
        .bind(&opportunity.trace_id)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!(
                    "failed to record arbitrage opportunity {}: {error}",
                    opportunity.id
                ),
            )
        })?;

        Ok(())
    }

    async fn record_arbitrage_opportunity_validation(
        &self,
        validation: &ArbitrageOpportunityValidationView,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO arbitrage_opportunity_validations (
              id,
              opportunity_id,
              status,
              gross_edge,
              net_edge,
              fee_estimate,
              slippage_buffer,
              validated_capacity,
              book_age_ms,
              reason_codes_json,
              validation_payload_json,
              validated_at,
              trace_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            ON CONFLICT (id) DO UPDATE
            SET
              status = EXCLUDED.status,
              gross_edge = EXCLUDED.gross_edge,
              net_edge = EXCLUDED.net_edge,
              fee_estimate = EXCLUDED.fee_estimate,
              slippage_buffer = EXCLUDED.slippage_buffer,
              validated_capacity = EXCLUDED.validated_capacity,
              book_age_ms = EXCLUDED.book_age_ms,
              reason_codes_json = EXCLUDED.reason_codes_json,
              validation_payload_json = EXCLUDED.validation_payload_json,
              validated_at = EXCLUDED.validated_at,
              trace_id = EXCLUDED.trace_id
            "#,
        )
        .bind(&validation.id)
        .bind(&validation.opportunity_id)
        .bind(validation.status.as_str())
        .bind(validation.gross_edge.value())
        .bind(validation.net_edge.value())
        .bind(validation.fee_estimate.value())
        .bind(validation.slippage_buffer.value())
        .bind(validation.validated_capacity.value())
        .bind(i64::try_from(validation.book_age_ms).unwrap_or(i64::MAX))
        .bind(Json(validation.reason_codes.clone()))
        .bind(Json(validation.validation_payload.clone()))
        .bind(validation.validated_at)
        .bind(&validation.trace_id)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!(
                    "failed to record arbitrage opportunity validation {}: {error}",
                    validation.id
                ),
            )
        })?;

        Ok(())
    }

    async fn expire_arbitrage_opportunities(
        &self,
        observed_before: OffsetDateTime,
        trace_id: &str,
    ) -> Result<Vec<ArbitrageOpportunityView>> {
        let rows = sqlx::query(
            r#"
            UPDATE arbitrage_opportunities
            SET
              status = 'expired',
              trace_id = $2
            WHERE observed_at < $1
              AND status <> 'expired'
            RETURNING
              id,
              scan_id,
              market_id,
              opportunity_type,
              status,
              gross_edge,
              price_sum,
              capacity,
              yes_price,
              no_price,
              yes_size,
              no_size,
              observed_at,
              reason_codes_json,
              analysis_payload_json,
              trace_id,
              NULL::TEXT AS validation_id,
              NULL::TEXT AS validation_status,
              NULL::NUMERIC AS validation_gross_edge,
              NULL::NUMERIC AS validation_net_edge,
              NULL::NUMERIC AS validation_fee_estimate,
              NULL::NUMERIC AS validation_slippage_buffer,
              NULL::NUMERIC AS validation_validated_capacity,
              NULL::BIGINT AS validation_book_age_ms,
              NULL::JSONB AS validation_reason_codes_json,
              NULL::JSONB AS validation_payload_json,
              NULL::TIMESTAMPTZ AS validation_validated_at,
              NULL::TEXT AS validation_trace_id
            "#,
        )
        .bind(observed_before)
        .bind(trace_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!("failed to expire arbitrage opportunities: {error}"),
            )
        })?;

        rows.iter().map(parse_arbitrage_opportunity_row).collect()
    }

    async fn list_arbitrage_scans(
        &self,
        filters: &ArbitrageScanListFilters,
    ) -> Result<Vec<ArbitrageScanView>> {
        let rows = sqlx::query(
            r#"
            SELECT
              id,
              started_at,
              finished_at,
              market_count,
              snapshot_count,
              opportunity_count,
              scanner_version,
              metadata_json,
              trace_id
            FROM arbitrage_scans
            ORDER BY started_at DESC, id ASC
            LIMIT $1
            "#,
        )
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list arbitrage scans: {error}"),
            )
        })?;

        rows.iter().map(parse_arbitrage_scan_row).collect()
    }

    async fn list_arbitrage_opportunities(
        &self,
        filters: &ArbitrageOpportunityListFilters,
    ) -> Result<Vec<ArbitrageOpportunityView>> {
        let rows = sqlx::query(
            r#"
            SELECT
              o.id,
              o.scan_id,
              o.market_id,
              o.opportunity_type,
              o.status,
              o.gross_edge,
              o.price_sum,
              o.capacity,
              o.yes_price,
              o.no_price,
              o.yes_size,
              o.no_size,
              o.observed_at,
              o.reason_codes_json,
              o.analysis_payload_json,
              o.trace_id,
              v.id AS validation_id,
              v.status AS validation_status,
              v.gross_edge AS validation_gross_edge,
              v.net_edge AS validation_net_edge,
              v.fee_estimate AS validation_fee_estimate,
              v.slippage_buffer AS validation_slippage_buffer,
              v.validated_capacity AS validation_validated_capacity,
              v.book_age_ms AS validation_book_age_ms,
              v.reason_codes_json AS validation_reason_codes_json,
              v.validation_payload_json AS validation_payload_json,
              v.validated_at AS validation_validated_at,
              v.trace_id AS validation_trace_id
            FROM arbitrage_opportunities o
            LEFT JOIN LATERAL (
              SELECT
                id,
                opportunity_id,
                status,
                gross_edge,
                net_edge,
                fee_estimate,
                slippage_buffer,
                validated_capacity,
                book_age_ms,
                reason_codes_json,
                validation_payload_json,
                validated_at,
                trace_id
              FROM arbitrage_opportunity_validations
              WHERE opportunity_id = o.id
              ORDER BY validated_at DESC, id ASC
              LIMIT 1
            ) v ON TRUE
            WHERE ($1::TEXT IS NULL OR o.market_id = $1)
              AND ($2::TEXT IS NULL OR o.opportunity_type = $2)
              AND ($3::TEXT IS NULL OR o.status = $3)
              AND (
                $4::TEXT IS NULL
                OR ($4 = 'unvalidated' AND v.id IS NULL)
                OR ($4 <> 'unvalidated' AND v.status = $4)
              )
              AND ($5::NUMERIC IS NULL OR v.net_edge >= $5)
              AND ($6::TIMESTAMPTZ IS NULL OR o.observed_at >= $6)
              AND (NOT $7::BOOL OR o.status <> 'expired')
            ORDER BY o.observed_at DESC, o.id ASC
            LIMIT $8
            "#,
        )
        .bind(filters.market_id.as_deref())
        .bind(
            filters
                .opportunity_type
                .map(ArbitrageOpportunityType::as_str),
        )
        .bind(filters.status.map(ArbitrageOpportunityStatus::as_str))
        .bind(
            filters
                .validation_status
                .map(ArbitrageValidationStatus::as_str),
        )
        .bind(filters.min_net_edge.map(Edge::value))
        .bind(filters.observed_after)
        .bind(filters.active_only)
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list arbitrage opportunities: {error}"),
            )
        })?;

        rows.iter().map(parse_arbitrage_opportunity_row).collect()
    }

    async fn record_arbitrage_analysis_run(
        &self,
        analysis: &ArbitrageAnalysisRunView,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO arbitrage_analysis_runs (
              id,
              generated_at,
              lookback_hours,
              opportunity_count,
              market_count,
              summary_payload_json,
              trace_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (id) DO UPDATE
            SET
              generated_at = EXCLUDED.generated_at,
              lookback_hours = EXCLUDED.lookback_hours,
              opportunity_count = EXCLUDED.opportunity_count,
              market_count = EXCLUDED.market_count,
              summary_payload_json = EXCLUDED.summary_payload_json,
              trace_id = EXCLUDED.trace_id
            "#,
        )
        .bind(&analysis.id)
        .bind(analysis.generated_at)
        .bind(i64::from(analysis.lookback_hours))
        .bind(i64::from(analysis.opportunity_count))
        .bind(i64::from(analysis.market_count))
        .bind(Json(analysis.summary_payload.clone()))
        .bind(&analysis.trace_id)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!(
                    "failed to record arbitrage analysis run {}: {error}",
                    analysis.id
                ),
            )
        })?;

        Ok(())
    }

    async fn list_arbitrage_analysis_runs(
        &self,
        filters: &ArbitrageAnalysisRunListFilters,
    ) -> Result<Vec<ArbitrageAnalysisRunView>> {
        let rows = sqlx::query(
            r#"
            SELECT
              id,
              generated_at,
              lookback_hours,
              opportunity_count,
              market_count,
              summary_payload_json,
              trace_id
            FROM arbitrage_analysis_runs
            ORDER BY generated_at DESC, id ASC
            LIMIT $1
            "#,
        )
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list arbitrage analysis runs: {error}"),
            )
        })?;

        rows.iter().map(parse_arbitrage_analysis_run_row).collect()
    }

    async fn record_arbitrage_event(
        &self,
        event: &ArbitrageEventView,
    ) -> Result<ArbitrageEventView> {
        let row = sqlx::query(
            r#"
            INSERT INTO arbitrage_events (
              id,
              event_type,
              resource_type,
              resource_id,
              payload_json,
              occurred_at,
              trace_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (id) DO UPDATE
            SET
              event_type = EXCLUDED.event_type,
              resource_type = EXCLUDED.resource_type,
              resource_id = EXCLUDED.resource_id,
              payload_json = EXCLUDED.payload_json,
              occurred_at = EXCLUDED.occurred_at,
              trace_id = EXCLUDED.trace_id
            RETURNING
              sequence,
              id,
              event_type,
              resource_type,
              resource_id,
              payload_json,
              occurred_at,
              trace_id
            "#,
        )
        .bind(&event.id)
        .bind(event.event_type.as_str())
        .bind(&event.resource_type)
        .bind(&event.resource_id)
        .bind(Json(event.payload.clone()))
        .bind(event.occurred_at)
        .bind(&event.trace_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPSERT_FAILED",
                format!("failed to record arbitrage event {}: {error}", event.id),
            )
        })?;

        parse_arbitrage_event_row(&row)
    }

    async fn list_arbitrage_events(
        &self,
        filters: &ArbitrageEventListFilters,
    ) -> Result<Vec<ArbitrageEventView>> {
        let after_sequence = filters
            .after_sequence
            .map(|sequence| {
                i64::try_from(sequence).map_err(|error| {
                    AppError::invalid_input(
                        "ARBITRAGE_EVENT_SEQUENCE_OUT_OF_RANGE",
                        format!("arbitrage event sequence does not fit i64: {error}"),
                    )
                })
            })
            .transpose()?;
        let rows = sqlx::query(
            r#"
            SELECT
              sequence,
              id,
              event_type,
              resource_type,
              resource_id,
              payload_json,
              occurred_at,
              trace_id
            FROM arbitrage_events
            WHERE ($1::BIGINT IS NULL OR sequence > $1)
            ORDER BY sequence ASC
            LIMIT $2
            "#,
        )
        .bind(after_sequence)
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list arbitrage events: {error}"),
            )
        })?;

        rows.iter().map(parse_arbitrage_event_row).collect()
    }

    async fn prune_arbitrage_events(&self, occurred_before: OffsetDateTime) -> Result<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM arbitrage_events
            WHERE occurred_at < $1
            "#,
        )
        .bind(occurred_before)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_DELETE_FAILED",
                format!("failed to prune arbitrage events: {error}"),
            )
        })?;

        Ok(result.rows_affected())
    }
}

#[async_trait]
impl MarketEventStore for PostgresMarketEventStore {
    async fn list_markets(&self, filters: &MarketListFilters) -> Result<Vec<MarketView>> {
        let rows = sqlx::query(
            r#"
            SELECT
              m.id,
              m.question,
              m.category,
              m.status,
              m.best_bid,
              m.best_ask,
              m.mid_price,
              m.volume_24h,
              m.ambiguity_level,
              m.tradability_status,
              r.resolution_source,
              r.edge_case_notes,
              m.polymarket_condition_id,
              m.polymarket_yes_asset_id,
              m.polymarket_no_asset_id,
              m.updated_at,
              m.version
            FROM markets m
            INNER JOIN market_resolution_rules r ON r.market_id = m.id
            WHERE ($1::TEXT IS NULL OR m.status = $1)
              AND ($2::TEXT IS NULL OR m.tradability_status = $2)
            ORDER BY m.updated_at DESC, m.id ASC
            LIMIT $3
            "#,
        )
        .bind(filters.status.map(MarketStatus::as_str))
        .bind(filters.tradability_status.map(TradabilityStatus::as_str))
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list markets: {error}"),
            )
        })?;

        rows.iter().map(parse_market_row).collect()
    }

    async fn get_market(&self, market_id: &str) -> Result<Option<MarketView>> {
        let row = sqlx::query(
            r#"
            SELECT
              m.id,
              m.question,
              m.category,
              m.status,
              m.best_bid,
              m.best_ask,
              m.mid_price,
              m.volume_24h,
              m.ambiguity_level,
              m.tradability_status,
              r.resolution_source,
              r.edge_case_notes,
              m.polymarket_condition_id,
              m.polymarket_yes_asset_id,
              m.polymarket_no_asset_id,
              m.updated_at,
              m.version
            FROM markets m
            INNER JOIN market_resolution_rules r ON r.market_id = m.id
            WHERE m.id = $1
            "#,
        )
        .bind(market_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to fetch market {market_id}: {error}"),
            )
        })?;

        row.as_ref().map(parse_market_row).transpose()
    }

    async fn get_signal(&self, signal_id: &str) -> Result<Option<SignalView>> {
        let row = sqlx::query(
            r#"
            SELECT
              s.id,
              s.market_id,
              s.event_id,
              s.action,
              s.side,
              s.market_price,
              s.fair_price,
              s.edge,
              s.confidence,
              s.lifecycle_state,
              s.reason,
              s.risk_decision,
              s.approved_by_user_id,
              s.approved_at,
              s.rejected_by_user_id,
              s.rejected_at,
              s.updated_at,
              s.version,
              COALESCE(
                array_agg(sel.evidence_id ORDER BY sel.evidence_id)
                FILTER (WHERE sel.evidence_id IS NOT NULL),
                '{}'::TEXT[]
              ) AS evidence_ids
            FROM signals s
            LEFT JOIN signal_evidence_links sel ON sel.signal_id = s.id
            WHERE s.id = $1
            GROUP BY
              s.id,
              s.market_id,
              s.event_id,
              s.action,
              s.side,
              s.market_price,
              s.fair_price,
              s.edge,
              s.confidence,
              s.lifecycle_state,
              s.reason,
              s.risk_decision,
              s.approved_by_user_id,
              s.approved_at,
              s.rejected_by_user_id,
              s.rejected_at,
              s.updated_at,
              s.version
            "#,
        )
        .bind(signal_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to fetch signal {signal_id}: {error}"),
            )
        })?;

        row.as_ref().map(parse_signal_row).transpose()
    }

    async fn list_events(&self, filters: &EventListFilters) -> Result<Vec<EventView>> {
        let rows = sqlx::query(
            r#"
            SELECT
              e.id,
              e.source,
              e.summary,
              e.relevance_score,
              e.confidence,
              e.status,
              e.reason_trace,
              e.created_at,
              e.updated_at,
              e.version,
              COALESCE(
                array_agg(eml.market_id ORDER BY eml.market_id)
                FILTER (WHERE eml.market_id IS NOT NULL),
                '{}'::TEXT[]
              ) AS related_market_ids
            FROM events e
            LEFT JOIN event_market_links eml ON eml.event_id = e.id
            WHERE ($1::TEXT IS NULL OR e.status = $1)
            GROUP BY
              e.id,
              e.source,
              e.summary,
              e.relevance_score,
              e.confidence,
              e.status,
              e.reason_trace,
              e.created_at,
              e.updated_at,
              e.version
            ORDER BY e.updated_at DESC, e.id ASC
            LIMIT $2
            "#,
        )
        .bind(filters.status.map(EventStatus::as_str))
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list events: {error}"),
            )
        })?;

        rows.iter().map(parse_event_row).collect()
    }

    async fn list_evidences(&self, filters: &EvidenceListFilters) -> Result<Vec<EvidenceView>> {
        let rows = sqlx::query(
            r#"
            SELECT
              id,
              market_id,
              event_id,
              direction,
              strength,
              source_reliability,
              novelty,
              resolution_relevance,
              status,
              expires_at,
              created_at,
              updated_at,
              version
            FROM evidences
            WHERE ($1::TEXT IS NULL OR market_id = $1)
              AND ($2::TEXT IS NULL OR event_id = $2)
              AND ($3::TEXT IS NULL OR status = $3)
            ORDER BY created_at DESC, id ASC
            LIMIT $4
            "#,
        )
        .bind(filters.market_id.as_deref())
        .bind(filters.event_id.as_deref())
        .bind(filters.status.map(EvidenceStatus::as_str))
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list evidences: {error}"),
            )
        })?;

        rows.iter().map(parse_evidence_row).collect()
    }

    async fn list_signals(&self, filters: &SignalListFilters) -> Result<Vec<SignalView>> {
        let rows = sqlx::query(
            r#"
            SELECT
              s.id,
              s.market_id,
              s.event_id,
              s.action,
              s.side,
              s.market_price,
              s.fair_price,
              s.edge,
              s.confidence,
              s.lifecycle_state,
              s.reason,
              s.risk_decision,
              s.approved_by_user_id,
              s.approved_at,
              s.rejected_by_user_id,
              s.rejected_at,
              s.updated_at,
              s.version,
              COALESCE(
                array_agg(sel.evidence_id ORDER BY sel.evidence_id)
                FILTER (WHERE sel.evidence_id IS NOT NULL),
                '{}'::TEXT[]
              ) AS evidence_ids
            FROM signals s
            LEFT JOIN signal_evidence_links sel ON sel.signal_id = s.id
            WHERE ($1::TEXT IS NULL OR s.market_id = $1)
              AND ($2::TEXT IS NULL OR s.event_id = $2)
              AND ($3::TEXT IS NULL OR s.lifecycle_state = $3)
            GROUP BY
              s.id,
              s.market_id,
              s.event_id,
              s.action,
              s.side,
              s.market_price,
              s.fair_price,
              s.edge,
              s.confidence,
              s.lifecycle_state,
              s.reason,
              s.risk_decision,
              s.approved_by_user_id,
              s.approved_at,
              s.rejected_by_user_id,
              s.rejected_at,
              s.updated_at,
              s.version
            ORDER BY s.updated_at DESC, s.id ASC
            LIMIT $4
            "#,
        )
        .bind(filters.market_id.as_deref())
        .bind(filters.event_id.as_deref())
        .bind(filters.lifecycle_state.map(SignalLifecycleState::as_str))
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list signals: {error}"),
            )
        })?;

        rows.iter().map(parse_signal_row).collect()
    }

    async fn list_probability_estimates(
        &self,
        filters: &ProbabilityEstimateListFilters,
    ) -> Result<Vec<ProbabilityEstimateView>> {
        let rows = sqlx::query(
            r#"
            SELECT
              id,
              market_id,
              event_id,
              signal_id,
              prior_price,
              posterior_price,
              fair_price,
              market_price,
              edge,
              confidence,
              time_horizon,
              model_version,
              reason_codes_json,
              evidence_count,
              created_at
            FROM probability_estimates
            WHERE ($1::TEXT IS NULL OR market_id = $1)
              AND ($2::TEXT IS NULL OR event_id = $2)
              AND ($3::TEXT IS NULL OR signal_id = $3)
            ORDER BY created_at DESC, id ASC
            LIMIT $4
            "#,
        )
        .bind(filters.market_id.as_deref())
        .bind(filters.event_id.as_deref())
        .bind(filters.signal_id.as_deref())
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list probability estimates: {error}"),
            )
        })?;

        rows.iter().map(parse_probability_estimate_row).collect()
    }

    async fn list_signal_transitions(
        &self,
        filters: &SignalTransitionListFilters,
    ) -> Result<Vec<SignalTransitionView>> {
        let rows = sqlx::query(
            r#"
            SELECT
              id,
              signal_id,
              from_state,
              to_state,
              trigger_type,
              trigger_payload_json,
              created_at
            FROM signal_transitions
            WHERE signal_id = $1
            ORDER BY created_at DESC, id ASC
            LIMIT $2
            "#,
        )
        .bind(&filters.signal_id)
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list signal transitions: {error}"),
            )
        })?;

        rows.iter().map(parse_signal_transition_row).collect()
    }

    async fn list_order_drafts(
        &self,
        filters: &OrderDraftListFilters,
    ) -> Result<Vec<OrderDraftView>> {
        let rows = sqlx::query(
            r#"
            SELECT
              id,
              signal_id,
              signal_version,
              market_id,
              connector_name,
              side,
              limit_price,
              quantity,
              notional,
              status,
              created_by_user_id,
              external_order_id,
              submitted_at,
              failure_code,
              failure_message,
              created_at,
              updated_at,
              version
            FROM order_drafts
            WHERE ($1::TEXT IS NULL OR signal_id = $1)
              AND ($2::TEXT IS NULL OR connector_name = $2)
              AND ($3::TEXT IS NULL OR status = $3)
            ORDER BY created_at DESC, id ASC
            LIMIT $4
            "#,
        )
        .bind(filters.signal_id.as_deref())
        .bind(filters.connector_name.as_deref())
        .bind(filters.status.map(OrderDraftStatus::as_str))
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list order drafts: {error}"),
            )
        })?;

        rows.iter().map(parse_order_draft_row).collect()
    }

    async fn list_execution_requests(
        &self,
        filters: &ExecutionRequestListFilters,
    ) -> Result<Vec<ExecutionRequestView>> {
        let rows = sqlx::query(
            r#"
            SELECT
              id,
              signal_id,
              signal_version,
              order_draft_id,
              connector_name,
              mode,
              requested_by_user_id,
              status,
              reason,
              external_order_id,
              submitted_at,
              failure_code,
              failure_message,
              created_at,
              updated_at,
              version
            FROM execution_requests
            WHERE ($1::TEXT IS NULL OR signal_id = $1)
              AND ($2::TEXT IS NULL OR connector_name = $2)
              AND ($3::TEXT IS NULL OR status = $3)
            ORDER BY created_at DESC, id ASC
            LIMIT $4
            "#,
        )
        .bind(filters.signal_id.as_deref())
        .bind(filters.connector_name.as_deref())
        .bind(filters.status.map(ExecutionRequestStatus::as_str))
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list execution requests: {error}"),
            )
        })?;

        rows.iter().map(parse_execution_request_row).collect()
    }

    async fn get_order_by_external_ref(
        &self,
        connector_name: &str,
        external_order_id: &str,
    ) -> Result<OrderView> {
        let row = sqlx::query(
            r#"
            SELECT
              id,
              signal_id,
              execution_request_id,
              order_draft_id,
              market_id,
              connector_name,
              account_id,
              external_order_id,
              side,
              limit_price,
              quantity,
              filled_quantity,
              avg_fill_price,
              status,
              submitted_at,
              updated_at,
              version
            FROM orders
            WHERE connector_name = $1
              AND external_order_id = $2
            LIMIT 1
            "#,
        )
        .bind(connector_name)
        .bind(external_order_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!(
                    "failed to load order for connector={} external_order_id={}: {error}",
                    connector_name, external_order_id
                ),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(
                "ORDER_NOT_FOUND",
                format!(
                    "order was not found for connector={} external_order_id={}",
                    connector_name, external_order_id
                ),
            )
        })?;

        parse_order_row(&row)
    }

    async fn list_orders(&self, filters: &OrderListFilters) -> Result<Vec<OrderView>> {
        let rows = sqlx::query(
            r#"
            SELECT
              id,
              signal_id,
              execution_request_id,
              order_draft_id,
              market_id,
              connector_name,
              account_id,
              external_order_id,
              side,
              limit_price,
              quantity,
              filled_quantity,
              avg_fill_price,
              status,
              submitted_at,
              updated_at,
              version
            FROM orders
            WHERE ($1::TEXT IS NULL OR signal_id = $1)
              AND ($2::TEXT IS NULL OR market_id = $2)
              AND ($3::TEXT IS NULL OR connector_name = $3)
              AND ($4::TEXT IS NULL OR status = $4)
            ORDER BY updated_at DESC, id ASC
            LIMIT $5
            "#,
        )
        .bind(filters.signal_id.as_deref())
        .bind(filters.market_id.as_deref())
        .bind(filters.connector_name.as_deref())
        .bind(filters.status.map(OrderStatus::as_str))
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list orders: {error}"),
            )
        })?;

        rows.iter().map(parse_order_row).collect()
    }

    async fn list_trades(&self, filters: &TradeListFilters) -> Result<Vec<TradeView>> {
        let rows = sqlx::query(
            r#"
            SELECT
              id,
              order_id,
              signal_id,
              market_id,
              connector_name,
              external_trade_id,
              side,
              price,
              quantity,
              fee,
              executed_at
            FROM trades
            WHERE ($1::TEXT IS NULL OR order_id = $1)
              AND ($2::TEXT IS NULL OR signal_id = $2)
              AND ($3::TEXT IS NULL OR market_id = $3)
              AND ($4::TEXT IS NULL OR connector_name = $4)
            ORDER BY executed_at DESC, id ASC
            LIMIT $5
            "#,
        )
        .bind(filters.order_id.as_deref())
        .bind(filters.signal_id.as_deref())
        .bind(filters.market_id.as_deref())
        .bind(filters.connector_name.as_deref())
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list trades: {error}"),
            )
        })?;

        rows.iter().map(parse_trade_row).collect()
    }

    async fn list_positions(&self, filters: &PositionListFilters) -> Result<Vec<PositionView>> {
        let rows = sqlx::query(
            r#"
            SELECT
              id,
              market_id,
              connector_name,
              account_id,
              side,
              net_quantity,
              avg_cost,
              mark_price,
              unrealized_pnl,
              realized_pnl,
              updated_at,
              version
            FROM positions
            WHERE ($1::TEXT IS NULL OR market_id = $1)
              AND ($2::TEXT IS NULL OR connector_name = $2)
              AND ($3::TEXT IS NULL OR side = $3)
            ORDER BY updated_at DESC, id ASC
            LIMIT $4
            "#,
        )
        .bind(filters.market_id.as_deref())
        .bind(filters.connector_name.as_deref())
        .bind(filters.side.map(SignalSide::as_str))
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list positions: {error}"),
            )
        })?;

        rows.iter().map(parse_position_row).collect()
    }

    async fn recompute_signal(
        &self,
        command: &RecomputeSignalCommand,
    ) -> Result<RecomputeSignalResult> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin signal recompute transaction: {error}"),
            )
        })?;

        let signal_row = sqlx::query(
            r#"
            SELECT
              s.id,
              s.market_id,
              s.event_id,
              s.action,
              s.side,
              s.market_price,
              s.fair_price,
              s.edge,
              s.confidence,
              s.lifecycle_state,
              s.reason,
              s.risk_decision,
              s.approved_by_user_id,
              s.approved_at,
              s.rejected_by_user_id,
              s.rejected_at,
              s.updated_at,
              s.version,
              COALESCE(
                array_agg(sel.evidence_id ORDER BY sel.evidence_id)
                FILTER (WHERE sel.evidence_id IS NOT NULL),
                '{}'::TEXT[]
              ) AS evidence_ids
            FROM signals s
            LEFT JOIN signal_evidence_links sel ON sel.signal_id = s.id
            WHERE s.id = $1
            GROUP BY
              s.id,
              s.market_id,
              s.event_id,
              s.action,
              s.side,
              s.market_price,
              s.fair_price,
              s.edge,
              s.confidence,
              s.lifecycle_state,
              s.reason,
              s.risk_decision,
              s.approved_by_user_id,
              s.approved_at,
              s.rejected_by_user_id,
              s.rejected_at,
              s.updated_at,
              s.version
            FOR UPDATE
            "#,
        )
        .bind(&command.signal_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to lock signal {}: {error}", command.signal_id),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(
                "SIGNAL_NOT_FOUND",
                format!("signal was not found: {}", command.signal_id),
            )
        })?;
        let current_signal = parse_signal_row(&signal_row)?;

        let market = fetch_market_by_id(&mut transaction, &current_signal.market_id)
            .await?
            .ok_or_else(|| {
                AppError::not_found(
                    "MARKET_NOT_FOUND",
                    format!("market was not found: {}", current_signal.market_id),
                )
            })?;

        let evidences = fetch_evidences_for_signal(
            &mut transaction,
            &current_signal.market_id,
            &current_signal.event_id,
        )
        .await?;
        let source_health =
            fetch_source_health_adjustment_for_event(&mut transaction, &current_signal.event_id)
                .await?;
        let estimate_id = format!("est_{}", Uuid::now_v7());
        let draft = build_recompute_signal_draft_with_source_health(
            &current_signal,
            &market,
            &evidences,
            &command.reason,
            source_health.as_ref(),
            &estimate_id,
        )?;

        sqlx::query(
            r#"
            INSERT INTO probability_estimates (
              id,
              market_id,
              event_id,
              signal_id,
              prior_price,
              posterior_price,
              fair_price,
              market_price,
              edge,
              confidence,
              time_horizon,
              model_version,
              reason_codes_json,
              evidence_count,
              trace_id,
              created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            "#,
        )
        .bind(&draft.estimate.id)
        .bind(&draft.estimate.market_id)
        .bind(&draft.estimate.event_id)
        .bind(draft.estimate.signal_id.as_deref())
        .bind(draft.estimate.prior_price.value())
        .bind(draft.estimate.posterior_price.value())
        .bind(draft.estimate.fair_price.value())
        .bind(draft.estimate.market_price.value())
        .bind(draft.estimate.edge.value())
        .bind(draft.estimate.confidence.value())
        .bind(draft.estimate.time_horizon.as_str())
        .bind(&draft.estimate.model_version)
        .bind(Json(draft.estimate.reason_codes.clone()))
        .bind(i32::try_from(draft.estimate.evidence_count).unwrap_or(i32::MAX))
        .bind(&command.trace_id)
        .bind(draft.estimate.created_at)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_INSERT_FAILED",
                format!("failed to insert probability estimate: {error}"),
            )
        })?;

        sqlx::query(
            r#"
            UPDATE signals
            SET
              action = $1,
              side = $2,
              market_price = $3,
              fair_price = $4,
              edge = $5,
              confidence = $6,
              lifecycle_state = $7,
              reason = $8,
              risk_decision = $9,
              approved_by_user_id = NULL,
              approved_at = NULL,
              rejected_by_user_id = NULL,
              rejected_at = NULL,
              estimate_id = $10,
              updated_at = $11,
              version = $12,
              trace_id = $13
            WHERE id = $14
            "#,
        )
        .bind(draft.next_signal.action.as_str())
        .bind(draft.next_signal.side.as_str())
        .bind(draft.next_signal.market_price.value())
        .bind(draft.next_signal.fair_price.value())
        .bind(draft.next_signal.edge.value())
        .bind(draft.next_signal.confidence.value())
        .bind(draft.next_signal.lifecycle_state.as_str())
        .bind(&draft.next_signal.reason)
        .bind(&draft.next_signal.risk_decision)
        .bind(&draft.estimate.id)
        .bind(draft.next_signal.updated_at)
        .bind(draft.next_signal.version)
        .bind(&command.trace_id)
        .bind(&draft.next_signal.id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!("failed to update signal {}: {error}", draft.next_signal.id),
            )
        })?;

        sqlx::query(
            r#"
            DELETE FROM signal_evidence_links
            WHERE signal_id = $1
            "#,
        )
        .bind(&draft.next_signal.id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_DELETE_FAILED",
                format!(
                    "failed to reset signal evidence links for {}: {error}",
                    draft.next_signal.id
                ),
            )
        })?;

        for evidence_id in &draft.next_signal.evidence_ids {
            sqlx::query(
                r#"
                INSERT INTO signal_evidence_links (signal_id, evidence_id, created_at)
                VALUES ($1, $2, $3)
                ON CONFLICT (signal_id, evidence_id) DO NOTHING
                "#,
            )
            .bind(&draft.next_signal.id)
            .bind(evidence_id)
            .bind(draft.next_signal.updated_at)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_INSERT_FAILED",
                    format!(
                        "failed to insert signal-evidence link {} -> {}: {error}",
                        draft.next_signal.id, evidence_id
                    ),
                )
            })?;
        }

        let transition = if let Some(transition) = draft.transition {
            let view = SignalTransitionView {
                id: format!("sgt_{}", Uuid::now_v7()),
                signal_id: draft.next_signal.id.clone(),
                from_state: transition.from_state,
                to_state: transition.to_state,
                trigger_type: transition.trigger_type,
                trigger_payload: transition.trigger_payload,
                created_at: transition.created_at,
            };

            sqlx::query(
                r#"
                INSERT INTO signal_transitions (
                  id,
                  signal_id,
                  from_state,
                  to_state,
                  trigger_type,
                  trigger_payload_json,
                  trace_id,
                  created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
            )
            .bind(&view.id)
            .bind(&view.signal_id)
            .bind(view.from_state.as_str())
            .bind(view.to_state.as_str())
            .bind(&view.trigger_type)
            .bind(Json(view.trigger_payload.clone()))
            .bind(&command.trace_id)
            .bind(view.created_at)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_INSERT_FAILED",
                    format!("failed to insert signal transition: {error}"),
                )
            })?;

            Some(view)
        } else {
            None
        };

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit signal recompute transaction: {error}"),
            )
        })?;

        Ok(RecomputeSignalResult {
            signal: draft.next_signal,
            estimate: draft.estimate,
            transition,
        })
    }

    async fn approve_signal(
        &self,
        signal_id: &str,
        approved_by_user_id: &str,
        approval_reason: &str,
        trace_id: &str,
        expected_version: Option<i64>,
    ) -> Result<SignalView> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin signal approval transaction: {error}"),
            )
        })?;

        let signal_row = sqlx::query(
            r#"
            SELECT
              s.id,
              s.market_id,
              s.event_id,
              s.action,
              s.side,
              s.market_price,
              s.fair_price,
              s.edge,
              s.confidence,
              s.lifecycle_state,
              s.reason,
              s.risk_decision,
              s.approved_by_user_id,
              s.approved_at,
              s.rejected_by_user_id,
              s.rejected_at,
              s.updated_at,
              s.version,
              COALESCE(
                array_agg(sel.evidence_id ORDER BY sel.evidence_id)
                FILTER (WHERE sel.evidence_id IS NOT NULL),
                '{}'::TEXT[]
              ) AS evidence_ids
            FROM signals s
            LEFT JOIN signal_evidence_links sel ON sel.signal_id = s.id
            WHERE s.id = $1
            GROUP BY
              s.id,
              s.market_id,
              s.event_id,
              s.action,
              s.side,
              s.market_price,
              s.fair_price,
              s.edge,
              s.confidence,
              s.lifecycle_state,
              s.reason,
              s.risk_decision,
              s.approved_by_user_id,
              s.approved_at,
              s.rejected_by_user_id,
              s.rejected_at,
              s.updated_at,
              s.version
            FOR UPDATE
            "#,
        )
        .bind(signal_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to lock signal {signal_id}: {error}"),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(
                "SIGNAL_NOT_FOUND",
                format!("signal was not found: {signal_id}"),
            )
        })?;
        let current_signal = parse_signal_row(&signal_row)?;

        if let Some(expected_version) = expected_version {
            if current_signal.version != expected_version {
                return Err(AppError::conflict(
                    "STATE_VERSION_MISMATCH",
                    "signal version does not match the expected_version",
                ));
            }
        }

        if current_signal.approved_by_user_id.is_some() {
            return Err(AppError::conflict(
                "STATE_SIGNAL_ALREADY_APPROVED",
                "signal has already been approved",
            ));
        }

        if current_signal.rejected_by_user_id.is_some() {
            return Err(AppError::conflict(
                "STATE_SIGNAL_ALREADY_REJECTED",
                "signal has already been rejected for the current version",
            ));
        }

        let approved_at = OffsetDateTime::now_utc();
        let next_signal = SignalView {
            id: current_signal.id.clone(),
            market_id: current_signal.market_id.clone(),
            event_id: current_signal.event_id.clone(),
            action: current_signal.action,
            side: current_signal.side,
            market_price: current_signal.market_price,
            fair_price: current_signal.fair_price,
            edge: current_signal.edge,
            confidence: current_signal.confidence,
            lifecycle_state: current_signal.lifecycle_state,
            reason: current_signal.reason.clone(),
            risk_decision: approval_reason.to_string(),
            evidence_ids: current_signal.evidence_ids.clone(),
            approved_by_user_id: Some(approved_by_user_id.to_string()),
            approved_at: Some(approved_at),
            rejected_by_user_id: None,
            rejected_at: None,
            updated_at: approved_at,
            version: current_signal.version + 1,
        };

        sqlx::query(
            r#"
            UPDATE signals
            SET
              risk_decision = $1,
              approved_by_user_id = $2,
              approved_at = $3,
              rejected_by_user_id = NULL,
              rejected_at = NULL,
              updated_at = $4,
              version = $5,
              trace_id = $6
            WHERE id = $7
            "#,
        )
        .bind(&next_signal.risk_decision)
        .bind(next_signal.approved_by_user_id.as_deref())
        .bind(next_signal.approved_at)
        .bind(next_signal.updated_at)
        .bind(next_signal.version)
        .bind(trace_id)
        .bind(signal_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!("failed to approve signal {signal_id}: {error}"),
            )
        })?;

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit signal approval transaction: {error}"),
            )
        })?;

        Ok(next_signal)
    }

    async fn reject_signal(
        &self,
        signal_id: &str,
        rejected_by_user_id: &str,
        rejection_reason: &str,
        trace_id: &str,
        expected_version: Option<i64>,
    ) -> Result<SignalView> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin signal rejection transaction: {error}"),
            )
        })?;

        let signal_row = sqlx::query(
            r#"
            SELECT
              s.id,
              s.market_id,
              s.event_id,
              s.action,
              s.side,
              s.market_price,
              s.fair_price,
              s.edge,
              s.confidence,
              s.lifecycle_state,
              s.reason,
              s.risk_decision,
              s.approved_by_user_id,
              s.approved_at,
              s.rejected_by_user_id,
              s.rejected_at,
              s.updated_at,
              s.version,
              COALESCE(
                array_agg(sel.evidence_id ORDER BY sel.evidence_id)
                FILTER (WHERE sel.evidence_id IS NOT NULL),
                '{}'::TEXT[]
              ) AS evidence_ids
            FROM signals s
            LEFT JOIN signal_evidence_links sel ON sel.signal_id = s.id
            WHERE s.id = $1
            GROUP BY
              s.id,
              s.market_id,
              s.event_id,
              s.action,
              s.side,
              s.market_price,
              s.fair_price,
              s.edge,
              s.confidence,
              s.lifecycle_state,
              s.reason,
              s.risk_decision,
              s.approved_by_user_id,
              s.approved_at,
              s.rejected_by_user_id,
              s.rejected_at,
              s.updated_at,
              s.version
            FOR UPDATE
            "#,
        )
        .bind(signal_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to lock signal {signal_id}: {error}"),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(
                "SIGNAL_NOT_FOUND",
                format!("signal was not found: {signal_id}"),
            )
        })?;
        let current_signal = parse_signal_row(&signal_row)?;

        if let Some(expected_version) = expected_version {
            if current_signal.version != expected_version {
                return Err(AppError::conflict(
                    "STATE_VERSION_MISMATCH",
                    "signal version does not match the expected_version",
                ));
            }
        }

        if current_signal.approved_by_user_id.is_some() {
            return Err(AppError::conflict(
                "STATE_SIGNAL_ALREADY_APPROVED",
                "approved signals cannot be rejected for the current version",
            ));
        }

        if current_signal.rejected_by_user_id.is_some() {
            return Err(AppError::conflict(
                "STATE_SIGNAL_ALREADY_REJECTED",
                "signal has already been rejected for the current version",
            ));
        }

        let rejected_at = OffsetDateTime::now_utc();
        let next_signal = SignalView {
            id: current_signal.id.clone(),
            market_id: current_signal.market_id.clone(),
            event_id: current_signal.event_id.clone(),
            action: current_signal.action,
            side: current_signal.side,
            market_price: current_signal.market_price,
            fair_price: current_signal.fair_price,
            edge: current_signal.edge,
            confidence: current_signal.confidence,
            lifecycle_state: current_signal.lifecycle_state,
            reason: current_signal.reason.clone(),
            risk_decision: rejection_reason.to_string(),
            evidence_ids: current_signal.evidence_ids.clone(),
            approved_by_user_id: None,
            approved_at: None,
            rejected_by_user_id: Some(rejected_by_user_id.to_string()),
            rejected_at: Some(rejected_at),
            updated_at: rejected_at,
            version: current_signal.version + 1,
        };

        sqlx::query(
            r#"
            UPDATE signals
            SET
              risk_decision = $1,
              approved_by_user_id = NULL,
              approved_at = NULL,
              rejected_by_user_id = $2,
              rejected_at = $3,
              updated_at = $4,
              version = $5,
              trace_id = $6
            WHERE id = $7
            "#,
        )
        .bind(&next_signal.risk_decision)
        .bind(next_signal.rejected_by_user_id.as_deref())
        .bind(next_signal.rejected_at)
        .bind(next_signal.updated_at)
        .bind(next_signal.version)
        .bind(trace_id)
        .bind(signal_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!("failed to reject signal {signal_id}: {error}"),
            )
        })?;

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit signal rejection transaction: {error}"),
            )
        })?;

        Ok(next_signal)
    }

    async fn submit_execution_request(
        &self,
        command: &SubmitExecutionStoreCommand,
    ) -> Result<ExecutionSubmissionResult> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin execution request transaction: {error}"),
            )
        })?;

        let signal_row = sqlx::query(
            r#"
            SELECT
              s.id,
              s.market_id,
              s.event_id,
              s.action,
              s.side,
              s.market_price,
              s.fair_price,
              s.edge,
              s.confidence,
              s.lifecycle_state,
              s.reason,
              s.risk_decision,
              s.approved_by_user_id,
              s.approved_at,
              s.rejected_by_user_id,
              s.rejected_at,
              s.updated_at,
              s.version,
              COALESCE(
                array_agg(sel.evidence_id ORDER BY sel.evidence_id)
                FILTER (WHERE sel.evidence_id IS NOT NULL),
                '{}'::TEXT[]
              ) AS evidence_ids
            FROM signals s
            LEFT JOIN signal_evidence_links sel ON sel.signal_id = s.id
            WHERE s.id = $1
            GROUP BY
              s.id,
              s.market_id,
              s.event_id,
              s.action,
              s.side,
              s.market_price,
              s.fair_price,
              s.edge,
              s.confidence,
              s.lifecycle_state,
              s.reason,
              s.risk_decision,
              s.approved_by_user_id,
              s.approved_at,
              s.rejected_by_user_id,
              s.rejected_at,
              s.updated_at,
              s.version
            FOR UPDATE
            "#,
        )
        .bind(&command.signal_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to lock signal {}: {error}", command.signal_id),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(
                "SIGNAL_NOT_FOUND",
                format!("signal was not found: {}", command.signal_id),
            )
        })?;
        let signal = parse_signal_row(&signal_row)?;

        if let Some(expected_signal_version) = command.expected_signal_version {
            if signal.version != expected_signal_version {
                return Err(AppError::conflict(
                    "STATE_VERSION_MISMATCH",
                    "signal version does not match the expected_signal_version",
                ));
            }
        }

        validate_signal_for_execution(&signal, command.mode)?;

        let existing_request = sqlx::query(
            r#"
            SELECT id
            FROM execution_requests
            WHERE signal_id = $1 AND signal_version = $2
            LIMIT 1
            "#,
        )
        .bind(&signal.id)
        .bind(signal.version)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!(
                    "failed to check existing execution request for {}: {error}",
                    signal.id
                ),
            )
        })?;

        if existing_request.is_some() {
            return Err(AppError::conflict(
                "STATE_EXECUTION_REQUEST_ALREADY_EXISTS",
                "an execution request already exists for the current signal version",
            ));
        }

        let now = OffsetDateTime::now_utc();
        let order_draft = OrderDraftView {
            id: format!("odr_{}", Uuid::now_v7()),
            signal_id: signal.id.clone(),
            signal_version: signal.version,
            market_id: signal.market_id.clone(),
            connector_name: command.connector_name.clone(),
            side: signal.side,
            limit_price: command.limit_price,
            quantity: command.quantity,
            notional: compute_order_notional(command.limit_price, command.quantity)?,
            status: OrderDraftStatus::Queued,
            created_by_user_id: command.requested_by_user_id.clone(),
            external_order_id: None,
            submitted_at: None,
            failure_code: None,
            failure_message: None,
            created_at: now,
            updated_at: now,
            version: 1,
        };

        sqlx::query(
            r#"
            INSERT INTO order_drafts (
              id,
              signal_id,
              signal_version,
              market_id,
              connector_name,
              side,
              limit_price,
              quantity,
              notional,
              status,
              created_by_user_id,
              external_order_id,
              submitted_at,
              failure_code,
              failure_message,
              created_at,
              updated_at,
              version,
              trace_id
            )
            VALUES (
              $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19
            )
            "#,
        )
        .bind(&order_draft.id)
        .bind(&order_draft.signal_id)
        .bind(order_draft.signal_version)
        .bind(&order_draft.market_id)
        .bind(&order_draft.connector_name)
        .bind(order_draft.side.as_str())
        .bind(order_draft.limit_price.value())
        .bind(order_draft.quantity.value())
        .bind(order_draft.notional.value())
        .bind(order_draft.status.as_str())
        .bind(&order_draft.created_by_user_id)
        .bind(&order_draft.external_order_id)
        .bind(order_draft.submitted_at)
        .bind(&order_draft.failure_code)
        .bind(&order_draft.failure_message)
        .bind(order_draft.created_at)
        .bind(order_draft.updated_at)
        .bind(order_draft.version)
        .bind(&command.trace_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_INSERT_FAILED",
                format!("failed to insert order draft {}: {error}", order_draft.id),
            )
        })?;

        let execution_request = ExecutionRequestView {
            id: format!("exr_{}", Uuid::now_v7()),
            signal_id: signal.id,
            signal_version: signal.version,
            order_draft_id: order_draft.id.clone(),
            connector_name: command.connector_name.clone(),
            mode: command.mode,
            requested_by_user_id: command.requested_by_user_id.clone(),
            status: ExecutionRequestStatus::Queued,
            reason: command.reason.clone(),
            external_order_id: None,
            submitted_at: None,
            failure_code: None,
            failure_message: None,
            created_at: now,
            updated_at: now,
            version: 1,
        };

        sqlx::query(
            r#"
            INSERT INTO execution_requests (
              id,
              signal_id,
              signal_version,
              order_draft_id,
              connector_name,
              mode,
              risk_state_version,
              requested_by_user_id,
              status,
              reason,
              external_order_id,
              submitted_at,
              failure_code,
              failure_message,
              created_at,
              updated_at,
              version,
              trace_id
            )
            VALUES (
              $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18
            )
            "#,
        )
        .bind(&execution_request.id)
        .bind(&execution_request.signal_id)
        .bind(execution_request.signal_version)
        .bind(&execution_request.order_draft_id)
        .bind(&execution_request.connector_name)
        .bind(execution_request.mode.as_str())
        .bind(command.risk_state_version)
        .bind(&execution_request.requested_by_user_id)
        .bind(execution_request.status.as_str())
        .bind(&execution_request.reason)
        .bind(&execution_request.external_order_id)
        .bind(execution_request.submitted_at)
        .bind(&execution_request.failure_code)
        .bind(&execution_request.failure_message)
        .bind(execution_request.created_at)
        .bind(execution_request.updated_at)
        .bind(execution_request.version)
        .bind(&command.trace_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_INSERT_FAILED",
                format!(
                    "failed to insert execution request {}: {error}",
                    execution_request.id
                ),
            )
        })?;

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit execution request transaction: {error}"),
            )
        })?;

        Ok(ExecutionSubmissionResult {
            order_draft,
            execution_request,
        })
    }

    async fn list_dispatch_candidates(
        &self,
        filters: &DispatchExecutionListFilters,
    ) -> Result<Vec<ExecutionDispatchCandidate>> {
        let rows = sqlx::query(
            r#"
            SELECT
              od.id AS order_draft_id,
              od.signal_id AS order_draft_signal_id,
              od.signal_version AS order_draft_signal_version,
              od.market_id AS order_draft_market_id,
              od.connector_name AS order_draft_connector_name,
              od.side AS order_draft_side,
              od.limit_price AS order_draft_limit_price,
              od.quantity AS order_draft_quantity,
              od.notional AS order_draft_notional,
              od.status AS order_draft_status,
              od.created_by_user_id AS order_draft_created_by_user_id,
              od.external_order_id AS order_draft_external_order_id,
              od.submitted_at AS order_draft_submitted_at,
              od.failure_code AS order_draft_failure_code,
              od.failure_message AS order_draft_failure_message,
              od.created_at AS order_draft_created_at,
              od.updated_at AS order_draft_updated_at,
              od.version AS order_draft_version,
              er.id AS execution_request_id,
              er.signal_id AS execution_request_signal_id,
              er.signal_version AS execution_request_signal_version,
              er.order_draft_id AS execution_request_order_draft_id,
              er.connector_name AS execution_request_connector_name,
              er.mode AS execution_request_mode,
              er.requested_by_user_id AS execution_request_requested_by_user_id,
              er.status AS execution_request_status,
              er.reason AS execution_request_reason,
              er.external_order_id AS execution_request_external_order_id,
              er.submitted_at AS execution_request_submitted_at,
              er.failure_code AS execution_request_failure_code,
              er.failure_message AS execution_request_failure_message,
              er.created_at AS execution_request_created_at,
              er.updated_at AS execution_request_updated_at,
              er.version AS execution_request_version
            FROM execution_requests er
            INNER JOIN order_drafts od ON od.id = er.order_draft_id
            WHERE er.status = 'queued'
              AND od.status = 'queued'
              AND ($1::TEXT IS NULL OR er.connector_name = $1)
            ORDER BY er.created_at ASC, er.id ASC
            LIMIT $2
            "#,
        )
        .bind(filters.connector_name.as_deref())
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list dispatch candidates: {error}"),
            )
        })?;

        rows.iter().map(parse_dispatch_candidate_row).collect()
    }

    async fn list_reconciliation_candidates(
        &self,
        filters: &ReconcileExecutionListFilters,
    ) -> Result<Vec<ExecutionReconciliationCandidate>> {
        let rows = sqlx::query(
            r#"
            SELECT
              od.id AS order_draft_id,
              od.signal_id AS order_draft_signal_id,
              od.signal_version AS order_draft_signal_version,
              od.market_id AS order_draft_market_id,
              od.connector_name AS order_draft_connector_name,
              od.side AS order_draft_side,
              od.limit_price AS order_draft_limit_price,
              od.quantity AS order_draft_quantity,
              od.notional AS order_draft_notional,
              od.status AS order_draft_status,
              od.created_by_user_id AS order_draft_created_by_user_id,
              od.external_order_id AS order_draft_external_order_id,
              od.submitted_at AS order_draft_submitted_at,
              od.failure_code AS order_draft_failure_code,
              od.failure_message AS order_draft_failure_message,
              od.created_at AS order_draft_created_at,
              od.updated_at AS order_draft_updated_at,
              od.version AS order_draft_version,
              er.id AS execution_request_id,
              er.signal_id AS execution_request_signal_id,
              er.signal_version AS execution_request_signal_version,
              er.order_draft_id AS execution_request_order_draft_id,
              er.connector_name AS execution_request_connector_name,
              er.mode AS execution_request_mode,
              er.requested_by_user_id AS execution_request_requested_by_user_id,
              er.status AS execution_request_status,
              er.reason AS execution_request_reason,
              er.external_order_id AS execution_request_external_order_id,
              er.submitted_at AS execution_request_submitted_at,
              er.failure_code AS execution_request_failure_code,
              er.failure_message AS execution_request_failure_message,
              er.created_at AS execution_request_created_at,
              er.updated_at AS execution_request_updated_at,
              er.version AS execution_request_version,
              o.id AS order_id,
              o.signal_id AS order_signal_id,
              o.execution_request_id AS order_execution_request_id,
              o.order_draft_id AS order_order_draft_id,
              o.market_id AS order_market_id,
              o.connector_name AS order_connector_name,
              o.account_id AS order_account_id,
              o.external_order_id AS order_external_order_id,
              o.side AS order_side,
              o.limit_price AS order_limit_price,
              o.quantity AS order_quantity,
              o.filled_quantity AS order_filled_quantity,
              o.avg_fill_price AS order_avg_fill_price,
              o.status AS order_status,
              o.submitted_at AS order_submitted_at,
              o.updated_at AS order_updated_at,
              o.version AS order_version
            FROM execution_requests er
            INNER JOIN order_drafts od ON od.id = er.order_draft_id
            LEFT JOIN orders o ON o.execution_request_id = er.id
            WHERE er.status = 'submitted'
              AND od.status = 'submitted'
              AND (
                o.id IS NULL
                OR (
                  o.status IN ('submitted', 'open', 'partially_filled')
                  AND o.filled_quantity < o.quantity
                )
              )
              AND ($1::TEXT IS NULL OR er.connector_name = $1)
            ORDER BY COALESCE(o.updated_at, er.updated_at) ASC, er.id ASC
            LIMIT $2
            "#,
        )
        .bind(filters.connector_name.as_deref())
        .bind(i64::from(filters.limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to list reconciliation candidates: {error}"),
            )
        })?;

        rows.iter()
            .map(parse_reconciliation_candidate_row)
            .collect()
    }

    async fn mark_execution_submitted(
        &self,
        execution_request_id: &str,
        account_id: &str,
        external_order_id: &str,
        trace_id: &str,
    ) -> Result<ExecutionDispatchResult> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin execution dispatch transaction: {error}"),
            )
        })?;

        let request_row = sqlx::query(
            r#"
            SELECT
              id,
              signal_id,
              signal_version,
              order_draft_id,
              connector_name,
              mode,
              requested_by_user_id,
              status,
              reason,
              external_order_id,
              submitted_at,
              failure_code,
              failure_message,
              created_at,
              updated_at,
              version
            FROM execution_requests
            WHERE id = $1
            FOR UPDATE
            "#,
        )
        .bind(execution_request_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to lock execution request {execution_request_id}: {error}"),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(
                "EXECUTION_REQUEST_NOT_FOUND",
                format!("execution request was not found: {execution_request_id}"),
            )
        })?;
        let request = parse_execution_request_row(&request_row)?;

        if request.status != ExecutionRequestStatus::Queued {
            return Err(AppError::conflict(
                "STATE_EXECUTION_REQUEST_NOT_DISPATCHABLE",
                "execution request is no longer queued",
            ));
        }

        let order_draft_row = sqlx::query(
            r#"
            SELECT
              id,
              signal_id,
              signal_version,
              market_id,
              connector_name,
              side,
              limit_price,
              quantity,
              notional,
              status,
              created_by_user_id,
              external_order_id,
              submitted_at,
              failure_code,
              failure_message,
              created_at,
              updated_at,
              version
            FROM order_drafts
            WHERE id = $1
            FOR UPDATE
            "#,
        )
        .bind(&request.order_draft_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!(
                    "failed to lock order draft {}: {error}",
                    request.order_draft_id
                ),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(
                "ORDER_DRAFT_NOT_FOUND",
                format!("order draft was not found: {}", request.order_draft_id),
            )
        })?;
        let order_draft = parse_order_draft_row(&order_draft_row)?;

        if order_draft.status != OrderDraftStatus::Queued {
            return Err(AppError::conflict(
                "STATE_ORDER_DRAFT_NOT_DISPATCHABLE",
                "order draft is no longer queued",
            ));
        }

        let submitted_at = OffsetDateTime::now_utc();
        let next_order_draft = OrderDraftView {
            status: OrderDraftStatus::Submitted,
            external_order_id: Some(external_order_id.to_string()),
            submitted_at: Some(submitted_at),
            failure_code: None,
            failure_message: None,
            updated_at: submitted_at,
            version: order_draft.version + 1,
            ..order_draft
        };
        let next_request = ExecutionRequestView {
            status: ExecutionRequestStatus::Submitted,
            external_order_id: Some(external_order_id.to_string()),
            submitted_at: Some(submitted_at),
            failure_code: None,
            failure_message: None,
            updated_at: submitted_at,
            version: request.version + 1,
            ..request
        };
        let submitted_order = OrderView {
            id: format!("ord_{}", Uuid::now_v7()),
            signal_id: next_request.signal_id.clone(),
            execution_request_id: next_request.id.clone(),
            order_draft_id: next_order_draft.id.clone(),
            market_id: next_order_draft.market_id.clone(),
            connector_name: next_request.connector_name.clone(),
            account_id: account_id.to_string(),
            external_order_id: external_order_id.to_string(),
            side: next_order_draft.side,
            limit_price: next_order_draft.limit_price,
            quantity: next_order_draft.quantity,
            filled_quantity: Quantity::new(Decimal::ZERO)?,
            avg_fill_price: Probability::new(Decimal::ZERO)?,
            status: OrderStatus::Submitted,
            submitted_at,
            updated_at: submitted_at,
            version: 1,
        };

        sqlx::query(
            r#"
            UPDATE order_drafts
            SET
              status = $1,
              external_order_id = $2,
              submitted_at = $3,
              failure_code = NULL,
              failure_message = NULL,
              updated_at = $4,
              version = $5,
              trace_id = $6
            WHERE id = $7
            "#,
        )
        .bind(next_order_draft.status.as_str())
        .bind(next_order_draft.external_order_id.as_deref())
        .bind(next_order_draft.submitted_at)
        .bind(next_order_draft.updated_at)
        .bind(next_order_draft.version)
        .bind(trace_id)
        .bind(&next_order_draft.id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!(
                    "failed to update order draft {}: {error}",
                    next_order_draft.id
                ),
            )
        })?;

        sqlx::query(
            r#"
            UPDATE execution_requests
            SET
              status = $1,
              external_order_id = $2,
              submitted_at = $3,
              failure_code = NULL,
              failure_message = NULL,
              updated_at = $4,
              version = $5,
              trace_id = $6
            WHERE id = $7
            "#,
        )
        .bind(next_request.status.as_str())
        .bind(next_request.external_order_id.as_deref())
        .bind(next_request.submitted_at)
        .bind(next_request.updated_at)
        .bind(next_request.version)
        .bind(trace_id)
        .bind(&next_request.id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!(
                    "failed to update execution request {}: {error}",
                    next_request.id
                ),
            )
        })?;

        sqlx::query(
            r#"
            INSERT INTO orders (
              id,
              signal_id,
              execution_request_id,
              order_draft_id,
              market_id,
              connector_name,
              account_id,
              external_order_id,
              side,
              limit_price,
              quantity,
              filled_quantity,
              avg_fill_price,
              status,
              submitted_at,
              updated_at,
              trace_id,
              version
            )
            VALUES (
              $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18
            )
            "#,
        )
        .bind(&submitted_order.id)
        .bind(&submitted_order.signal_id)
        .bind(&submitted_order.execution_request_id)
        .bind(&submitted_order.order_draft_id)
        .bind(&submitted_order.market_id)
        .bind(&submitted_order.connector_name)
        .bind(&submitted_order.account_id)
        .bind(&submitted_order.external_order_id)
        .bind(submitted_order.side.as_str())
        .bind(submitted_order.limit_price.value())
        .bind(submitted_order.quantity.value())
        .bind(submitted_order.filled_quantity.value())
        .bind(submitted_order.avg_fill_price.value())
        .bind(submitted_order.status.as_str())
        .bind(submitted_order.submitted_at)
        .bind(submitted_order.updated_at)
        .bind(trace_id)
        .bind(submitted_order.version)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_INSERT_FAILED",
                format!(
                    "failed to insert submitted order {}: {error}",
                    submitted_order.id
                ),
            )
        })?;

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit execution dispatch transaction: {error}"),
            )
        })?;

        Ok(ExecutionDispatchResult {
            order_draft: next_order_draft,
            execution_request: next_request,
        })
    }

    async fn mark_order_open(&self, order_id: &str, trace_id: &str) -> Result<OrderView> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin order status polling transaction: {error}"),
            )
        })?;

        let order_row = sqlx::query(
            r#"
            SELECT
              id,
              signal_id,
              execution_request_id,
              order_draft_id,
              market_id,
              connector_name,
              account_id,
              external_order_id,
              side,
              limit_price,
              quantity,
              filled_quantity,
              avg_fill_price,
              status,
              submitted_at,
              updated_at,
              version
            FROM orders
            WHERE id = $1
            FOR UPDATE
            "#,
        )
        .bind(order_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to lock order {order_id}: {error}"),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(
                "ORDER_NOT_FOUND",
                format!("order was not found: {order_id}"),
            )
        })?;
        let order = parse_order_row(&order_row)?;
        let current_status = order.status;

        let next_order = match current_status {
            OrderStatus::Submitted => OrderView {
                status: OrderStatus::Open,
                updated_at: OffsetDateTime::now_utc(),
                version: order.version + 1,
                ..order
            },
            OrderStatus::Open => order,
            _ => {
                return Err(AppError::conflict(
                    "STATE_ORDER_NOT_POLLABLE",
                    "only submitted/open orders can be polled as open",
                ));
            }
        };

        if next_order.status != current_status {
            sqlx::query(
                r#"
                UPDATE orders
                SET
                  status = $1,
                  updated_at = $2,
                  trace_id = $3,
                  version = $4
                WHERE id = $5
                "#,
            )
            .bind(next_order.status.as_str())
            .bind(next_order.updated_at)
            .bind(trace_id)
            .bind(next_order.version)
            .bind(&next_order.id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_UPDATE_FAILED",
                    format!("failed to update order {} as open: {error}", next_order.id),
                )
            })?;
        }

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit order status polling transaction: {error}"),
            )
        })?;

        Ok(next_order)
    }

    async fn mark_order_canceled(&self, order_id: &str, trace_id: &str) -> Result<OrderView> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin order cancel transaction: {error}"),
            )
        })?;

        let order_row = sqlx::query(
            r#"
            SELECT
              id,
              signal_id,
              execution_request_id,
              order_draft_id,
              market_id,
              connector_name,
              account_id,
              external_order_id,
              side,
              limit_price,
              quantity,
              filled_quantity,
              avg_fill_price,
              status,
              submitted_at,
              updated_at,
              version
            FROM orders
            WHERE id = $1
            FOR UPDATE
            "#,
        )
        .bind(order_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to lock order {order_id}: {error}"),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(
                "ORDER_NOT_FOUND",
                format!("order was not found: {order_id}"),
            )
        })?;
        let order = parse_order_row(&order_row)?;
        let current_status = order.status;

        let next_order = match current_status {
            OrderStatus::Submitted | OrderStatus::Open | OrderStatus::PartiallyFilled => {
                OrderView {
                    status: OrderStatus::Canceled,
                    updated_at: OffsetDateTime::now_utc(),
                    version: order.version + 1,
                    ..order
                }
            }
            OrderStatus::Canceled => order,
            _ => {
                return Err(AppError::conflict(
                    "STATE_ORDER_NOT_CANCELABLE",
                    "only submitted/open/partially_filled orders can be canceled",
                ));
            }
        };

        if next_order.status != current_status {
            sqlx::query(
                r#"
                UPDATE orders
                SET
                  status = $1,
                  updated_at = $2,
                  trace_id = $3,
                  version = $4
                WHERE id = $5
                "#,
            )
            .bind(next_order.status.as_str())
            .bind(next_order.updated_at)
            .bind(trace_id)
            .bind(next_order.version)
            .bind(&next_order.id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_UPDATE_FAILED",
                    format!("failed to cancel order {}: {error}", next_order.id),
                )
            })?;

            let request_row = sqlx::query(
                r#"
                SELECT
                  id,
                  signal_id,
                  signal_version,
                  order_draft_id,
                  connector_name,
                  mode,
                  requested_by_user_id,
                  status,
                  reason,
                  external_order_id,
                  submitted_at,
                  failure_code,
                  failure_message,
                  created_at,
                  updated_at,
                  version
                FROM execution_requests
                WHERE id = $1
                FOR UPDATE
                "#,
            )
            .bind(&next_order.execution_request_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_QUERY_FAILED",
                    format!(
                        "failed to lock execution request {} for order cancel: {error}",
                        next_order.execution_request_id
                    ),
                )
            })?
            .ok_or_else(|| {
                AppError::not_found(
                    "EXECUTION_REQUEST_NOT_FOUND",
                    format!(
                        "execution request was not found: {}",
                        next_order.execution_request_id
                    ),
                )
            })?;
            let request = parse_execution_request_row(&request_row)?;

            if request.status == ExecutionRequestStatus::Submitted {
                sqlx::query(
                    r#"
                    UPDATE execution_requests
                    SET
                      status = $1,
                      updated_at = $2,
                      trace_id = $3,
                      version = $4
                    WHERE id = $5
                    "#,
                )
                .bind(ExecutionRequestStatus::Canceled.as_str())
                .bind(next_order.updated_at)
                .bind(trace_id)
                .bind(request.version + 1)
                .bind(&request.id)
                .execute(&mut *transaction)
                .await
                .map_err(|error| {
                    db_error(
                        "POSTGRES_UPDATE_FAILED",
                        format!("failed to cancel execution request {}: {error}", request.id),
                    )
                })?;
            }
        }

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit order cancel transaction: {error}"),
            )
        })?;

        Ok(next_order)
    }

    async fn mark_execution_failed(
        &self,
        execution_request_id: &str,
        failure_code: &str,
        failure_message: &str,
        trace_id: &str,
    ) -> Result<ExecutionDispatchResult> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin execution failure transaction: {error}"),
            )
        })?;

        let request_row = sqlx::query(
            r#"
            SELECT
              id,
              signal_id,
              signal_version,
              order_draft_id,
              connector_name,
              mode,
              requested_by_user_id,
              status,
              reason,
              external_order_id,
              submitted_at,
              failure_code,
              failure_message,
              created_at,
              updated_at,
              version
            FROM execution_requests
            WHERE id = $1
            FOR UPDATE
            "#,
        )
        .bind(execution_request_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to lock execution request {execution_request_id}: {error}"),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(
                "EXECUTION_REQUEST_NOT_FOUND",
                format!("execution request was not found: {execution_request_id}"),
            )
        })?;
        let request = parse_execution_request_row(&request_row)?;

        if request.status != ExecutionRequestStatus::Queued {
            return Err(AppError::conflict(
                "STATE_EXECUTION_REQUEST_NOT_DISPATCHABLE",
                "execution request is no longer queued",
            ));
        }

        let order_draft_row = sqlx::query(
            r#"
            SELECT
              id,
              signal_id,
              signal_version,
              market_id,
              connector_name,
              side,
              limit_price,
              quantity,
              notional,
              status,
              created_by_user_id,
              external_order_id,
              submitted_at,
              failure_code,
              failure_message,
              created_at,
              updated_at,
              version
            FROM order_drafts
            WHERE id = $1
            FOR UPDATE
            "#,
        )
        .bind(&request.order_draft_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!(
                    "failed to lock order draft {}: {error}",
                    request.order_draft_id
                ),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(
                "ORDER_DRAFT_NOT_FOUND",
                format!("order draft was not found: {}", request.order_draft_id),
            )
        })?;
        let order_draft = parse_order_draft_row(&order_draft_row)?;

        if order_draft.status != OrderDraftStatus::Queued {
            return Err(AppError::conflict(
                "STATE_ORDER_DRAFT_NOT_DISPATCHABLE",
                "order draft is no longer queued",
            ));
        }

        let failed_at = OffsetDateTime::now_utc();
        let next_order_draft = OrderDraftView {
            status: OrderDraftStatus::Rejected,
            external_order_id: None,
            submitted_at: None,
            failure_code: Some(failure_code.to_string()),
            failure_message: Some(failure_message.to_string()),
            updated_at: failed_at,
            version: order_draft.version + 1,
            ..order_draft
        };
        let next_request = ExecutionRequestView {
            status: ExecutionRequestStatus::Failed,
            external_order_id: None,
            submitted_at: None,
            failure_code: Some(failure_code.to_string()),
            failure_message: Some(failure_message.to_string()),
            updated_at: failed_at,
            version: request.version + 1,
            ..request
        };

        sqlx::query(
            r#"
            UPDATE order_drafts
            SET
              status = $1,
              external_order_id = NULL,
              submitted_at = NULL,
              failure_code = $2,
              failure_message = $3,
              updated_at = $4,
              version = $5,
              trace_id = $6
            WHERE id = $7
            "#,
        )
        .bind(next_order_draft.status.as_str())
        .bind(next_order_draft.failure_code.as_deref())
        .bind(next_order_draft.failure_message.as_deref())
        .bind(next_order_draft.updated_at)
        .bind(next_order_draft.version)
        .bind(trace_id)
        .bind(&next_order_draft.id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!(
                    "failed to update order draft {}: {error}",
                    next_order_draft.id
                ),
            )
        })?;

        sqlx::query(
            r#"
            UPDATE execution_requests
            SET
              status = $1,
              external_order_id = NULL,
              submitted_at = NULL,
              failure_code = $2,
              failure_message = $3,
              updated_at = $4,
              version = $5,
              trace_id = $6
            WHERE id = $7
            "#,
        )
        .bind(next_request.status.as_str())
        .bind(next_request.failure_code.as_deref())
        .bind(next_request.failure_message.as_deref())
        .bind(next_request.updated_at)
        .bind(next_request.version)
        .bind(trace_id)
        .bind(&next_request.id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_UPDATE_FAILED",
                format!(
                    "failed to update execution request {}: {error}",
                    next_request.id
                ),
            )
        })?;

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit execution failure transaction: {error}"),
            )
        })?;

        Ok(ExecutionDispatchResult {
            order_draft: next_order_draft,
            execution_request: next_request,
        })
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
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin execution reconciliation transaction: {error}"),
            )
        })?;

        let request_row = sqlx::query(
            r#"
            SELECT
              id,
              signal_id,
              signal_version,
              order_draft_id,
              connector_name,
              mode,
              requested_by_user_id,
              status,
              reason,
              external_order_id,
              submitted_at,
              failure_code,
              failure_message,
              created_at,
              updated_at,
              version
            FROM execution_requests
            WHERE id = $1
            FOR UPDATE
            "#,
        )
        .bind(execution_request_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!(
                    "failed to lock reconciliation execution request {execution_request_id}: {error}"
                ),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(
                "EXECUTION_REQUEST_NOT_FOUND",
                format!("execution request was not found: {execution_request_id}"),
            )
        })?;
        let request = parse_execution_request_row(&request_row)?;

        if request.status != ExecutionRequestStatus::Submitted {
            return Err(AppError::conflict(
                "STATE_EXECUTION_REQUEST_NOT_RECONCILABLE",
                "execution request is not in submitted state",
            ));
        }

        let existing_order_row = sqlx::query(
            r#"
            SELECT
              id,
              signal_id,
              execution_request_id,
              order_draft_id,
              market_id,
              connector_name,
              account_id,
              external_order_id,
              side,
              limit_price,
              quantity,
              filled_quantity,
              avg_fill_price,
              status,
              submitted_at,
              updated_at,
              version
            FROM orders
            WHERE execution_request_id = $1
            FOR UPDATE
            "#,
        )
        .bind(&request.id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!(
                    "failed to check existing reconciled order for execution request {}: {error}",
                    request.id
                ),
            )
        })?;
        let existing_order = existing_order_row
            .as_ref()
            .map(parse_order_row)
            .transpose()?;

        let existing_trade_row = sqlx::query(
            r#"
            SELECT id
            FROM trades
            WHERE connector_name = $1
              AND external_trade_id = $2
            LIMIT 1
            "#,
        )
        .bind(&request.connector_name)
        .bind(external_trade_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!(
                    "failed to check existing trade {} for connector {}: {error}",
                    external_trade_id, request.connector_name
                ),
            )
        })?;
        if existing_trade_row.is_some() {
            return Err(AppError::conflict(
                "STATE_TRADE_ALREADY_RECORDED",
                "external trade id has already been recorded",
            ));
        }

        let order_draft_row = sqlx::query(
            r#"
            SELECT
              id,
              signal_id,
              signal_version,
              market_id,
              connector_name,
              side,
              limit_price,
              quantity,
              notional,
              status,
              created_by_user_id,
              external_order_id,
              submitted_at,
              failure_code,
              failure_message,
              created_at,
              updated_at,
              version
            FROM order_drafts
            WHERE id = $1
            FOR UPDATE
            "#,
        )
        .bind(&request.order_draft_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!(
                    "failed to lock order draft {} for reconciliation: {error}",
                    request.order_draft_id
                ),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(
                "ORDER_DRAFT_NOT_FOUND",
                format!("order draft was not found: {}", request.order_draft_id),
            )
        })?;
        let order_draft = parse_order_draft_row(&order_draft_row)?;

        if order_draft.status != OrderDraftStatus::Submitted {
            return Err(AppError::conflict(
                "STATE_ORDER_DRAFT_NOT_RECONCILABLE",
                "order draft is not in submitted state",
            ));
        }

        let external_order_id = request
            .external_order_id
            .clone()
            .or_else(|| order_draft.external_order_id.clone())
            .ok_or_else(|| {
                AppError::conflict(
                    "STATE_EXTERNAL_ORDER_ID_MISSING",
                    "submitted execution request is missing external_order_id",
                )
            })?;

        let signal_row = sqlx::query(
            r#"
            SELECT id, lifecycle_state, updated_at, version
            FROM signals
            WHERE id = $1
            FOR UPDATE
            "#,
        )
        .bind(&request.signal_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!("failed to lock signal {}: {error}", request.signal_id),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(
                "SIGNAL_NOT_FOUND",
                format!("signal was not found: {}", request.signal_id),
            )
        })?;
        let current_signal_state = SignalLifecycleState::from_str(&decode_column::<String>(
            &signal_row,
            "lifecycle_state",
        )?)?;
        let current_signal_version: i64 = decode_column(&signal_row, "version")?;

        let position_row = sqlx::query(
            r#"
            SELECT
              id,
              market_id,
              connector_name,
              account_id,
              side,
              net_quantity,
              avg_cost,
              mark_price,
              unrealized_pnl,
              realized_pnl,
              updated_at,
              version
            FROM positions
            WHERE connector_name = $1
              AND account_id = $2
              AND market_id = $3
              AND side = $4
            FOR UPDATE
            "#,
        )
        .bind(&request.connector_name)
        .bind(account_id)
        .bind(&order_draft.market_id)
        .bind(order_draft.side.as_str())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_QUERY_FAILED",
                format!(
                    "failed to lock position for connector={} account={} market={} side={}: {error}",
                    request.connector_name,
                    account_id,
                    order_draft.market_id,
                    order_draft.side.as_str(),
                ),
            )
        })?;

        let now = OffsetDateTime::now_utc();
        let submitted_at = request
            .submitted_at
            .or(order_draft.submitted_at)
            .unwrap_or(now);
        let order = if let Some(current) = existing_order {
            if !matches!(
                current.status,
                OrderStatus::Submitted | OrderStatus::Open | OrderStatus::PartiallyFilled
            ) {
                return Err(AppError::conflict(
                    "STATE_ORDER_NOT_RECONCILABLE",
                    "existing order is not in a reconcilable state",
                ));
            }

            let next_filled_quantity_value =
                current.filled_quantity.value() + filled_quantity.value();
            if next_filled_quantity_value > current.quantity.value() {
                return Err(AppError::conflict(
                    "STATE_FILL_QUANTITY_EXCEEDS_ORDER",
                    "filled quantity exceeds order quantity",
                ));
            }

            let next_filled_quantity = Quantity::new(next_filled_quantity_value)?;
            let next_avg_fill_price = weighted_fill_price(
                current.avg_fill_price,
                current.filled_quantity,
                fill_price,
                filled_quantity,
            )?;
            let next_status = if next_filled_quantity.value() == current.quantity.value() {
                OrderStatus::Filled
            } else {
                OrderStatus::PartiallyFilled
            };

            OrderView {
                filled_quantity: next_filled_quantity,
                avg_fill_price: next_avg_fill_price,
                status: next_status,
                updated_at: now,
                version: current.version + 1,
                ..current
            }
        } else {
            if filled_quantity.value() > order_draft.quantity.value() {
                return Err(AppError::conflict(
                    "STATE_FILL_QUANTITY_EXCEEDS_ORDER",
                    "filled quantity exceeds queued order quantity",
                ));
            }

            let next_status = if filled_quantity.value() == order_draft.quantity.value() {
                OrderStatus::Filled
            } else {
                OrderStatus::PartiallyFilled
            };

            OrderView {
                id: format!("ord_{}", Uuid::now_v7()),
                signal_id: request.signal_id.clone(),
                execution_request_id: request.id.clone(),
                order_draft_id: order_draft.id.clone(),
                market_id: order_draft.market_id.clone(),
                connector_name: request.connector_name.clone(),
                account_id: account_id.to_string(),
                external_order_id,
                side: order_draft.side,
                limit_price: order_draft.limit_price,
                quantity: order_draft.quantity,
                filled_quantity,
                avg_fill_price: fill_price,
                status: next_status,
                submitted_at,
                updated_at: now,
                version: 1,
            }
        };
        let trade = TradeView {
            id: format!("trd_{}", Uuid::now_v7()),
            order_id: order.id.clone(),
            signal_id: order.signal_id.clone(),
            market_id: order.market_id.clone(),
            connector_name: order.connector_name.clone(),
            external_trade_id: external_trade_id.to_string(),
            side: order.side,
            price: fill_price,
            quantity: filled_quantity,
            fee,
            executed_at: now,
        };
        let position_key = in_memory_position_key(
            &order.connector_name,
            account_id,
            &order.market_id,
            order.side,
        );
        let position = if let Some(row) = position_row.as_ref() {
            build_next_position(
                parse_position_row(row)?,
                filled_quantity,
                fill_price,
                trace_id,
            )?
        } else {
            PositionView {
                id: position_key,
                market_id: order.market_id.clone(),
                connector_name: order.connector_name.clone(),
                account_id: account_id.to_string(),
                side: order.side,
                net_quantity: filled_quantity,
                avg_cost: fill_price,
                mark_price: fill_price,
                unrealized_pnl: SignedUsdAmount::new(Decimal::ZERO)?,
                realized_pnl: SignedUsdAmount::new(Decimal::ZERO)?,
                updated_at: now,
                version: 1,
            }
        };

        if existing_order_row.is_some() {
            sqlx::query(
                r#"
                UPDATE orders
                SET
                  filled_quantity = $1,
                  avg_fill_price = $2,
                  status = $3,
                  updated_at = $4,
                  trace_id = $5,
                  version = $6
                WHERE id = $7
                "#,
            )
            .bind(order.filled_quantity.value())
            .bind(order.avg_fill_price.value())
            .bind(order.status.as_str())
            .bind(order.updated_at)
            .bind(trace_id)
            .bind(order.version)
            .bind(&order.id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_UPDATE_FAILED",
                    format!("failed to update reconciled order {}: {error}", order.id),
                )
            })?;
        } else {
            sqlx::query(
                r#"
                INSERT INTO orders (
                  id,
                  signal_id,
                  execution_request_id,
                  order_draft_id,
                  market_id,
                  connector_name,
                  account_id,
                  external_order_id,
                  side,
                  limit_price,
                  quantity,
                  filled_quantity,
                  avg_fill_price,
                  status,
                  submitted_at,
                  updated_at,
                  trace_id,
                  version
                )
                VALUES (
                  $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18
                )
                "#,
            )
            .bind(&order.id)
            .bind(&order.signal_id)
            .bind(&order.execution_request_id)
            .bind(&order.order_draft_id)
            .bind(&order.market_id)
            .bind(&order.connector_name)
            .bind(&order.account_id)
            .bind(&order.external_order_id)
            .bind(order.side.as_str())
            .bind(order.limit_price.value())
            .bind(order.quantity.value())
            .bind(order.filled_quantity.value())
            .bind(order.avg_fill_price.value())
            .bind(order.status.as_str())
            .bind(order.submitted_at)
            .bind(order.updated_at)
            .bind(trace_id)
            .bind(order.version)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_INSERT_FAILED",
                    format!("failed to insert reconciled order {}: {error}", order.id),
                )
            })?;
        }

        sqlx::query(
            r#"
            INSERT INTO trades (
              id,
              order_id,
              signal_id,
              market_id,
              connector_name,
              external_trade_id,
              side,
              price,
              quantity,
              fee,
              executed_at,
              trace_id,
              created_at
            )
            VALUES (
              $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13
            )
            "#,
        )
        .bind(&trade.id)
        .bind(&trade.order_id)
        .bind(&trade.signal_id)
        .bind(&trade.market_id)
        .bind(&trade.connector_name)
        .bind(&trade.external_trade_id)
        .bind(trade.side.as_str())
        .bind(trade.price.value())
        .bind(trade.quantity.value())
        .bind(trade.fee.value())
        .bind(trade.executed_at)
        .bind(trace_id)
        .bind(now)
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            db_error(
                "POSTGRES_INSERT_FAILED",
                format!("failed to insert reconciled trade {}: {error}", trade.id),
            )
        })?;

        if position_row.is_some() {
            sqlx::query(
                r#"
                UPDATE positions
                SET
                  net_quantity = $1,
                  avg_cost = $2,
                  mark_price = $3,
                  unrealized_pnl = $4,
                  realized_pnl = $5,
                  updated_at = $6,
                  trace_id = $7,
                  version = $8
                WHERE id = $9
                "#,
            )
            .bind(position.net_quantity.value())
            .bind(position.avg_cost.value())
            .bind(position.mark_price.value())
            .bind(position.unrealized_pnl.value())
            .bind(position.realized_pnl.value())
            .bind(position.updated_at)
            .bind(trace_id)
            .bind(position.version)
            .bind(&position.id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_UPDATE_FAILED",
                    format!("failed to update position {}: {error}", position.id),
                )
            })?;
        } else {
            sqlx::query(
                r#"
                INSERT INTO positions (
                  id,
                  market_id,
                  connector_name,
                  account_id,
                  side,
                  net_quantity,
                  avg_cost,
                  mark_price,
                  unrealized_pnl,
                  realized_pnl,
                  updated_at,
                  trace_id,
                  version
                )
                VALUES (
                  $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13
                )
                "#,
            )
            .bind(&position.id)
            .bind(&position.market_id)
            .bind(&position.connector_name)
            .bind(&position.account_id)
            .bind(position.side.as_str())
            .bind(position.net_quantity.value())
            .bind(position.avg_cost.value())
            .bind(position.mark_price.value())
            .bind(position.unrealized_pnl.value())
            .bind(position.realized_pnl.value())
            .bind(position.updated_at)
            .bind(trace_id)
            .bind(position.version)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_INSERT_FAILED",
                    format!("failed to insert position {}: {error}", position.id),
                )
            })?;
        }

        if current_signal_state != SignalLifecycleState::Executed {
            sqlx::query(
                r#"
                UPDATE signals
                SET
                  lifecycle_state = $1,
                  updated_at = $2,
                  version = $3
                WHERE id = $4
                "#,
            )
            .bind(SignalLifecycleState::Executed.as_str())
            .bind(now)
            .bind(current_signal_version + 1)
            .bind(&request.signal_id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_UPDATE_FAILED",
                    format!(
                        "failed to update signal {} after reconciliation: {error}",
                        request.signal_id
                    ),
                )
            })?;

            sqlx::query(
                r#"
                INSERT INTO signal_transitions (
                  id,
                  signal_id,
                  from_state,
                  to_state,
                  trigger_type,
                  trigger_payload,
                  created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
            )
            .bind(format!("sgt_{}", Uuid::now_v7()))
            .bind(&request.signal_id)
            .bind(current_signal_state.as_str())
            .bind(SignalLifecycleState::Executed.as_str())
            .bind("execution_fill_reconciled")
            .bind(Json(json!({
                "execution_request_id": request.id,
                "external_trade_id": external_trade_id,
                "trace_id": trace_id,
            })))
            .bind(now)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_INSERT_FAILED",
                    format!(
                        "failed to insert signal transition for signal {}: {error}",
                        request.signal_id
                    ),
                )
            })?;
        }

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit execution reconciliation transaction: {error}"),
            )
        })?;

        Ok(ExecutionFillResult {
            order,
            trade,
            position,
        })
    }

    async fn ingest_fixture_bundle(
        &self,
        bundle: &FixtureBundle,
        trace_id: &str,
    ) -> Result<FixtureIngestionReport> {
        let mut transaction = self.pool.begin().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_BEGIN_FAILED",
                format!("failed to begin market/event ingestion transaction: {error}"),
            )
        })?;

        for market in &bundle.markets {
            sqlx::query(
                r#"
                INSERT INTO markets (
                  id,
                  question,
                  category,
                  status,
                  best_bid,
                  best_ask,
                  mid_price,
                  volume_24h,
                  ambiguity_level,
                  tradability_status,
                  polymarket_condition_id,
                  polymarket_yes_asset_id,
                  polymarket_no_asset_id,
                  updated_at,
                  version,
                  trace_id
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
                ON CONFLICT (id) DO UPDATE
                SET
                  question = EXCLUDED.question,
                  category = EXCLUDED.category,
                  status = EXCLUDED.status,
                  best_bid = EXCLUDED.best_bid,
                  best_ask = EXCLUDED.best_ask,
                  mid_price = EXCLUDED.mid_price,
                  volume_24h = EXCLUDED.volume_24h,
                  ambiguity_level = EXCLUDED.ambiguity_level,
                  tradability_status = EXCLUDED.tradability_status,
                  polymarket_condition_id = EXCLUDED.polymarket_condition_id,
                  polymarket_yes_asset_id = EXCLUDED.polymarket_yes_asset_id,
                  polymarket_no_asset_id = EXCLUDED.polymarket_no_asset_id,
                  updated_at = EXCLUDED.updated_at,
                  version = EXCLUDED.version,
                  trace_id = EXCLUDED.trace_id
                "#,
            )
            .bind(&market.id)
            .bind(&market.question)
            .bind(&market.category)
            .bind(market.status.as_str())
            .bind(market.best_bid.value())
            .bind(market.best_ask.value())
            .bind(market.mid_price.value())
            .bind(market.volume_24h.value())
            .bind(market.ambiguity_level.as_str())
            .bind(market.tradability_status.as_str())
            .bind(&market.polymarket_condition_id)
            .bind(&market.polymarket_yes_asset_id)
            .bind(&market.polymarket_no_asset_id)
            .bind(market.updated_at)
            .bind(market.version)
            .bind(trace_id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_INSERT_FAILED",
                    format!("failed to upsert market {}: {error}", market.id),
                )
            })?;

            sqlx::query(
                r#"
                INSERT INTO market_resolution_rules (
                  market_id,
                  resolution_source,
                  edge_case_notes,
                  updated_at,
                  version,
                  trace_id
                )
                VALUES ($1, $2, $3, $4, $5, $6)
                ON CONFLICT (market_id) DO UPDATE
                SET
                  resolution_source = EXCLUDED.resolution_source,
                  edge_case_notes = EXCLUDED.edge_case_notes,
                  updated_at = EXCLUDED.updated_at,
                  version = EXCLUDED.version,
                  trace_id = EXCLUDED.trace_id
                "#,
            )
            .bind(&market.id)
            .bind(&market.resolution_source)
            .bind(&market.edge_case_notes)
            .bind(market.updated_at)
            .bind(market.version)
            .bind(trace_id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_INSERT_FAILED",
                    format!(
                        "failed to upsert market resolution rules for {}: {error}",
                        market.id
                    ),
                )
            })?;
        }

        for event in &bundle.events {
            let raw_payload = serde_json::to_value(event).map_err(|error| {
                AppError::internal(
                    "RAW_EVENT_SERIALIZE_FAILED",
                    format!("failed to serialize event fixture {}: {error}", event.id),
                )
            })?;

            let raw_event_id = if let Some(raw_event_id) = event.raw_event_id.as_deref() {
                raw_event_id.to_string()
            } else {
                let raw_event_id = format!("raw_{}", event.id);
                let hash = format!("fixture_hash_{}", event.id);

                sqlx::query(
                    r#"
                    INSERT INTO raw_events (
                      id,
                      source,
                      hash,
                      raw_payload,
                      ingested_at,
                      trace_id
                    )
                    VALUES ($1, $2, $3, $4, $5, $6)
                    ON CONFLICT (id) DO UPDATE
                    SET
                      source = EXCLUDED.source,
                      hash = EXCLUDED.hash,
                      raw_payload = EXCLUDED.raw_payload,
                      ingested_at = EXCLUDED.ingested_at,
                      trace_id = EXCLUDED.trace_id
                    "#,
                )
                .bind(&raw_event_id)
                .bind(&event.source)
                .bind(hash)
                .bind(Json(raw_payload))
                .bind(event.updated_at)
                .bind(trace_id)
                .execute(&mut *transaction)
                .await
                .map_err(|error| {
                    db_error(
                        "POSTGRES_INSERT_FAILED",
                        format!("failed to upsert raw event {}: {error}", event.id),
                    )
                })?;

                raw_event_id
            };

            sqlx::query(
                r#"
                INSERT INTO events (
                  id,
                  raw_event_id,
                  source,
                  summary,
                  relevance_score,
                  confidence,
                  status,
                  reason_trace,
                  created_at,
                  updated_at,
                  version,
                  trace_id
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                ON CONFLICT (id) DO UPDATE
                SET
                  raw_event_id = EXCLUDED.raw_event_id,
                  source = EXCLUDED.source,
                  summary = EXCLUDED.summary,
                  relevance_score = EXCLUDED.relevance_score,
                  confidence = EXCLUDED.confidence,
                  status = EXCLUDED.status,
                  reason_trace = EXCLUDED.reason_trace,
                  created_at = EXCLUDED.created_at,
                  updated_at = EXCLUDED.updated_at,
                  version = EXCLUDED.version,
                  trace_id = EXCLUDED.trace_id
                "#,
            )
            .bind(&event.id)
            .bind(&raw_event_id)
            .bind(&event.source)
            .bind(&event.summary)
            .bind(event.relevance_score.value())
            .bind(event.confidence.value())
            .bind(event.status.as_str())
            .bind(&event.reason_trace)
            .bind(event.created_at)
            .bind(event.updated_at)
            .bind(event.version)
            .bind(trace_id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_INSERT_FAILED",
                    format!("failed to upsert event {}: {error}", event.id),
                )
            })?;

            sqlx::query("DELETE FROM event_market_links WHERE event_id = $1")
                .bind(&event.id)
                .execute(&mut *transaction)
                .await
                .map_err(|error| {
                    db_error(
                        "POSTGRES_DELETE_FAILED",
                        format!(
                            "failed to reset event market links for {}: {error}",
                            event.id
                        ),
                    )
                })?;

            for market_id in &event.related_market_ids {
                sqlx::query(
                    r#"
                    INSERT INTO event_market_links (event_id, market_id, created_at)
                    VALUES ($1, $2, $3)
                    ON CONFLICT (event_id, market_id) DO NOTHING
                    "#,
                )
                .bind(&event.id)
                .bind(market_id)
                .bind(event.created_at)
                .execute(&mut *transaction)
                .await
                .map_err(|error| {
                    db_error(
                        "POSTGRES_INSERT_FAILED",
                        format!(
                            "failed to insert event-market link {} -> {}: {error}",
                            event.id, market_id
                        ),
                    )
                })?;
            }
        }

        for evidence in &bundle.evidences {
            sqlx::query(
                r#"
                INSERT INTO evidences (
                  id,
                  market_id,
                  event_id,
                  direction,
                  strength,
                  source_reliability,
                  novelty,
                  resolution_relevance,
                  status,
                  expires_at,
                  created_at,
                  updated_at,
                  version,
                  trace_id
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                ON CONFLICT (id) DO UPDATE
                SET
                  market_id = EXCLUDED.market_id,
                  event_id = EXCLUDED.event_id,
                  direction = EXCLUDED.direction,
                  strength = EXCLUDED.strength,
                  source_reliability = EXCLUDED.source_reliability,
                  novelty = EXCLUDED.novelty,
                  resolution_relevance = EXCLUDED.resolution_relevance,
                  status = EXCLUDED.status,
                  expires_at = EXCLUDED.expires_at,
                  created_at = EXCLUDED.created_at,
                  updated_at = EXCLUDED.updated_at,
                  version = EXCLUDED.version,
                  trace_id = EXCLUDED.trace_id
                "#,
            )
            .bind(&evidence.id)
            .bind(&evidence.market_id)
            .bind(&evidence.event_id)
            .bind(evidence.direction.as_str())
            .bind(evidence.strength.value())
            .bind(evidence.source_reliability.value())
            .bind(evidence.novelty.value())
            .bind(evidence.resolution_relevance.value())
            .bind(evidence.status.as_str())
            .bind(evidence.expires_at)
            .bind(evidence.created_at)
            .bind(evidence.updated_at)
            .bind(evidence.version)
            .bind(trace_id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_INSERT_FAILED",
                    format!("failed to upsert evidence {}: {error}", evidence.id),
                )
            })?;
        }

        for signal in &bundle.signals {
            sqlx::query(
                r#"
                INSERT INTO signals (
                  id,
                  market_id,
                  event_id,
                  action,
                  side,
                  market_price,
                  fair_price,
                  edge,
                  confidence,
                  lifecycle_state,
                  reason,
                  risk_decision,
                  approved_by_user_id,
                  approved_at,
                  rejected_by_user_id,
                  rejected_at,
                  updated_at,
                  version,
                  trace_id
                )
                VALUES (
                  $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17,
                  $18, $19
                )
                ON CONFLICT (id) DO UPDATE
                SET
                  market_id = EXCLUDED.market_id,
                  event_id = EXCLUDED.event_id,
                  action = EXCLUDED.action,
                  side = EXCLUDED.side,
                  market_price = EXCLUDED.market_price,
                  fair_price = EXCLUDED.fair_price,
                  edge = EXCLUDED.edge,
                  confidence = EXCLUDED.confidence,
                  lifecycle_state = EXCLUDED.lifecycle_state,
                  reason = EXCLUDED.reason,
                  risk_decision = EXCLUDED.risk_decision,
                  approved_by_user_id = EXCLUDED.approved_by_user_id,
                  approved_at = EXCLUDED.approved_at,
                  rejected_by_user_id = EXCLUDED.rejected_by_user_id,
                  rejected_at = EXCLUDED.rejected_at,
                  updated_at = EXCLUDED.updated_at,
                  version = EXCLUDED.version,
                  trace_id = EXCLUDED.trace_id
                "#,
            )
            .bind(&signal.id)
            .bind(&signal.market_id)
            .bind(&signal.event_id)
            .bind(signal.action.as_str())
            .bind(signal.side.as_str())
            .bind(signal.market_price.value())
            .bind(signal.fair_price.value())
            .bind(signal.edge.value())
            .bind(signal.confidence.value())
            .bind(signal.lifecycle_state.as_str())
            .bind(&signal.reason)
            .bind(&signal.risk_decision)
            .bind(&signal.approved_by_user_id)
            .bind(signal.approved_at)
            .bind(&signal.rejected_by_user_id)
            .bind(signal.rejected_at)
            .bind(signal.updated_at)
            .bind(signal.version)
            .bind(trace_id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                db_error(
                    "POSTGRES_INSERT_FAILED",
                    format!("failed to upsert signal {}: {error}", signal.id),
                )
            })?;

            sqlx::query("DELETE FROM signal_evidence_links WHERE signal_id = $1")
                .bind(&signal.id)
                .execute(&mut *transaction)
                .await
                .map_err(|error| {
                    db_error(
                        "POSTGRES_DELETE_FAILED",
                        format!(
                            "failed to reset signal evidence links for {}: {error}",
                            signal.id
                        ),
                    )
                })?;

            for evidence_id in &signal.evidence_ids {
                sqlx::query(
                    r#"
                    INSERT INTO signal_evidence_links (signal_id, evidence_id, created_at)
                    VALUES ($1, $2, $3)
                    ON CONFLICT (signal_id, evidence_id) DO NOTHING
                    "#,
                )
                .bind(&signal.id)
                .bind(evidence_id)
                .bind(signal.updated_at)
                .execute(&mut *transaction)
                .await
                .map_err(|error| {
                    db_error(
                        "POSTGRES_INSERT_FAILED",
                        format!(
                            "failed to insert signal-evidence link {} -> {}: {error}",
                            signal.id, evidence_id
                        ),
                    )
                })?;
            }
        }

        transaction.commit().await.map_err(|error| {
            db_error(
                "POSTGRES_TRANSACTION_COMMIT_FAILED",
                format!("failed to commit market/event ingestion transaction: {error}"),
            )
        })?;

        Ok(FixtureIngestionReport {
            markets_upserted: bundle.markets.len(),
            events_upserted: bundle.events.len(),
            evidences_upserted: bundle.evidences.len(),
            signals_upserted: bundle.signals.len(),
        })
    }
}

async fn fetch_market_by_id(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    market_id: &str,
) -> Result<Option<MarketView>> {
    let row = sqlx::query(
        r#"
        SELECT
          m.id,
          m.question,
          m.category,
          m.status,
          m.best_bid,
          m.best_ask,
          m.mid_price,
          m.volume_24h,
          m.ambiguity_level,
          m.tradability_status,
          r.resolution_source,
          r.edge_case_notes,
          m.polymarket_condition_id,
          m.polymarket_yes_asset_id,
          m.polymarket_no_asset_id,
          m.updated_at,
          m.version
        FROM markets m
        INNER JOIN market_resolution_rules r ON r.market_id = m.id
        WHERE m.id = $1
        "#,
    )
    .bind(market_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to fetch market {market_id}: {error}"),
        )
    })?;

    row.as_ref().map(parse_market_row).transpose()
}

async fn fetch_evidences_for_signal(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    market_id: &str,
    event_id: &str,
) -> Result<Vec<EvidenceView>> {
    let rows = sqlx::query(
        r#"
        SELECT
          id,
          market_id,
          event_id,
          direction,
          strength,
          source_reliability,
          novelty,
          resolution_relevance,
          status,
          expires_at,
          created_at,
          updated_at,
          version
        FROM evidences
        WHERE market_id = $1
          AND event_id = $2
        ORDER BY created_at DESC, id ASC
        "#,
    )
    .bind(market_id)
    .bind(event_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to fetch evidences for {market_id}/{event_id}: {error}"),
        )
    })?;

    rows.iter().map(parse_evidence_row).collect()
}

async fn fetch_source_health_adjustment_for_event(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event_id: &str,
) -> Result<Option<SourceHealthAdjustment>> {
    let row = sqlx::query(
        r#"
        SELECT
          e.source,
          nsh.health_score
        FROM events e
        LEFT JOIN news_source_health nsh ON nsh.source = e.source
        WHERE e.id = $1
        "#,
    )
    .bind(event_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|error| {
        db_error(
            "POSTGRES_QUERY_FAILED",
            format!("failed to fetch source health for event {event_id}: {error}"),
        )
    })?;

    let Some(row) = row else {
        return Ok(None);
    };
    let health_score: Option<Decimal> = decode_column(&row, "health_score")?;
    let Some(health_score) = health_score else {
        return Ok(None);
    };

    Ok(Some(SourceHealthAdjustment {
        source: decode_column(&row, "source")?,
        health_score: Probability::new(health_score).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode event source health_score: {error}"),
            )
        })?,
    }))
}

fn parse_market_row(row: &sqlx::postgres::PgRow) -> Result<MarketView> {
    let status_raw: String = decode_column(row, "status")?;
    let ambiguity_level_raw: String = decode_column(row, "ambiguity_level")?;
    let tradability_status_raw: String = decode_column(row, "tradability_status")?;
    let best_bid: Decimal = decode_column(row, "best_bid")?;
    let best_ask: Decimal = decode_column(row, "best_ask")?;
    let mid_price: Decimal = decode_column(row, "mid_price")?;
    let volume_24h: Decimal = decode_column(row, "volume_24h")?;

    Ok(MarketView {
        id: decode_column(row, "id")?,
        question: decode_column(row, "question")?,
        category: decode_column(row, "category")?,
        status: MarketStatus::from_str(&status_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode market status: {error}"),
            )
        })?,
        best_bid: Probability::new(best_bid).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode market best_bid: {error}"),
            )
        })?,
        best_ask: Probability::new(best_ask).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode market best_ask: {error}"),
            )
        })?,
        mid_price: Probability::new(mid_price).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode market mid_price: {error}"),
            )
        })?,
        volume_24h: UsdAmount::new(volume_24h).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode market volume_24h: {error}"),
            )
        })?,
        ambiguity_level: AmbiguityLevel::from_str(&ambiguity_level_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode ambiguity level: {error}"),
            )
        })?,
        tradability_status: TradabilityStatus::from_str(&tradability_status_raw).map_err(
            |error| {
                db_error(
                    "POSTGRES_DECODE_FAILED",
                    format!("failed to decode tradability status: {error}"),
                )
            },
        )?,
        resolution_source: decode_column(row, "resolution_source")?,
        edge_case_notes: decode_column(row, "edge_case_notes")?,
        polymarket_condition_id: decode_column(row, "polymarket_condition_id")?,
        polymarket_yes_asset_id: decode_column(row, "polymarket_yes_asset_id")?,
        polymarket_no_asset_id: decode_column(row, "polymarket_no_asset_id")?,
        updated_at: decode_column(row, "updated_at")?,
        version: decode_column(row, "version")?,
    })
}

fn parse_news_source_health_row(row: &sqlx::postgres::PgRow) -> Result<NewsSourceHealthView> {
    let reliability: Decimal = decode_column(row, "reliability")?;
    let health_score: Decimal = decode_column(row, "health_score")?;
    let consecutive_failures: i64 = decode_column(row, "consecutive_failures")?;
    let items_fetched: i64 = decode_column(row, "items_fetched")?;
    let items_inserted: i64 = decode_column(row, "items_inserted")?;
    let items_deduped: i64 = decode_column(row, "items_deduped")?;

    Ok(NewsSourceHealthView {
        source: decode_column(row, "source")?,
        source_type: decode_column(row, "source_type")?,
        enabled: decode_column(row, "enabled")?,
        reliability: Probability::new(reliability).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode news source reliability: {error}"),
            )
        })?,
        last_success_at: decode_column(row, "last_success_at")?,
        last_error_at: decode_column(row, "last_error_at")?,
        consecutive_failures: i64_to_u64("consecutive_failures", consecutive_failures)?,
        items_fetched: i64_to_u64("items_fetched", items_fetched)?,
        items_inserted: i64_to_u64("items_inserted", items_inserted)?,
        items_deduped: i64_to_u64("items_deduped", items_deduped)?,
        health_score: Probability::new(health_score).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode news source health_score: {error}"),
            )
        })?,
        last_error: decode_column(row, "last_error")?,
        updated_at: decode_column(row, "updated_at")?,
    })
}

fn parse_news_raw_event_row(row: &sqlx::postgres::PgRow) -> Result<NewsRawEventView> {
    let raw_payload: Json<Value> = decode_column(row, "raw_payload")?;

    Ok(NewsRawEventView {
        id: decode_column(row, "id")?,
        source: decode_column(row, "source")?,
        source_type: decode_column(row, "source_type")?,
        external_id: decode_column(row, "external_id")?,
        title: decode_column(row, "title")?,
        url: decode_column(row, "url")?,
        author: decode_column(row, "author")?,
        published_at: decode_column(row, "published_at")?,
        event_time: decode_column(row, "event_time")?,
        hash: decode_column(row, "hash")?,
        raw_payload: raw_payload.0,
        ingested_at: decode_column(row, "ingested_at")?,
        trace_id: decode_column(row, "trace_id")?,
    })
}

fn parse_arbitrage_scan_row(row: &sqlx::postgres::PgRow) -> Result<ArbitrageScanView> {
    let metadata_json: Json<Value> = decode_column(row, "metadata_json")?;
    let market_count: i32 = decode_column(row, "market_count")?;
    let snapshot_count: i32 = decode_column(row, "snapshot_count")?;
    let opportunity_count: i32 = decode_column(row, "opportunity_count")?;

    Ok(ArbitrageScanView {
        id: decode_column(row, "id")?,
        started_at: decode_column(row, "started_at")?,
        finished_at: decode_column(row, "finished_at")?,
        market_count: nonnegative_i32_to_u32("market_count", market_count)?,
        snapshot_count: nonnegative_i32_to_u32("snapshot_count", snapshot_count)?,
        opportunity_count: nonnegative_i32_to_u32("opportunity_count", opportunity_count)?,
        scanner_version: decode_column(row, "scanner_version")?,
        metadata: metadata_json.0,
        trace_id: decode_column(row, "trace_id")?,
    })
}

fn parse_arbitrage_opportunity_row(
    row: &sqlx::postgres::PgRow,
) -> Result<ArbitrageOpportunityView> {
    let opportunity_type_raw: String = decode_column(row, "opportunity_type")?;
    let status_raw: String = decode_column(row, "status")?;
    let gross_edge: Decimal = decode_column(row, "gross_edge")?;
    let price_sum: Decimal = decode_column(row, "price_sum")?;
    let capacity: Decimal = decode_column(row, "capacity")?;
    let yes_price: Decimal = decode_column(row, "yes_price")?;
    let no_price: Decimal = decode_column(row, "no_price")?;
    let yes_size: Decimal = decode_column(row, "yes_size")?;
    let no_size: Decimal = decode_column(row, "no_size")?;
    let reason_codes_json: Json<Vec<String>> = decode_column(row, "reason_codes_json")?;
    let analysis_payload_json: Json<Value> = decode_column(row, "analysis_payload_json")?;

    Ok(ArbitrageOpportunityView {
        id: decode_column(row, "id")?,
        scan_id: decode_column(row, "scan_id")?,
        market_id: decode_column(row, "market_id")?,
        opportunity_type: ArbitrageOpportunityType::from_str(&opportunity_type_raw).map_err(
            |error| {
                db_error(
                    "POSTGRES_DECODE_FAILED",
                    format!("failed to decode arbitrage opportunity_type: {error}"),
                )
            },
        )?,
        status: ArbitrageOpportunityStatus::from_str(&status_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode arbitrage opportunity status: {error}"),
            )
        })?,
        gross_edge: Edge::new(gross_edge).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode arbitrage gross_edge: {error}"),
            )
        })?,
        price_sum,
        capacity: Quantity::new(capacity).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode arbitrage capacity: {error}"),
            )
        })?,
        yes_price: Probability::new(yes_price).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode arbitrage yes_price: {error}"),
            )
        })?,
        no_price: Probability::new(no_price).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode arbitrage no_price: {error}"),
            )
        })?,
        yes_size: Quantity::new(yes_size).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode arbitrage yes_size: {error}"),
            )
        })?,
        no_size: Quantity::new(no_size).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode arbitrage no_size: {error}"),
            )
        })?,
        observed_at: decode_column(row, "observed_at")?,
        reason_codes: reason_codes_json.0,
        analysis_payload: analysis_payload_json.0,
        trace_id: decode_column(row, "trace_id")?,
        validation: parse_arbitrage_validation_from_row(row)?,
    })
}

fn parse_arbitrage_validation_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<Option<ArbitrageOpportunityValidationView>> {
    let Some(id) = decode_column::<Option<String>>(row, "validation_id")? else {
        return Ok(None);
    };
    let status_raw = required_optional_column::<String>(row, "validation_status", &id)?;
    let gross_edge = required_optional_column::<Decimal>(row, "validation_gross_edge", &id)?;
    let net_edge = required_optional_column::<Decimal>(row, "validation_net_edge", &id)?;
    let fee_estimate = required_optional_column::<Decimal>(row, "validation_fee_estimate", &id)?;
    let slippage_buffer =
        required_optional_column::<Decimal>(row, "validation_slippage_buffer", &id)?;
    let validated_capacity =
        required_optional_column::<Decimal>(row, "validation_validated_capacity", &id)?;
    let book_age_ms = required_optional_column::<i64>(row, "validation_book_age_ms", &id)?;
    let reason_codes_json =
        required_optional_column::<Json<Vec<String>>>(row, "validation_reason_codes_json", &id)?;
    let validation_payload_json =
        required_optional_column::<Json<Value>>(row, "validation_payload_json", &id)?;

    Ok(Some(ArbitrageOpportunityValidationView {
        id,
        opportunity_id: decode_column(row, "id")?,
        status: ArbitrageValidationStatus::from_str(&status_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode arbitrage validation status: {error}"),
            )
        })?,
        gross_edge: Edge::new(gross_edge).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode arbitrage validation gross_edge: {error}"),
            )
        })?,
        net_edge: Edge::new(net_edge).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode arbitrage validation net_edge: {error}"),
            )
        })?,
        fee_estimate: Edge::new(fee_estimate).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode arbitrage validation fee_estimate: {error}"),
            )
        })?,
        slippage_buffer: Edge::new(slippage_buffer).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode arbitrage validation slippage_buffer: {error}"),
            )
        })?,
        validated_capacity: Quantity::new(validated_capacity).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode arbitrage validation capacity: {error}"),
            )
        })?,
        book_age_ms: i64_to_u64("validation_book_age_ms", book_age_ms)?,
        reason_codes: reason_codes_json.0,
        validation_payload: validation_payload_json.0,
        validated_at: required_optional_column(row, "validation_validated_at", "validation")?,
        trace_id: required_optional_column(row, "validation_trace_id", "validation")?,
    }))
}

fn parse_arbitrage_analysis_run_row(
    row: &sqlx::postgres::PgRow,
) -> Result<ArbitrageAnalysisRunView> {
    let lookback_hours: i32 = decode_column(row, "lookback_hours")?;
    let opportunity_count: i32 = decode_column(row, "opportunity_count")?;
    let market_count: i32 = decode_column(row, "market_count")?;
    let summary_payload_json: Json<Value> = decode_column(row, "summary_payload_json")?;

    Ok(ArbitrageAnalysisRunView {
        id: decode_column(row, "id")?,
        generated_at: decode_column(row, "generated_at")?,
        lookback_hours: nonnegative_i32_to_u32("lookback_hours", lookback_hours)?
            .min(u32::from(u16::MAX)) as u16,
        opportunity_count: nonnegative_i32_to_u32("opportunity_count", opportunity_count)?,
        market_count: nonnegative_i32_to_u32("market_count", market_count)?,
        summary_payload: summary_payload_json.0,
        trace_id: decode_column(row, "trace_id")?,
    })
}

fn parse_arbitrage_event_row(row: &sqlx::postgres::PgRow) -> Result<ArbitrageEventView> {
    let sequence: i64 = decode_column(row, "sequence")?;
    let event_type_raw: String = decode_column(row, "event_type")?;
    let payload_json: Json<Value> = decode_column(row, "payload_json")?;

    Ok(ArbitrageEventView {
        sequence: i64_to_u64("sequence", sequence)?,
        id: decode_column(row, "id")?,
        event_type: ArbitrageEventType::from_str(&event_type_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode arbitrage event type: {error}"),
            )
        })?,
        resource_type: decode_column(row, "resource_type")?,
        resource_id: decode_column(row, "resource_id")?,
        payload: payload_json.0,
        occurred_at: decode_column(row, "occurred_at")?,
        trace_id: decode_column(row, "trace_id")?,
    })
}

fn parse_event_row(row: &sqlx::postgres::PgRow) -> Result<EventView> {
    let status_raw: String = decode_column(row, "status")?;
    let relevance_score: Decimal = decode_column(row, "relevance_score")?;
    let confidence: Decimal = decode_column(row, "confidence")?;

    Ok(EventView {
        id: decode_column(row, "id")?,
        source: decode_column(row, "source")?,
        summary: decode_column(row, "summary")?,
        relevance_score: Probability::new(relevance_score).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode event relevance_score: {error}"),
            )
        })?,
        confidence: Probability::new(confidence).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode event confidence: {error}"),
            )
        })?,
        status: EventStatus::from_str(&status_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode event status: {error}"),
            )
        })?,
        related_market_ids: decode_column(row, "related_market_ids")?,
        reason_trace: decode_column(row, "reason_trace")?,
        created_at: decode_column(row, "created_at")?,
        updated_at: decode_column(row, "updated_at")?,
        version: decode_column(row, "version")?,
    })
}

fn parse_evidence_row(row: &sqlx::postgres::PgRow) -> Result<EvidenceView> {
    let direction_raw: String = decode_column(row, "direction")?;
    let status_raw: String = decode_column(row, "status")?;
    let strength: Decimal = decode_column(row, "strength")?;
    let source_reliability: Decimal = decode_column(row, "source_reliability")?;
    let novelty: Decimal = decode_column(row, "novelty")?;
    let resolution_relevance: Decimal = decode_column(row, "resolution_relevance")?;

    Ok(EvidenceView {
        id: decode_column(row, "id")?,
        market_id: decode_column(row, "market_id")?,
        event_id: decode_column(row, "event_id")?,
        direction: EvidenceDirection::from_str(&direction_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode evidence direction: {error}"),
            )
        })?,
        strength: Probability::new(strength).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode evidence strength: {error}"),
            )
        })?,
        source_reliability: Probability::new(source_reliability).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode evidence source_reliability: {error}"),
            )
        })?,
        novelty: Probability::new(novelty).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode evidence novelty: {error}"),
            )
        })?,
        resolution_relevance: Probability::new(resolution_relevance).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode evidence resolution_relevance: {error}"),
            )
        })?,
        status: EvidenceStatus::from_str(&status_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode evidence status: {error}"),
            )
        })?,
        expires_at: decode_column(row, "expires_at")?,
        created_at: decode_column(row, "created_at")?,
        updated_at: decode_column(row, "updated_at")?,
        version: decode_column(row, "version")?,
    })
}

fn parse_signal_row(row: &sqlx::postgres::PgRow) -> Result<SignalView> {
    let action_raw: String = decode_column(row, "action")?;
    let side_raw: String = decode_column(row, "side")?;
    let lifecycle_state_raw: String = decode_column(row, "lifecycle_state")?;
    let market_price: Decimal = decode_column(row, "market_price")?;
    let fair_price: Decimal = decode_column(row, "fair_price")?;
    let edge: Decimal = decode_column(row, "edge")?;
    let confidence: Decimal = decode_column(row, "confidence")?;

    Ok(SignalView {
        id: decode_column(row, "id")?,
        market_id: decode_column(row, "market_id")?,
        event_id: decode_column(row, "event_id")?,
        action: SignalAction::from_str(&action_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode signal action: {error}"),
            )
        })?,
        side: SignalSide::from_str(&side_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode signal side: {error}"),
            )
        })?,
        market_price: Probability::new(market_price).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode signal market_price: {error}"),
            )
        })?,
        fair_price: Probability::new(fair_price).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode signal fair_price: {error}"),
            )
        })?,
        edge: Edge::new(edge).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode signal edge: {error}"),
            )
        })?,
        confidence: Probability::new(confidence).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode signal confidence: {error}"),
            )
        })?,
        lifecycle_state: SignalLifecycleState::from_str(&lifecycle_state_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode signal lifecycle_state: {error}"),
            )
        })?,
        reason: decode_column(row, "reason")?,
        risk_decision: decode_column(row, "risk_decision")?,
        evidence_ids: decode_column(row, "evidence_ids")?,
        approved_by_user_id: decode_column(row, "approved_by_user_id")?,
        approved_at: decode_column(row, "approved_at")?,
        rejected_by_user_id: decode_column(row, "rejected_by_user_id")?,
        rejected_at: decode_column(row, "rejected_at")?,
        updated_at: decode_column(row, "updated_at")?,
        version: decode_column(row, "version")?,
    })
}

fn parse_probability_estimate_row(row: &sqlx::postgres::PgRow) -> Result<ProbabilityEstimateView> {
    let prior_price: Decimal = decode_column(row, "prior_price")?;
    let posterior_price: Decimal = decode_column(row, "posterior_price")?;
    let fair_price: Decimal = decode_column(row, "fair_price")?;
    let market_price: Decimal = decode_column(row, "market_price")?;
    let edge: Decimal = decode_column(row, "edge")?;
    let confidence: Decimal = decode_column(row, "confidence")?;
    let time_horizon_raw: String = decode_column(row, "time_horizon")?;
    let reason_codes_json: Json<Vec<String>> = decode_column(row, "reason_codes_json")?;
    let evidence_count: i32 = decode_column(row, "evidence_count")?;

    Ok(ProbabilityEstimateView {
        id: decode_column(row, "id")?,
        market_id: decode_column(row, "market_id")?,
        event_id: decode_column(row, "event_id")?,
        signal_id: decode_column(row, "signal_id")?,
        prior_price: Probability::new(prior_price).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode prior_price: {error}"),
            )
        })?,
        posterior_price: Probability::new(posterior_price).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode posterior_price: {error}"),
            )
        })?,
        fair_price: Probability::new(fair_price).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode fair_price: {error}"),
            )
        })?,
        market_price: Probability::new(market_price).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode market_price: {error}"),
            )
        })?,
        edge: Edge::new(edge).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode estimate edge: {error}"),
            )
        })?,
        confidence: Probability::new(confidence).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode estimate confidence: {error}"),
            )
        })?,
        time_horizon: TimeHorizon::from_str(&time_horizon_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode time_horizon: {error}"),
            )
        })?,
        model_version: decode_column(row, "model_version")?,
        reason_codes: reason_codes_json.0,
        evidence_count: evidence_count.max(0) as u32,
        created_at: decode_column(row, "created_at")?,
    })
}

fn parse_signal_transition_row(row: &sqlx::postgres::PgRow) -> Result<SignalTransitionView> {
    let from_state_raw: String = decode_column(row, "from_state")?;
    let to_state_raw: String = decode_column(row, "to_state")?;
    let trigger_payload_json: Json<Value> = decode_column(row, "trigger_payload_json")?;

    Ok(SignalTransitionView {
        id: decode_column(row, "id")?,
        signal_id: decode_column(row, "signal_id")?,
        from_state: SignalLifecycleState::from_str(&from_state_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode from_state: {error}"),
            )
        })?,
        to_state: SignalLifecycleState::from_str(&to_state_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode to_state: {error}"),
            )
        })?,
        trigger_type: decode_column(row, "trigger_type")?,
        trigger_payload: trigger_payload_json.0,
        created_at: decode_column(row, "created_at")?,
    })
}

fn parse_order_draft_row(row: &sqlx::postgres::PgRow) -> Result<OrderDraftView> {
    let side_raw: String = decode_column(row, "side")?;
    let limit_price: Decimal = decode_column(row, "limit_price")?;
    let quantity: Decimal = decode_column(row, "quantity")?;
    let notional: Decimal = decode_column(row, "notional")?;
    let status_raw: String = decode_column(row, "status")?;

    Ok(OrderDraftView {
        id: decode_column(row, "id")?,
        signal_id: decode_column(row, "signal_id")?,
        signal_version: decode_column(row, "signal_version")?,
        market_id: decode_column(row, "market_id")?,
        connector_name: decode_column(row, "connector_name")?,
        side: SignalSide::from_str(&side_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode order draft side: {error}"),
            )
        })?,
        limit_price: Probability::new(limit_price).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode order draft limit_price: {error}"),
            )
        })?,
        quantity: Quantity::new(quantity).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode order draft quantity: {error}"),
            )
        })?,
        notional: UsdAmount::new(notional).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode order draft notional: {error}"),
            )
        })?,
        status: OrderDraftStatus::from_str(&status_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode order draft status: {error}"),
            )
        })?,
        created_by_user_id: decode_column(row, "created_by_user_id")?,
        external_order_id: decode_column(row, "external_order_id")?,
        submitted_at: decode_column(row, "submitted_at")?,
        failure_code: decode_column(row, "failure_code")?,
        failure_message: decode_column(row, "failure_message")?,
        created_at: decode_column(row, "created_at")?,
        updated_at: decode_column(row, "updated_at")?,
        version: decode_column(row, "version")?,
    })
}

fn parse_execution_request_row(row: &sqlx::postgres::PgRow) -> Result<ExecutionRequestView> {
    let mode_raw: String = decode_column(row, "mode")?;
    let status_raw: String = decode_column(row, "status")?;

    Ok(ExecutionRequestView {
        id: decode_column(row, "id")?,
        signal_id: decode_column(row, "signal_id")?,
        signal_version: decode_column(row, "signal_version")?,
        order_draft_id: decode_column(row, "order_draft_id")?,
        connector_name: decode_column(row, "connector_name")?,
        mode: polyedge_domain::SystemMode::from_str(&mode_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode execution request mode: {error}"),
            )
        })?,
        requested_by_user_id: decode_column(row, "requested_by_user_id")?,
        status: ExecutionRequestStatus::from_str(&status_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode execution request status: {error}"),
            )
        })?,
        reason: decode_column(row, "reason")?,
        external_order_id: decode_column(row, "external_order_id")?,
        submitted_at: decode_column(row, "submitted_at")?,
        failure_code: decode_column(row, "failure_code")?,
        failure_message: decode_column(row, "failure_message")?,
        created_at: decode_column(row, "created_at")?,
        updated_at: decode_column(row, "updated_at")?,
        version: decode_column(row, "version")?,
    })
}

fn parse_dispatch_candidate_row(row: &sqlx::postgres::PgRow) -> Result<ExecutionDispatchCandidate> {
    let order_draft_side_raw: String = decode_column(row, "order_draft_side")?;
    let order_draft_limit_price: Decimal = decode_column(row, "order_draft_limit_price")?;
    let order_draft_quantity: Decimal = decode_column(row, "order_draft_quantity")?;
    let order_draft_notional: Decimal = decode_column(row, "order_draft_notional")?;
    let order_draft_status_raw: String = decode_column(row, "order_draft_status")?;
    let execution_request_mode_raw: String = decode_column(row, "execution_request_mode")?;
    let execution_request_status_raw: String = decode_column(row, "execution_request_status")?;

    Ok(ExecutionDispatchCandidate {
        order_draft: OrderDraftView {
            id: decode_column(row, "order_draft_id")?,
            signal_id: decode_column(row, "order_draft_signal_id")?,
            signal_version: decode_column(row, "order_draft_signal_version")?,
            market_id: decode_column(row, "order_draft_market_id")?,
            connector_name: decode_column(row, "order_draft_connector_name")?,
            side: SignalSide::from_str(&order_draft_side_raw).map_err(|error| {
                db_error(
                    "POSTGRES_DECODE_FAILED",
                    format!("failed to decode dispatch order draft side: {error}"),
                )
            })?,
            limit_price: Probability::new(order_draft_limit_price).map_err(|error| {
                db_error(
                    "POSTGRES_DECODE_FAILED",
                    format!("failed to decode dispatch order draft limit_price: {error}"),
                )
            })?,
            quantity: Quantity::new(order_draft_quantity).map_err(|error| {
                db_error(
                    "POSTGRES_DECODE_FAILED",
                    format!("failed to decode dispatch order draft quantity: {error}"),
                )
            })?,
            notional: UsdAmount::new(order_draft_notional).map_err(|error| {
                db_error(
                    "POSTGRES_DECODE_FAILED",
                    format!("failed to decode dispatch order draft notional: {error}"),
                )
            })?,
            status: OrderDraftStatus::from_str(&order_draft_status_raw).map_err(|error| {
                db_error(
                    "POSTGRES_DECODE_FAILED",
                    format!("failed to decode dispatch order draft status: {error}"),
                )
            })?,
            created_by_user_id: decode_column(row, "order_draft_created_by_user_id")?,
            external_order_id: decode_column(row, "order_draft_external_order_id")?,
            submitted_at: decode_column(row, "order_draft_submitted_at")?,
            failure_code: decode_column(row, "order_draft_failure_code")?,
            failure_message: decode_column(row, "order_draft_failure_message")?,
            created_at: decode_column(row, "order_draft_created_at")?,
            updated_at: decode_column(row, "order_draft_updated_at")?,
            version: decode_column(row, "order_draft_version")?,
        },
        execution_request: ExecutionRequestView {
            id: decode_column(row, "execution_request_id")?,
            signal_id: decode_column(row, "execution_request_signal_id")?,
            signal_version: decode_column(row, "execution_request_signal_version")?,
            order_draft_id: decode_column(row, "execution_request_order_draft_id")?,
            connector_name: decode_column(row, "execution_request_connector_name")?,
            mode: polyedge_domain::SystemMode::from_str(&execution_request_mode_raw).map_err(
                |error| {
                    db_error(
                        "POSTGRES_DECODE_FAILED",
                        format!("failed to decode dispatch execution request mode: {error}"),
                    )
                },
            )?,
            requested_by_user_id: decode_column(row, "execution_request_requested_by_user_id")?,
            status: ExecutionRequestStatus::from_str(&execution_request_status_raw).map_err(
                |error| {
                    db_error(
                        "POSTGRES_DECODE_FAILED",
                        format!("failed to decode dispatch execution request status: {error}"),
                    )
                },
            )?,
            reason: decode_column(row, "execution_request_reason")?,
            external_order_id: decode_column(row, "execution_request_external_order_id")?,
            submitted_at: decode_column(row, "execution_request_submitted_at")?,
            failure_code: decode_column(row, "execution_request_failure_code")?,
            failure_message: decode_column(row, "execution_request_failure_message")?,
            created_at: decode_column(row, "execution_request_created_at")?,
            updated_at: decode_column(row, "execution_request_updated_at")?,
            version: decode_column(row, "execution_request_version")?,
        },
    })
}

fn parse_reconciliation_candidate_row(
    row: &sqlx::postgres::PgRow,
) -> Result<ExecutionReconciliationCandidate> {
    let candidate = parse_dispatch_candidate_row(row)?;
    let order = if decode_column::<Option<String>>(row, "order_id")?.is_some() {
        Some(parse_order_row_with_prefix(row, "order_")?)
    } else {
        None
    };
    Ok(ExecutionReconciliationCandidate {
        order_draft: candidate.order_draft,
        execution_request: candidate.execution_request,
        order,
    })
}

fn parse_order_row(row: &sqlx::postgres::PgRow) -> Result<OrderView> {
    parse_order_row_with_prefix(row, "")
}

fn parse_order_row_with_prefix(row: &sqlx::postgres::PgRow, prefix: &str) -> Result<OrderView> {
    let side_raw: String = decode_column(row, &format!("{prefix}side"))?;
    let limit_price: Decimal = decode_column(row, &format!("{prefix}limit_price"))?;
    let quantity: Decimal = decode_column(row, &format!("{prefix}quantity"))?;
    let filled_quantity: Decimal = decode_column(row, &format!("{prefix}filled_quantity"))?;
    let avg_fill_price: Decimal = decode_column(row, &format!("{prefix}avg_fill_price"))?;
    let status_raw: String = decode_column(row, &format!("{prefix}status"))?;

    Ok(OrderView {
        id: decode_column(row, &format!("{prefix}id"))?,
        signal_id: decode_column(row, &format!("{prefix}signal_id"))?,
        execution_request_id: decode_column(row, &format!("{prefix}execution_request_id"))?,
        order_draft_id: decode_column(row, &format!("{prefix}order_draft_id"))?,
        market_id: decode_column(row, &format!("{prefix}market_id"))?,
        connector_name: decode_column(row, &format!("{prefix}connector_name"))?,
        account_id: decode_column(row, &format!("{prefix}account_id"))?,
        external_order_id: decode_column(row, &format!("{prefix}external_order_id"))?,
        side: SignalSide::from_str(&side_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode order side: {error}"),
            )
        })?,
        limit_price: Probability::new(limit_price).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode order limit_price: {error}"),
            )
        })?,
        quantity: Quantity::new(quantity).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode order quantity: {error}"),
            )
        })?,
        filled_quantity: Quantity::new(filled_quantity).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode order filled_quantity: {error}"),
            )
        })?,
        avg_fill_price: Probability::new(avg_fill_price).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode order avg_fill_price: {error}"),
            )
        })?,
        status: OrderStatus::from_str(&status_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode order status: {error}"),
            )
        })?,
        submitted_at: decode_column(row, &format!("{prefix}submitted_at"))?,
        updated_at: decode_column(row, &format!("{prefix}updated_at"))?,
        version: decode_column(row, &format!("{prefix}version"))?,
    })
}

fn parse_trade_row(row: &sqlx::postgres::PgRow) -> Result<TradeView> {
    let side_raw: String = decode_column(row, "side")?;
    let price: Decimal = decode_column(row, "price")?;
    let quantity: Decimal = decode_column(row, "quantity")?;
    let fee: Decimal = decode_column(row, "fee")?;

    Ok(TradeView {
        id: decode_column(row, "id")?,
        order_id: decode_column(row, "order_id")?,
        signal_id: decode_column(row, "signal_id")?,
        market_id: decode_column(row, "market_id")?,
        connector_name: decode_column(row, "connector_name")?,
        external_trade_id: decode_column(row, "external_trade_id")?,
        side: SignalSide::from_str(&side_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode trade side: {error}"),
            )
        })?,
        price: Probability::new(price).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode trade price: {error}"),
            )
        })?,
        quantity: Quantity::new(quantity).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode trade quantity: {error}"),
            )
        })?,
        fee: UsdAmount::new(fee).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode trade fee: {error}"),
            )
        })?,
        executed_at: decode_column(row, "executed_at")?,
    })
}

fn parse_position_row(row: &sqlx::postgres::PgRow) -> Result<PositionView> {
    let side_raw: String = decode_column(row, "side")?;
    let net_quantity: Decimal = decode_column(row, "net_quantity")?;
    let avg_cost: Decimal = decode_column(row, "avg_cost")?;
    let mark_price: Decimal = decode_column(row, "mark_price")?;
    let unrealized_pnl: Decimal = decode_column(row, "unrealized_pnl")?;
    let realized_pnl: Decimal = decode_column(row, "realized_pnl")?;

    Ok(PositionView {
        id: decode_column(row, "id")?,
        market_id: decode_column(row, "market_id")?,
        connector_name: decode_column(row, "connector_name")?,
        account_id: decode_column(row, "account_id")?,
        side: SignalSide::from_str(&side_raw).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode position side: {error}"),
            )
        })?,
        net_quantity: Quantity::new(net_quantity).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode position net_quantity: {error}"),
            )
        })?,
        avg_cost: Probability::new(avg_cost).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode position avg_cost: {error}"),
            )
        })?,
        mark_price: Probability::new(mark_price).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode position mark_price: {error}"),
            )
        })?,
        unrealized_pnl: SignedUsdAmount::new(unrealized_pnl).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode position unrealized_pnl: {error}"),
            )
        })?,
        realized_pnl: SignedUsdAmount::new(realized_pnl).map_err(|error| {
            db_error(
                "POSTGRES_DECODE_FAILED",
                format!("failed to decode position realized_pnl: {error}"),
            )
        })?,
        updated_at: decode_column(row, "updated_at")?,
        version: decode_column(row, "version")?,
    })
}

fn in_memory_position_key(
    connector_name: &str,
    account_id: &str,
    market_id: &str,
    side: SignalSide,
) -> String {
    format!(
        "{connector_name}:{account_id}:{market_id}:{}",
        side.as_str()
    )
}

fn build_next_position(
    current: PositionView,
    filled_quantity: Quantity,
    fill_price: Probability,
    _trace_id: &str,
) -> Result<PositionView> {
    let next_quantity_value = current.net_quantity.value() + filled_quantity.value();
    let next_quantity = Quantity::new(next_quantity_value)?;
    let total_cost = (current.avg_cost.value() * current.net_quantity.value())
        + (fill_price.value() * filled_quantity.value());
    let avg_cost = if next_quantity.value().is_zero() {
        Probability::new(Decimal::ZERO)?
    } else {
        Probability::new(
            (total_cost / next_quantity.value())
                .round_dp_with_strategy(Probability::SCALE, RoundingStrategy::MidpointNearestEven),
        )?
    };
    let mark_price = fill_price;
    let unrealized_pnl = compute_unrealized_pnl(next_quantity, avg_cost, mark_price)?;

    Ok(PositionView {
        avg_cost,
        mark_price,
        net_quantity: next_quantity,
        unrealized_pnl,
        updated_at: OffsetDateTime::now_utc(),
        version: current.version + 1,
        ..current
    })
}

fn weighted_fill_price(
    current_avg_fill_price: Probability,
    current_filled_quantity: Quantity,
    fill_price: Probability,
    fill_quantity: Quantity,
) -> Result<Probability> {
    let next_filled_quantity_value = current_filled_quantity.value() + fill_quantity.value();
    if next_filled_quantity_value <= Decimal::ZERO {
        return Probability::new(Decimal::ZERO);
    }

    let weighted_cost = (current_avg_fill_price.value() * current_filled_quantity.value())
        + (fill_price.value() * fill_quantity.value());
    Probability::new(
        (weighted_cost / next_filled_quantity_value)
            .round_dp_with_strategy(Probability::SCALE, RoundingStrategy::MidpointNearestEven),
    )
}

fn compute_unrealized_pnl(
    quantity: Quantity,
    avg_cost: Probability,
    mark_price: Probability,
) -> Result<SignedUsdAmount> {
    let raw = (mark_price.value() - avg_cost.value()) * quantity.value();
    SignedUsdAmount::new(raw.round_dp_with_strategy(
        SignedUsdAmount::SCALE,
        RoundingStrategy::MidpointNearestEven,
    ))
}

fn validate_signal_for_execution(
    signal: &SignalView,
    mode: polyedge_domain::SystemMode,
) -> Result<()> {
    if signal.rejected_by_user_id.is_some() {
        return Err(AppError::conflict(
            "STATE_SIGNAL_REJECTED_FOR_EXECUTION",
            "rejected signals cannot be submitted for execution",
        ));
    }

    if mode == polyedge_domain::SystemMode::ManualConfirm && signal.approved_by_user_id.is_none() {
        return Err(AppError::conflict(
            "STATE_SIGNAL_NOT_APPROVED_FOR_EXECUTION",
            "manual_confirm execution requires an approved signal version",
        ));
    }

    if !matches!(
        signal.lifecycle_state,
        SignalLifecycleState::New | SignalLifecycleState::Active
    ) {
        return Err(AppError::conflict(
            "STATE_SIGNAL_NOT_EXECUTABLE",
            "only new or active signals can be submitted for execution",
        ));
    }

    Ok(())
}

fn compute_order_notional(limit_price: Probability, quantity: Quantity) -> Result<UsdAmount> {
    let notional = (limit_price.value() * quantity.value())
        .round_dp_with_strategy(UsdAmount::SCALE, RoundingStrategy::MidpointNearestEven);
    UsdAmount::new(notional).map_err(|error| {
        AppError::invalid_input(
            "ORDER_NOTIONAL_INVALID",
            format!("failed to compute order notional: {error}"),
        )
    })
}

fn raw_news_dedup_keys(event: &NewsRawEventInsert) -> Vec<String> {
    let mut keys = vec![
        format!("id:{}", event.id),
        format!("source_hash:{}:{}", event.source, event.hash),
    ];

    if let Some(external_id) = event.external_id.as_deref() {
        keys.push(format!("source_external_id:{}:{external_id}", event.source));
    }

    if let Some(url) = event.url.as_deref() {
        keys.push(format!("source_url:{}:{url}", event.source));
    }

    keys
}

fn raw_news_event_view_from_insert(event: &NewsRawEventInsert) -> NewsRawEventView {
    NewsRawEventView {
        id: event.id.clone(),
        source: event.source.clone(),
        source_type: event.source_type.clone(),
        external_id: event.external_id.clone(),
        title: event.title.clone(),
        url: event.url.clone(),
        author: event.author.clone(),
        published_at: event.published_at,
        event_time: event.event_time,
        hash: event.hash.clone(),
        raw_payload: event.raw_payload.clone(),
        ingested_at: event.ingested_at,
        trace_id: event.trace_id.clone(),
    }
}

fn usize_to_i64(value: usize) -> Result<i64> {
    i64::try_from(value).map_err(|error| {
        AppError::invalid_input(
            "NEWS_COUNT_OUT_OF_RANGE",
            format!("news ingestion count does not fit i64: {error}"),
        )
    })
}

fn usize_to_u64(value: usize) -> Result<u64> {
    u64::try_from(value).map_err(|error| {
        AppError::invalid_input(
            "NEWS_COUNT_OUT_OF_RANGE",
            format!("news ingestion count does not fit u64: {error}"),
        )
    })
}

fn add_news_count(left: u64, right: u64) -> Result<u64> {
    left.checked_add(right).ok_or_else(|| {
        AppError::invalid_input(
            "NEWS_COUNT_OUT_OF_RANGE",
            "news ingestion count exceeds u64 range",
        )
    })
}

fn i64_to_u64(column: &str, value: i64) -> Result<u64> {
    u64::try_from(value).map_err(|error| {
        db_error(
            "POSTGRES_DECODE_FAILED",
            format!("failed to decode column {column} as nonnegative count: {error}"),
        )
    })
}

fn nonnegative_i32_to_u32(column: &str, value: i32) -> Result<u32> {
    u32::try_from(value).map_err(|error| {
        db_error(
            "POSTGRES_DECODE_FAILED",
            format!("failed to decode column {column} as nonnegative count: {error}"),
        )
    })
}

fn latest_validation_for_opportunity(
    validations: &HashMap<String, ArbitrageOpportunityValidationView>,
    opportunity_id: &str,
) -> Option<ArbitrageOpportunityValidationView> {
    validations
        .values()
        .filter(|validation| validation.opportunity_id == opportunity_id)
        .max_by(|left, right| {
            left.validated_at
                .cmp(&right.validated_at)
                .then_with(|| left.id.cmp(&right.id))
        })
        .cloned()
}

fn clamped_error_message(value: &str) -> String {
    value.chars().take(1_000).collect()
}

fn required_optional_column<T>(
    row: &sqlx::postgres::PgRow,
    column: &str,
    context: &str,
) -> Result<T>
where
    T: for<'r> sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres>,
{
    let value: Option<T> = decode_column(row, column)?;
    value.ok_or_else(|| {
        db_error(
            "POSTGRES_DECODE_FAILED",
            format!("missing column {column} while decoding {context}"),
        )
    })
}

fn decode_column<T>(row: &sqlx::postgres::PgRow, column: &str) -> Result<T>
where
    T: for<'r> sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres>,
{
    row.try_get(column).map_err(|error| {
        db_error(
            "POSTGRES_DECODE_FAILED",
            format!("failed to decode column {column}: {error}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{InMemoryMarketEventStore, PostgresMarketEventStore};
    use polyedge_application::{
        ArbitrageEventListFilters, ArbitrageEventType, ArbitrageEventView, ArbitrageStore,
        MarketEventStore, NewsIngestionStore, NewsSourceFailureUpdate, NewsSourceHealthListFilters,
        NewsSourceSuccessUpdate, RecomputeSignalCommand, demo_fixture_bundle,
    };
    use polyedge_domain::{Probability, Result};
    use rust_decimal::Decimal;
    use sqlx::{Executor, postgres::PgPoolOptions};
    use std::error::Error;
    use time::{Duration, OffsetDateTime};
    use uuid::Uuid;

    static TEST_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

    fn quote_pg_ident(value: &str) -> String {
        format!(r#""{}""#, value.replace('"', r#""""#))
    }

    fn arbitrage_event(id: &str, occurred_at: OffsetDateTime) -> ArbitrageEventView {
        ArbitrageEventView {
            sequence: 0,
            id: id.to_string(),
            event_type: ArbitrageEventType::ScanStarted,
            resource_type: "scan".to_string(),
            resource_id: id.to_string(),
            payload: serde_json::json!({ "scan_id": id }),
            occurred_at,
            trace_id: "trc_arbitrage_event_test".to_string(),
        }
    }

    #[tokio::test]
    async fn in_memory_arbitrage_events_prune_old_records_and_keep_sequences() -> Result<()> {
        let store = InMemoryMarketEventStore::new();
        let old_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(10);
        let fresh_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(30);

        let old = store
            .record_arbitrage_event(&arbitrage_event("scan_old", old_at))
            .await?;
        let fresh = store
            .record_arbitrage_event(&arbitrage_event("scan_fresh", fresh_at))
            .await?;

        assert_eq!(old.sequence, 1);
        assert_eq!(fresh.sequence, 2);

        let pruned = store
            .prune_arbitrage_events(OffsetDateTime::UNIX_EPOCH + Duration::seconds(20))
            .await?;
        assert_eq!(pruned, 1);

        let remaining = store
            .list_arbitrage_events(&ArbitrageEventListFilters::new(None, Some(10))?)
            .await?;
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, "scan_fresh");
        assert_eq!(remaining[0].sequence, 2);

        let resumed = store
            .list_arbitrage_events(&ArbitrageEventListFilters::new(Some(1), Some(10))?)
            .await?;
        assert_eq!(resumed.len(), 1);
        assert_eq!(resumed[0].id, "scan_fresh");

        Ok(())
    }

    #[tokio::test]
    async fn in_memory_news_source_health_tracks_counts_failures_and_filters() -> Result<()> {
        let store = InMemoryMarketEventStore::new();
        let reliability = Probability::new(Decimal::new(90, 2))?;
        let first_seen = OffsetDateTime::UNIX_EPOCH + Duration::seconds(1);
        let failed_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(2);
        let official_seen = OffsetDateTime::UNIX_EPOCH + Duration::seconds(3);

        store
            .record_news_source_success(&NewsSourceSuccessUpdate {
                source: "rss_feed".to_string(),
                source_type: "news".to_string(),
                reliability,
                fetched: 3,
                inserted: 2,
                deduped: 1,
                observed_at: first_seen,
                trace_id: "trc_success".to_string(),
            })
            .await?;
        store
            .record_news_source_failure(&NewsSourceFailureUpdate {
                source: "rss_feed".to_string(),
                source_type: "news".to_string(),
                reliability,
                error_message: "upstream timeout".to_string(),
                observed_at: failed_at,
                trace_id: "trc_failure".to_string(),
            })
            .await?;
        store
            .record_news_source_success(&NewsSourceSuccessUpdate {
                source: "sec_feed".to_string(),
                source_type: "official".to_string(),
                reliability,
                fetched: 1,
                inserted: 1,
                deduped: 0,
                observed_at: official_seen,
                trace_id: "trc_official".to_string(),
            })
            .await?;

        let all_sources = store
            .list_news_source_health(&NewsSourceHealthListFilters::new(None, Some(10))?)
            .await?;
        assert_eq!(all_sources.len(), 2);
        assert_eq!(all_sources[0].source, "sec_feed");

        let news_sources = store
            .list_news_source_health(&NewsSourceHealthListFilters::new(
                Some(" news ".to_string()),
                Some(10),
            )?)
            .await?;
        assert_eq!(news_sources.len(), 1);

        let rss_feed = &news_sources[0];
        assert_eq!(rss_feed.source, "rss_feed");
        assert_eq!(rss_feed.items_fetched, 3);
        assert_eq!(rss_feed.items_inserted, 2);
        assert_eq!(rss_feed.items_deduped, 1);
        assert_eq!(rss_feed.consecutive_failures, 1);
        assert_eq!(rss_feed.last_success_at, Some(first_seen));
        assert_eq!(rss_feed.last_error_at, Some(failed_at));
        assert_eq!(rss_feed.last_error.as_deref(), Some("upstream timeout"));
        assert_eq!(
            rss_feed.health_score,
            Probability::new(Decimal::new(70, 2))?
        );

        Ok(())
    }

    #[tokio::test]
    async fn in_memory_recompute_discounts_degraded_event_source_health() -> Result<()> {
        let store = InMemoryMarketEventStore::new();
        let bundle = demo_fixture_bundle();
        store
            .ingest_fixture_bundle(&bundle, "trc_seed_recompute_health")
            .await?;
        store
            .record_news_source_failure(&NewsSourceFailureUpdate {
                source: "reuters".to_string(),
                source_type: "news".to_string(),
                reliability: Probability::new(Decimal::new(90, 2))?,
                error_message: "source timeout".to_string(),
                observed_at: OffsetDateTime::UNIX_EPOCH + Duration::seconds(10),
                trace_id: "trc_degrade_reuters".to_string(),
            })
            .await?;

        let result = store
            .recompute_signal(&RecomputeSignalCommand {
                signal_id: "sig_2412".to_string(),
                reason: "test source health adjustment".to_string(),
                trace_id: "trc_recompute".to_string(),
            })
            .await?;

        assert!(
            result
                .estimate
                .reason_codes
                .contains(&"source_health_degraded".to_string())
        );

        Ok(())
    }

    #[tokio::test]
    async fn postgres_news_source_health_round_trips_filters_and_migrates_index()
    -> std::result::Result<(), Box<dyn Error>> {
        let Some(database_url) = std::env::var("POLYEDGE_TEST_DATABASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
        else {
            return Ok(());
        };

        let schema = format!("polyedge_test_{}", Uuid::now_v7().simple());
        let quoted_schema = quote_pg_ident(&schema);
        let admin_pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await?;
        admin_pool
            .execute(format!("CREATE SCHEMA {quoted_schema}").as_str())
            .await?;

        let test_result: std::result::Result<(), Box<dyn Error>> = async {
            let search_path_schema = quoted_schema.clone();
            let pool = PgPoolOptions::new()
                .max_connections(2)
                .after_connect(move |connection, _meta| {
                    let search_path_schema = search_path_schema.clone();
                    Box::pin(async move {
                        connection
                            .execute(format!("SET search_path TO {search_path_schema}").as_str())
                            .await?;
                        Ok(())
                    })
                })
                .connect(&database_url)
                .await?;
            TEST_MIGRATOR.run(&pool).await?;

            let index_exists: bool = sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
                .bind(format!(
                    "{schema}.news_source_health_source_type_updated_at_idx"
                ))
                .fetch_one(&admin_pool)
                .await?;
            assert!(index_exists);

            let store = PostgresMarketEventStore::new(pool.clone());
            let reliability = Probability::new(Decimal::new(90, 2))?;
            let news_seen = OffsetDateTime::UNIX_EPOCH + Duration::seconds(1);
            let news_failed = OffsetDateTime::UNIX_EPOCH + Duration::seconds(2);
            let official_seen = OffsetDateTime::UNIX_EPOCH + Duration::seconds(3);

            store
                .record_news_source_success(&NewsSourceSuccessUpdate {
                    source: "wire_feed".to_string(),
                    source_type: "news".to_string(),
                    reliability,
                    fetched: 4,
                    inserted: 3,
                    deduped: 1,
                    observed_at: news_seen,
                    trace_id: "trc_pg_success".to_string(),
                })
                .await?;
            store
                .record_news_source_failure(&NewsSourceFailureUpdate {
                    source: "wire_feed".to_string(),
                    source_type: "news".to_string(),
                    reliability,
                    error_message: "upstream timeout".to_string(),
                    observed_at: news_failed,
                    trace_id: "trc_pg_failure".to_string(),
                })
                .await?;
            store
                .record_news_source_success(&NewsSourceSuccessUpdate {
                    source: "sec_feed".to_string(),
                    source_type: "official".to_string(),
                    reliability,
                    fetched: 1,
                    inserted: 1,
                    deduped: 0,
                    observed_at: official_seen,
                    trace_id: "trc_pg_official".to_string(),
                })
                .await?;

            let all_sources = store
                .list_news_source_health(&NewsSourceHealthListFilters::new(None, Some(10))?)
                .await?;
            assert_eq!(all_sources.len(), 2);
            assert_eq!(all_sources[0].source, "sec_feed");

            let news_sources = store
                .list_news_source_health(&NewsSourceHealthListFilters::new(
                    Some("news".to_string()),
                    Some(10),
                )?)
                .await?;
            assert_eq!(news_sources.len(), 1);

            let wire_feed = &news_sources[0];
            assert_eq!(wire_feed.source, "wire_feed");
            assert_eq!(wire_feed.items_fetched, 4);
            assert_eq!(wire_feed.items_inserted, 3);
            assert_eq!(wire_feed.items_deduped, 1);
            assert_eq!(wire_feed.consecutive_failures, 1);
            assert_eq!(wire_feed.last_error.as_deref(), Some("upstream timeout"));
            assert_eq!(
                wire_feed.health_score,
                Probability::new(Decimal::new(70, 2))?
            );

            pool.close().await;
            Ok(())
        }
        .await;

        admin_pool
            .execute(format!("DROP SCHEMA IF EXISTS {quoted_schema} CASCADE").as_str())
            .await?;
        admin_pool.close().await;

        test_result
    }
}
