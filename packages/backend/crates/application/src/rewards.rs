use crate::orderbook_cache::CachedOrderBook;
use async_trait::async_trait;
use polyedge_domain::{AppError, Result, SortOrder};
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    str::FromStr,
    sync::Arc,
};
use time::{Duration as TimeDuration, OffsetDateTime};
use tokio::sync::{RwLock, watch};

const DEFAULT_LIST_LIMIT: u16 = 100;
const MAX_LIST_LIMIT: u16 = 500;
const DEFAULT_TICK: Decimal = Decimal::from_parts(1, 0, 0, false, 2);
const MIN_POLYMARKET_ORDER_NOTIONAL_USD: Decimal = Decimal::ONE;
pub const REWARD_PRICE_HISTORY_CANDLE_INTERVAL_SEC: i32 = 300;
pub const REWARD_AI_CANDLE_SOURCE_INTERVAL_SEC: i32 = REWARD_PRICE_HISTORY_CANDLE_INTERVAL_SEC;
// 24 hourly AI candles backed by 12 source 5m candles each.
pub const REWARD_AI_CANDLE_SOURCE_LIMIT_PER_TOKEN: u16 = 288;
pub const REWARD_AI_CANDLE_INTERVAL_SEC: i32 = 60 * 60;
pub const REWARD_AI_CANDLE_LIMIT_PER_TOKEN: u16 = 24;

include!("rewards/models.rs");
include!("rewards/quote_selection_models.rs");
include!("rewards/ai_advisory_models.rs");
include!("rewards/ai_advisory_payload.rs");
include!("rewards/info_risk_models.rs");
include!("rewards/config_impl.rs");
include!("rewards/runtime_models.rs");
include!("rewards/pagination.rs");
include!("rewards/control.rs");
include!("rewards/service.rs");
include!("rewards/service_snapshot.rs");
include!("rewards/planner.rs");
include!("rewards/planner_selection.rs");
include!("rewards/planner_live.rs");
include!("rewards/opportunity_metrics.rs");
include!("rewards/provider_prefilter.rs");
include!("rewards/engine.rs");
include!("rewards/helpers.rs");

#[cfg(test)]
mod provider_cache_tests {
    include!("rewards/provider_cache_tests.rs");
}

#[cfg(test)]
mod ai_advisory_tests {
    include!("rewards/ai_advisory_tests.rs");
}

#[cfg(test)]
mod opportunity_metrics_tests {
    use super::*;

    include!("rewards/opportunity_metrics_tests.rs");
}

#[cfg(test)]
mod provider_prefilter_tests {
    include!("rewards/provider_prefilter_tests.rs");
}
