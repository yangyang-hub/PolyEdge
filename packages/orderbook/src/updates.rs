use polyedge_application::{
    CachedOrderBook, OrderbookStreamEvent, OrderbookStreamReason, RewardBotService,
};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio::sync::broadcast;
use tracing::{debug, warn};

#[derive(Clone)]
pub struct OrderbookUpdateBroadcaster {
    sequence: Arc<AtomicU64>,
    tx: broadcast::Sender<OrderbookStreamEvent>,
    reward_bot_service: Option<Arc<RewardBotService>>,
}

impl OrderbookUpdateBroadcaster {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity.max(1));
        Self {
            sequence: Arc::new(AtomicU64::new(0)),
            tx,
            reward_bot_service: None,
        }
    }

    pub fn with_reward_candles(capacity: usize, reward_bot_service: Arc<RewardBotService>) -> Self {
        let (tx, _) = broadcast::channel(capacity.max(1));
        Self {
            sequence: Arc::new(AtomicU64::new(0)),
            tx,
            reward_bot_service: Some(reward_bot_service),
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
        let Some(service) = self.reward_bot_service.clone() else {
            return;
        };
        tokio::spawn(async move {
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
        });
    }
}
