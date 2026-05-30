use serde::Deserialize;
use serde_json::Value as JsonValue;
use time::format_description::well_known::Rfc3339;

const GAMMA_MARKETS_PATH: &str = "markets/keyset";
const MAX_GAMMA_MARKET_PAGES: usize = 5;
const GAMMA_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
pub struct PolymarketGammaMarket {
    pub id: String,
    pub slug: Option<String>,
    pub question: String,
    pub category: String,
    pub status: MarketStatus,
    pub best_bid: Probability,
    pub best_ask: Probability,
    pub mid_price: Probability,
    pub volume_24h: UsdAmount,
    pub ambiguity_level: AmbiguityLevel,
    pub tradability_status: TradabilityStatus,
    pub resolution_source: String,
    pub edge_case_notes: Vec<String>,
    pub condition_id: String,
    pub yes_asset_id: String,
    pub no_asset_id: String,
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone)]
pub struct PolymarketGammaConnector {
    gamma_host: String,
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct GammaMarketPage {
    markets: Vec<RawGammaMarket>,
    next_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawGammaMarket {
    id: String,
    #[serde(default)]
    slug: Option<String>,
    question: Option<String>,
    category: Option<String>,
    #[serde(rename = "conditionId")]
    condition_id: Option<String>,
    #[serde(rename = "resolutionSource")]
    resolution_source: Option<String>,
    description: Option<String>,
    #[serde(default)]
    active: bool,
    #[serde(default)]
    closed: bool,
    #[serde(default)]
    archived: bool,
    #[serde(rename = "enableOrderBook", default)]
    enable_order_book: bool,
    #[serde(rename = "acceptingOrders", default)]
    accepting_orders: bool,
    #[serde(rename = "bestBid", default)]
    best_bid: Option<JsonValue>,
    #[serde(rename = "bestAsk", default)]
    best_ask: Option<JsonValue>,
    #[serde(rename = "lastTradePrice", default)]
    last_trade_price: Option<JsonValue>,
    #[serde(rename = "outcomePrices", default)]
    outcome_prices: Option<JsonValue>,
    #[serde(default)]
    outcomes: Option<JsonValue>,
    #[serde(rename = "clobTokenIds", default)]
    clob_token_ids: Option<JsonValue>,
    #[serde(rename = "volume24hrClob", default)]
    volume_24h_clob: Option<JsonValue>,
    #[serde(rename = "volume24hr", default)]
    volume_24h: Option<JsonValue>,
    #[serde(default)]
    volume: Option<JsonValue>,
    #[serde(rename = "updatedAt")]
    updated_at: Option<String>,
    #[serde(rename = "endDate")]
    end_date: Option<String>,
    #[serde(default)]
    events: Vec<RawGammaEvent>,
}

#[derive(Debug, Deserialize)]
struct RawGammaEvent {
    title: Option<String>,
    #[serde(rename = "resolutionSource")]
    resolution_source: Option<String>,
}

impl PolymarketGammaConnector {
    pub fn new(gamma_host: &str) -> Result<Self> {
        let gamma_host = gamma_host.trim().trim_end_matches('/').to_string();
        if gamma_host.is_empty() {
            return Err(AppError::invalid_input(
                "POLYMARKET_GAMMA_HOST_REQUIRED",
                "polymarket gamma_host must not be empty",
            ));
        }

        Ok(Self {
            gamma_host,
            client: reqwest::Client::builder()
                .timeout(GAMMA_TIMEOUT)
                .build()
                .map_err(|error| {
                    AppError::internal(
                        "POLYMARKET_GAMMA_CLIENT_BUILD_FAILED",
                        format!("failed to build Polymarket gamma HTTP client: {error}"),
                    )
                })?,
        })
    }

    pub async fn fetch_markets(&self, limit: u16) -> Result<Vec<PolymarketGammaMarket>> {
        let target_len = usize::from(limit.max(1));
        let mut cursor: Option<String> = None;
        let mut markets = Vec::with_capacity(target_len);

        for _ in 0..MAX_GAMMA_MARKET_PAGES {
            let page = self.fetch_market_page(limit, cursor.as_deref()).await?;
            for raw in page.markets {
                if let Some(market) = map_gamma_market(raw)? {
                    markets.push(market);
                    if markets.len() >= target_len {
                        return Ok(markets);
                    }
                }
            }

            let next_cursor = page.next_cursor.unwrap_or_default();
            if next_cursor.trim().is_empty() {
                break;
            }
            cursor = Some(next_cursor);
        }

        Ok(markets)
    }

    pub async fn fetch_market(&self, market_id: &str) -> Result<Option<PolymarketGammaMarket>> {
        let market_id = market_id.trim();
        if market_id.is_empty() {
            return Err(AppError::invalid_input(
                "POLYMARKET_GAMMA_MARKET_ID_REQUIRED",
                "market_id must not be empty",
            ));
        }

        let url = reqwest::Url::parse(&format!("{}/markets/{market_id}", self.gamma_host))
            .map_err(|error| {
                AppError::invalid_input(
                    "POLYMARKET_GAMMA_URL_INVALID",
                    format!("failed to construct Polymarket Gamma market URL: {error}"),
                )
            })?;
        let response = self.client.get(url).send().await.map_err(|error| {
            AppError::dependency_unavailable(
                "POLYMARKET_GAMMA_MARKET_REQUEST_FAILED",
                format!("failed to request Polymarket Gamma market {market_id}: {error}"),
            )
        })?;

        if response.status().as_u16() == 404 {
            return Ok(None);
        }

        let status = response.status();
        if !status.is_success() {
            return Err(AppError::dependency_unavailable(
                "POLYMARKET_GAMMA_MARKET_STATUS_FAILED",
                format!("Polymarket Gamma market {market_id} returned HTTP {status}"),
            ));
        }

        let raw = response.json::<RawGammaMarket>().await.map_err(|error| {
            AppError::dependency_unavailable(
                "POLYMARKET_GAMMA_MARKET_DECODE_FAILED",
                format!("failed to decode Polymarket Gamma market {market_id}: {error}"),
            )
        })?;

        map_gamma_market(raw)
    }

    async fn fetch_market_page(
        &self,
        limit: u16,
        cursor: Option<&str>,
    ) -> Result<GammaMarketPage> {
        let mut url =
            reqwest::Url::parse(&format!("{}/{}", self.gamma_host, GAMMA_MARKETS_PATH)).map_err(
                |error| {
                    AppError::invalid_input(
                        "POLYMARKET_GAMMA_MARKETS_URL_INVALID",
                        format!("failed to construct Polymarket Gamma markets URL: {error}"),
                    )
                },
            )?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("active", "true");
            query.append_pair("closed", "false");
            query.append_pair("archived", "false");
            query.append_pair("order", "volume24hr");
            query.append_pair("ascending", "false");
            query.append_pair("limit", &limit.max(1).to_string());
            if let Some(cursor) = cursor {
                query.append_pair("next_cursor", cursor);
            }
        }

        let response = self.client.get(url).send().await.map_err(|error| {
            AppError::dependency_unavailable(
                "POLYMARKET_GAMMA_MARKETS_REQUEST_FAILED",
                format!("failed to request Polymarket Gamma markets: {error}"),
            )
        })?;
        let status = response.status();
        if !status.is_success() {
            return Err(AppError::dependency_unavailable(
                "POLYMARKET_GAMMA_MARKETS_STATUS_FAILED",
                format!("Polymarket Gamma markets returned HTTP {status}"),
            ));
        }

        response.json::<GammaMarketPage>().await.map_err(|error| {
            AppError::dependency_unavailable(
                "POLYMARKET_GAMMA_MARKETS_DECODE_FAILED",
                format!("failed to decode Polymarket Gamma markets: {error}"),
            )
        })
    }
}

fn map_gamma_market(raw: RawGammaMarket) -> Result<Option<PolymarketGammaMarket>> {
    let condition_id = normalize_optional_text(raw.condition_id.clone());
    let question = normalize_optional_text(raw.question.clone());
    let token_ids = parse_string_array(raw.clob_token_ids.clone());

    let (Some(condition_id), Some(question)) = (condition_id, question) else {
        return Ok(None);
    };
    if token_ids.len() < 2 {
        return Ok(None);
    };
    let resolution_source = resolution_source(&raw);
    let edge_case_notes = edge_case_notes(&raw, &condition_id);

    let outcome_prices = parse_decimal_array(raw.outcome_prices.clone());
    let outcomes = parse_string_array(raw.outcomes.clone());
    let yes_index = outcomes
        .iter()
        .position(|outcome| outcome.eq_ignore_ascii_case("yes"))
        .unwrap_or(0);
    let yes_price = outcome_prices
        .get(yes_index)
        .copied()
        .or_else(|| parse_decimal_value(raw.last_trade_price.clone()));
    let best_bid_decimal = parse_decimal_value(raw.best_bid.clone()).or(yes_price);
    let best_ask_decimal = parse_decimal_value(raw.best_ask.clone()).or(yes_price);
    let (best_bid, best_ask) = normalize_bid_ask(best_bid_decimal, best_ask_decimal)?;
    let mid_price = Probability::new((best_bid.value() + best_ask.value()) / Decimal::from(2))?;
    let volume_24h = UsdAmount::new(
        parse_decimal_value(raw.volume_24h_clob.clone())
            .or_else(|| parse_decimal_value(raw.volume_24h.clone()))
            .or_else(|| parse_decimal_value(raw.volume.clone()))
            .unwrap_or(Decimal::ZERO)
            .max(Decimal::ZERO),
    )?;
    let updated_at = parse_rfc3339(raw.updated_at.as_deref()).unwrap_or_else(OffsetDateTime::now_utc);
    let status = if raw.closed {
        MarketStatus::Closed
    } else {
        MarketStatus::Open
    };
    let tradability_status = if !raw.active || raw.archived {
        TradabilityStatus::Blocked
    } else if raw.enable_order_book && raw.accepting_orders {
        TradabilityStatus::Tradable
    } else {
        TradabilityStatus::ObserveOnly
    };
    let ambiguity_level = if resolution_source.trim().is_empty() {
        AmbiguityLevel::Medium
    } else {
        AmbiguityLevel::Low
    };
    let version = updated_at.unix_timestamp().max(1);

    Ok(Some(PolymarketGammaMarket {
        id: raw.id,
        slug: normalize_optional_text(raw.slug),
        question,
        category: normalize_optional_text(raw.category.clone())
            .unwrap_or_else(|| "Polymarket".to_string()),
        status,
        best_bid,
        best_ask,
        mid_price,
        volume_24h,
        ambiguity_level,
        tradability_status,
        resolution_source,
        edge_case_notes,
        condition_id,
        yes_asset_id: token_ids[0].clone(),
        no_asset_id: token_ids[1].clone(),
        updated_at,
        version,
    }))
}

fn normalize_bid_ask(
    best_bid: Option<Decimal>,
    best_ask: Option<Decimal>,
) -> Result<(Probability, Probability)> {
    let bid = clamp_probability(best_bid.unwrap_or_else(default_probability));
    let ask = clamp_probability(best_ask.unwrap_or(bid));
    let normalized_ask = ask.max(bid);

    Ok((Probability::new(bid)?, Probability::new(normalized_ask)?))
}

fn default_probability() -> Decimal {
    Decimal::from_str("0.5").expect("static probability must be valid")
}

fn clamp_probability(value: Decimal) -> Decimal {
    value.clamp(Decimal::ZERO, Decimal::ONE)
}

fn parse_string_array(value: Option<JsonValue>) -> Vec<String> {
    parse_json_array(value)
        .into_iter()
        .filter_map(|value| match value {
            JsonValue::String(text) => normalize_optional_text(Some(text)),
            JsonValue::Number(number) => Some(number.to_string()),
            _ => None,
        })
        .collect()
}

fn parse_decimal_array(value: Option<JsonValue>) -> Vec<Decimal> {
    parse_json_array(value)
        .into_iter()
        .filter_map(|value| parse_decimal_value(Some(value)))
        .collect()
}

fn parse_json_array(value: Option<JsonValue>) -> Vec<JsonValue> {
    match value {
        Some(JsonValue::Array(items)) => items,
        Some(JsonValue::String(raw)) => {
            serde_json::from_str::<Vec<JsonValue>>(raw.trim()).unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

fn parse_decimal_value(value: Option<JsonValue>) -> Option<Decimal> {
    match value? {
        JsonValue::Number(number) => Decimal::from_str(&number.to_string()).ok(),
        JsonValue::String(raw) => Decimal::from_str(raw.trim()).ok(),
        _ => None,
    }
}

fn parse_rfc3339(value: Option<&str>) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(value?.trim(), &Rfc3339).ok()
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    let normalized = value?.trim().to_string();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn resolution_source(raw: &RawGammaMarket) -> String {
    normalize_optional_text(raw.resolution_source.clone())
        .or_else(|| {
            raw.events
                .iter()
                .find_map(|event| normalize_optional_text(event.resolution_source.clone()))
        })
        .or_else(|| normalize_optional_text(raw.description.clone()))
        .unwrap_or_else(|| "Polymarket Gamma market metadata.".to_string())
}

fn edge_case_notes(raw: &RawGammaMarket, condition_id: &str) -> Vec<String> {
    let mut notes = vec![format!("Polymarket condition id: {condition_id}")];
    if let Some(end_date) = normalize_optional_text(raw.end_date.clone()) {
        notes.push(format!("Market end date: {end_date}"));
    }
    if let Some(title) = raw
        .events
        .iter()
        .find_map(|event| normalize_optional_text(event.title.clone()))
    {
        notes.push(format!("Event: {title}"));
    }
    notes
}
