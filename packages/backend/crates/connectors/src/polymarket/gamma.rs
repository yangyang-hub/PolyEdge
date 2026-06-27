use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value as JsonValue;
use time::format_description::well_known::Rfc3339;

const GAMMA_TIMEOUT: Duration = Duration::from_secs(15);
const GAMMA_MAX_PAGES: usize = 1_000;
const GAMMA_MAX_RETRIES: u32 = 3;
const GAMMA_RETRY_BASE_DELAY: Duration = Duration::from_millis(500);
const GAMMA_RATE_LIMIT_MAX_RETRIES: u32 = 5;
const GAMMA_RATE_LIMIT_BASE_DELAY: Duration = Duration::from_secs(2);
const GAMMA_CONDITION_BATCH_SIZE: usize = 50;
const RESPONSE_PREVIEW_BYTES: usize = 300;

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
    pub liquidity_usd: UsdAmount,
    pub start_at: Option<OffsetDateTime>,
    pub end_at: Option<OffsetDateTime>,
    pub event_start_at: Option<OffsetDateTime>,
    pub event_end_at: Option<OffsetDateTime>,
    pub has_reviewed_dates: bool,
    pub ambiguity_level: AmbiguityLevel,
    pub tradability_status: TradabilityStatus,
    pub resolution_source: String,
    pub edge_case_notes: Vec<String>,
    pub condition_id: String,
    pub yes_asset_id: String,
    pub no_asset_id: String,
    pub outcome_token_ids: Vec<String>,
    pub outcomes: Vec<String>,
    pub outcome_prices: Vec<Decimal>,
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone)]
pub struct PolymarketGammaConnector {
    gamma_host: String,
    client: reqwest::Client,
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
    #[serde(rename = "liquidityClob", default)]
    liquidity_clob: Option<JsonValue>,
    #[serde(default)]
    liquidity: Option<JsonValue>,
    #[serde(rename = "updatedAt")]
    updated_at: Option<String>,
    #[serde(rename = "startDate")]
    start_date: Option<String>,
    #[serde(rename = "startDateIso")]
    start_date_iso: Option<String>,
    #[serde(rename = "endDate")]
    end_date: Option<String>,
    #[serde(rename = "endDateIso")]
    end_date_iso: Option<String>,
    #[serde(rename = "hasReviewedDates", default)]
    has_reviewed_dates: bool,
    #[serde(default)]
    events: Vec<RawGammaEvent>,
}

#[derive(Debug, Deserialize)]
struct RawGammaEvent {
    title: Option<String>,
    #[serde(rename = "startDate")]
    start_date: Option<String>,
    #[serde(rename = "endDate")]
    end_date: Option<String>,
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

    pub async fn fetch_markets(&self, page_size: u16) -> Result<Vec<PolymarketGammaMarket>> {
        let mut markets = Vec::new();
        let mut market_ids = std::collections::HashSet::new();
        let limit = page_size.max(1);

        for page_index in 0..GAMMA_MAX_PAGES {
            let offset = (page_index as u64) * (limit as u64);
            let Some(page) = self.fetch_market_page_offset(limit, offset).await? else {
                tracing::info!(offset, "Gamma markets pagination boundary reached (422)");
                break;
            };
            let had_items = !page.is_empty();
            let page_len = page.len();

            for raw in page {
                if let Some(market) = map_gamma_market(raw)?
                    && market_ids.insert(market.id.clone())
                {
                    markets.push(market);
                }
            }

            if !had_items || page_len < limit as usize {
                break;
            }
        }

        Ok(markets)
    }

