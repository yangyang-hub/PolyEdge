#![allow(clippy::too_many_arguments)]

mod arbitrage;
mod copytrade;
mod execution;
mod list_filters;
mod market_event;
mod news_ingestion;
mod orderbook_cache;
mod rewards;
mod risk;
mod system_mode;
pub mod wallet_analysis;

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
pub use copytrade::{
    AddTrackedWalletInput, CopyAccountState, CopyBookLevel, CopyControlAction, CopyControlCommand,
    CopyControlCommandStatus, CopyDecision, CopyEvent, CopyEventSeverity, CopyFill, CopyOrder,
    CopyOrderBook, CopyOrderSide, CopyOrderStatus, CopyPosition, CopySimulationOutcome,
    CopySizingMode, CopySkipReason, CopyTradeConfig, CopyTradeConfigPatch, CopyTradeMode,
    CopyTradeRunReport, CopyTradeService, CopyTradeSnapshot, CopyTradeStatus, CopyTradeStore,
    SourceTrade, TrackedWallet, TrackedWalletStatus, WalletActionInput, WalletActivityInput,
    WalletAnalysisStats, WalletFeedInput, WalletPositionInput, build_wallet_analysis,
    check_skip_reasons, compute_copy_size, new_copy_event, normalize_address,
    run_copy_simulation_tick, validate_copytrade_list_limit,
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
#[cfg(any(test, feature = "test-fixtures"))]
pub use market_event::demo_fixture_bundle;
pub use market_event::{
    EventListFilters, EventView, EvidenceListFilters, EvidenceView, FixtureBundle,
    FixtureEventRecord, FixtureEvidenceRecord, FixtureIngestionReport, FixtureMarketRecord,
    FixtureSignalRecord, MarketCategoryView, MarketEventService, MarketEventStore,
    MarketListFilters, MarketSortField, MarketView, ProbabilityEstimateListFilters,
    ProbabilityEstimateView, RecomputeSignalCommand, RecomputeSignalDraft, RecomputeSignalResult,
    SignalListFilters, SignalTransitionDraft, SignalTransitionListFilters, SignalTransitionView,
    SignalView, SortOrder, SourceHealthAdjustment, build_recompute_signal_draft,
    build_recompute_signal_draft_with_source_health,
};
pub use news_ingestion::{
    NewsIngestSourceCommand, NewsIngestionItem, NewsIngestionService, NewsIngestionStore,
    NewsRawEventInsert, NewsRawEventListFilters, NewsRawEventView, NewsSourceFailureUpdate,
    NewsSourceHealthListFilters, NewsSourceHealthView, NewsSourceIngestionReport,
    NewsSourceSuccessUpdate, degraded_health_score,
};
pub use orderbook_cache::{BookSource, CachedBookLevel, CachedOrderBook, OrderbookCache};
pub use rewards::{
    ManagedRewardOrder, ManagedRewardOrderStatus, PostFillStrategy, RewardAccountState,
    RewardBookLevel, RewardBotConfig, RewardBotConfigPatch, RewardBotRunReport, RewardBotService,
    RewardBotSnapshot, RewardBotStatus, RewardBotStore, RewardControlAction, RewardControlCommand,
    RewardControlCommandStatus, RewardFill, RewardFillRole, RewardMarket, RewardOrderBook,
    RewardOrderSide, RewardPosition, RewardQuoteLeg, RewardQuotePlan, RewardRiskEvent,
    RewardRiskSeverity, RewardSimulationOutcome, RewardToken, build_reward_quote_plans,
    new_risk_event, run_reward_simulation_tick, select_reward_book_token_ids,
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
