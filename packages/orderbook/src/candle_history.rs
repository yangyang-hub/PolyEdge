use polyedge_application::{
    REWARD_PRICE_HISTORY_CANDLE_INTERVAL_SEC, RewardBotService, RewardMarket,
    RewardMarketCandleSample,
};
use polyedge_connectors::{PolymarketPriceHistoryPoint, PolymarketRewardsConnector};
use polyedge_domain::{AppError, Result};
use polyedge_infrastructure::AppState;
use rust_decimal::Decimal;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use time::OffsetDateTime;
use tracing::{info, warn};

const MIN_HISTORY_SYNC_INTERVAL_SECS: u64 = 60;
const MAX_HISTORY_SYNC_INTERVAL_SECS: u64 = 3_600;
const MIN_HISTORY_REQUEST_DELAY_MS: u64 = 250;
const MAX_HISTORY_REQUEST_DELAY_MS: u64 = 10_000;
const MIN_HISTORY_LOOKBACK_SECS: u64 = REWARD_PRICE_HISTORY_CANDLE_INTERVAL_SEC as u64;
const MAX_HISTORY_LOOKBACK_SECS: u64 = 24 * 60 * 60;

#[derive(Debug, Default)]
struct RewardCandleHistorySyncReport {
    tokens_selected: usize,
    tokens_requested: usize,
    points_fetched: usize,
    samples_saved: usize,
    failures: usize,
    stopped_early: bool,
}

pub async fn run_reward_candle_history_sync_loop(state: AppState) {
    if !state
        .settings
        .orderbook_stream
        .reward_candle_history_enabled
    {
        info!("reward candle history sync is disabled");
        return;
    }

    let sync_interval = Duration::from_secs(
        state
            .settings
            .orderbook_stream
            .reward_candle_history_sync_interval_secs
            .clamp(
                MIN_HISTORY_SYNC_INTERVAL_SECS,
                MAX_HISTORY_SYNC_INTERVAL_SECS,
            ),
    );
    let request_delay = Duration::from_millis(
        state
            .settings
            .orderbook_stream
            .reward_candle_history_request_delay_ms
            .clamp(MIN_HISTORY_REQUEST_DELAY_MS, MAX_HISTORY_REQUEST_DELAY_MS),
    );
    let connector = match PolymarketRewardsConnector::new(&state.settings.polymarket.clob_host) {
        Ok(connector) => connector,
        Err(error) => {
            warn!(
                error = %error,
                "failed to initialize reward candle price history connector"
            );
            return;
        }
    };
    let mut first_cycle = true;

    loop {
        let cycle_started = Instant::now();
        let lookback_secs = reward_candle_history_lookback_secs(&state, first_cycle);
        match sync_reward_candle_history_once(&state, &connector, lookback_secs, request_delay)
            .await
        {
            Ok(report) => {
                if first_cycle && report.tokens_selected > 0 {
                    first_cycle = false;
                }
                info!(
                    tokens_selected = report.tokens_selected,
                    tokens_requested = report.tokens_requested,
                    points_fetched = report.points_fetched,
                    samples_saved = report.samples_saved,
                    failures = report.failures,
                    stopped_early = report.stopped_early,
                    lookback_secs,
                    request_delay_ms = request_delay.as_millis(),
                    "synced reward candle history",
                );
            }
            Err(error) => warn!(
                error = %error,
                lookback_secs,
                "reward candle history sync failed",
            ),
        }

        let elapsed = cycle_started.elapsed();
        if elapsed < sync_interval {
            tokio::time::sleep(sync_interval - elapsed).await;
        }
    }
}

