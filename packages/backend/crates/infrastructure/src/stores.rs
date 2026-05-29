use async_trait::async_trait;
use polyedge_application::{
    AuditLogEntry, AuditLogSink, CopyAccountState, CopyEvent, CopyEventSeverity, CopyFill, CopyOrder,
    CopyOrderStatus, CopyOrderSide, CopyPosition, CopySimulationOutcome, CopySizingMode,
    CopyTradeConfig, CopyTradeMode, CopyTradeStore, IdempotencyBegin, IdempotencyRequest,
    IdempotencyStore, ManagedRewardOrder, ManagedRewardOrderStatus, ModeSnapshot, ModeStateStore,
    ModeTransitionCommand, PostFillStrategy, RewardAccountState, RewardBotConfig, RewardBotMode,
    RewardBotStore, RewardFill, RewardFillRole, RewardMarket, RewardOrderSide, RewardPosition,
    RewardQuotePlan, RewardRiskEvent, RewardRiskSeverity, RewardSimulationOutcome, RewardToken,
    RiskStateSnapshot, RiskStateStore, SourceTrade, TrackedWallet, TrackedWalletStatus,
    WalletAnalysisStats,
};
use polyedge_domain::{
    AppError, ExposureRatio, IdempotencyStatus, Result, SignedUsdAmount, SystemMode,
};
use rust_decimal::Decimal;
use serde_json::Value;
use sqlx::{PgPool, Row, types::Json};
use std::{
    collections::{BTreeMap, HashMap},
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
include!("stores/rewards.rs");
include!("stores/copytrade.rs");
include!("stores/orderbook_cache.rs");
include!("stores/helpers.rs");
include!("stores/types.rs");