    pub async fn fetch_markets_by_condition_ids(
        &self,
        condition_ids: &[String],
    ) -> Result<Vec<PolymarketGammaMarket>> {
        let mut normalized = Vec::new();
        let mut seen_conditions = std::collections::HashSet::new();
        for condition_id in condition_ids {
            let condition_id = condition_id.trim();
            if condition_id.is_empty() || !seen_conditions.insert(condition_id.to_string()) {
                continue;
            }
            normalized.push(condition_id.to_string());
        }
        if normalized.is_empty() {
            return Ok(Vec::new());
        }

        let mut markets = Vec::new();
        let mut seen_markets = std::collections::HashSet::new();
        for chunk in normalized.chunks(GAMMA_CONDITION_BATCH_SIZE) {
            let page = self.fetch_market_page_by_condition_ids(chunk).await?;
            for raw in page {
                if let Some(market) = map_gamma_market(raw)?
                    && seen_markets.insert(market.id.clone())
                {
                    markets.push(market);
                }
            }
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

        let body = response.bytes().await.map_err(|error| {
            AppError::dependency_unavailable(
                "POLYMARKET_GAMMA_MARKET_DECODE_FAILED",
                format!("failed to read Polymarket Gamma market {market_id} response body: {error}"),
            )
        })?;
        let raw = decode_json_body::<RawGammaMarket>(
            &body,
            "POLYMARKET_GAMMA_MARKET_DECODE_FAILED",
            &format!("Polymarket Gamma market {market_id}"),
        )?;

        map_gamma_market(raw)
    }

    /// Returns `Ok(None)` when the Gamma API returns 422 (offset exceeds server
    /// limit), signaling that pagination has been exhausted.  Returns
    /// `Ok(Some(vec))` on success, and `Err` on transport/server errors that
    /// are worth propagating.
    async fn fetch_market_page_offset(
        &self,
        limit: u16,
        offset: u64,
    ) -> Result<Option<Vec<RawGammaMarket>>> {
        let url = format!("{}/markets", self.gamma_host);
        let mut url = reqwest::Url::parse(&url).map_err(|error| {
            AppError::invalid_input(
                "POLYMARKET_GAMMA_MARKETS_URL_INVALID",
                format!("failed to construct Polymarket Gamma markets URL: {error}"),
            )
        })?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("active", "true");
            query.append_pair("closed", "false");
            query.append_pair("archived", "false");
            query.append_pair("order", "volume24hr");
            query.append_pair("ascending", "false");
            query.append_pair("limit", &limit.max(1).to_string());
            query.append_pair("offset", &offset.to_string());
        }

        let mut is_rate_limited = false;
        for attempt in 0..=GAMMA_RATE_LIMIT_MAX_RETRIES {
            let response = self.client.get(url.clone()).send().await.map_err(|error| {
                AppError::dependency_unavailable(
                    "POLYMARKET_GAMMA_MARKETS_REQUEST_FAILED",
                    format!("failed to request Polymarket Gamma markets: {error}"),
                )
            })?;
            let status = response.status();

            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                is_rate_limited = true;
                if attempt < GAMMA_RATE_LIMIT_MAX_RETRIES {
                    let delay = GAMMA_RATE_LIMIT_BASE_DELAY * 2u32.pow(attempt);
                    tracing::warn!(
                        offset,
                        attempt = attempt + 1,
                        "Gamma markets rate limited (429), retrying after {:?}",
                        delay,
                    );
                    tokio::time::sleep(delay).await;
                    continue;
                }
                return Err(AppError::dependency_unavailable(
                    "POLYMARKET_GAMMA_MARKETS_STATUS_FAILED",
                    format!("Polymarket Gamma markets returned HTTP {status} after {} retries", GAMMA_RATE_LIMIT_MAX_RETRIES),
                ));
            }

            // 422 Unprocessable Entity at large offsets signals pagination
            // boundary — the server rejects offsets beyond its configured
            // limit.  Treat this as end-of-data rather than a retryable error.
            if status == reqwest::StatusCode::UNPROCESSABLE_ENTITY {
                tracing::debug!(
                    offset,
                    "Gamma markets returned 422 — offset exceeds server limit"
                );
                return Ok(None);
            }

            if !status.is_success() {
                if attempt < GAMMA_MAX_RETRIES && !is_rate_limited {
                    let delay = GAMMA_RETRY_BASE_DELAY * 2u32.pow(attempt);
                    tracing::warn!(
                        offset,
                        attempt = attempt + 1,
                        status = %status,
                        "Gamma markets returned HTTP {}, retrying after {:?}",
                        status,
                        delay,
                    );
                    tokio::time::sleep(delay).await;
                    continue;
                }
                return Err(AppError::dependency_unavailable(
                    "POLYMARKET_GAMMA_MARKETS_STATUS_FAILED",
                    format!("Polymarket Gamma markets returned HTTP {status}"),
                ));
            }

            let body = response.bytes().await.map_err(|error| {
                AppError::dependency_unavailable(
                    "POLYMARKET_GAMMA_MARKETS_DECODE_FAILED",
                    format!("failed to read Polymarket Gamma markets response body: {error}"),
                )
            })?;
            return decode_json_body::<Vec<RawGammaMarket>>(
                &body,
                "POLYMARKET_GAMMA_MARKETS_DECODE_FAILED",
                "Polymarket Gamma markets",
            )
            .map(Some);
        }

