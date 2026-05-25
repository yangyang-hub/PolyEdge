#![allow(clippy::too_many_arguments)]

mod arbitrage;
mod execution;
mod market_event;
mod news_ingestion;
mod rewards;
mod risk;
mod system_mode;

pub use arbitrage::{
    ArbitrageAnalysisRunListFilters, ArbitrageAnalysisRunView, ArbitrageAnalysisSummary,
    ArbitrageEventListFilters, ArbitrageEventType, ArbitrageEventView, ArbitrageMarketSummary,
    ArbitrageOpportunityDraft, ArbitrageOpportunityListFilters, ArbitrageOpportunityStatus,
    ArbitrageOpportunityType, ArbitrageOpportunityValidationView, ArbitrageOpportunityView,
    ArbitrageScanListFilters, ArbitrageScanView, ArbitrageService, ArbitrageStore,
    ArbitrageTypeCount, ArbitrageValidationConfig, ArbitrageValidationStatus,
    MarketBookSnapshotView, build_arbitrage_analysis, detect_arbitrage_opportunities,
    market_book_snapshot_id, validate_arbitrage_opportunity,
};
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
    SignalTransitionListFilters, SignalTransitionView, SignalView, SourceHealthAdjustment,
    build_recompute_signal_draft, build_recompute_signal_draft_with_source_health,
    demo_fixture_bundle,
};
pub use news_ingestion::{
    NewsIngestSourceCommand, NewsIngestionItem, NewsIngestionService, NewsIngestionStore,
    NewsRawEventInsert, NewsRawEventListFilters, NewsRawEventView, NewsSourceFailureUpdate,
    NewsSourceHealthListFilters, NewsSourceHealthView, NewsSourceIngestionReport,
    NewsSourceSuccessUpdate, degraded_health_score,
};
pub use rewards::{
    ManagedRewardOrder, ManagedRewardOrderStatus, RewardBookLevel, RewardBotConfig,
    RewardBotConfigPatch, RewardBotMode, RewardBotRunReport, RewardBotService, RewardBotSnapshot,
    RewardBotStatus, RewardBotStore, RewardMarket, RewardOrderBook, RewardOrderSide,
    RewardPosition, RewardQuoteLeg, RewardQuotePlan, RewardRiskEvent, RewardRiskSeverity,
    RewardToken, build_reward_quote_plans, new_risk_event, select_reward_book_token_ids,
    validate_reward_list_limit,
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
