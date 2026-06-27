// Polymarket public Data API connector (data-api.polymarket.com): per-wallet
// trade activity and current positions. Used by the copy-trading subsystem to
// analyze tracked wallets and detect their new trades. No authentication: these
// endpoints are public. This file is `include!`d by `polymarket.rs`; it relies
// on that module's shared imports plus the `Deserialize`/`JsonValue`/
// `parse_decimal_value`/`normalize_optional_text` items brought in by `gamma.rs`.

const MAX_DATA_API_LIMIT: u16 = 500;
const MAX_DATA_API_POSITION_PAGES: usize = 100;
const DATA_API_TIMEOUT: Duration = Duration::from_secs(15);

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

/// Closed (settled) position from the Data API `/closed-positions` endpoint.
#[derive(Debug, Clone)]
pub struct PolymarketClosedPosition {
    pub proxy_wallet: String,
    pub asset: String,
    pub condition_id: String,
    pub avg_price: Decimal,
    pub total_bought: Decimal,
    pub realized_pnl: Decimal,
    pub cur_price: Decimal,
    pub timestamp: OffsetDateTime,
    pub title: String,
    pub slug: String,
    pub outcome: String,
    pub outcome_index: i64,
    pub opposite_outcome: String,
    pub end_date: String,
}

/// Trade record from the Data API `/trades` endpoint.
#[derive(Debug, Clone)]
pub struct PolymarketTrade {
    pub proxy_wallet: String,
    pub side: String,
    pub asset: String,
    pub condition_id: String,
    pub size: Decimal,
    pub price: Decimal,
    pub timestamp: OffsetDateTime,
    pub title: String,
    pub slug: String,
    pub outcome: String,
    pub outcome_index: i64,
    pub transaction_hash: String,
}

/// Leaderboard entry from `/v1/leaderboard`.
#[derive(Debug, Clone)]
pub struct PolymarketLeaderboardEntry {
    pub rank: i64,
    pub proxy_wallet: String,
    pub user_name: String,
    pub vol: Decimal,
    pub pnl: Decimal,
    pub profile_image: String,
    pub x_username: String,
    pub verified_badge: bool,
}

