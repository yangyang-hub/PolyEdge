use polyedge_domain::{AppError, Result};
use rust_decimal::Decimal;
use serde::Deserialize;
use std::str::FromStr;
use time::OffsetDateTime;

const LAST_CURSOR: &str = "LTE=";
const MAX_REWARD_MARKET_PAGES: usize = 20;

#[derive(Debug, Clone)]
pub struct PolymarketRewardToken {
    pub token_id: String,
    pub outcome: String,
    pub price: Option<Decimal>,
}

#[derive(Debug, Clone)]
pub struct PolymarketRewardMarket {
    pub condition_id: String,
    pub question: String,
    pub market_slug: String,
    pub event_slug: String,
    pub image: String,
    pub rewards_max_spread: Decimal,
    pub rewards_min_size: Decimal,
    pub total_daily_rate: Decimal,
    pub tokens: Vec<PolymarketRewardToken>,
    pub active: bool,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct PolymarketRewardBookLevel {
    pub price: Decimal,
    pub size: Decimal,
}

#[derive(Debug, Clone)]
pub struct PolymarketRewardOrderBook {
    pub token_id: String,
    pub bids: Vec<PolymarketRewardBookLevel>,
    pub asks: Vec<PolymarketRewardBookLevel>,
    pub observed_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct PolymarketRewardsConnector {
    clob_host: String,
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct RawRewardToken {
    token_id: Option<String>,
    outcome: Option<String>,
    price: Option<Decimal>,
}

#[derive(Debug, Deserialize)]
struct RawRewardConfig {
    rate_per_day: Option<Decimal>,
}

#[derive(Debug, Deserialize)]
struct RawRewardMarket {
    condition_id: String,
    question: Option<String>,
    market_slug: Option<String>,
    event_slug: Option<String>,
    image: Option<String>,
    rewards_max_spread: Option<Decimal>,
    rewards_min_size: Option<Decimal>,
    tokens: Option<Vec<RawRewardToken>>,
    rewards_config: Option<Vec<RawRewardConfig>>,
    total_daily_rate: Option<Decimal>,
    native_daily_rate: Option<Decimal>,
    sponsored_daily_rate: Option<Decimal>,
}

#[derive(Debug, Deserialize)]
struct RewardMarketsResponse {
    count: usize,
    next_cursor: Option<String>,
    data: Vec<RawRewardMarket>,
}

#[derive(Debug, Deserialize)]
struct RawBookLevel {
    price: Option<String>,
    size: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawOrderBook {
    asset_id: Option<String>,
    bids: Option<Vec<RawBookLevel>>,
    asks: Option<Vec<RawBookLevel>>,
}

impl PolymarketRewardsConnector {
    pub fn new(clob_host: &str) -> Result<Self> {
        let clob_host = clob_host.trim().trim_end_matches('/').to_string();
        if clob_host.is_empty() {
            return Err(AppError::invalid_input(
                "POLYMARKET_CLOB_HOST_REQUIRED",
                "polymarket clob_host must not be empty",
            ));
        }

        Ok(Self {
            clob_host,
            client: reqwest::Client::new(),
        })
    }

    pub async fn fetch_current_markets(&self) -> Result<Vec<PolymarketRewardMarket>> {
        let mut markets = Vec::new();
        let mut cursor: Option<String> = None;

        for _ in 0..MAX_REWARD_MARKET_PAGES {
            let mut url =
                reqwest::Url::parse(&format!("{}/rewards/markets/current", self.clob_host))
                    .map_err(|error| {
                        AppError::invalid_input(
                            "POLYMARKET_REWARDS_URL_INVALID",
                            format!("failed to construct rewards markets URL: {error}"),
                        )
                    })?;
            if let Some(cursor) = cursor.as_ref() {
                url.query_pairs_mut().append_pair("next_cursor", cursor);
            }

            let response = self.client.get(url).send().await.map_err(|error| {
                AppError::dependency_unavailable(
                    "POLYMARKET_REWARDS_MARKETS_REQUEST_FAILED",
                    format!("failed to request Polymarket rewards markets: {error}"),
                )
            })?;
            let status = response.status();
            if !status.is_success() {
                return Err(AppError::dependency_unavailable(
                    "POLYMARKET_REWARDS_MARKETS_STATUS_FAILED",
                    format!("Polymarket rewards markets returned HTTP {status}"),
                ));
            }

            let payload = response
                .json::<RewardMarketsResponse>()
                .await
                .map_err(|error| {
                    AppError::dependency_unavailable(
                        "POLYMARKET_REWARDS_MARKETS_DECODE_FAILED",
                        format!("failed to decode Polymarket rewards markets: {error}"),
                    )
                })?;

            for raw in payload.data {
                let market = map_reward_market(raw);
                if market.tokens.len() >= 2 {
                    markets.push(market);
                }
            }

            let next_cursor = payload.next_cursor.unwrap_or_default();
            if next_cursor.is_empty() || next_cursor == LAST_CURSOR || payload.count == 0 {
                break;
            }
            cursor = Some(next_cursor);
        }

        Ok(markets)
    }

    pub async fn fetch_order_books(
        &self,
        token_ids: &[String],
    ) -> Result<Vec<PolymarketRewardOrderBook>> {
        let mut books = Vec::new();

        for token_id in token_ids {
            match self.fetch_order_book(token_id).await {
                Ok(Some(book)) => books.push(book),
                Ok(None) => {}
                Err(error) => {
                    tracing::warn!(token_id = %token_id, error = %error, "failed to fetch reward order book");
                }
            }
        }

        Ok(books)
    }

    async fn fetch_order_book(&self, token_id: &str) -> Result<Option<PolymarketRewardOrderBook>> {
        let mut url =
            reqwest::Url::parse(&format!("{}/book", self.clob_host)).map_err(|error| {
                AppError::invalid_input(
                    "POLYMARKET_BOOK_URL_INVALID",
                    format!("failed to construct order book URL: {error}"),
                )
            })?;
        url.query_pairs_mut().append_pair("token_id", token_id);

        let response = self.client.get(url).send().await.map_err(|error| {
            AppError::dependency_unavailable(
                "POLYMARKET_BOOK_REQUEST_FAILED",
                format!("failed to request Polymarket order book for token_id={token_id}: {error}"),
            )
        })?;

        if response.status().as_u16() == 404 {
            return Ok(None);
        }

        let status = response.status();
        if !status.is_success() {
            return Err(AppError::dependency_unavailable(
                "POLYMARKET_BOOK_STATUS_FAILED",
                format!("Polymarket order book returned HTTP {status} for token_id={token_id}"),
            ));
        }

        let raw = response.json::<RawOrderBook>().await.map_err(|error| {
            AppError::dependency_unavailable(
                "POLYMARKET_BOOK_DECODE_FAILED",
                format!("failed to decode Polymarket order book for token_id={token_id}: {error}"),
            )
        })?;

        Ok(Some(PolymarketRewardOrderBook {
            token_id: raw.asset_id.unwrap_or_else(|| token_id.to_string()),
            bids: parse_levels(raw.bids, SortDirection::Descending),
            asks: parse_levels(raw.asks, SortDirection::Ascending),
            observed_at: OffsetDateTime::now_utc(),
        }))
    }
}

fn map_reward_market(raw: RawRewardMarket) -> PolymarketRewardMarket {
    let now = OffsetDateTime::now_utc();
    let configured_daily_rate = raw.native_daily_rate.unwrap_or(Decimal::ZERO)
        + raw.sponsored_daily_rate.unwrap_or(Decimal::ZERO);
    let rewards_daily_rate = raw
        .rewards_config
        .unwrap_or_default()
        .into_iter()
        .fold(Decimal::ZERO, |sum, config| {
            sum + config.rate_per_day.unwrap_or(Decimal::ZERO)
        });
    let total_daily_rate = raw.total_daily_rate.unwrap_or_else(|| {
        if configured_daily_rate > Decimal::ZERO {
            configured_daily_rate
        } else {
            rewards_daily_rate
        }
    });

    PolymarketRewardMarket {
        condition_id: raw.condition_id.clone(),
        question: raw.question.unwrap_or_else(|| raw.condition_id.clone()),
        market_slug: raw.market_slug.unwrap_or_else(|| raw.condition_id.clone()),
        event_slug: raw.event_slug.unwrap_or_default(),
        image: raw.image.unwrap_or_default(),
        rewards_max_spread: raw.rewards_max_spread.unwrap_or(Decimal::ZERO),
        rewards_min_size: raw.rewards_min_size.unwrap_or(Decimal::ZERO),
        total_daily_rate,
        tokens: raw
            .tokens
            .unwrap_or_default()
            .into_iter()
            .filter_map(map_reward_token)
            .collect(),
        active: true,
        updated_at: now,
    }
}

fn map_reward_token(raw: RawRewardToken) -> Option<PolymarketRewardToken> {
    let token_id = raw.token_id.unwrap_or_default();
    if token_id.trim().is_empty() {
        return None;
    }

    Some(PolymarketRewardToken {
        token_id,
        outcome: raw.outcome.unwrap_or_default(),
        price: raw.price,
    })
}

#[derive(Debug, Clone, Copy)]
enum SortDirection {
    Ascending,
    Descending,
}

fn parse_levels(
    levels: Option<Vec<RawBookLevel>>,
    direction: SortDirection,
) -> Vec<PolymarketRewardBookLevel> {
    let mut parsed = levels
        .unwrap_or_default()
        .into_iter()
        .filter_map(|level| {
            let price = parse_decimal(level.price.as_deref())?;
            let size = parse_decimal(level.size.as_deref())?;
            if size <= Decimal::ZERO {
                return None;
            }
            Some(PolymarketRewardBookLevel { price, size })
        })
        .collect::<Vec<_>>();

    parsed.sort_by(|left, right| match direction {
        SortDirection::Ascending => left.price.cmp(&right.price),
        SortDirection::Descending => right.price.cmp(&left.price),
    });
    parsed
}

fn parse_decimal(value: Option<&str>) -> Option<Decimal> {
    let raw = value?.trim();
    if raw.is_empty() {
        return None;
    }
    Decimal::from_str(raw).ok()
}
