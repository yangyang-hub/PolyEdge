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
            confirmed_at: observed_at,
            source: BookSource::Ws,
        }
    }

    #[test]
    fn broadcaster_assigns_monotonic_sequences() {
        let broadcaster = OrderbookUpdateBroadcaster::new(4);
        let mut rx = broadcaster.subscribe();

        broadcaster.publish(OrderbookStreamReason::Book, test_book("a", 300_001, "0.40"));
        broadcaster.publish(
            OrderbookStreamReason::PriceChange,
            test_book("b", 300_002, "0.41"),
        );

        let first = rx.try_recv().expect("first event");
        let second = rx.try_recv().expect("second event");
        assert_eq!(first.sequence, 1);
        assert_eq!(second.sequence, 2);
        assert_eq!(first.book.token_id, "a");
        assert_eq!(second.book.token_id, "b");
    }
}
