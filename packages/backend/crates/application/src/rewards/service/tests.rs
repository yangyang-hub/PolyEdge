use super::*;

#[test]
fn worker_running_requires_enabled_config_and_fresh_heartbeat() {
    let now = OffsetDateTime::now_utc();
    let enabled = RewardBotConfig {
        enabled: true,
        ..RewardBotConfig::default()
    };

    assert!(reward_worker_is_running(
        &enabled,
        Some(now - TimeDuration::seconds(30)),
        now,
    ));
    assert!(!reward_worker_is_running(
        &enabled,
        Some(now - TimeDuration::minutes(3)),
        now,
    ));
    assert!(!reward_worker_is_running(
        &RewardBotConfig::default(),
        Some(now),
        now,
    ));
}

#[test]
fn transient_live_orderbook_skip_reasons_are_not_carried() {
    assert!(live_orderbook_skip_reason_is_transient(Some(
        "missing fresh orderbook midpoint for live quote",
    )));
    assert!(live_orderbook_skip_reason_is_transient(Some(
        "waiting for fresh orderbook data from subscription: YES orderbook unavailable",
    )));
    assert!(live_orderbook_skip_reason_is_transient(Some(
        "YES orderbook stale: age_ms=50000, max_age_ms=45000",
    )));
    assert!(!live_orderbook_skip_reason_is_transient(Some(
        "YES bid-3 is outside the rewards spread limit",
    )));
    assert!(!live_orderbook_skip_reason_is_transient(None));
}
