use async_trait::async_trait;
use polyedge_domain::Result;
use std::sync::Arc;
use time::{Duration, OffsetDateTime};

const RAW_EVENTS_UNLINKED_RETENTION_DAYS: i64 = 30;
const RAW_EVENTS_LINKED_RETENTION_DAYS: i64 = 90;
const EXPIRED_CACHE_GRACE_DAYS: i64 = 7;
const REWARD_CANDLE_RETENTION_DAYS: i64 = 30;
const CONTROL_COMMAND_COMPLETED_RETENTION_DAYS: i64 = 30;
const CONTROL_COMMAND_FAILED_RETENTION_DAYS: i64 = 90;
const OUTBOX_PUBLISHED_RETENTION_DAYS: i64 = 30;
const OUTBOX_FAILED_RETENTION_DAYS: i64 = 90;
const EXTERNAL_EVENT_PROCESSED_RETENTION_DAYS: i64 = 90;
const EXTERNAL_EVENT_STALE_UNPROCESSED_RETENTION_DAYS: i64 = 7;
const LLM_CALL_RETENTION_DAYS: i64 = 180;
const AUDIT_RETENTION_DAYS: i64 = 365;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DatabaseMaintenanceCutoffs {
    pub now: OffsetDateTime,
    pub raw_events_unlinked_before: OffsetDateTime,
    pub raw_events_linked_before: OffsetDateTime,
    pub expired_cache_before: OffsetDateTime,
    pub reward_candles_before: OffsetDateTime,
    pub control_commands_completed_before: OffsetDateTime,
    pub control_commands_failed_before: OffsetDateTime,
    pub outbox_published_before: OffsetDateTime,
    pub outbox_failed_before: OffsetDateTime,
    pub external_event_processed_before: OffsetDateTime,
    pub external_event_stale_unprocessed_before: OffsetDateTime,
    pub llm_calls_before: OffsetDateTime,
    pub audit_before: OffsetDateTime,
}

impl DatabaseMaintenanceCutoffs {
    #[must_use]
    pub fn from_now(now: OffsetDateTime) -> Self {
        Self {
            now,
            raw_events_unlinked_before: now - Duration::days(RAW_EVENTS_UNLINKED_RETENTION_DAYS),
            raw_events_linked_before: now - Duration::days(RAW_EVENTS_LINKED_RETENTION_DAYS),
            expired_cache_before: now - Duration::days(EXPIRED_CACHE_GRACE_DAYS),
            reward_candles_before: now - Duration::days(REWARD_CANDLE_RETENTION_DAYS),
            control_commands_completed_before: now
                - Duration::days(CONTROL_COMMAND_COMPLETED_RETENTION_DAYS),
            control_commands_failed_before: now
                - Duration::days(CONTROL_COMMAND_FAILED_RETENTION_DAYS),
            outbox_published_before: now - Duration::days(OUTBOX_PUBLISHED_RETENTION_DAYS),
            outbox_failed_before: now - Duration::days(OUTBOX_FAILED_RETENTION_DAYS),
            external_event_processed_before: now
                - Duration::days(EXTERNAL_EVENT_PROCESSED_RETENTION_DAYS),
            external_event_stale_unprocessed_before: now
                - Duration::days(EXTERNAL_EVENT_STALE_UNPROCESSED_RETENTION_DAYS),
            llm_calls_before: now - Duration::days(LLM_CALL_RETENTION_DAYS),
            audit_before: now - Duration::days(AUDIT_RETENTION_DAYS),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DatabaseMaintenanceReport {
    pub idempotency_keys_deleted: u64,
    pub outbox_events_deleted: u64,
    pub external_event_dedup_deleted: u64,
    pub llm_calls_deleted: u64,
    pub raw_events_deleted: u64,
    pub reward_market_advisories_deleted: u64,
    pub reward_market_info_risks_deleted: u64,
    pub reward_market_candles_deleted: u64,
    pub reward_control_commands_deleted: u64,
    pub audit_logs_deleted: u64,
    pub mode_transitions_deleted: u64,
}

impl DatabaseMaintenanceReport {
    #[must_use]
    pub fn total_deleted(self) -> u64 {
        self.idempotency_keys_deleted
            .saturating_add(self.outbox_events_deleted)
            .saturating_add(self.external_event_dedup_deleted)
            .saturating_add(self.llm_calls_deleted)
            .saturating_add(self.raw_events_deleted)
            .saturating_add(self.reward_market_advisories_deleted)
            .saturating_add(self.reward_market_info_risks_deleted)
            .saturating_add(self.reward_market_candles_deleted)
            .saturating_add(self.reward_control_commands_deleted)
            .saturating_add(self.audit_logs_deleted)
            .saturating_add(self.mode_transitions_deleted)
    }
}

#[async_trait]
pub trait DatabaseMaintenanceStore: Send + Sync {
    async fn prune_database_history(
        &self,
        cutoffs: DatabaseMaintenanceCutoffs,
    ) -> Result<DatabaseMaintenanceReport>;
}

pub struct DatabaseMaintenanceService {
    store: Arc<dyn DatabaseMaintenanceStore>,
}

impl DatabaseMaintenanceService {
    #[must_use]
    pub fn new(store: Arc<dyn DatabaseMaintenanceStore>) -> Self {
        Self { store }
    }

    pub async fn prune_history(&self, now: OffsetDateTime) -> Result<DatabaseMaintenanceReport> {
        self.store
            .prune_database_history(DatabaseMaintenanceCutoffs::from_now(now))
            .await
    }
}