async fn sync_reward_candle_history_once(
    state: &AppState,
    connector: &PolymarketRewardsConnector,
    lookback_secs: u64,
    request_delay: Duration,
) -> Result<RewardCandleHistorySyncReport> {
    let tokens = reward_candle_history_token_ids(state).await?;
    let mut report = RewardCandleHistorySyncReport {
        tokens_selected: tokens.len(),
        ..RewardCandleHistorySyncReport::default()
    };
    if tokens.is_empty() {
        return Ok(report);
    }

    let end = OffsetDateTime::now_utc();
    let start = end - time::Duration::seconds(lookback_secs as i64);
    let fidelity_minutes = (REWARD_PRICE_HISTORY_CANDLE_INTERVAL_SEC / 60).max(1) as u16;

    for (index, token_id) in tokens.iter().enumerate() {
        if index > 0 {
            tokio::time::sleep(request_delay).await;
        }
        report.tokens_requested += 1;
        match connector
            .fetch_price_history(token_id, start, end, fidelity_minutes)
            .await
        {
            Ok(points) => {
                report.points_fetched += points.len();
                report.samples_saved += record_price_history_points(
                    state.reward_bot_service.clone(),
                    token_id,
                    &points,
                    start,
                    end,
                )
                .await?;
            }
            Err(error) => {
                report.failures += 1;
                warn!(
                    token_id,
                    error = %error,
                    "failed to sync reward candle price history for token"
                );
                if price_history_error_stops_cycle(&error) {
                    report.stopped_early = true;
                    break;
                }
            }
        }
    }

    Ok(report)
}

async fn record_price_history_points(
    service: Arc<RewardBotService>,
    token_id: &str,
    points: &[PolymarketPriceHistoryPoint],
    start: OffsetDateTime,
    end: OffsetDateTime,
) -> Result<usize> {
    let mut saved = 0usize;
    for point in points {
        if point.observed_at < start || point.observed_at > end {
            continue;
        }
        let sample = reward_candle_sample_from_price_history(token_id, point)?;
        service.record_market_candle_sample(&sample).await?;
        saved += 1;
    }
    Ok(saved)
}

fn reward_candle_sample_from_price_history(
    token_id: &str,
    point: &PolymarketPriceHistoryPoint,
) -> Result<RewardMarketCandleSample> {
    let interval_sec = REWARD_PRICE_HISTORY_CANDLE_INTERVAL_SEC;
    let bucket_start = reward_candle_bucket_start(point.observed_at, interval_sec)?;
    Ok(RewardMarketCandleSample {
        token_id: token_id.to_string(),
        interval_sec,
        bucket_start,
        midpoint: point.price,
        // Polymarket price-history is not a bid/ask source. Keep the existing
        // candle schema by storing the provider price as both close-side fields
        // and zero spread, while docs identify these candles as price-history
        // derived rather than orderbook-derived.
        best_bid: point.price,
        best_ask: point.price,
        spread_cents: Decimal::ZERO,
        observed_at: point.observed_at,
    })
}

fn reward_candle_bucket_start(
    observed_at: OffsetDateTime,
    interval_sec: i32,
) -> Result<OffsetDateTime> {
    if interval_sec <= 0 {
        return Err(AppError::invalid_input(
            "REWARD_CANDLE_INTERVAL_INVALID",
            "reward market candle interval must be positive",
        ));
    }
    let interval = i64::from(interval_sec);
    let bucket_seconds = observed_at.unix_timestamp().div_euclid(interval) * interval;
    OffsetDateTime::from_unix_timestamp(bucket_seconds).map_err(|error| {
        AppError::invalid_input(
            "REWARD_CANDLE_BUCKET_INVALID",
            format!("failed to build reward candle bucket timestamp: {error}"),
        )
    })
}

async fn reward_candle_history_token_ids(state: &AppState) -> Result<Vec<String>> {
    let max_tokens = state
        .settings
        .orderbook_stream
        .reward_candle_history_max_tokens_per_cycle;
    if max_tokens == 0 {
        return Ok(Vec::new());
    }

    let mut markets = state
        .reward_bot_service
        .list_active_reward_markets()
        .await?;
    markets.sort_by(|left, right| {
        right
            .total_daily_rate
            .cmp(&left.total_daily_rate)
            .then_with(|| left.condition_id.cmp(&right.condition_id))
    });
    Ok(reward_candle_history_token_ids_from_markets(
        &markets, max_tokens,
    ))
}

