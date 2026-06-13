use polyedge_application::{CachedOrderBook, OrderbookStreamEvent, OrderbookStreamReason};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio::sync::broadcast;
use tracing::debug;

#[derive(Clone)]
pub struct OrderbookUpdateBroadcaster {
    sequence: Arc<AtomicU64>,
    tx: broadcast::Sender<OrderbookStreamEvent>,
}

impl OrderbookUpdateBroadcaster {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity.max(1));
        Self {
            sequence: Arc::new(AtomicU64::new(0)),
            tx,
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
            book,
        };
        if self.tx.send(event).is_err() {
            debug!("orderbook update broadcast skipped because there are no subscribers");
        }
    }
}
