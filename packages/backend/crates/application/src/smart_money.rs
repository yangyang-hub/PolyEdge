#![allow(clippy::too_many_arguments)]

use async_trait::async_trait;
use polyedge_domain::{AppError, Result};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, str::FromStr, sync::Arc};
use time::OffsetDateTime;

const DEFAULT_SMART_MONEY_LIST_LIMIT: u16 = 100;
const MAX_SMART_MONEY_LIST_LIMIT: u16 = 500;
const SMART_MONEY_SCORING_VERSION: &str = "smart_money_v1";

include!("smart_money/models.rs");
include!("smart_money/service.rs");
include!("smart_money/scoring.rs");
include!("smart_money/signal.rs");
include!("smart_money/advisory_payload.rs");
include!("smart_money/helpers.rs");
include!("smart_money/tests.rs");
