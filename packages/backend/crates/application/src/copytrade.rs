#![allow(clippy::too_many_arguments)]

use async_trait::async_trait;
use polyedge_domain::{AppError, Result};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{str::FromStr, sync::Arc};
use time::OffsetDateTime;

const DEFAULT_LIST_LIMIT: u16 = 100;
const MAX_LIST_LIMIT: u16 = 500;

include!("copytrade/models.rs");
include!("copytrade/control.rs");
include!("copytrade/inputs.rs");
include!("copytrade/service.rs");
include!("copytrade/analysis.rs");
include!("copytrade/helpers.rs");
include!("copytrade/tests.rs");
