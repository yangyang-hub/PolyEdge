// Polymarket public Data API connector. V3 uses only wallet-scoped current
// positions for risk reconciliation; this module must not expose discovery,
// leaderboard, profile, activity, trade-history, or closed-position APIs.

const MAX_DATA_API_LIMIT: u16 = 500;
const MAX_DATA_API_POSITION_PAGES: usize = 100;
const DATA_API_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, PartialEq)]
pub struct PolymarketWalletPosition {
    pub token_id: String,
    pub quantity: Decimal,
    pub average_price: Decimal,
    pub realized_pnl: Decimal,
}

#[derive(Debug, Clone)]
pub struct PolymarketDataApiConnector {
    data_api_host: String,
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct RawWalletPosition {
    #[serde(default)]
    asset: Option<String>,
    #[serde(default)]
    size: Option<JsonValue>,
    #[serde(rename = "avgPrice", default)]
    average_price: Option<JsonValue>,
    #[serde(rename = "realizedPnl", default)]
    realized_pnl: Option<JsonValue>,
}

impl PolymarketDataApiConnector {
    pub fn new(data_api_host: &str) -> Result<Self> {
        let data_api_host = data_api_host.trim().trim_end_matches('/').to_string();
        if data_api_host.is_empty() {
            return Err(AppError::invalid_input(
                "POLYMARKET_DATA_API_HOST_REQUIRED",
                "Polymarket data API host must not be empty",
            ));
        }
        reqwest::Url::parse(&data_api_host).map_err(|error| {
            AppError::invalid_input(
                "POLYMARKET_DATA_API_HOST_INVALID",
                format!("Polymarket data API host is invalid: {error}"),
            )
        })?;
        let client = reqwest::Client::builder()
            .timeout(DATA_API_TIMEOUT)
            .build()
            .map_err(|error| {
                AppError::internal(
                    "POLYMARKET_DATA_API_CLIENT_BUILD_FAILED",
                    format!("failed to build Polymarket data API client: {error}"),
                )
            })?;
        Ok(Self {
            data_api_host,
            client,
        })
    }

    /// Fetch current positions for one explicitly configured wallet/funder.
    /// The server is responsible for retaining only database-known token IDs.
    pub async fn fetch_wallet_positions(
        &self,
        address: &str,
    ) -> Result<Vec<PolymarketWalletPosition>> {
        let address = normalize_data_api_address(address)?;
        let mut positions = Vec::new();
        let mut seen_tokens = std::collections::HashSet::new();
        let mut offset = 0u32;

        for _ in 0..MAX_DATA_API_POSITION_PAGES {
            let url = self.positions_url(&address, offset)?;
            let raws = self.fetch_positions_page(url, &address).await?;
            let raw_count = raws.len();
            for raw in raws {
                let position = map_wallet_position(raw)?;
                if !seen_tokens.insert(position.token_id.clone()) {
                    return Err(AppError::dependency_unavailable(
                        "POLYMARKET_DATA_API_POSITION_DUPLICATE",
                        "Polymarket positions response contains a duplicate token",
                    ));
                }
                positions.push(position);
            }
            if raw_count < usize::from(MAX_DATA_API_LIMIT) {
                return Ok(positions);
            }
            offset = offset.saturating_add(raw_count as u32);
        }

        Err(AppError::dependency_unavailable(
            "POLYMARKET_DATA_API_POSITION_MAX_PAGES_EXCEEDED",
            "Polymarket positions response exceeded the bounded page limit",
        ))
    }

    fn positions_url(&self, address: &str, offset: u32) -> Result<reqwest::Url> {
        let mut url = reqwest::Url::parse(&format!("{}/positions", self.data_api_host)).map_err(
            |error| {
                AppError::invalid_input(
                    "POLYMARKET_DATA_API_URL_INVALID",
                    format!("failed to construct Polymarket positions URL: {error}"),
                )
            },
        )?;
        let mut query = url.query_pairs_mut();
        query.append_pair("user", address);
        query.append_pair("sizeThreshold", "0");
        query.append_pair("limit", &MAX_DATA_API_LIMIT.to_string());
        query.append_pair("offset", &offset.to_string());
        drop(query);
        Ok(url)
    }

