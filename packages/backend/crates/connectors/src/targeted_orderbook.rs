//! Targeted Polymarket order-book reads.
//!
//! This connector intentionally has no reward-market catalog, Gamma market
//! discovery, price-history, event, news, AI, or provider functionality. The
//! caller supplies the exact token ids required by configured strategies,
//! managed orders, and non-zero positions.

use polyedge_domain::{AppError, Result};
use rust_decimal::Decimal;
use serde::{Deserialize, de::DeserializeOwned};
use std::{collections::HashSet, str::FromStr};
use time::OffsetDateTime;
use tokio::task::JoinSet;

const ENRICH_CONCURRENCY: usize = 3;
const RESPONSE_PREVIEW_BYTES: usize = 300;

#[derive(Debug, Clone)]
pub struct PolymarketOrderBookLevel {
    pub price: Decimal,
    pub size: Decimal,
}

#[derive(Debug, Clone)]
pub struct PolymarketOrderBook {
    pub token_id: String,
    pub bids: Vec<PolymarketOrderBookLevel>,
    pub asks: Vec<PolymarketOrderBookLevel>,
    pub observed_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct PolymarketOrderBookConnector {
    clob_host: String,
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct RawBookLevel {
    price: Option<String>,
    size: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawOrderBook {
    asset_id: Option<String>,
    timestamp: Option<String>,
    bids: Option<Vec<RawBookLevel>>,
    asks: Option<Vec<RawBookLevel>>,
}

impl PolymarketOrderBookConnector {
    pub fn new(clob_host: &str) -> Result<Self> {
        let clob_host = clob_host.trim().trim_end_matches('/').to_string();
        if clob_host.is_empty() {
            return Err(AppError::invalid_input(
                "POLYMARKET_CLOB_HOST_REQUIRED",
                "polymarket clob_host must not be empty",
            ));
        }
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|error| {
                AppError::internal(
                    "POLYMARKET_HTTP_CLIENT_FAILED",
                    format!("failed to create Polymarket HTTP client: {error}"),
                )
            })?;
        Ok(Self { clob_host, client })
    }
}

fn decode_json_body<T: DeserializeOwned>(
    body: &[u8],
    code: &'static str,
    label: &str,
) -> Result<T> {
    serde_json::from_slice(body).map_err(|error| {
        AppError::dependency_unavailable(
            code,
            format!(
                "failed to decode {label}: {error}; body_preview=\"{}\"",
                response_body_preview(body)
            ),
        )
    })
}

fn response_body_preview(body: &[u8]) -> String {
    let preview_len = body.len().min(RESPONSE_PREVIEW_BYTES);
    let mut preview = String::new();
    for ch in String::from_utf8_lossy(&body[..preview_len]).chars() {
        preview.extend(ch.escape_debug());
    }
    preview
}

include!("targeted_orderbook/requests.rs");
