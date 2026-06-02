use async_trait::async_trait;
use polyedge_domain::{AppError, Edge, Probability, Quantity, Result};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashSet},
    str::FromStr,
    sync::Arc,
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::pagination::{PageQuery, Paginated};

const DEFAULT_REPEAT_WINDOW_SECONDS: i64 = 300;

include!("arbitrage/models.rs");
include!("arbitrage/service.rs");
include!("arbitrage/detection.rs");
include!("arbitrage/analysis.rs");
include!("arbitrage/tests.rs");
