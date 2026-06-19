#![allow(clippy::too_many_arguments)]

mod arbitrage;
mod copytrade;
mod execution;
mod list_filters;
mod market_event;
mod news_ingestion;
mod orderbook_cache;
mod orderbook_registry;
pub mod pagination;
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
    AddTrackedWalletInput, CopyControlAction, CopyControlCommand, CopyControlCommandStatus,
    CopyEvent, CopyEventSeverity, CopyOrderSide, CopySizingMode, CopyTradeConfig,
    CopyTradeConfigPatch, CopyTradeMode, CopyTradeRunReport, CopyTradeService, CopyTradeSnapshot,
    CopyTradeStatus, CopyTradeStore, SourceTrade, TrackedWallet, TrackedWalletStatus,
    WalletActionInput, WalletActivityInput, WalletAnalysisStats, WalletFeedInput,
    WalletPositionInput, build_wallet_analysis, new_copy_event, normalize_address,
    validate_copytrade_list_limit,
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
    MarketListFilters, MarketSortField, MarketUpsertOptions, MarketView,
    ProbabilityEstimateListFilters, ProbabilityEstimateView, RecomputeSignalCommand,
    RecomputeSignalDraft, RecomputeSignalResult, SignalListFilters, SignalTransitionDraft,
    SignalTransitionListFilters, SignalTransitionView, SignalView, SortOrder,
    SourceHealthAdjustment, build_recompute_signal_draft,
    build_recompute_signal_draft_with_source_health,
};
pub use news_ingestion::{
    NewsIngestSourceCommand, NewsIngestionItem, NewsIngestionService, NewsIngestionStore,
    NewsRawEventInsert, NewsRawEventListFilters, NewsRawEventView, NewsSourceFailureUpdate,
    NewsSourceHealthListFilters, NewsSourceHealthView, NewsSourceIngestionReport,
    NewsSourceSuccessUpdate, degraded_health_score,
};
pub use orderbook_cache::{
    BookSource, CachedBookLevel, CachedOrderBook, OrderbookCache, OrderbookStreamEvent,
    OrderbookStreamReason,
};
pub use orderbook_registry::OrderbookSubscriptionRegistry;
pub use pagination::{PageMeta, PageQuery, Paginated};
pub use rewards::{
    BookSnapshot, ManagedRewardOrder, ManagedRewardOrderStatus, PostFillStrategy,
    RewardAccountState, RewardAiAdvisoryDecision, RewardAiAdvisoryRequest, RewardAiProvider,
    RewardAiRequestFormat, RewardAiSuitability, RewardBookLevel, RewardBookSideMetrics,
    RewardBotConfig, RewardBotConfigPatch, RewardBotRunReport, RewardBotService, RewardBotSnapshot,
    RewardBotStatus, RewardBotStore, RewardCandidateFilter, RewardCandidateMarket,
    RewardControlAction, RewardControlCommand, RewardControlCommandStatus, RewardExecutionMode,
    RewardFill, RewardFillRole, RewardInfoDirectionalRisk, RewardInfoRiskAssessmentDecision,
    RewardInfoRiskAssessmentRequest, RewardInfoRiskLevel, RewardInfoRiskSource, RewardInfoRiskType,
    RewardListPage, RewardLiveCycle, RewardLiveQuoteMaterialization, RewardLowCompetitionMetrics,
    RewardLowCompetitionMode, RewardLowCompetitionObservation, RewardLowCompetitionShadowReport,
    RewardMarket, RewardMarketAdvisory, RewardMarketBookMetrics, RewardMarketInfoRisk,
    RewardOrderBook, RewardOrderListQuery, RewardOrderPage, RewardOrderSide, RewardOrderSortField,
    RewardOrderStatusFilter, RewardPlanQuoteMode, RewardPosition, RewardQuoteLeg, RewardQuoteMode,
    RewardQuotePlan, RewardQuotePlanListQuery, RewardQuotePlanPage, RewardQuotePlanSortField,
    RewardRiskEvent, RewardRiskSeverity, RewardSelectionMode, RewardStrategyBucket,
    RewardTickOutcome, RewardToken, apply_low_competition_metrics_to_quote_plans,
    apply_reward_ai_advisories, apply_reward_info_risks, build_low_competition_observations,
    build_reward_ai_advisory_request, build_reward_info_risk_assessment_request,
    build_reward_quote_plans, materialize_reward_quote_plan_for_live_orderbook,
    new_risk_event, reward_market_books_available, select_reward_book_token_ids,
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
