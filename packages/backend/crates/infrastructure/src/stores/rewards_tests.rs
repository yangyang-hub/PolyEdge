#[cfg(test)]
mod rewards_tests {
    use super::*;

    fn running_command(started_at: OffsetDateTime) -> RewardControlCommand {
        RewardControlCommand {
            id: "rewcmd_lease".to_string(),
            action: RewardControlAction::RunOnce,
            account_id: Some("reward_live".to_string()),
            reason: "lease test".to_string(),
            status: RewardControlCommandStatus::Running,
            requested_at: started_at,
            started_at: Some(started_at),
            completed_at: None,
            trace_id: Some("trc_old".to_string()),
            error: None,
        }
    }

    #[tokio::test]
    async fn stale_running_reward_command_is_reclaimed() {
        let store = InMemoryRewardBotStore::new();
        let now = OffsetDateTime::now_utc();
        store
            .enqueue_control_command(running_command(
                now - REWARD_CONTROL_COMMAND_LEASE - Duration::seconds(1),
            ))
            .await
            .expect("enqueue command");

        let claimed = store
            .claim_next_control_command("trc_new", now)
            .await
            .expect("claim command")
            .expect("stale command reclaimed");

        assert_eq!(claimed.status, RewardControlCommandStatus::Running);
        assert_eq!(claimed.started_at, Some(now));
        assert_eq!(claimed.trace_id.as_deref(), Some("trc_new"));
    }

    #[tokio::test]
    async fn fresh_running_reward_command_is_not_reclaimed() {
        let store = InMemoryRewardBotStore::new();
        let now = OffsetDateTime::now_utc();
        store
            .enqueue_control_command(running_command(now - Duration::minutes(1)))
            .await
            .expect("enqueue command");

        assert!(
            store
                .claim_next_control_command("trc_new", now)
                .await
                .expect("claim command")
                .is_none()
        );
    }
}
