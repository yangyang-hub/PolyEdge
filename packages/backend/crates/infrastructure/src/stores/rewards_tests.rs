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

    fn position(account_id: &str, token_id: &str, size: i64) -> RewardPosition {
        RewardPosition {
            account_id: account_id.to_string(),
            condition_id: format!("cond_{token_id}"),
            token_id: token_id.to_string(),
            outcome: "YES".to_string(),
            size: Decimal::from(size),
            avg_price: Decimal::new(49, 2),
            realized_pnl: Decimal::ZERO,
            updated_at: OffsetDateTime::now_utc(),
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

    #[tokio::test]
    async fn external_account_sync_replaces_only_the_target_account_positions() {
        let store = InMemoryRewardBotStore::new();
        let now = OffsetDateTime::now_utc();
        let account_a = RewardAccountState::fresh("account_a", Decimal::from(100), now);
        let account_b = RewardAccountState::fresh("account_b", Decimal::from(100), now);

        store
            .apply_account_sync(&account_a, Some(&[position("account_a", "old", 5)]), "trc_a")
            .await
            .expect("seed account a");
        store
            .apply_account_sync(&account_b, Some(&[position("account_b", "other", 7)]), "trc_b")
            .await
            .expect("seed account b");
        store
            .apply_account_sync(
                &account_a,
                Some(&[position("account_a", "replacement", 9)]),
                "trc_replace",
            )
            .await
            .expect("replace account a");

        let account_a_positions = store
            .list_account_positions("account_a")
            .await
            .expect("list account a");
        assert_eq!(account_a_positions.len(), 1);
        assert_eq!(account_a_positions[0].token_id, "replacement");
        assert_eq!(
            store
                .list_account_positions("account_b")
                .await
                .expect("list account b")[0]
                .token_id,
            "other"
        );
    }

    #[tokio::test]
    async fn failed_external_position_sync_preserves_positions() {
        let store = InMemoryRewardBotStore::new();
        let now = OffsetDateTime::now_utc();
        let mut account = RewardAccountState::fresh("account_a", Decimal::from(100), now);
        store
            .apply_account_sync(&account, Some(&[position("account_a", "held", 5)]), "trc_seed")
            .await
            .expect("seed account");

        account.available_usd = Decimal::from(80);
        store
            .apply_account_sync(&account, None, "trc_balance_only")
            .await
            .expect("apply balance-only sync");

        assert_eq!(
            store
                .list_account_positions("account_a")
                .await
                .expect("list account a")[0]
                .token_id,
            "held"
        );
        assert_eq!(
            store
                .load_account_state(&RewardBotConfig {
                    account_id: "account_a".to_string(),
                    ..RewardBotConfig::default()
                })
                .await
                .expect("load account")
                .available_usd,
            Decimal::from(80)
        );
    }
}