fn reward_candle_history_token_ids_from_markets(
    markets: &[RewardMarket],
    max_tokens: usize,
) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut token_ids = Vec::new();
    for market in markets {
        for token in &market.tokens {
            let token_id = token.token_id.trim();
            if token_id.is_empty() || !seen.insert(token_id.to_string()) {
                continue;
            }
            token_ids.push(token_id.to_string());
            if token_ids.len() >= max_tokens {
                return token_ids;
            }
        }
    }
    token_ids
}

fn reward_candle_history_lookback_secs(state: &AppState, first_cycle: bool) -> u64 {
    let settings = &state.settings.orderbook_stream;
    let configured = if first_cycle {
        settings.reward_candle_history_backfill_secs
    } else {
        settings.reward_candle_history_incremental_secs
    };
    configured.clamp(MIN_HISTORY_LOOKBACK_SECS, MAX_HISTORY_LOOKBACK_SECS)
}

fn price_history_error_stops_cycle(error: &AppError) -> bool {
    match error.code() {
        "POLYMARKET_PRICE_HISTORY_REQUEST_FAILED" | "POLYMARKET_PRICE_HISTORY_DECODE_FAILED" => {
            true
        }
        "POLYMARKET_PRICE_HISTORY_STATUS_FAILED" => {
            let message = error.message().to_ascii_lowercase();
            message.contains("http 401")
                || message.contains("http 403")
                || message.contains("http 408")
                || message.contains("http 409")
                || message.contains("http 429")
                || message.contains("http 500")
                || message.contains("http 502")
                || message.contains("http 503")
                || message.contains("http 504")
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    fn test_market(condition_id: &str, rate: &str, tokens: &[&str]) -> RewardMarket {
        RewardMarket {
            condition_id: condition_id.to_string(),
            question: String::new(),
            market_slug: String::new(),
            event_slug: String::new(),
            category: String::new(),
            image: String::new(),
            rewards_max_spread: Decimal::ZERO,
            rewards_min_size: Decimal::ZERO,
            total_daily_rate: Decimal::from_str_exact(rate).expect("rate"),
            liquidity_usd: Decimal::ZERO,
            volume_24h_usd: Decimal::ZERO,
            market_spread_cents: Decimal::ZERO,
            end_at: None,
            ambiguity_level: String::new(),
            market_synced_at: None,
            tokens: tokens
                .iter()
                .map(|token_id| polyedge_application::RewardToken {
                    token_id: (*token_id).to_string(),
                    outcome: String::new(),
                    price: None,
                })
                .collect(),
            active: true,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    #[test]
    fn reward_candle_history_token_selection_dedupes_and_caps() {
        let markets = vec![
            test_market("low", "1", &["a", "b"]),
            test_market("high", "3", &["b", "c"]),
            test_market("mid", "2", &["d", ""]),
        ];
        let mut sorted = markets;
        sorted.sort_by(|left, right| right.total_daily_rate.cmp(&left.total_daily_rate));

        assert_eq!(
            reward_candle_history_token_ids_from_markets(&sorted, 3),
            vec!["b".to_string(), "c".to_string(), "d".to_string()]
        );
    }

    #[test]
    fn reward_candle_sample_from_history_uses_five_minute_bucket() {
        let point = PolymarketPriceHistoryPoint {
            observed_at: OffsetDateTime::from_unix_timestamp(1_700_000_123).unwrap(),
            price: Decimal::from_str_exact("0.42").unwrap(),
        };
        let sample = reward_candle_sample_from_price_history("token", &point).unwrap();

        assert_eq!(sample.token_id, "token");
        assert_eq!(sample.bucket_start.unix_timestamp(), 1_700_000_100);
        assert_eq!(sample.midpoint, Decimal::from_str_exact("0.42").unwrap());
        assert_eq!(sample.best_bid, sample.midpoint);
        assert_eq!(sample.best_ask, sample.midpoint);
        assert_eq!(sample.spread_cents, Decimal::ZERO);
    }

    #[test]
    fn price_history_rate_limit_errors_stop_cycle() {
        let error = AppError::dependency_unavailable(
            "POLYMARKET_PRICE_HISTORY_STATUS_FAILED",
            "Polymarket price history returned HTTP 429",
        );
        assert!(price_history_error_stops_cycle(&error));
    }
}
