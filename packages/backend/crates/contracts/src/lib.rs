//! HTTP API contract DTOs for PolyEdge.
//!
//! Every request/response payload lives here so the API app and clients share a
//! single definition. Types are grouped by domain under `dto/` and inlined with
//! `include!`, keeping each DTO in the crate root (`polyedge_contracts::Xxx`) so
//! external import paths are unaffected by the file split.

use polyedge_domain::{
    AmbiguityLevel, Edge, EventStatus, EvidenceDirection, EvidenceStatus, ExecutionRequestStatus,
    ExposureRatio, MarketStatus, OrderDraftStatus, OrderStatus, Probability, Quantity,
    SignalAction, SignalLifecycleState, SignalSide, SignedUsdAmount, SystemMode, TimeHorizon,
    TradabilityStatus, UsdAmount,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use time::OffsetDateTime;

include!("dto/common.rs");
include!("dto/system.rs");
include!("dto/market.rs");
include!("dto/news.rs");
include!("dto/signal.rs");
include!("dto/risk.rs");
include!("dto/execution.rs");
include!("dto/arbitrage.rs");
include!("dto/callback.rs");
include!("dto/query.rs");
include!("dto/orderbook.rs");
include!("dto/wallet_analysis.rs");
