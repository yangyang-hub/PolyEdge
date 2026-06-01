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
    MarketBookSnapshotView, MarketCategoryView, MarketEventStore, MarketListFilters,
    MarketSortField, MarketView, NewsIngestionStore, NewsRawEventInsert, NewsRawEventListFilters,
    NewsRawEventView, NewsSourceFailureUpdate, NewsSourceHealthListFilters, NewsSourceHealthView,
    NewsSourceSuccessUpdate, OrderDraftListFilters, OrderDraftView, OrderListFilters, OrderView,
    PositionListFilters, PositionView, ProbabilityEstimateListFilters, ProbabilityEstimateView,
    RecomputeSignalCommand, RecomputeSignalResult, ReconcileExecutionListFilters,
    SignalListFilters, SignalTransitionListFilters, SignalTransitionView, SignalView, SortOrder,
    SourceHealthAdjustment, SubmitExecutionStoreCommand, TradeListFilters, TradeView,
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

mod in_memory;
mod postgres;

pub use in_memory::InMemoryMarketEventStore;
pub use postgres::PostgresMarketEventStore;

#[cfg(test)]
mod tests;

include!("helpers/fetch.rs");
include!("helpers/market_rows.rs");
include!("helpers/news_rows.rs");
include!("helpers/arbitrage_rows.rs");
include!("helpers/event_rows.rs");
include!("helpers/execution_rows.rs");
include!("helpers/calculations.rs");