    async fn fetch_positions_page(
        &self,
        url: reqwest::Url,
        address: &str,
    ) -> Result<Vec<RawWalletPosition>> {
        let response = self.client.get(url).send().await.map_err(|error| {
            AppError::dependency_unavailable(
                "POLYMARKET_DATA_API_REQUEST_FAILED",
                format!("failed to request Polymarket positions for {address}: {error}"),
            )
        })?;
        let status = response.status();
        if !status.is_success() {
            return Err(AppError::dependency_unavailable(
                "POLYMARKET_DATA_API_STATUS_FAILED",
                format!("Polymarket positions for {address} returned HTTP {status}"),
            ));
        }
        response.json().await.map_err(|error| {
            AppError::dependency_unavailable(
                "POLYMARKET_DATA_API_DECODE_FAILED",
                format!("failed to decode Polymarket positions for {address}: {error}"),
            )
        })
    }
}

fn normalize_data_api_address(address: &str) -> Result<String> {
    let trimmed = address.trim().to_ascii_lowercase();
    let valid = trimmed.len() == 42
        && trimmed.starts_with("0x")
        && trimmed[2..]
            .chars()
            .all(|character| character.is_ascii_hexdigit());
    if !valid {
        return Err(AppError::invalid_input(
            "POLYMARKET_DATA_API_ADDRESS_INVALID",
            "wallet address must be a 0x-prefixed 40-hex string",
        ));
    }
    Ok(trimmed)
}

fn map_wallet_position(raw: RawWalletPosition) -> Result<PolymarketWalletPosition> {
    let token_id = normalize_optional_text(raw.asset).ok_or_else(|| {
        AppError::dependency_unavailable(
            "POLYMARKET_DATA_API_POSITION_TOKEN_MISSING",
            "Polymarket position is missing its token ID",
        )
    })?;
    let quantity = required_decimal(raw.size, "quantity")?;
    let average_price = required_decimal(raw.average_price, "average price")?;
    let realized_pnl = optional_decimal(raw.realized_pnl, "realized PnL")?;
    if quantity < Decimal::ZERO || average_price < Decimal::ZERO || average_price >= Decimal::ONE {
        return Err(AppError::dependency_unavailable(
            "POLYMARKET_DATA_API_POSITION_INVALID",
            "Polymarket position contains an invalid quantity or average price",
        ));
    }
    Ok(PolymarketWalletPosition {
        token_id,
        quantity,
        average_price,
        realized_pnl,
    })
}

fn required_decimal(value: Option<JsonValue>, field: &str) -> Result<Decimal> {
    parse_decimal_value(value).ok_or_else(|| {
        AppError::dependency_unavailable(
            "POLYMARKET_DATA_API_POSITION_DECIMAL_INVALID",
            format!("Polymarket position {field} is missing or invalid"),
        )
    })
}

fn optional_decimal(value: Option<JsonValue>, field: &str) -> Result<Decimal> {
    match value {
        Some(value) => parse_decimal_value(Some(value)).ok_or_else(|| {
            AppError::dependency_unavailable(
                "POLYMARKET_DATA_API_POSITION_DECIMAL_INVALID",
                format!("Polymarket position {field} is invalid"),
            )
        }),
        None => Ok(Decimal::ZERO),
    }
}

#[cfg(test)]
mod data_api_tests {
    use super::*;

    #[test]
    fn rejects_invalid_wallet_address() {
        assert!(normalize_data_api_address("0x1234").is_err());
    }

    #[test]
    fn maps_minimal_position() {
        let raw = RawWalletPosition {
            asset: Some("token-1".to_string()),
            size: Some(JsonValue::String("4.5".to_string())),
            average_price: Some(JsonValue::String("0.42".to_string())),
            realized_pnl: None,
        };
        let position = map_wallet_position(raw).expect("valid position");
        assert_eq!(position.token_id, "token-1");
        assert_eq!(position.quantity, Decimal::new(45, 1));
        assert_eq!(position.average_price, Decimal::new(42, 2));
    }
}