/// Public profile from the Gamma API `/public-profile`.
#[derive(Debug, Clone)]
pub struct PolymarketPublicProfile {
    pub name: String,
    pub pseudonym: String,
    pub bio: String,
    pub x_username: String,
    pub profile_image: String,
    pub created_at: String,
    pub verified_badge: bool,
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

#[derive(Debug, Deserialize)]
struct RawClosedPosition {
    #[serde(rename = "proxyWallet", default)]
    proxy_wallet: Option<String>,
    #[serde(default)]
    asset: Option<String>,
    #[serde(rename = "conditionId", default)]
    condition_id: Option<String>,
    #[serde(rename = "avgPrice", default)]
    avg_price: Option<JsonValue>,
    #[serde(rename = "totalBought", default)]
    total_bought: Option<JsonValue>,
    #[serde(rename = "realizedPnl", default)]
    realized_pnl: Option<JsonValue>,
    #[serde(rename = "curPrice", default)]
    cur_price: Option<JsonValue>,
    #[serde(default)]
    timestamp: Option<JsonValue>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    slug: Option<String>,
    #[serde(default)]
    outcome: Option<String>,
    #[serde(rename = "outcomeIndex", default)]
    outcome_index: Option<JsonValue>,
    #[serde(rename = "oppositeOutcome", default)]
    opposite_outcome: Option<String>,
    #[serde(rename = "endDate", default)]
    end_date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawTrade {
    #[serde(rename = "proxyWallet", default)]
    proxy_wallet: Option<String>,
    #[serde(default)]
    side: Option<String>,
    #[serde(default)]
    asset: Option<String>,
    #[serde(rename = "conditionId", default)]
    condition_id: Option<String>,
    #[serde(default)]
    size: Option<JsonValue>,
    #[serde(default)]
    price: Option<JsonValue>,
    #[serde(default)]
    timestamp: Option<JsonValue>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    slug: Option<String>,
    #[serde(default)]
    outcome: Option<String>,
    #[serde(rename = "outcomeIndex", default)]
    outcome_index: Option<JsonValue>,
    #[serde(rename = "transactionHash", default)]
    transaction_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct RawPortfolioValue {
    #[serde(default)]
    user: Option<String>,
    #[serde(default)]
    value: Option<JsonValue>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct RawTraded {
    #[serde(default)]
    user: Option<String>,
    #[serde(default)]
    traded: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RawLeaderboardEntry {
    #[serde(default)]
    rank: Option<JsonValue>,
    #[serde(rename = "proxyWallet", default)]
    proxy_wallet: Option<String>,
    #[serde(rename = "userName", default)]
    user_name: Option<String>,
    #[serde(default)]
    vol: Option<JsonValue>,
    #[serde(default)]
    pnl: Option<JsonValue>,
    #[serde(rename = "profileImage", default)]
    profile_image: Option<String>,
    #[serde(rename = "xUsername", default)]
    x_username: Option<String>,
    #[serde(rename = "verifiedBadge", default)]
    verified_badge: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct RawPublicProfile {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    pseudonym: Option<String>,
    #[serde(default)]
    bio: Option<String>,
    #[serde(rename = "xUsername", default)]
    x_username: Option<String>,
    #[serde(rename = "profileImage", default)]
    profile_image: Option<String>,
    #[serde(rename = "createdAt", default)]
    created_at: Option<String>,
    #[serde(rename = "verifiedBadge", default)]
    verified_badge: Option<bool>,
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
            client: reqwest::Client::builder()
                .timeout(DATA_API_TIMEOUT)
                .build()
                .map_err(|error| {
                    AppError::internal(
                        "POLYMARKET_DATA_API_CLIENT_BUILD_FAILED",
                        format!("failed to build Polymarket data API HTTP client: {error}"),
                    )
                })?,
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
        let mut positions = Vec::new();
        let mut seen_assets = std::collections::HashSet::new();
        let mut offset = 0u32;

        for _ in 0..MAX_DATA_API_POSITION_PAGES {
            let mut url =
                reqwest::Url::parse(&format!("{}/positions", self.data_api_host)).map_err(
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
                query.append_pair("sizeThreshold", "0");
                query.append_pair("limit", &MAX_DATA_API_LIMIT.to_string());
                query.append_pair("offset", &offset.to_string());
            }

            let raws = self
                .fetch_json::<Vec<RawWalletPosition>>(url, "positions", &address)
                .await?;
            let raw_count = raws.len();
            positions.extend(
                raws.into_iter()
                    .filter_map(map_wallet_position)
                    .filter(|position| seen_assets.insert(position.asset.clone())),
            );
            if raw_count < usize::from(MAX_DATA_API_LIMIT) {
                return Ok(positions);
            }
            offset = offset.saturating_add(raw_count as u32);
        }

        Err(AppError::dependency_unavailable(
            "POLYMARKET_DATA_API_POSITION_MAX_PAGES_EXCEEDED",
            format!(
                "Polymarket positions for {address} exceeded {MAX_DATA_API_POSITION_PAGES} pages"
            ),
        ))
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

    /// Like `fetch_json` but returns `None` on 404 instead of an error.
    async fn fetch_json_optional<T: serde::de::DeserializeOwned>(
        &self,
        url: reqwest::Url,
        resource: &str,
        address: &str,
    ) -> Result<Option<T>> {
        let response = self.client.get(url).send().await.map_err(|error| {
            AppError::dependency_unavailable(
                "POLYMARKET_DATA_API_REQUEST_FAILED",
                format!("failed to request Polymarket {resource} for {address}: {error}"),
            )
        })?;
        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !status.is_success() {
            return Err(AppError::dependency_unavailable(
                "POLYMARKET_DATA_API_STATUS_FAILED",
                format!("Polymarket {resource} for {address} returned HTTP {status}"),
            ));
        }

        Ok(Some(response.json::<T>().await.map_err(|error| {
            AppError::dependency_unavailable(
                "POLYMARKET_DATA_API_DECODE_FAILED",
                format!("failed to decode Polymarket {resource} for {address}: {error}"),
            )
        })?))
    }

    /// Fetch a wallet's closed (settled) positions, sorted by realized P&L descending.
    pub async fn fetch_closed_positions(
        &self,
        address: &str,
        limit: u16,
        offset: u32,
    ) -> Result<Vec<PolymarketClosedPosition>> {
        let address = normalize_data_api_address(address)?;
        let mut url = reqwest::Url::parse(&format!(
            "{}/closed-positions",
            self.data_api_host
        ))
        .map_err(|error| {
            AppError::invalid_input(
                "POLYMARKET_DATA_API_URL_INVALID",
                format!("failed to construct Polymarket closed-positions URL: {error}"),
            )
        })?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("user", &address);
            query.append_pair("limit", &limit.clamp(1, 50).to_string());
            query.append_pair("offset", &offset.to_string());
            query.append_pair("sortBy", "REALIZEDPNL");
            query.append_pair("sortDirection", "DESC");
        }

        let raws = self
            .fetch_json::<Vec<RawClosedPosition>>(url, "closed-positions", &address)
            .await?;
        Ok(raws.into_iter().filter_map(map_closed_position).collect())
    }

    /// Fetch a wallet's trade history (up to 10,000 trades).
    pub async fn fetch_trades(
        &self,
        address: &str,
        limit: u16,
        offset: u32,
    ) -> Result<Vec<PolymarketTrade>> {
        let address = normalize_data_api_address(address)?;
        let mut url =
            reqwest::Url::parse(&format!("{}/trades", self.data_api_host)).map_err(|error| {
                AppError::invalid_input(
                    "POLYMARKET_DATA_API_URL_INVALID",
                    format!("failed to construct Polymarket trades URL: {error}"),
                )
            })?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("user", &address);
            query.append_pair("limit", &limit.clamp(1, 1000).to_string());
            query.append_pair("offset", &offset.to_string());
            query.append_pair("takerOnly", "false");
        }

        let raws = self
            .fetch_json::<Vec<RawTrade>>(url, "trades", &address)
            .await?;
        Ok(raws.into_iter().filter_map(map_trade).collect())
    }

    /// Fetch the total value of a wallet's current positions.
    pub async fn fetch_portfolio_value(&self, address: &str) -> Result<Decimal> {
        let address = normalize_data_api_address(address)?;
        let mut url =
            reqwest::Url::parse(&format!("{}/value", self.data_api_host)).map_err(|error| {
                AppError::invalid_input(
                    "POLYMARKET_DATA_API_URL_INVALID",
                    format!("failed to construct Polymarket value URL: {error}"),
                )
            })?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("user", &address);
        }

        let raws = self
            .fetch_json::<Vec<RawPortfolioValue>>(url, "value", &address)
            .await?;
        Ok(raws
            .into_iter()
            .next()
            .and_then(|v| parse_decimal_value(v.value))
            .unwrap_or(Decimal::ZERO))
    }

    /// Fetch the total number of distinct markets a wallet has traded.
    pub async fn fetch_total_markets_traded(&self, address: &str) -> Result<i64> {
        let address = normalize_data_api_address(address)?;
        let mut url =
            reqwest::Url::parse(&format!("{}/traded", self.data_api_host)).map_err(|error| {
                AppError::invalid_input(
                    "POLYMARKET_DATA_API_URL_INVALID",
                    format!("failed to construct Polymarket traded URL: {error}"),
                )
            })?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("user", &address);
        }

        let raw = self
            .fetch_json::<RawTraded>(url, "traded", &address)
            .await?;
        Ok(raw.traded.unwrap_or(0))
    }

    /// Fetch a wallet's leaderboard entry (rank, volume, P&L).
    pub async fn fetch_leaderboard_entry(
        &self,
        address: &str,
    ) -> Result<Option<PolymarketLeaderboardEntry>> {
        let address = normalize_data_api_address(address)?;
        let mut url = reqwest::Url::parse(&format!(
            "{}/v1/leaderboard",
            self.data_api_host
        ))
        .map_err(|error| {
            AppError::invalid_input(
                "POLYMARKET_DATA_API_URL_INVALID",
                format!("failed to construct Polymarket leaderboard URL: {error}"),
            )
        })?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("user", &address);
            query.append_pair("category", "OVERALL");
            query.append_pair("timePeriod", "ALL");
            query.append_pair("limit", "1");
        }

        let raws = self
            .fetch_json::<Vec<RawLeaderboardEntry>>(url, "leaderboard", &address)
            .await?;
        Ok(raws.into_iter().next().and_then(map_leaderboard_entry))
    }

