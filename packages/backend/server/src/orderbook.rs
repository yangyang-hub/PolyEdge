//! Targeted orderbook polling for manually managed markets.
//!
//! This runtime never discovers markets. Its token universe is rebuilt from
//! active subscription wallets, open managed orders, and non-zero positions in
//! Postgres. Exceeding the configured token ceiling is an error: silently
//! truncating would remove risk coverage for an account.

use crate::store::PostgresStore;
use polyedge_connectors::{
    PolymarketRewardBookLevel, PolymarketRewardOrderBook, PolymarketRewardsConnector,
};
use polyedge_domain::{AppError, Result};
use rust_decimal::Decimal;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};
use time::OffsetDateTime;
use tokio::sync::{RwLock, watch};

pub const DEFAULT_MAX_TOKENS: usize = 1_000;
pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq)]
pub struct BookLevel {
    pub price: Decimal,
    pub size: Decimal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CachedOrderBook {
    pub token_id: String,
    pub bids: Vec<BookLevel>,
    pub asks: Vec<BookLevel>,
    /// Upstream content timestamp.
    pub observed_at: OffsetDateTime,
    /// Local time at which this server successfully confirmed the snapshot.
    pub confirmed_at: OffsetDateTime,
}

impl CachedOrderBook {
    #[must_use]
    pub fn best_bid(&self) -> Option<Decimal> {
        self.bids.first().map(|level| level.price)
    }

    #[must_use]
    pub fn best_ask(&self) -> Option<Decimal> {
        self.asks.first().map(|level| level.price)
    }

    #[must_use]
    pub fn is_fresh_at(&self, now: OffsetDateTime, freshness_ms: i64) -> bool {
        if freshness_ms <= 0 || now < self.confirmed_at {
            return false;
        }
        (now - self.confirmed_at).whole_milliseconds() <= i128::from(freshness_ms)
    }
}

#[derive(Debug, Clone)]
pub struct OrderbookRuntimeConfig {
    pub clob_host: String,
    pub max_tokens: usize,
    pub poll_interval: Duration,
}

impl Default for OrderbookRuntimeConfig {
    fn default() -> Self {
        Self {
            clob_host: "https://clob.polymarket.com".to_string(),
            max_tokens: DEFAULT_MAX_TOKENS,
            poll_interval: DEFAULT_POLL_INTERVAL,
        }
    }
}

#[derive(Clone)]
pub struct OrderbookSupervisor {
    store: PostgresStore,
    connector: PolymarketRewardsConnector,
    cache: Arc<RwLock<HashMap<String, CachedOrderBook>>>,
    config: OrderbookRuntimeConfig,
}

impl OrderbookSupervisor {
    pub fn new(store: PostgresStore, config: OrderbookRuntimeConfig) -> Result<Self> {
        if config.max_tokens == 0 {
            return Err(AppError::invalid_input(
                "ORDERBOOK_MAX_TOKENS_INVALID",
                "orderbook max_tokens must be greater than zero",
            ));
        }
        if config.poll_interval.is_zero() {
            return Err(AppError::invalid_input(
                "ORDERBOOK_POLL_INTERVAL_INVALID",
                "orderbook poll interval must be greater than zero",
            ));
        }
        let connector = PolymarketRewardsConnector::new(&config.clob_host)?;
        Ok(Self {
            store,
            connector,
            cache: Arc::new(RwLock::new(HashMap::new())),
            config,
        })
    }

    #[must_use]
    pub fn cache_handle(&self) -> Arc<RwLock<HashMap<String, CachedOrderBook>>> {
        Arc::clone(&self.cache)
    }

    pub async fn get(&self, token_id: &str) -> Option<CachedOrderBook> {
        self.cache.read().await.get(token_id).cloned()
    }

    pub async fn snapshot(&self) -> Vec<CachedOrderBook> {
        let mut books = self
            .cache
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        books.sort_by(|left, right| left.token_id.cmp(&right.token_id));
        books
    }

    /// Refresh exactly the required token set. A partial upstream response is
    /// retained in cache but reported as an error so callers cannot assume all
    /// live actions have fresh books.
    pub async fn refresh_once(&self) -> Result<usize> {
        let tokens = load_required_tokens(&self.store, self.config.max_tokens).await?;
        if tokens.is_empty() {
            self.cache.write().await.clear();
            return Ok(0);
        }

        let requested = tokens.iter().cloned().collect::<HashSet<_>>();
        let fetched = self.connector.fetch_order_books(&tokens).await?;
        let confirmed_at = OffsetDateTime::now_utc();
        let fetched_ids = fetched
            .iter()
            .map(|book| book.token_id.clone())
            .collect::<HashSet<_>>();

        let mut cache = self.cache.write().await;
        cache.retain(|token_id, _| requested.contains(token_id));
        for book in fetched {
            if requested.contains(&book.token_id) {
                cache.insert(book.token_id.clone(), normalize_book(book, confirmed_at));
            }
        }
        drop(cache);

        if fetched_ids.len() != requested.len() || !requested.is_subset(&fetched_ids) {
            let mut missing = requested
                .difference(&fetched_ids)
                .cloned()
                .collect::<Vec<_>>();
            missing.sort();
            return Err(AppError::dependency_unavailable(
                "ORDERBOOK_REFRESH_PARTIAL",
                format!(
                    "Polymarket returned {} of {} required books; missing token ids: {}",
                    fetched_ids.len(),
                    requested.len(),
                    missing.join(",")
                ),
            ));
        }

        Ok(fetched_ids.len())
    }

