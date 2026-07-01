use async_trait::async_trait;
use polyedge_application::{
    AuditLogEntry, AuditLogSink, CopyControlAction, CopyControlCommand, CopyControlCommandStatus,
    CopyEvent, CopyEventSeverity, CopyOrderSide, CopySizingMode, CopyTradeConfig, CopyTradeMode,
    CopyTradeStore, DatabaseMaintenanceCutoffs, DatabaseMaintenanceReport,
    DatabaseMaintenanceStore, HighProbabilityBacktestExitRuleReport,
    HighProbabilityBacktestPersistReport, HighProbabilityBacktestReport,
    HighProbabilityBacktestResult, HighProbabilityBacktestRun, HighProbabilityBacktestTrade,
    HighProbabilityBucketStats, HighProbabilityConfig, HighProbabilityDecision,
    HighProbabilityMarketOutcome, HighProbabilityMarketOutcomeStatus, HighProbabilityMode,
    HighProbabilityObservation, HighProbabilityObserveCandidate,
    HighProbabilityRewardCandleSampleInput, HighProbabilitySample, HighProbabilitySampleOutcome,
    HighProbabilitySampleQuery, HighProbabilityStore, HighProbabilityTriggerKind, IdempotencyBegin,
    IdempotencyRequest, IdempotencyStore, ManagedRewardOrder, ManagedRewardOrderStatus,
    ModeSnapshot, ModeStateStore, ModeTransitionCommand, PostFillStrategy, RewardAccountState,
    RewardAiAdvisoryRequest, RewardAiProvider, RewardAiRequestFormat, RewardAiSuitability,
    RewardBotConfig, RewardBotStore, RewardCandidateFilter, RewardControlAction,
    RewardControlCommand, RewardControlCommandStatus, RewardEventTimeConfidence, RewardFill,
    RewardFillRole, RewardGammaEventDateMode, RewardHistoryPruneReport, RewardInfoDirectionalRisk,
    RewardInfoRiskAssessmentRequest, RewardInfoRiskLevel, RewardInfoRiskSource, RewardInfoRiskType,
    RewardLlmCallDailyStats, RewardLlmCallRecord, RewardLowCompetitionMode,
    RewardLowCompetitionObservation, RewardMarket, RewardMarketAdvisory, RewardMarketCandle,
    RewardMarketCandleSample, RewardMarketEventWindow, RewardMarketInfoRisk, RewardMergeIntent,
    RewardMergeIntentStatus, RewardOrderListQuery, RewardOrderPage, RewardOrderSide,
    RewardOrderSortField, RewardOrderStatusFilter, RewardPlanQuoteMode, RewardPosition,
    RewardQuoteMode, RewardQuotePlan, RewardQuotePlanBlockerCounts, RewardQuotePlanCounts,
    RewardQuotePlanListQuery, RewardQuotePlanPage, RewardQuotePlanSortField, RewardRiskEvent,
    RewardRiskSeverity, RewardSelectionMode, RewardStrategyBucket, RewardStrategyProfile,
    RewardTickOutcome, RewardToken, RewardUnknownEventTimeMode, RiskStateSnapshot, RiskStateStore,
    SmartMoneyConfig, SmartMoneyMode, SmartMoneySide, SmartMoneyStore, SmartSignal,
    SmartSignalAdvisory, SmartSignalAdvisoryLookup, SmartSignalDecision, SmartSignalDecisionValue,
    SmartSignalStatus, SmartWalletCandidate, SmartWalletCandidateStatus, SmartWalletProfile,
    SmartWalletScore, SmartWalletTier, SmartWalletTrade, SortOrder, SourceTrade, TrackedWallet,
    TrackedWalletStatus, WalletAnalysisStats, refresh_reward_quote_plan_readiness,
    reward_order_counts_as_external_open,
};
use polyedge_domain::{
    AppError, ExposureRatio, IdempotencyStatus, Result, SignedUsdAmount, SystemMode,
};
use rust_decimal::Decimal;
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Row, types::Json};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    str::FromStr,
    sync::Arc,
};
use time::{Duration, OffsetDateTime};
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

const SYSTEM_RUNTIME_STATE_ID: &str = "global";
const RISK_STATE_ID: &str = "global";

fn db_error(code: &'static str, context: impl Into<String>) -> AppError {
    AppError::dependency_unavailable(code, context.into())
}

fn i64_count_to_u64(value: i64) -> u64 {
    u64::try_from(value.max(0)).unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalEventBegin {
    New,
    Replay,
}

include!("stores/runtime_config.rs");
include!("stores/mode_state.rs");
include!("stores/risk_state.rs");
include!("stores/idempotency.rs");
include!("stores/external_event.rs");
include!("stores/audit.rs");
include!("stores/maintenance.rs");
include!("stores/high_probability.rs");
include!("stores/rewards/postgres_market_methods.rs");
include!("stores/rewards/postgres_candles.rs");
include!("stores/rewards.rs");
include!("stores/copytrade.rs");
include!("stores/smart_money.rs");
include!("stores/orderbook_cache.rs");
include!("stores/orderbook_registry.rs");
include!("stores/helpers.rs");
include!("stores/types.rs");
include!("stores/orderbook_registry_tests.rs");
include!("stores/orderbook_cache_tests.rs");
include!("stores/rewards_tests.rs");