    /// Fetch top leaderboard wallets for discovery.
    pub async fn fetch_leaderboard(
        &self,
        limit: u16,
        offset: u32,
    ) -> Result<Vec<PolymarketLeaderboardEntry>> {
        let mut url = reqwest::Url::parse(&format!(
            "{}/v1/leaderboard",
            self.data_api_host
        ))
        .map_err(|error| {
            AppError::invalid_input(
                "POLYMARKET_DATA_API_URL_INVALID",
                format!("failed to construct Polymarket leaderboard URL: {error}"),
            )
        })?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("category", "OVERALL");
            query.append_pair("timePeriod", "ALL");
            query.append_pair("limit", &limit.clamp(1, MAX_DATA_API_LIMIT).to_string());
            query.append_pair("offset", &offset.to_string());
        }

        let raws = self
            .fetch_json::<Vec<RawLeaderboardEntry>>(url, "leaderboard", "global")
            .await?;
        Ok(raws.into_iter().filter_map(map_leaderboard_entry).collect())
    }

    /// Fetch a user's public profile from the Gamma API.
    pub async fn fetch_public_profile(
        &self,
        gamma_host: &str,
        address: &str,
    ) -> Result<Option<PolymarketPublicProfile>> {
        let address = normalize_data_api_address(address)?;
        let gamma_host = gamma_host.trim().trim_end_matches('/');
        let mut url = reqwest::Url::parse(&format!("{gamma_host}/public-profile")).map_err(
            |error| {
                AppError::invalid_input(
                    "POLYMARKET_GAMMA_URL_INVALID",
                    format!("failed to construct Polymarket public-profile URL: {error}"),
                )
            },
        )?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("address", &address);
        }

