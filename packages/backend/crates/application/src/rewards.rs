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
/// Live orderbook validation failures are market-state observations, not
/// durable market exclusions. Keep the persisted skip short so fast paths do
/// not immediately retry, while the next full decision tick can revalidate
/// against the latest books.
pub const REWARD_LIVE_ORDERBOOK_VALIDATION_SKIP_TTL: TimeDuration = TimeDuration::seconds(60);

include!("rewards/models.rs");
include!("rewards/quote_selection_models.rs");
include!("rewards/event_window_source_models.rs");
include!("rewards/ai_advisory_models.rs");
include!("rewards/ai_advisory_payload.rs");
include!("rewards/info_risk_models.rs");
include!("rewards/provider_models.rs");
include!("rewards/run_ledger_models.rs");
include!("rewards/config_impl.rs");
include!("rewards/runtime_models.rs");
include!("rewards/action_request.rs");
include!("rewards/action_planner.rs");
include!("rewards/event_window.rs");
include!("rewards/pagination.rs");
include!("rewards/control.rs");
include!("rewards/service.rs");
include!("rewards/service_snapshot.rs");
include!("rewards/planner.rs");
include!("rewards/planner_selection.rs");
include!("rewards/planner_live.rs");
include!("rewards/opportunity_metrics.rs");
include!("rewards/fair_value.rs");
include!("rewards/market_selection.rs");
include!("rewards/provider_prefilter.rs");
include!("rewards/engine.rs");
include!("rewards/helpers.rs");
include!("rewards/strategy_input.rs");
include!("rewards/replay_v2.rs");
include!("rewards/replay.rs");

#[cfg(test)]
mod provider_cache_tests {
    include!("rewards/provider_cache_tests.rs");
}

#[cfg(test)]
mod opportunity_metrics_tests {
    use super::*;

    include!("rewards/opportunity_metrics_tests.rs");
}

#[cfg(test)]
mod fair_value_tests {
    use super::*;

    include!("rewards/fair_value_tests.rs");
}

#[cfg(test)]
mod market_selection_tests {
    use super::*;

    include!("rewards/market_selection_tests.rs");
}

#[cfg(test)]
mod provider_prefilter_tests {
    include!("rewards/provider_prefilter_tests.rs");
}

#[cfg(test)]
mod engine_tests {
    use super::*;

    include!("rewards/engine_tests.rs");
}

#[cfg(test)]
mod strategy_input_tests {
    include!("rewards/strategy_input_tests.rs");
}

#[cfg(test)]
mod replay_tests {
    include!("rewards/replay_tests.rs");
}

#[cfg(test)]
mod action_planner_tests {
    use super::*;

    include!("rewards/action_planner_tests.rs");
}
