#![allow(clippy::too_many_arguments)]

mod execution;
mod list_filters;
mod maintenance;
mod market_event;
mod news_ingestion;
mod orderbook_cache;
mod orderbook_registry;
pub mod pagination;
mod rewards;
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
    REWARD_LIVE_ORDERBOOK_VALIDATION_SKIP_TTL, REWARD_PRICE_HISTORY_CANDLE_INTERVAL_SEC,
    RewardAccountState, RewardActionPlanner, RewardActionPlannerContext, RewardAiAdvisoryDecision,
    RewardAiAdvisoryRequest, RewardAiProvider, RewardAiRequestFormat, RewardBookLevel,
    RewardBookSideMetrics, RewardBotConfig, RewardBotConfigPatch, RewardBotRunReport,
    RewardBotService, RewardBotSnapshot, RewardBotStatus, RewardBotStore, RewardCandidateFilter,
    RewardCandidateMarket, RewardControlAction, RewardControlCommand, RewardControlCommandStatus,
    RewardDecisionEngine, RewardDecisionSet, RewardEventTimeConfidence,
    RewardEventWindowAssessment, RewardEventWindowStatus, RewardExecutionMode,
    RewardExitStrategySource, RewardFairValueComponent, RewardFairValueDecision,
    RewardFairValueEstimate, RewardFill, RewardFillRole, RewardGammaEventDateMode,
    RewardHistoryPruneReport, RewardInfoDirectionalRisk, RewardInfoRiskAssessmentDecision,
    RewardInfoRiskAssessmentRequest, RewardInfoRiskLevel, RewardInfoRiskSource, RewardInfoRiskType,
    RewardListPage, RewardLiveCycle, RewardLiveEngineInput, RewardLiveQuoteMaterialization,
    RewardLlmCallDailyStats, RewardLlmCallRecord, RewardMarket, RewardMarketAdvisory,
    RewardMarketBookMetrics, RewardMarketCandle, RewardMarketCandleSample, RewardMarketEventWindow,
    RewardMarketInfoRisk, RewardMarketSelectionMetrics, RewardMergeActionProposal,
    RewardMergeIntent, RewardMergeIntentStatus, RewardOpportunityMetrics, RewardOrderActionIntent,
    RewardOrderActionProposal, RewardOrderBook, RewardOrderListQuery, RewardOrderPage,
    RewardOrderSide, RewardOrderSortField, RewardOrderStatusFilter, RewardOrderTransition,
    RewardOrderTransitionListQuery, RewardOrderTransitionPage, RewardPlanQuoteMode, RewardPosition,
    RewardProviderAction, RewardProviderDecision, RewardProviderPreLlmCandidateKind,
    RewardProviderRequest, RewardQuoteEdge, RewardQuoteLeg, RewardQuoteMode, RewardQuotePlan,
    RewardQuotePlanBlockerCounts, RewardQuotePlanCounts, RewardQuotePlanListQuery,
    RewardQuotePlanPage, RewardQuotePlanSortField, RewardQuoteReadiness, RewardRiskEvent,
    RewardRiskSeverity, RewardSelectionMode, RewardStrategyAction, RewardStrategyActionListQuery,
    RewardStrategyActionPage, RewardStrategyActionStatus, RewardStrategyActionType,
    RewardStrategyBucket, RewardStrategyDecision, RewardStrategyDecisionListQuery,
    RewardStrategyDecisionPage, RewardStrategyInput, RewardStrategyProfile, RewardStrategyRun,
    RewardStrategyRunListQuery, RewardStrategyRunPage, RewardStrategyRunStart,
    RewardStrategyRunStatus, RewardStrategyRunTrigger, RewardTickOutcome, RewardToken,
    RewardTokenQuote, RewardUnknownEventTimeMode, apply_first_quote_entry_gates,
    apply_reward_ai_advisories, apply_reward_event_windows_to_quote_plans,
    apply_reward_fair_value_to_quote_plan, apply_reward_fair_values_to_quote_plans,
    apply_reward_info_risks, apply_reward_live_funding_precheck,
    apply_reward_market_selection_to_quote_plans, apply_reward_opportunity_metrics_to_quote_plans,
    build_reward_ai_advisory_request, build_reward_info_risk_assessment_request,
    build_reward_quote_plans, mark_reward_pre_ai_eligible_quote_plans,
    materialize_reward_quote_plan_for_live_orderbook, new_risk_event,
    refresh_reward_live_quote_plan_readiness, refresh_reward_opportunity_metrics_for_quote_plans,
    refresh_reward_quote_plan_readiness, reward_ai_edge_buffer_cents, reward_ai_effective_action,
    reward_ai_effective_request_format, reward_ai_model_is_glm_reasoning,
    reward_ai_model_requires_openai_chat_completions,
    reward_ai_model_uses_openai_chat_completions_endpoint, reward_ai_size_multiplier,
    reward_condition_has_active_exposure, reward_config_hash,
    reward_external_order_id_counts_as_external, reward_info_risk_effective_action,
    reward_info_risk_size_multiplier, reward_order_counts_as_external_open,
    reward_order_transition_from_order_change, reward_provider_cache_refresh_due,
    reward_provider_plan_passes_pre_llm_gate, reward_provider_pre_llm_candidate_kind,
    reward_quote_plan_blocker_codes, reward_quote_plan_event_window_blocks_new_buy,
    reward_quote_plan_event_window_cancels_open_buy, reward_quote_plan_readiness,
    reward_quote_plan_reason_code, reward_strategy_actions_from_tick_outcome,
    reward_strategy_decisions_from_plans, scale_double_legs_for_budget,
    scale_double_legs_for_weighted_budget, scale_single_leg_for_budget,
    select_reward_book_token_ids, validate_reward_list_limit,
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
