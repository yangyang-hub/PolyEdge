use crate::ModeStateStore;
use async_trait::async_trait;
use polyedge_domain::{AppError, Result, SortOrder, SystemMode};
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    str::FromStr,
    sync::Arc,
};
use time::OffsetDateTime;

const DEFAULT_LIST_LIMIT: u16 = 100;
const MAX_LIST_LIMIT: u16 = 500;
const MAX_REWARD_RUN_MARKET_LIMIT: u16 = u16::MAX;
const DEFAULT_TICK: Decimal = Decimal::from_parts(1, 0, 0, false, 2);

include!("rewards/models.rs");
include!("rewards/pagination.rs");
include!("rewards/control.rs");
include!("rewards/service.rs");
include!("rewards/planner.rs");
include!("rewards/engine.rs");
include!("rewards/helpers.rs");
include!("rewards/tests.rs");
