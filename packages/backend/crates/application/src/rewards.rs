use async_trait::async_trait;
use polyedge_domain::{AppError, Result, SortOrder};
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
    sync::Arc,
};
use time::{Duration as TimeDuration, OffsetDateTime};
use tokio::sync::{RwLock, watch};

const DEFAULT_LIST_LIMIT: u16 = 100;
const MAX_LIST_LIMIT: u16 = 500;
const DEFAULT_TICK: Decimal = Decimal::from_parts(1, 0, 0, false, 2);

include!("rewards/models.rs");
include!("rewards/quote_selection_models.rs");
include!("rewards/ai_advisory_models.rs");
include!("rewards/info_risk_models.rs");
include!("rewards/config_impl.rs");
include!("rewards/runtime_models.rs");
include!("rewards/pagination.rs");
include!("rewards/control.rs");
include!("rewards/service.rs");
include!("rewards/planner.rs");
include!("rewards/planner_selection.rs");
include!("rewards/engine.rs");
include!("rewards/helpers.rs");
