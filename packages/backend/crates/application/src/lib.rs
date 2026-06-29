#![allow(clippy::too_many_arguments)]

mod copytrade;
mod execution;
mod high_probability;
mod list_filters;
mod maintenance;
mod market_event;
mod news_ingestion;
mod orderbook_cache;
mod orderbook_registry;
pub mod pagination;
mod rewards;
mod risk;
mod smart_money;
mod system_mode;
pub mod wallet_analysis;

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
pub use high_probability::{
    HighProbabilityBacktestExitRuleReport, HighProbabilityBacktestPersistReport,
    HighProbabilityBacktestReport, HighProbabilityBacktestResult, HighProbabilityBacktestRun,
    HighProbabilityBacktestTrade, HighProbabilityBucketRefreshReport, HighProbabilityBucketStats,
    HighProbabilityConfig, HighProbabilityDecision, HighProbabilityMarketOutcome,
    HighProbabilityMarketOutcomeStatus, HighProbabilityMode, HighProbabilityObservation,
    HighProbabilityObserveCandidate, HighProbabilityObserveReport, HighProbabilityOrderbookQuote,
    HighProbabilityResearchReport, HighProbabilityRewardCandleSampleInput, HighProbabilitySample,
    HighProbabilitySampleBuildReport, HighProbabilitySampleOutcome, HighProbabilitySampleQuery,
    HighProbabilityService, HighProbabilitySnapshot, HighProbabilityStore,
    HighProbabilityTriggerKind, build_high_probability_bucket_stats,
    build_high_probability_samples_from_reward_candles, validate_high_probability_list_limit,
    validate_high_probability_sample_input_limit,
};
pub use maintenance::{
    DatabaseMaintenanceCutoffs, DatabaseMaintenanceReport, DatabaseMaintenanceService,
    DatabaseMaintenanceStore,
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
    REWARD_AI_CANDLE_INTERVAL_SEC, REWARD_AI_CANDLE_LIMIT_PER_TOKEN,
    REWARD_AI_CANDLE_SOURCE_INTERVAL_SEC, REWARD_AI_CANDLE_SOURCE_LIMIT_PER_TOKEN,
    REWARD_PRICE_HISTORY_CANDLE_INTERVAL_SEC, RewardAccountState, RewardAiAdvisoryDecision,
    RewardAiAdvisoryRequest, RewardAiProvider, RewardAiRequestFormat, RewardAiSuitability,
    RewardBookLevel, RewardBookSideMetrics, RewardBotConfig, RewardBotConfigPatch,
    RewardBotRunReport, RewardBotService, RewardBotSnapshot, RewardBotStatus, RewardBotStore,
    RewardCandidateFilter, RewardCandidateMarket, RewardControlAction, RewardControlCommand,
    RewardControlCommandStatus, RewardEventTimeConfidence, RewardEventWindowAssessment,
    RewardEventWindowStatus, RewardExecutionMode, RewardFill, RewardFillRole,
    RewardGammaEventDateMode, RewardHistoryPruneReport, RewardInfoDirectionalRisk,
    RewardInfoRiskAssessmentDecision, RewardInfoRiskAssessmentRequest, RewardInfoRiskLevel,
    RewardInfoRiskSource, RewardInfoRiskType, RewardListPage, RewardLiveCycle,
    RewardLiveQuoteMaterialization, RewardLlmCallDailyStats, RewardLlmCallRecord,
    RewardLowCompetitionMetrics, RewardLowCompetitionMode, RewardLowCompetitionObservation,
    RewardLowCompetitionShadowReport, RewardMarket, RewardMarketAdvisory, RewardMarketBookMetrics,
    RewardMarketCandle, RewardMarketCandleSample, RewardMarketEventWindow, RewardMarketInfoRisk,
    RewardOpportunityMetrics, RewardOrderBook, RewardOrderListQuery, RewardOrderPage,
    RewardOrderSide, RewardOrderSortField, RewardOrderStatusFilter, RewardPlanQuoteMode,
    RewardPosition, RewardProviderDecision, RewardProviderPreLlmCandidateKind,
    RewardProviderRequest, RewardQuoteLeg, RewardQuoteMode, RewardQuotePlan,
    RewardQuotePlanBlockerCounts, RewardQuotePlanCounts, RewardQuotePlanListQuery,
    RewardQuotePlanPage, RewardQuotePlanSortField, RewardQuoteReadiness, RewardRiskEvent,
    RewardRiskSeverity, RewardSelectionMode, RewardStrategyBucket, RewardTickOutcome, RewardToken,
    RewardTokenQuote, RewardUnknownEventTimeMode, apply_first_quote_entry_gates,
    apply_reward_ai_advisories, apply_reward_event_windows_to_quote_plans, apply_reward_info_risks,
    apply_reward_opportunity_metrics_to_quote_plans, build_reward_ai_advisory_request,
    build_reward_info_risk_assessment_request, build_reward_quote_plans,
    materialize_reward_quote_plan_for_live_orderbook, new_risk_event,
    refresh_reward_opportunity_metrics_for_quote_plans, refresh_reward_quote_plan_readiness,
    reward_ai_advisory_blocks_quote, reward_ai_effective_request_format,
    reward_ai_model_requires_openai_chat_completions,
    reward_ai_strategy_hint_max_condition_notional_usd, reward_condition_has_active_exposure,
    reward_external_order_id_counts_as_external, reward_market_books_available,
    reward_order_counts_as_external_open, reward_provider_cache_refresh_due,
    reward_provider_plan_passes_pre_llm_gate, reward_provider_pre_llm_candidate_kind,
    reward_quote_plan_event_window_blocks_new_buy, reward_quote_plan_event_window_cancels_open_buy,
    reward_quote_plan_readiness, scale_double_legs_for_budget, scale_single_leg_for_budget,
    select_reward_book_token_ids, validate_reward_list_limit,
};
pub use risk::{
    ApproveSignalCommand, ApproveSignalReceipt, KillSwitchReceipt, RejectSignalCommand,
    RejectSignalReceipt, ReleaseKillSwitchCommand, RiskPolicy, RiskService, RiskStateSnapshot,
    RiskStateStore, RiskStateView, TriggerKillSwitchCommand,
};
pub use smart_money::{
    SmartMoneyConfig, SmartMoneyConfigPatch, SmartMoneyMode, SmartMoneyService, SmartMoneySide,
    SmartMoneySnapshot, SmartMoneyStatus, SmartMoneyStore, SmartSignal, SmartSignalAdvisory,
    SmartSignalAdvisoryContext, SmartSignalAdvisoryDecision, SmartSignalAdvisoryLookup,
    SmartSignalAdvisoryRequest, SmartSignalBookQuote, SmartSignalDecision,
    SmartSignalDecisionValue, SmartSignalGenerationReport, SmartSignalStatus, SmartWalletCandidate,
    SmartWalletCandidateStatus, SmartWalletCandidateStatusUpdate, SmartWalletProfile,
    SmartWalletScore, SmartWalletTier, SmartWalletTrade, build_smart_wallet_score,
    normalize_smart_wallet_address, validate_smart_money_list_limit,
};
pub use system_mode::{
    AuditLogEntry, AuditLogSink, AuthenticatedActor, IdempotencyBegin, IdempotencyRequest,
    IdempotencyStore, ModeSnapshot, ModeStateStore, ModeTransitionCommand, SystemModeService,
    SystemModeTransitionReceipt,
};