        unreachable!()
    }

    async fn fetch_market_page_by_condition_ids(
        &self,
        condition_ids: &[String],
    ) -> Result<Vec<RawGammaMarket>> {
        let url = format!("{}/markets", self.gamma_host);
        let mut url = reqwest::Url::parse(&url).map_err(|error| {
            AppError::invalid_input(
                "POLYMARKET_GAMMA_MARKETS_URL_INVALID",
                format!("failed to construct Polymarket Gamma markets URL: {error}"),
            )
        })?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("limit", &condition_ids.len().max(1).to_string());
            for condition_id in condition_ids {
                query.append_pair("condition_ids", condition_id);
            }
        }

        let mut is_rate_limited = false;
        for attempt in 0..=GAMMA_RATE_LIMIT_MAX_RETRIES {
            let response = self.client.get(url.clone()).send().await.map_err(|error| {
                AppError::dependency_unavailable(
                    "POLYMARKET_GAMMA_MARKETS_REQUEST_FAILED",
                    format!("failed to request Polymarket Gamma markets by condition id: {error}"),
                )
            })?;
            let status = response.status();

            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                is_rate_limited = true;
                if attempt < GAMMA_RATE_LIMIT_MAX_RETRIES {
                    let delay = GAMMA_RATE_LIMIT_BASE_DELAY * 2u32.pow(attempt);
                    tracing::warn!(
                        condition_count = condition_ids.len(),
                        attempt = attempt + 1,
                        "Gamma markets condition query rate limited (429), retrying after {:?}",
                        delay,
                    );
                    tokio::time::sleep(delay).await;
                    continue;
                }
                return Err(AppError::dependency_unavailable(
                    "POLYMARKET_GAMMA_MARKETS_STATUS_FAILED",
                    format!(
                        "Polymarket Gamma markets condition query returned HTTP {status} after {} retries",
                        GAMMA_RATE_LIMIT_MAX_RETRIES
                    ),
                ));
            }

            if !status.is_success() {
                if attempt < GAMMA_MAX_RETRIES && !is_rate_limited {
                    let delay = GAMMA_RETRY_BASE_DELAY * 2u32.pow(attempt);
                    tracing::warn!(
                        condition_count = condition_ids.len(),
                        attempt = attempt + 1,
                        status = %status,
                        "Gamma markets condition query returned HTTP {}, retrying after {:?}",
                        status,
                        delay,
                    );
                    tokio::time::sleep(delay).await;
                    continue;
                }
                return Err(AppError::dependency_unavailable(
                    "POLYMARKET_GAMMA_MARKETS_STATUS_FAILED",
                    format!("Polymarket Gamma markets condition query returned HTTP {status}"),
                ));
            }

            let body = response.bytes().await.map_err(|error| {
                AppError::dependency_unavailable(
                    "POLYMARKET_GAMMA_MARKETS_DECODE_FAILED",
                    format!(
                        "failed to read Polymarket Gamma markets condition query response body: {error}"
                    ),
                )
            })?;
            return decode_json_body::<Vec<RawGammaMarket>>(
                &body,
                "POLYMARKET_GAMMA_MARKETS_DECODE_FAILED",
                "Polymarket Gamma markets condition query",
            );
        }

        unreachable!()
    }
}

fn decode_json_body<T: DeserializeOwned>(body: &[u8], code: &'static str, label: &str) -> Result<T> {
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
    let explicit_resolution_source = explicit_resolution_source(&raw);
    let resolution_source = resolution_source(&raw, explicit_resolution_source.as_deref());
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
    let liquidity_usd = UsdAmount::new(
        parse_decimal_value(raw.liquidity_clob.clone())
            .or_else(|| parse_decimal_value(raw.liquidity.clone()))
            .unwrap_or(Decimal::ZERO)
            .max(Decimal::ZERO),
    )?;
    let start_at = parse_rfc3339(raw.start_date_iso.as_deref())
        .or_else(|| parse_rfc3339(raw.start_date.as_deref()));
    let end_at = parse_rfc3339(raw.end_date_iso.as_deref())
        .or_else(|| parse_rfc3339(raw.end_date.as_deref()));
    let event_start_at = raw
        .events
        .iter()
        .filter_map(|event| parse_rfc3339(event.start_date.as_deref()))
        .min()
        .or(start_at);
    let event_end_at = raw
        .events
        .iter()
        .filter_map(|event| parse_rfc3339(event.end_date.as_deref()))
        .max()
        .or(end_at);
    let updated_at =
        parse_rfc3339(raw.updated_at.as_deref()).unwrap_or_else(OffsetDateTime::now_utc);
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
    let ambiguity_level = if explicit_resolution_source.is_some() {
        AmbiguityLevel::Low
    } else if normalize_optional_text(raw.description.clone()).is_some() {
        AmbiguityLevel::Medium
    } else {
        AmbiguityLevel::High
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
        liquidity_usd,
        start_at,
        end_at,
        event_start_at,
        event_end_at,
        has_reviewed_dates: raw.has_reviewed_dates,
        ambiguity_level,
        tradability_status,
        resolution_source,
        edge_case_notes,
        condition_id,
        yes_asset_id: token_ids[0].clone(),
        no_asset_id: token_ids[1].clone(),
        outcome_token_ids: token_ids,
        outcomes,
        outcome_prices,
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

fn explicit_resolution_source(raw: &RawGammaMarket) -> Option<String> {
    normalize_optional_text(raw.resolution_source.clone())
        .or_else(|| {
            raw.events
                .iter()
                .find_map(|event| normalize_optional_text(event.resolution_source.clone()))
        })
}

fn resolution_source(raw: &RawGammaMarket, explicit: Option<&str>) -> String {
    explicit
        .map(str::to_string)
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
