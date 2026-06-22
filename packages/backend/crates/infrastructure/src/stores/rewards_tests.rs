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

    fn open_order(id: &str, external_order_id: Option<&str>) -> ManagedRewardOrder {
        let now = OffsetDateTime::now_utc();
        ManagedRewardOrder {
            id: id.to_string(),
            account_id: "reward_live".to_string(),
            condition_id: "cond_live".to_string(),
            token_id: format!("token_{id}"),
            outcome: "YES".to_string(),
            side: RewardOrderSide::Buy,
            price: Decimal::new(49, 2),
            size: Decimal::from(20),
            strategy_bucket: RewardStrategyBucket::Standard,
            external_order_id: external_order_id.map(str::to_string),
            status: ManagedRewardOrderStatus::Open,
            scoring: false,
            reason: "test order".to_string(),
            filled_size: Decimal::ZERO,
            reward_earned: Decimal::ZERO,
            last_scored_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn order_with_status(
        id: &str,
        status: ManagedRewardOrderStatus,
        updated_at: OffsetDateTime,
    ) -> ManagedRewardOrder {
        let mut order = open_order(id, Some(&format!("pm_{id}")));
        order.status = status;
        order.created_at = updated_at;
        order.updated_at = updated_at;
        order
    }

    fn reward_event(id: &str, created_at: OffsetDateTime) -> RewardRiskEvent {
        RewardRiskEvent {
            id: id.to_string(),
            account_id: Some("reward_live".to_string()),
            condition_id: Some("cond_live".to_string()),
            external_order_id: None,
            event_type: "test_event".to_string(),
            severity: RewardRiskSeverity::Info,
            message: "test event".to_string(),
            metadata: json!({}),
            created_at,
        }
    }

    fn reward_fill(id: &str, created_at: OffsetDateTime) -> RewardFill {
        RewardFill {
            id: id.to_string(),
            order_id: "old_filled".to_string(),
            account_id: "reward_live".to_string(),
            condition_id: "cond_live".to_string(),
            token_id: "token_old_filled".to_string(),
            outcome: "YES".to_string(),
            side: RewardOrderSide::Buy,
            price: Decimal::new(49, 2),
            size: Decimal::from(20),
            notional_usd: Decimal::new(980, 2),
            role: RewardFillRole::Maker,
            realized_pnl: Decimal::ZERO,
            reason: "test fill".to_string(),
            trace_id: "trc_fill".to_string(),
            created_at,
        }
    }

    fn low_competition_observation(
        id: &str,
        observed_at: OffsetDateTime,
    ) -> RewardLowCompetitionObservation {
        RewardLowCompetitionObservation {
            id: id.to_string(),
            account_id: "reward_live".to_string(),
            condition_id: "cond_live".to_string(),
            market_slug: "live-market".to_string(),
            question: "Live market".to_string(),
            observed_at,
            mode: RewardLowCompetitionMode::Observe,
            planned_notional_usd: Decimal::from(10),
            qualified_competition_usd: Decimal::from(100),
            estimated_reward_per_100_usd_day: Decimal::from(3),
            competition_density: Decimal::ONE,
            exit_depth_usd: Decimal::from(50),
            exit_slippage_cents: None,
            midpoint_range_cents: None,
            top_of_book_flip_count: None,
            sample_count: 5,
            sample_insufficient: false,
            eligible_for_low_competition: true,
            final_eligible: false,
            ai_blocked: false,
            info_risk_blocked: false,
            standard_plan_overlap: false,
            rejection_reasons: Vec::new(),
            created_at: observed_at,
        }
    }

    fn candidate_market() -> RewardMarket {
        let now = OffsetDateTime::now_utc();
        RewardMarket {
            condition_id: "cond_candidate".to_string(),
            question: "Candidate market".to_string(),
            market_slug: "candidate-market".to_string(),
            event_slug: "candidate-event".to_string(),
            category: "politics".to_string(),
            image: String::new(),
            rewards_max_spread: Decimal::from(8),
            rewards_min_size: Decimal::from(5),
            total_daily_rate: Decimal::from(25),
            liquidity_usd: Decimal::from(10_000),
            volume_24h_usd: Decimal::from(25_000),
            market_spread_cents: Decimal::from(2),
            end_at: Some(now + Duration::days(30)),
            ambiguity_level: "low".to_string(),
            market_synced_at: Some(now),
            tokens: vec![
                RewardToken {
                    token_id: "yes_candidate".to_string(),
                    outcome: "YES".to_string(),
                    price: Some(Decimal::new(49, 2)),
                },
                RewardToken {
                    token_id: "no_candidate".to_string(),
                    outcome: "NO".to_string(),
                    price: Some(Decimal::new(51, 2)),
                },
            ],
            active: true,
            updated_at: now,
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
    async fn duplicate_pending_reward_command_is_coalesced() {
        let store = InMemoryRewardBotStore::new();
        let now = OffsetDateTime::now_utc();
        let mut first = running_command(now);
        first.id = "rewcmd_first".to_string();
        first.status = RewardControlCommandStatus::Pending;
        first.started_at = None;
        let mut duplicate = first.clone();
        duplicate.id = "rewcmd_duplicate".to_string();
        duplicate.trace_id = Some("trc_duplicate".to_string());

        assert!(
            store
                .enqueue_control_command(first)
                .await
                .expect("enqueue first command")
        );
        assert!(
            !store
                .enqueue_control_command(duplicate)
                .await
                .expect("coalesce duplicate command")
        );

        let claimed = store
            .claim_next_control_command("trc_claim", now)
            .await
            .expect("claim command")
            .expect("first command is claimable");

        assert_eq!(claimed.id, "rewcmd_first");
    }

    #[tokio::test]
    async fn external_open_order_count_excludes_local_intents() {
        let store = InMemoryRewardBotStore::new();
        store.orders.write().await.extend([
            open_order("submitted", Some("pm_order")),
            open_order("local_intent", None),
        ]);

        assert_eq!(
            store
                .count_open_orders("reward_live")
                .await
                .expect("count all open-like orders"),
            2
        );
        assert_eq!(
            store
                .count_external_open_orders("reward_live")
                .await
                .expect("count submitted open orders"),
            1
        );
    }

    #[tokio::test]
    async fn reward_history_prune_keeps_active_orders_and_ledger_rows() {
        let store = InMemoryRewardBotStore::new();
        let now = OffsetDateTime::now_utc();
        let cutoff = now - Duration::days(5);
        let old = cutoff - Duration::seconds(1);
        let recent = cutoff + Duration::seconds(1);

        store.orders.write().await.extend([
            order_with_status("old_cancelled", ManagedRewardOrderStatus::Cancelled, old),
            order_with_status("old_filled", ManagedRewardOrderStatus::Filled, old),
            order_with_status("old_error", ManagedRewardOrderStatus::Error, old),
            order_with_status("old_open", ManagedRewardOrderStatus::Open, old),
            order_with_status("old_planned", ManagedRewardOrderStatus::Planned, old),
            order_with_status("old_exit", ManagedRewardOrderStatus::ExitPending, old),
            order_with_status("recent_filled", ManagedRewardOrderStatus::Filled, recent),
        ]);
        store.events.write().await.extend([
            reward_event("old_event", old),
            reward_event("recent_event", recent),
        ]);
        store
            .low_competition_observations
            .write()
            .await
            .extend([
                low_competition_observation("old_observation", old),
                low_competition_observation("recent_observation", recent),
            ]);
        store.fills.write().await.push(reward_fill("fill_old", old));
        store
            .positions
            .write()
            .await
            .insert(("reward_live".to_string(), "token_position".to_string()), position("reward_live", "token_position", 3));

        let report = store
            .prune_history(cutoff)
            .await
            .expect("prune reward history");

        assert_eq!(report.terminal_orders_deleted, 3);
        assert_eq!(report.risk_events_deleted, 1);
        assert_eq!(report.low_competition_observations_deleted, 1);

        let order_ids = store
            .orders
            .read()
            .await
            .iter()
            .map(|order| order.id.clone())
            .collect::<HashSet<_>>();
        assert!(!order_ids.contains("old_cancelled"));
        assert!(!order_ids.contains("old_filled"));
        assert!(!order_ids.contains("old_error"));
        assert!(order_ids.contains("old_open"));
        assert!(order_ids.contains("old_planned"));
        assert!(order_ids.contains("old_exit"));
        assert!(order_ids.contains("recent_filled"));
        assert_eq!(store.fills.read().await.len(), 1);
        assert_eq!(store.positions.read().await.len(), 1);
        assert_eq!(store.events.read().await[0].id, "recent_event");
        assert_eq!(
            store.low_competition_observations.read().await[0].id,
            "recent_observation"
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
        assert_eq!(account_a_positions[0].size, Decimal::from(9));
        assert_eq!(
            store
                .list_account_positions("account_b")
                .await
                .expect("list account b")[0]
                .token_id,
            "other"
        );

        store
            .apply_account_sync(&account_a, Some(&[]), "trc_empty")
            .await
            .expect("apply empty account snapshot");
        assert!(
            store
                .list_account_positions("account_a")
                .await
                .expect("list empty account a")
                .is_empty()
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


    #[tokio::test]
    async fn in_memory_candidate_filter_matches_binary_midpoint_rules() {
        let store = InMemoryRewardBotStore::new();
        let filter = RewardBotConfig::default().candidate_filter();
        let valid = candidate_market();
        let mut invalid_outcome = valid.clone();
        invalid_outcome.condition_id = "invalid_outcome".to_string();
        invalid_outcome.tokens[1].outcome = "MAYBE".to_string();
        let mut invalid_midpoint = valid.clone();
        invalid_midpoint.condition_id = "invalid_midpoint".to_string();
        invalid_midpoint.tokens[0].price = Some(Decimal::new(1, 2));
        invalid_midpoint.tokens[1].price = Some(Decimal::new(99, 2));
        let mut high_min_size = valid.clone();
        high_min_size.condition_id = "high_min_size".to_string();
        high_min_size.rewards_min_size = Decimal::from(1_000);

        store
            .upsert_markets(&[valid.clone(), invalid_outcome, invalid_midpoint, high_min_size])
            .await
            .expect("seed candidate markets");

        let candidates = store
            .list_candidate_markets(&filter, 100)
            .await
            .expect("list candidates");

        assert_eq!(candidates.len(), 2);
        assert!(candidates
            .iter()
            .any(|candidate| candidate.condition_id == valid.condition_id));
        assert!(candidates
            .iter()
            .any(|candidate| candidate.condition_id == "high_min_size"));
    }

    #[tokio::test]
    async fn in_memory_candidate_filter_keeps_high_min_size_markets_for_live_balance_check() {
        let store = InMemoryRewardBotStore::new();
        let filter = RewardBotConfig {
            quote_mode: RewardQuoteMode::Auto,
            selection_mode: RewardSelectionMode::Enforce,
            dominant_single_side_enabled: true,
            per_market_usd: Decimal::from(20),
            ..RewardBotConfig::default()
        }
        .candidate_filter();
        let mut valid = candidate_market();
        valid.rewards_min_size = Decimal::from(50);

        store
            .upsert_markets(&[valid.clone()])
            .await
            .expect("seed candidate markets");

        let candidates = store
            .list_candidate_markets(&filter, 100)
            .await
            .expect("list candidates");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].condition_id, valid.condition_id);
    }
}
