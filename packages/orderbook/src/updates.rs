use polyedge_application::{
    CachedOrderBook, OrderbookStreamEvent, OrderbookStreamReason, REWARD_AI_CANDLE_INTERVAL_SEC,
    RewardBotService,
};
use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, warn};

const REWARD_CANDLE_QUEUE_CAPACITY: usize = 4_096;
const REWARD_CANDLE_FLUSH_INTERVAL: Duration = Duration::from_secs(1);
const REWARD_CANDLE_MAX_PENDING: usize = 1_024;

#[derive(Clone)]
pub struct OrderbookUpdateBroadcaster {
    sequence: Arc<AtomicU64>,
    tx: broadcast::Sender<OrderbookStreamEvent>,
    reward_candle_tx: Option<mpsc::Sender<CachedOrderBook>>,
    dropped_reward_candles: Arc<AtomicU64>,
}

impl OrderbookUpdateBroadcaster {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity.max(1));
        Self {
            sequence: Arc::new(AtomicU64::new(0)),
            tx,
            reward_candle_tx: None,
            dropped_reward_candles: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn with_reward_candles(capacity: usize, reward_bot_service: Arc<RewardBotService>) -> Self {
        let (tx, _) = broadcast::channel(capacity.max(1));
        let (reward_candle_tx, reward_candle_rx) = mpsc::channel(REWARD_CANDLE_QUEUE_CAPACITY);
        tokio::spawn(run_reward_candle_writer(
            reward_bot_service,
            reward_candle_rx,
        ));
        Self {
            sequence: Arc::new(AtomicU64::new(0)),
            tx,
            reward_candle_tx: Some(reward_candle_tx),
            dropped_reward_candles: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<OrderbookStreamEvent> {
        self.tx.subscribe()
    }

    pub fn publish(&self, reason: OrderbookStreamReason, book: CachedOrderBook) {
        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        let event = OrderbookStreamEvent {
            sequence,
            reason,
            book: book.clone(),
        };
        if self.tx.send(event).is_err() {
            debug!("orderbook update broadcast skipped because there are no subscribers");
        }
        self.record_reward_candle(book);
    }

    fn record_reward_candle(&self, book: CachedOrderBook) {
        let Some(tx) = self.reward_candle_tx.as_ref() else {
            return;
        };
        match tx.try_send(book) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                let dropped = self.dropped_reward_candles.fetch_add(1, Ordering::Relaxed) + 1;
                if dropped == 1 || dropped.is_power_of_two() {
                    warn!(
                        dropped,
                        capacity = REWARD_CANDLE_QUEUE_CAPACITY,
                        "reward candle queue is full; dropping orderbook candle samples",
                    );
                }
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                debug!("reward candle writer stopped; skipping candle sample");
            }
        }
    }
}

async fn run_reward_candle_writer(
    service: Arc<RewardBotService>,
    mut rx: mpsc::Receiver<CachedOrderBook>,
) {
    let mut pending: HashMap<RewardCandleKey, CachedOrderBook> = HashMap::new();
    let mut flush_timer = tokio::time::interval(REWARD_CANDLE_FLUSH_INTERVAL);
    flush_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            maybe_book = rx.recv() => {
                match maybe_book {
                    Some(book) => {
                        push_pending_reward_candle(&mut pending, book);
                        if pending.len() >= REWARD_CANDLE_MAX_PENDING {
                            flush_reward_candles(&service, &mut pending).await;
                        }
                    }
                    None => {
                        flush_reward_candles(&service, &mut pending).await;
                        break;
                    }
                }
            }
            _ = flush_timer.tick() => {
                flush_reward_candles(&service, &mut pending).await;
            }
        }
    }
}

fn push_pending_reward_candle(
    pending: &mut HashMap<RewardCandleKey, CachedOrderBook>,
    book: CachedOrderBook,
) {
    let key = RewardCandleKey::from_book(&book);
    match pending.get_mut(&key) {
        Some(current) if current.observed_at > book.observed_at => {}
        Some(current) => *current = book,
        None => {
            pending.insert(key, book);
        }
    }
}

async fn flush_reward_candles(
    service: &Arc<RewardBotService>,
    pending: &mut HashMap<RewardCandleKey, CachedOrderBook>,
) {
    if pending.is_empty() {
        return;
    }

    let books = pending.drain().map(|(_, book)| book).collect::<Vec<_>>();
    for book in books {
        if let Err(error) = service
            .record_orderbook_candle_from_cached_book(&book)
            .await
        {
            warn!(
                token_id = %book.token_id,
                error = %error,
                "failed to record reward market candle from orderbook update",
            );
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RewardCandleKey {
    token_id: String,
    bucket_index: i64,
}

impl RewardCandleKey {
    fn from_book(book: &CachedOrderBook) -> Self {
        let interval_ms = i64::from(REWARD_AI_CANDLE_INTERVAL_SEC).saturating_mul(1_000);
        Self {
            token_id: book.token_id.clone(),
            bucket_index: book.observed_at.div_euclid(interval_ms.max(1)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polyedge_application::{BookSource, CachedBookLevel};
    use rust_decimal::Decimal;

    fn test_book(token_id: &str, observed_at: i64, price: &str) -> CachedOrderBook {
        let price = Decimal::from_str_exact(price).expect("decimal");
        CachedOrderBook {
            token_id: token_id.to_string(),
            bids: vec![CachedBookLevel {
                price,
                size: Decimal::ONE,
            }],
            asks: vec![CachedBookLevel {
                price,
                size: Decimal::ONE,
            }],
            observed_at,
            source: BookSource::Ws,
        }
    }

    #[test]
    fn pending_reward_candles_keep_latest_per_token_bucket() {
        let mut pending = HashMap::new();
        push_pending_reward_candle(&mut pending, test_book("a", 300_001, "0.40"));
        push_pending_reward_candle(&mut pending, test_book("a", 300_002, "0.41"));
        push_pending_reward_candle(&mut pending, test_book("a", 300_000, "0.39"));
        push_pending_reward_candle(&mut pending, test_book("b", 300_002, "0.42"));

        assert_eq!(pending.len(), 2);
        let a = pending
            .get(&RewardCandleKey {
                token_id: "a".to_string(),
                bucket_index: 1,
            })
            .expect("token a candle");
        assert_eq!(a.observed_at, 300_002);
        assert_eq!(a.bids[0].price, Decimal::from_str_exact("0.41").unwrap());
    }
}
