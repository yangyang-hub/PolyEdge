use async_trait::async_trait;
use crate::ModeStateStore;
use polyedge_domain::{AppError, Result, SystemMode};
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::{HashMap, HashSet}, str::FromStr, sync::Arc};
use time::OffsetDateTime;

const DEFAULT_LIST_LIMIT: u16 = 100;
const MAX_LIST_LIMIT: u16 = 500;
const DEFAULT_TICK: Decimal = Decimal::from_parts(1, 0, 0, false, 2);

include!("rewards/models.rs");
include!("rewards/service.rs");
include!("rewards/planner.rs");
include!("rewards/engine.rs");
include!("rewards/helpers.rs");
include!("rewards/tests.rs");
