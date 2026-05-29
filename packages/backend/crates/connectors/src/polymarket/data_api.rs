// Polymarket public Data API connector (data-api.polymarket.com): per-wallet
// trade activity and current positions. Used by the copy-trading subsystem to
// analyze tracked wallets and detect their new trades. No authentication: these
// endpoints are public. This file is `include!`d by `polymarket.rs`; it relies
// on that module's shared imports plus the `Deserialize`/`JsonValue`/
// `parse_decimal_value`/`normalize_optional_text` items brought in by `gamma.rs`.

const MAX_DATA_API_LIMIT: u16 = 500;

#[derive(Debug, Clone)]
pub struct PolymarketWalletActivity {
    pub proxy_wallet: String,
    /// Raw activity type, e.g. `TRADE`, `SPLIT`, `MERGE`, `REDEEM`, `REWARD`.
    pub kind: String,
    /// Raw side, e.g. `BUY` / `SELL` (only meaningful for `TRADE`).
    pub side: String,
    pub asset: String,
    pub condition_id: String,
    pub outcome: String,
    pub outcome_index: i64,
    pub title: String,
    pub slug: String,
    pub transaction_hash: String,
    pub price: Decimal,
    pub size: Decimal,
    pub usdc_size: Decimal,
    pub timestamp: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct PolymarketWalletPosition {
    pub asset: String,
    pub condition_id: String,
    pub outcome: String,
    pub title: String,
    pub slug: String,
    pub size: Decimal,
    pub avg_price: Decimal,
    pub cur_price: Decimal,
    pub realized_pnl: Decimal,
    pub cash_pnl: Decimal,
    pub percent_pnl: Decimal,
}

#[derive(Debug, Clone)]
pub struct PolymarketDataApiConnector {
    data_api_host: String,
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct RawWalletActivity {
    #[serde(rename = "proxyWallet", default)]
    proxy_wallet: Option<String>,
    #[serde(rename = "type", default)]
    kind: Option<String>,
    #[serde(default)]
    side: Option<String>,
    #[serde(default)]
    asset: Option<String>,
    #[serde(rename = "conditionId", default)]
    condition_id: Option<String>,
    #[serde(default)]
    outcome: Option<String>,
    #[serde(rename = "outcomeIndex", default)]
    outcome_index: Option<JsonValue>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    slug: Option<String>,
    #[serde(rename = "transactionHash", default)]
    transaction_hash: Option<String>,
    #[serde(default)]
    price: Option<JsonValue>,
    #[serde(default)]
    size: Option<JsonValue>,
    #[serde(rename = "usdcSize", default)]
    usdc_size: Option<JsonValue>,
    #[serde(default)]
    timestamp: Option<JsonValue>,
}

#[derive(Debug, Deserialize)]
struct RawWalletPosition {
    #[serde(default)]
    asset: Option<String>,
    #[serde(rename = "conditionId", default)]
    condition_id: Option<String>,
    #[serde(default)]
    outcome: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    slug: Option<String>,
    #[serde(default)]
    size: Option<JsonValue>,
    #[serde(rename = "avgPrice", default)]
    avg_price: Option<JsonValue>,
    #[serde(rename = "curPrice", default)]
    cur_price: Option<JsonValue>,
    #[serde(rename = "realizedPnl", default)]
    realized_pnl: Option<JsonValue>,
    #[serde(rename = "cashPnl", default)]
    cash_pnl: Option<JsonValue>,
    #[serde(rename = "percentPnl", default)]
    percent_pnl: Option<JsonValue>,
}

impl PolymarketDataApiConnector {
    pub fn new(data_api_host: &str) -> Result<Self> {
        let data_api_host = data_api_host.trim().trim_end_matches('/').to_string();
        if data_api_host.is_empty() {
            return Err(AppError::invalid_input(
                "POLYMARKET_DATA_API_HOST_REQUIRED",
                "polymarket data_api_host must not be empty",
            ));
        }

        Ok(Self {
            data_api_host,
            client: reqwest::Client::new(),
        })
    }

    /// Fetch a wallet's most recent trade activity (newest first).
    pub async fn fetch_wallet_activity(
        &self,
        address: &str,
        limit: u16,
    ) -> Result<Vec<PolymarketWalletActivity>> {
        let address = normalize_data_api_address(address)?;
        let mut url = reqwest::Url::parse(&format!("{}/activity", self.data_api_host)).map_err(
            |error| {
                AppError::invalid_input(
                    "POLYMARKET_DATA_API_URL_INVALID",
                    format!("failed to construct Polymarket activity URL: {error}"),
                )
            },
        )?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("user", &address);
            query.append_pair("limit", &limit.clamp(1, MAX_DATA_API_LIMIT).to_string());
            query.append_pair("offset", "0");
            query.append_pair("sortBy", "TIMESTAMP");
            query.append_pair("sortDirection", "DESC");
        }

        let raws = self
            .fetch_json::<Vec<RawWalletActivity>>(url, "activity", &address)
            .await?;
        Ok(raws.into_iter().filter_map(map_wallet_activity).collect())
    }