    pub async fn run(self: Arc<Self>, mut shutdown: watch::Receiver<bool>) {
        let mut interval = tokio::time::interval(self.config.poll_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        break;
                    }
                }
                _ = interval.tick() => {
                    if let Err(error) = self.refresh_once().await {
                        tracing::error!(
                            code = error.code(),
                            error = %error,
                            "targeted orderbook refresh failed"
                        );
                    }
                }
            }
        }
    }
}

/// Read-only token-universe query. There is deliberately no catalog or Gamma
/// source in this union.
pub async fn load_required_tokens(store: &PostgresStore, max_tokens: usize) -> Result<Vec<String>> {
    let pool = store.pool();
    let limit = i64::try_from(max_tokens.saturating_add(1)).map_err(|_| {
        AppError::invalid_input(
            "ORDERBOOK_MAX_TOKENS_INVALID",
            "orderbook max_tokens does not fit in i64",
        )
    })?;
    let mut tokens = sqlx::query_scalar::<_, String>(
        r#"
        SELECT token_id
        FROM (
            SELECT outcome.token_id
            FROM strategy_subscriptions subscription
            JOIN strategy_subscription_wallets target
              ON target.subscription_id = subscription.subscription_id
             AND target.follower_user_id = subscription.follower_user_id
            JOIN wallet_accounts wallet
              ON wallet.wallet_id = target.wallet_id
             AND wallet.owner_user_id = subscription.follower_user_id
            JOIN users follower_user ON follower_user.user_id = subscription.follower_user_id
            JOIN market_strategies strategy
              ON strategy.strategy_id = subscription.source_strategy_id
            JOIN users source_user ON source_user.user_id = strategy.owner_user_id
            JOIN strategy_versions version
              ON version.strategy_id = strategy.strategy_id
             AND version.status = 'published'
            JOIN strategy_quote_slots slot
              ON slot.strategy_version_id = version.strategy_version_id
             AND slot.enabled = TRUE
            JOIN managed_market_outcomes outcome
              ON outcome.market_id = strategy.market_id
             AND outcome.outcome = slot.outcome
            JOIN managed_markets market
              ON market.market_id = strategy.market_id
            WHERE target.enabled = TRUE
              AND wallet.status = 'active'
              AND wallet.trading_enabled = TRUE
              AND follower_user.status = 'active'
              AND source_user.status = 'active'
              AND strategy.status = 'active'
              AND strategy.active_from <= now()
              AND strategy.active_until > now()
              AND subscription.status = 'active'
              AND (subscription.active_until IS NULL OR subscription.active_until > now())
              AND market.status = 'open'

            UNION

            SELECT token_id
            FROM managed_orders
            WHERE status IN (
                'planned', 'submitting', 'open', 'partially_filled',
                'cancel_pending', 'unknown'
            )

            UNION

            SELECT token_id
            FROM positions
            WHERE quantity > 0
        ) required_tokens
        ORDER BY token_id
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        AppError::internal(
            "ORDERBOOK_TOKEN_QUERY_FAILED",
            format!("failed to load targeted orderbook tokens: {error}"),
        )
    })?;

    if tokens.len() > max_tokens {
        return Err(AppError::conflict(
            "ORDERBOOK_MAX_TOKENS_EXCEEDED",
            format!(
                "required token count exceeds configured maximum of {max_tokens}; no token was truncated"
            ),
        ));
    }
    tokens.dedup();
    Ok(tokens)
}

fn normalize_book(
    book: PolymarketRewardOrderBook,
    confirmed_at: OffsetDateTime,
) -> CachedOrderBook {
    let mut bids = normalize_levels(book.bids);
    bids.sort_by(|left, right| right.price.cmp(&left.price));
    let mut asks = normalize_levels(book.asks);
    asks.sort_by(|left, right| left.price.cmp(&right.price));
    CachedOrderBook {
        token_id: book.token_id,
        bids,
        asks,
        observed_at: book.observed_at,
        confirmed_at,
    }
}

fn normalize_levels(levels: Vec<PolymarketRewardBookLevel>) -> Vec<BookLevel> {
    levels
        .into_iter()
        .filter(|level| {
            level.price > Decimal::ZERO && level.price < Decimal::ONE && level.size > Decimal::ZERO
        })
        .map(|level| BookLevel {
            price: level.price,
            size: level.size,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn freshness_uses_confirmed_at_not_observed_at() {
        let now = OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(100);
        let book = CachedOrderBook {
            token_id: "yes".to_string(),
            bids: Vec::new(),
            asks: Vec::new(),
            observed_at: OffsetDateTime::UNIX_EPOCH,
            confirmed_at: now - time::Duration::milliseconds(900),
        };
        assert!(book.is_fresh_at(now, 1_000));
        assert!(!book.is_fresh_at(now, 800));
    }

    #[test]
    fn future_confirmation_fails_closed() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let book = CachedOrderBook {
            token_id: "yes".to_string(),
            bids: Vec::new(),
            asks: Vec::new(),
            observed_at: now,
            confirmed_at: now + time::Duration::seconds(1),
        };
        assert!(!book.is_fresh_at(now, 5_000));
    }
}