        self.fetch_json_optional::<RawPublicProfile>(url, "public-profile", &address)
            .await
            .map(|opt| opt.and_then(map_public_profile))
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

fn map_closed_position(raw: RawClosedPosition) -> Option<PolymarketClosedPosition> {
    let asset = normalize_optional_text(raw.asset)?;
    Some(PolymarketClosedPosition {
        proxy_wallet: normalize_optional_text(raw.proxy_wallet).unwrap_or_default(),
        asset,
        condition_id: normalize_optional_text(raw.condition_id).unwrap_or_default(),
        avg_price: parse_decimal_value(raw.avg_price).unwrap_or(Decimal::ZERO),
        total_bought: parse_decimal_value(raw.total_bought).unwrap_or(Decimal::ZERO),
        realized_pnl: parse_decimal_value(raw.realized_pnl).unwrap_or(Decimal::ZERO),
        cur_price: parse_decimal_value(raw.cur_price).unwrap_or(Decimal::ZERO),
        timestamp: parse_data_api_timestamp(raw.timestamp.as_ref()),
        title: normalize_optional_text(raw.title).unwrap_or_default(),
        slug: normalize_optional_text(raw.slug).unwrap_or_default(),
        outcome: normalize_optional_text(raw.outcome).unwrap_or_default(),
        outcome_index: parse_data_api_i64(raw.outcome_index.as_ref()),
        opposite_outcome: normalize_optional_text(raw.opposite_outcome).unwrap_or_default(),
        end_date: normalize_optional_text(raw.end_date).unwrap_or_default(),
    })
}

fn map_trade(raw: RawTrade) -> Option<PolymarketTrade> {
    let asset = normalize_optional_text(raw.asset)?;
    Some(PolymarketTrade {
        proxy_wallet: normalize_optional_text(raw.proxy_wallet).unwrap_or_default(),
        side: normalize_optional_text(raw.side)
            .unwrap_or_default()
            .to_uppercase(),
        asset,
        condition_id: normalize_optional_text(raw.condition_id).unwrap_or_default(),
        size: parse_decimal_value(raw.size).unwrap_or(Decimal::ZERO),
        price: parse_decimal_value(raw.price).unwrap_or(Decimal::ZERO),
        timestamp: parse_data_api_timestamp(raw.timestamp.as_ref()),
        title: normalize_optional_text(raw.title).unwrap_or_default(),
        slug: normalize_optional_text(raw.slug).unwrap_or_default(),
        outcome: normalize_optional_text(raw.outcome).unwrap_or_default(),
        outcome_index: parse_data_api_i64(raw.outcome_index.as_ref()),
        transaction_hash: normalize_optional_text(raw.transaction_hash).unwrap_or_default(),
    })
}

fn map_leaderboard_entry(raw: RawLeaderboardEntry) -> Option<PolymarketLeaderboardEntry> {
    let proxy_wallet = normalize_data_api_address(&normalize_optional_text(raw.proxy_wallet)?).ok()?;
    Some(PolymarketLeaderboardEntry {
        rank: parse_data_api_i64(raw.rank.as_ref()),
        proxy_wallet,
        user_name: normalize_optional_text(raw.user_name).unwrap_or_default(),
        vol: parse_decimal_value(raw.vol).unwrap_or(Decimal::ZERO),
        pnl: parse_decimal_value(raw.pnl).unwrap_or(Decimal::ZERO),
        profile_image: normalize_optional_text(raw.profile_image).unwrap_or_default(),
        x_username: normalize_optional_text(raw.x_username).unwrap_or_default(),
        verified_badge: raw.verified_badge.unwrap_or(false),
    })
}

fn map_public_profile(raw: RawPublicProfile) -> Option<PolymarketPublicProfile> {
    // At least one identifying field should be present.
    let name = normalize_optional_text(raw.name.clone()).unwrap_or_default();
    let pseudonym = normalize_optional_text(raw.pseudonym.clone()).unwrap_or_default();
    if name.is_empty() && pseudonym.is_empty() {
        return None;
    }
    Some(PolymarketPublicProfile {
        name,
        pseudonym,
        bio: normalize_optional_text(raw.bio).unwrap_or_default(),
        x_username: normalize_optional_text(raw.x_username).unwrap_or_default(),
        profile_image: normalize_optional_text(raw.profile_image).unwrap_or_default(),
        created_at: normalize_optional_text(raw.created_at).unwrap_or_default(),
        verified_badge: raw.verified_badge.unwrap_or(false),
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