    /// Fetch a wallet's current open positions.
    pub async fn fetch_wallet_positions(
        &self,
        address: &str,
    ) -> Result<Vec<PolymarketWalletPosition>> {
        let address = normalize_data_api_address(address)?;
        let mut url = reqwest::Url::parse(&format!("{}/positions", self.data_api_host)).map_err(
            |error| {
                AppError::invalid_input(
                    "POLYMARKET_DATA_API_URL_INVALID",
                    format!("failed to construct Polymarket positions URL: {error}"),
                )
            },
        )?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("user", &address);
            query.append_pair("sizeThreshold", "0.1");
            query.append_pair("limit", "500");
        }

        let raws = self
            .fetch_json::<Vec<RawWalletPosition>>(url, "positions", &address)
            .await?;
        Ok(raws.into_iter().filter_map(map_wallet_position).collect())
    }

    async fn fetch_json<T: serde::de::DeserializeOwned>(
        &self,
        url: reqwest::Url,
        resource: &str,
        address: &str,
    ) -> Result<T> {
        let response = self.client.get(url).send().await.map_err(|error| {
            AppError::dependency_unavailable(
                "POLYMARKET_DATA_API_REQUEST_FAILED",
                format!("failed to request Polymarket {resource} for {address}: {error}"),
            )
        })?;
        let status = response.status();
        if !status.is_success() {
            return Err(AppError::dependency_unavailable(
                "POLYMARKET_DATA_API_STATUS_FAILED",
                format!("Polymarket {resource} for {address} returned HTTP {status}"),
            ));
        }

        response.json::<T>().await.map_err(|error| {
            AppError::dependency_unavailable(
                "POLYMARKET_DATA_API_DECODE_FAILED",
                format!("failed to decode Polymarket {resource} for {address}: {error}"),
            )
        })
    }
}

fn normalize_data_api_address(address: &str) -> Result<String> {
    let trimmed = address.trim().to_lowercase();
    let is_hex_address = trimmed.len() == 42
        && trimmed.starts_with("0x")
        && trimmed[2..].chars().all(|character| character.is_ascii_hexdigit());
    if !is_hex_address {
        return Err(AppError::invalid_input(
            "POLYMARKET_DATA_API_ADDRESS_INVALID",
            format!("wallet address must be a 0x-prefixed 40-hex string, got {address}"),
        ));
    }
    Ok(trimmed)
}

fn map_wallet_activity(raw: RawWalletActivity) -> Option<PolymarketWalletActivity> {
    let asset = normalize_optional_text(raw.asset)?;
    let condition_id = normalize_optional_text(raw.condition_id).unwrap_or_default();
    let timestamp = parse_data_api_timestamp(raw.timestamp.as_ref());

    Some(PolymarketWalletActivity {
        proxy_wallet: normalize_optional_text(raw.proxy_wallet).unwrap_or_default(),
        kind: normalize_optional_text(raw.kind)
            .unwrap_or_default()
            .to_uppercase(),
        side: normalize_optional_text(raw.side)
            .unwrap_or_default()
            .to_uppercase(),
        asset,
        condition_id,
        outcome: normalize_optional_text(raw.outcome).unwrap_or_default(),
        outcome_index: parse_data_api_i64(raw.outcome_index.as_ref()),
        title: normalize_optional_text(raw.title).unwrap_or_default(),
        slug: normalize_optional_text(raw.slug).unwrap_or_default(),
        transaction_hash: normalize_optional_text(raw.transaction_hash).unwrap_or_default(),
        price: parse_decimal_value(raw.price).unwrap_or(Decimal::ZERO),
        size: parse_decimal_value(raw.size).unwrap_or(Decimal::ZERO),
        usdc_size: parse_decimal_value(raw.usdc_size).unwrap_or(Decimal::ZERO),
        timestamp,
    })
}

fn map_wallet_position(raw: RawWalletPosition) -> Option<PolymarketWalletPosition> {
    let asset = normalize_optional_text(raw.asset)?;
    Some(PolymarketWalletPosition {
        asset,
        condition_id: normalize_optional_text(raw.condition_id).unwrap_or_default(),
        outcome: normalize_optional_text(raw.outcome).unwrap_or_default(),
        title: normalize_optional_text(raw.title).unwrap_or_default(),
        slug: normalize_optional_text(raw.slug).unwrap_or_default(),
        size: parse_decimal_value(raw.size).unwrap_or(Decimal::ZERO),
        avg_price: parse_decimal_value(raw.avg_price).unwrap_or(Decimal::ZERO),
        cur_price: parse_decimal_value(raw.cur_price).unwrap_or(Decimal::ZERO),
        realized_pnl: parse_decimal_value(raw.realized_pnl).unwrap_or(Decimal::ZERO),
        cash_pnl: parse_decimal_value(raw.cash_pnl).unwrap_or(Decimal::ZERO),
        percent_pnl: parse_decimal_value(raw.percent_pnl).unwrap_or(Decimal::ZERO),
    })
}

fn parse_data_api_timestamp(value: Option<&JsonValue>) -> OffsetDateTime {
    let seconds = parse_data_api_i64(value);
    OffsetDateTime::from_unix_timestamp(seconds).unwrap_or_else(|_| OffsetDateTime::now_utc())
}

fn parse_data_api_i64(value: Option<&JsonValue>) -> i64 {
    value
        .and_then(|value| match value {
            JsonValue::Number(number) => number.as_i64(),
            JsonValue::String(text) => text.trim().parse::<i64>().ok(),
            _ => None,
        })
        .unwrap_or(0)
}
