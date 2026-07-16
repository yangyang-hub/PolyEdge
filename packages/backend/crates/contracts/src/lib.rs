//! HTTP API contract DTOs for PolyEdge.
//!
//! Only the V3 manual-market, multi-wallet HTTP surface is public.

use polyedge_domain::MarketStatus;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use time::OffsetDateTime;

include!("dto/common.rs");
include!("identity.rs");
include!("manual_trading.rs");
