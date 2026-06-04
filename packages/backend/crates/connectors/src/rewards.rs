use polyedge_domain::{AppError, Result};
use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

const LAST_CURSOR: &str = "LTE=";
const ENRICH_TIMEOUT: Duration = Duration::from_secs(10);
const ENRICH_MAX_RETRIES: u32 = 3;
const ENRICH_RETRY_BASE_DELAY: Duration = Duration::from_millis(500);
const MAX_REWARD_MARKET_PAGES: usize = 1_000;

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

#[derive(Debug, Deserialize)]
struct RawClobMarketToken {
    token_id: Option<String>,
    outcome: Option<String>,
    price: Option<Decimal>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct RawClobMarketDetail {
    condition_id: Option<String>,
    question: Option<String>,
    market_slug: Option<String>,
    image: Option<String>,
    tokens: Option<Vec<RawClobMarketToken>>,
}

const ENRICH_CONCURRENCY: usize = 10;

fn spawn_next_orderbook_fetch<'a>(
    tasks: &mut JoinSet<Result<Option<PolymarketRewardOrderBook>>>,
    pending: &mut std::slice::Iter<'a, String>,
    connector: &PolymarketRewardsConnector,
) {
    if let Some(token_id) = pending.next() {
        let conn = connector.clone();
        let tid = token_id.clone();
        tasks.spawn(async move { conn.fetch_order_book(&tid).await });
    }
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
            client: reqwest::Client::builder()
                .timeout(ENRICH_TIMEOUT)
                .build()
                .map_err(|error| {
                    AppError::internal(
                        "POLYMARKET_REWARDS_HTTP_CLIENT_BUILD_FAILED",
                        format!("failed to build Polymarket rewards HTTP client: {error}"),
                    )
                })?,
        })
    }

    pub async fn fetch_current_markets(&self) -> Result<Vec<PolymarketRewardMarket>> {
        let mut markets = Vec::new();
        let mut cursor: Option<String> = None;
        let mut seen_cursors = HashSet::new();
        let mut seen_condition_ids = HashSet::new();
        let mut completed = false;

        for _ in 0..MAX_REWARD_MARKET_PAGES {
            if let Some(cursor) = cursor.as_ref()
                && !seen_cursors.insert(cursor.clone())
            {
                return Err(AppError::dependency_unavailable(
                    "POLYMARKET_REWARDS_CURSOR_REPEATED",
                    format!("Polymarket rewards markets repeated cursor {cursor}"),
                ));
            }
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
                if seen_condition_ids.insert(market.condition_id.clone()) {
                    markets.push(market);
                }
            }

            let next_cursor = payload.next_cursor.unwrap_or_default();
            if next_cursor.is_empty() || next_cursor == LAST_CURSOR || payload.count == 0 {
                completed = true;
                break;
            }
            cursor = Some(next_cursor);
        }
        if !completed {
            return Err(AppError::dependency_unavailable(
                "POLYMARKET_REWARDS_MAX_PAGES_EXCEEDED",
                format!("Polymarket rewards markets exceeded {MAX_REWARD_MARKET_PAGES} pages"),
            ));
        }
        if markets.is_empty() {
            return Err(AppError::dependency_unavailable(
                "POLYMARKET_REWARDS_MARKETS_EMPTY",
                "Polymarket rewards markets returned an empty replacement catalog",
            ));
        }

        let raw_count = markets.len();
        let markets = self.enrich_reward_markets(markets).await?;
        tracing::info!(
            raw_count,
            enriched_count = markets.len(),
            dropped = raw_count - markets.len(),
            "fetched and enriched reward markets"
        );
        Ok(markets)
    }

    pub async fn fetch_order_books(
        &self,
        token_ids: &[String],
    ) -> Result<Vec<PolymarketRewardOrderBook>> {
        let mut pending = token_ids.iter();
        let mut tasks = JoinSet::new();
        let mut books = Vec::new();
        let mut failures = 0usize;

        for _ in 0..ENRICH_CONCURRENCY {
            spawn_next_orderbook_fetch(&mut tasks, &mut pending, self);
        }

        while let Some(result) = tasks.join_next().await {
            match result {
                Ok(Ok(Some(book))) => books.push(book),
                Ok(Ok(None)) => failures += 1,
                Ok(Err(error)) => {
                    failures += 1;
                    tracing::warn!(error = %error, "failed to fetch reward order book");
                }
                Err(error) => {
                    failures += 1;
                    tracing::warn!(error = %error, "order book task panicked");
                }
            }
            spawn_next_orderbook_fetch(&mut tasks, &mut pending, self);
        }

        if books.is_empty() && failures > 0 {
            return Err(AppError::dependency_unavailable(
                "POLYMARKET_BOOK_BATCH_FAILED",
                format!("failed to fetch all {failures} requested Polymarket order books"),
            ));
        }
        if failures > 0 {
            tracing::warn!(
                requested = token_ids.len(),
                fetched = books.len(),
                failures,
                "partially fetched Polymarket order books"
            );
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

    async fn fetch_market_detail(&self, condition_id: &str) -> Result<Option<RawClobMarketDetail>> {
        let url = format!("{}/markets/{condition_id}", self.clob_host);
        let response = self.client.get(&url).send().await.map_err(|error| {
            AppError::dependency_unavailable(
                "POLYMARKET_CLOB_MARKET_DETAIL_REQUEST_FAILED",
                format!("failed to fetch market detail for {condition_id}: {error}"),
            )
        })?;

        if response.status().as_u16() == 404 {
            return Ok(None);
        }

        let status = response.status();
        if !status.is_success() {
            return Err(AppError::dependency_unavailable(
                "POLYMARKET_CLOB_MARKET_DETAIL_STATUS_FAILED",
                format!("CLOB market detail returned HTTP {status} for {condition_id}"),
            ));
        }

        response
            .json::<RawClobMarketDetail>()
            .await
            .map(Some)
            .map_err(|error| {
                AppError::dependency_unavailable(
                    "POLYMARKET_CLOB_MARKET_DETAIL_DECODE_FAILED",
                    format!("failed to decode market detail for {condition_id}: {error}"),
                )
            })
    }

    async fn enrich_reward_markets(
        &self,
        markets: Vec<PolymarketRewardMarket>,
    ) -> Result<Vec<PolymarketRewardMarket>> {
        let semaphore = Arc::new(Semaphore::new(ENRICH_CONCURRENCY));
        let client = self.clone();
        let mut handles = Vec::with_capacity(markets.len());

        for market in &markets {
            let sem = semaphore.clone();
            let connector = client.clone();
            let cid = market.condition_id.clone();
            handles.push(tokio::spawn(async move {
                let _permit = sem.acquire().await.expect("semaphore closed");
                for attempt in 0..=ENRICH_MAX_RETRIES {
                    match connector.fetch_market_detail(&cid).await {
                        Ok(Some(detail)) => return Ok(detail),
                        Ok(None) => {
                            return Err(AppError::dependency_unavailable(
                                "POLYMARKET_CLOB_MARKET_DETAIL_NOT_FOUND",
                                format!("CLOB market detail was not found for {cid}"),
                            ));
                        }
                        Err(error) => {
                            if attempt < ENRICH_MAX_RETRIES {
                                let delay = ENRICH_RETRY_BASE_DELAY * 2u32.pow(attempt);
                                tracing::debug!(
                                    condition_id = %cid,
                                    attempt = attempt + 1,
                                    error = %error,
                                    "retrying market detail fetch after {:?}",
                                    delay,
                                );
                                tokio::time::sleep(delay).await;
                            } else {
                                tracing::warn!(
                                    condition_id = %cid,
                                    error = %error,
                                    "failed to enrich reward market after {} retries",
                                    ENRICH_MAX_RETRIES,
                                );
                                return Err(error);
                            }
                        }
                    }
                }
                unreachable!()
            }));
        }

        let mut enriched = Vec::with_capacity(handles.len());
        for (market, handle) in markets.into_iter().zip(handles) {
            let detail = handle.await.map_err(|error| {
                AppError::dependency_unavailable(
                    "POLYMARKET_REWARD_MARKET_ENRICH_TASK_FAILED",
                    format!(
                        "reward market enrichment task failed for {}: {error}",
                        market.condition_id
                    ),
                )
            })??;

            let mut market = market;
            dedupe_reward_tokens(&mut market.tokens);
            if market.question == market.condition_id {
                if let Some(question) = detail.question {
                    market.question = question;
                }
            }
            if market.image.is_empty() {
                if let Some(image) = detail.image {
                    market.image = image;
                }
            }
            if market.market_slug == market.condition_id {
                if let Some(slug) = detail.market_slug {
                    market.market_slug = slug;
                }
            }
            if market.tokens.len() < 2
                && let Some(tokens) = detail.tokens
            {
                market.tokens = tokens
                    .into_iter()
                    .filter_map(|raw| {
                        let token_id = raw.token_id.unwrap_or_default().trim().to_string();
                        if token_id.is_empty() {
                            return None;
                        }
                        Some(PolymarketRewardToken {
                            token_id,
                            outcome: raw.outcome.unwrap_or_default(),
                            price: raw.price,
                        })
                    })
                    .collect();
            }
            dedupe_reward_tokens(&mut market.tokens);
            if market.tokens.len() < 2 {
                return Err(AppError::dependency_unavailable(
                    "POLYMARKET_REWARD_MARKET_TOKENS_INCOMPLETE",
                    format!(
                        "reward market {} has fewer than two enriched tokens",
                        market.condition_id
                    ),
                ));
            }
            enriched.push(market);
        }

        Ok(enriched)
    }
}

fn dedupe_reward_tokens(tokens: &mut Vec<PolymarketRewardToken>) {
    let mut seen = HashSet::new();
    tokens.retain(|token| !token.token_id.trim().is_empty() && seen.insert(token.token_id.clone()));
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
    let token_id = raw.token_id.unwrap_or_default().trim().to_string();
    if token_id.is_empty() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reward_tokens_are_deduplicated_before_catalog_write() {
        let mut tokens = vec![
            PolymarketRewardToken {
                token_id: "yes".to_string(),
                outcome: "YES".to_string(),
                price: None,
            },
            PolymarketRewardToken {
                token_id: "yes".to_string(),
                outcome: "YES duplicate".to_string(),
                price: None,
            },
            PolymarketRewardToken {
                token_id: "no".to_string(),
                outcome: "NO".to_string(),
                price: None,
            },
        ];

        dedupe_reward_tokens(&mut tokens);

        assert_eq!(
            tokens
                .iter()
                .map(|token| token.token_id.as_str())
                .collect::<Vec<_>>(),
            vec!["yes", "no"]
        );
    }
}
