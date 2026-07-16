use polyedge_domain::{AppError, Probability, Quantity, Result};
use polymarket_client_sdk::auth::state::Authenticated;
use polymarket_client_sdk::auth::{Credentials, LocalSigner, Normal, Signer, Uuid};
use polymarket_client_sdk::clob::types::request::{
    BalanceAllowanceRequest, OrdersRequest, UpdateBalanceAllowanceRequest,
};
use polymarket_client_sdk::clob::types::response::{BalanceAllowanceResponse, OpenOrderResponse};
use polymarket_client_sdk::clob::types::{
    AssetType, OrderStatusType as SdkOrderStatusType, OrderType, Side, SignatureType,
};
use polymarket_client_sdk::clob::{Client as ClobClient, Config as ClobConfig};
use polymarket_client_sdk::error::{Error as PolymarketSdkError, Status as PolymarketSdkStatus};
use polymarket_client_sdk::types::{Address, U256};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::str::FromStr;
use std::time::Duration;
use time::OffsetDateTime;

pub const POLYMARKET_CONNECTOR_NAME: &str = "polymarket";
const POLYMARKET_MIN_NOTIONAL_USD: Decimal = Decimal::ONE;
const CLOB_TERMINAL_CURSOR: &str = "LTE=";
const CLOB_MAX_PAGES: usize = 1000;

include!("polymarket/models.rs");
include!("polymarket/live.rs");
include!("polymarket/order_reconciliation.rs");
include!("polymarket/helpers.rs");

fn parse_decimal_value(value: Option<JsonValue>) -> Option<Decimal> {
    match value? {
        JsonValue::Number(number) => number.to_string().parse().ok(),
        JsonValue::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

include!("polymarket/data_api.rs");
