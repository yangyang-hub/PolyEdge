#![allow(clippy::too_many_arguments)]

mod execution;
mod market_event;
mod news_ingestion;
mod risk;
mod system_mode;

pub use execution::{
    DEFAULT_EXECUTION_CONNECTOR, DispatchExecutionListFilters, ExecutionDispatchCandidate,
    ExecutionDispatchResult, ExecutionFillResult, ExecutionReconciliationCandidate,
    ExecutionRequestListFilters, ExecutionRequestView, ExecutionService,
    ExecutionSubmissionReceipt, ExecutionSubmissionResult, MarkExecutionFailedCommand,
    MarkExecutionSubmittedCommand, MarkOrderOpenCommand, OrderDraftListFilters, OrderDraftView,
    OrderListFilters, OrderView, PositionListFilters, PositionView, ReconcileExecutionFillCommand,
    ReconcileExecutionListFilters, ReconcileExternalTradeCommand, SubmitExecutionCommand,
    SubmitExecutionStoreCommand, SyncExternalOrderStatusCommand, TradeListFilters, TradeView,
};
pub use market_event::{
    EventListFilters, EventView, EvidenceListFilters, EvidenceView, FixtureBundle,
    FixtureEventRecord, FixtureEvidenceRecord, FixtureIngestionReport, FixtureMarketRecord,
    FixtureSignalRecord, MarketEventService, MarketEventStore, MarketListFilters, MarketView,
    ProbabilityEstimateListFilters, ProbabilityEstimateView, RecomputeSignalCommand,
    RecomputeSignalDraft, RecomputeSignalResult, SignalListFilters, SignalTransitionDraft,
    SignalTransitionListFilters, SignalTransitionView, SignalView, build_recompute_signal_draft,
    demo_fixture_bundle,
};
pub use news_ingestion::{
    NewsIngestSourceCommand, NewsIngestionItem, NewsIngestionService, NewsIngestionStore,
    NewsRawEventInsert, NewsSourceFailureUpdate, NewsSourceIngestionReport,
    NewsSourceSuccessUpdate,
};
pub use risk::{
    ApproveSignalCommand, ApproveSignalReceipt, KillSwitchReceipt, RejectSignalCommand,
    RejectSignalReceipt, ReleaseKillSwitchCommand, RiskPolicy, RiskService, RiskStateSnapshot,
    RiskStateStore, RiskStateView, TriggerKillSwitchCommand,
};
pub use system_mode::{
    AuditLogEntry, AuditLogSink, AuthenticatedActor, IdempotencyBegin, IdempotencyRequest,
    IdempotencyStore, ModeSnapshot, ModeStateStore, ModeTransitionCommand, SystemModeService,
    SystemModeTransitionReceipt,
};
