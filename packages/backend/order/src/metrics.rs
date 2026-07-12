use serde::Serialize;
use std::sync::atomic::{AtomicI64, AtomicU64, AtomicUsize, Ordering};

#[derive(Default)]
pub(crate) struct OrderbookRuntimeMetrics {
    ws_queue_depth: AtomicUsize,
    ws_queue_depth_peak: AtomicUsize,
    ws_events_coalesced: AtomicU64,
    ws_events_dropped: AtomicU64,
    poll_divergences: AtomicU64,
    poll_confirmations_rejected: AtomicU64,
    last_ws_event_at: AtomicI64,
    last_poll_success_at: AtomicI64,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub(crate) struct OrderbookRuntimeMetricsSnapshot {
    pub ws_queue_depth: usize,
    pub ws_queue_depth_peak: usize,
    pub ws_events_coalesced: u64,
    pub ws_events_dropped: u64,
    pub poll_divergences: u64,
    pub poll_confirmations_rejected: u64,
    pub last_ws_event_at: i64,
    pub last_poll_success_at: i64,
}

impl OrderbookRuntimeMetrics {
    pub(crate) fn set_ws_queue_depth(&self, depth: usize) {
        self.ws_queue_depth.store(depth, Ordering::Relaxed);
        self.ws_queue_depth_peak.fetch_max(depth, Ordering::Relaxed);
    }

    pub(crate) fn increment_coalesced(&self) {
        self.ws_events_coalesced.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn increment_dropped(&self) {
        self.ws_events_dropped.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn increment_poll_divergence(&self) {
        self.poll_divergences.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn increment_poll_confirmation_rejected(&self) {
        self.poll_confirmations_rejected
            .fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn observe_ws_event(&self, now_ms: i64) {
        self.last_ws_event_at.store(now_ms, Ordering::Relaxed);
    }

    pub(crate) fn observe_poll_success(&self, now_ms: i64) {
        self.last_poll_success_at.store(now_ms, Ordering::Relaxed);
    }

    pub(crate) fn snapshot(&self) -> OrderbookRuntimeMetricsSnapshot {
        OrderbookRuntimeMetricsSnapshot {
            ws_queue_depth: self.ws_queue_depth.load(Ordering::Relaxed),
            ws_queue_depth_peak: self.ws_queue_depth_peak.load(Ordering::Relaxed),
            ws_events_coalesced: self.ws_events_coalesced.load(Ordering::Relaxed),
            ws_events_dropped: self.ws_events_dropped.load(Ordering::Relaxed),
            poll_divergences: self.poll_divergences.load(Ordering::Relaxed),
            poll_confirmations_rejected: self.poll_confirmations_rejected.load(Ordering::Relaxed),
            last_ws_event_at: self.last_ws_event_at.load(Ordering::Relaxed),
            last_poll_success_at: self.last_poll_success_at.load(Ordering::Relaxed),
        }
    }
}
