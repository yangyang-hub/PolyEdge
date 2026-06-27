#![allow(clippy::too_many_arguments)]

use async_trait::async_trait;
use polyedge_domain::{AppError, Result};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    str::FromStr,
    sync::Arc,
};
use time::OffsetDateTime;

const DEFAULT_HIGH_PROBABILITY_LIST_LIMIT: u16 = 100;
const MAX_HIGH_PROBABILITY_LIST_LIMIT: u16 = 1_000;
const DEFAULT_HIGH_PROBABILITY_SAMPLE_INPUT_LIMIT: u32 = 50_000;
const MAX_HIGH_PROBABILITY_SAMPLE_INPUT_LIMIT: u32 = 250_000;
const HIGH_PROBABILITY_BUCKET_MODEL_VERSION: &str = "high_probability_bucket_v1";
const HIGH_PROBABILITY_MIN_BUCKET_SAMPLES: u64 = 30;

include!("high_probability/models.rs");
include!("high_probability/service.rs");
include!("high_probability/bucket_model.rs");
include!("high_probability/sample_builder.rs");
include!("high_probability/helpers.rs");
include!("high_probability/tests.rs");
