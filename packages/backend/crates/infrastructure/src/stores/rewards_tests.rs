#[cfg(test)]
mod rewards_tests {
    use super::*;
    use sqlx::{Executor, postgres::PgPoolOptions};
    use std::error::Error;

    static REWARD_TEST_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

    fn quote_pg_ident(value: &str) -> String {
        format!(r#""{}""#, value.replace('"', r#"""""#))
    }

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
            lease_owner: Some("trc_old".to_string()),
            lease_version: 1,
            lease_expires_at: Some(started_at + REWARD_CONTROL_COMMAND_LEASE),
            error: None,
        }
    }

    fn strategy_action(
        idempotency_key: &str,
        account_id: &str,
        status: RewardStrategyActionStatus,
        created_at: OffsetDateTime,
    ) -> RewardStrategyAction {
        RewardStrategyAction {
            action_id: 0,
            run_id: 1,
            account_id: account_id.to_string(),
            condition_id: Some("cond_live".to_string()),
            token_id: Some("token_live".to_string()),
            managed_order_id: Some(format!("order_{idempotency_key}")),
            external_order_id: None,
            action_type: RewardStrategyActionType::PlaceBuy,
            status,
            reason_code: "test_action".to_string(),
            reason: "test action".to_string(),
            idempotency_key: idempotency_key.to_string(),
            request_json: json!({}),
            result_json: json!({}),
            lease_owner: None,
            lease_expires_at: None,
            execution_attempts: 0,
            created_at,
            updated_at: created_at,
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

    fn merge_intent(id: &str, status: RewardMergeIntentStatus) -> RewardMergeIntent {
        let now = OffsetDateTime::now_utc();
        RewardMergeIntent {
            id: id.to_string(),
            account_id: "reward_live".to_string(),
            condition_id: "cond_merge".to_string(),
            yes_token_id: "yes_merge".to_string(),
            no_token_id: "no_merge".to_string(),
            merge_size: Decimal::from(5),
            yes_position_size: Decimal::from(5),
            no_position_size: Decimal::from(5),
            yes_avg_price: Decimal::new(45, 2),
            no_avg_price: Decimal::new(45, 2),
            status,
            reason: "merge test".to_string(),
            source_fill_id: format!("fill_{id}"),
            tx_hash: None,
            submitted_at: None,
            confirmed_at: None,
            failed_reason: None,
            retry_count: 0,
            trace_id: "trace_merge".to_string(),
            created_at: now,
            updated_at: now,
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
            strategy_profile: RewardStrategyProfile::Standard,
            exit_strategy_source: RewardExitStrategySource::Configured,
            exit_strategy_selected: None,
            exit_floor_price: None,
            exit_reselect_count: 0,
            exit_last_reselected_at: None,
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

    fn replay_fixture(now: OffsetDateTime) -> RewardDecisionReplayFixture {
        RewardDecisionReplayFixture {
            schema_version: polyedge_application::REWARD_DECISION_REPLAY_SCHEMA_VERSION,
            input: polyedge_application::RewardStrategyInput {
                now,
                force_orders: false,
                config: RewardBotConfig::default(),
                candidate_markets: Vec::new(),
                plans: Vec::new(),
                previous_plans: Vec::new(),
                pre_ai_eligible_condition_ids: Vec::new(),
                books: HashMap::new(),
                book_history: HashMap::new(),
                account: RewardAccountState::fresh("reward_live", Decimal::from(100), now),
                open_orders: Vec::new(),
                positions: Vec::new(),
                event_windows: Vec::new(),
            },
            providers: polyedge_application::RewardReplayProviderSnapshot::default(),
            final_state: None,
            expected_plans: Some(Vec::new()),
        }
    }

    #[tokio::test]
    async fn in_memory_strategy_replay_fixture_round_trip_requires_existing_run() {
        let store = InMemoryRewardBotStore::new();
        let now = OffsetDateTime::from_unix_timestamp(1_750_000_000).expect("fixed timestamp");
        let missing = RewardStrategyReplayFixture::capture(1, replay_fixture(now), now)
            .expect("capture missing fixture");
        let error = store
            .save_strategy_replay_fixture(&missing)
            .await
            .expect_err("fixture cannot precede run");
        assert_eq!(error.code(), "REWARD_STRATEGY_RUN_NOT_FOUND");

        let run_id = store
            .start_strategy_run(&RewardStrategyRunStart {
                account_id: "reward_live".to_string(),
                trace_id: "trace_replay_fixture".to_string(),
                trigger_type: RewardStrategyRunTrigger::Poll,
                config_hash: "config-hash".to_string(),
                config_json: json!({}),
                input_summary: json!({}),
                started_at: now,
            })
            .await
            .expect("start run");
        let fixture = RewardStrategyReplayFixture::capture(run_id, replay_fixture(now), now)
            .expect("capture fixture");
        store
            .save_strategy_replay_fixture(&fixture)
            .await
            .expect("save fixture");

        let stored = store
            .get_strategy_replay_fixture(run_id)
            .await
            .expect("load fixture")
            .expect("stored fixture");
        assert_eq!(stored, fixture);
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

    fn quote_plan_for_profile(profile: RewardStrategyProfile) -> RewardQuotePlan {
        let now = OffsetDateTime::now_utc();
        RewardQuotePlan {
            condition_id: "cond_shared".to_string(),
            market_slug: "shared-market".to_string(),
            question: "Shared condition".to_string(),
            score: match profile {
                RewardStrategyProfile::Standard => Decimal::from(20),
                RewardStrategyProfile::BalancedMerge => Decimal::from(15),
            },
            selection_score: Decimal::ZERO,
            eligible: true,
            pre_ai_eligible: true,
            quote_readiness: polyedge_application::RewardQuoteReadiness::ReadyToQuote,
            reason: "eligible".to_string(),
            strategy_bucket: RewardStrategyBucket::Standard,
            strategy_profile: profile,
            latest_run_id: None,
            quote_mode: RewardPlanQuoteMode::Double,
            recommended_quote_mode: Some(RewardPlanQuoteMode::Double),
            book_metrics: None,
            opportunity_metrics: None,
            selection_metrics: None,
            fair_value: None,
            ai_advisory: None,
            info_risk: None,
            event_window: None,
            midpoint: Some(Decimal::new(50, 2)),
            live_skip_until: None,
            live_skip_reason: None,
            first_quote_observed_at: None,
            ai_advisory_pending_since: None,
            info_risk_pending_since: None,
            total_daily_rate: Decimal::from(25),
            rewards_max_spread: Decimal::from(8),
            rewards_min_size: Decimal::from(5),
            orderbook_token_ids: vec!["yes_shared".to_string(), "no_shared".to_string()],
            legs: vec![polyedge_application::RewardQuoteLeg {
                token_id: "yes_shared".to_string(),
                outcome: "YES".to_string(),
                side: RewardOrderSide::Buy,
                price: Decimal::new(49, 2),
                size: Decimal::from(20),
                notional_usd: Decimal::new(980, 2),
            }],
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
        assert_eq!(claimed.lease_owner.as_deref(), Some("trc_new"));
        assert_eq!(claimed.lease_version, 2);
        assert_eq!(
            claimed.lease_expires_at,
            Some(now + REWARD_CONTROL_COMMAND_LEASE)
        );

        let stale_finalize = store
            .complete_control_command("rewcmd_lease", "trc_old", 1, now)
            .await;
        assert!(stale_finalize.is_err());

        store
            .complete_control_command("rewcmd_lease", "trc_new", 2, now)
            .await
            .expect("current lease owner completes command");
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
    async fn completed_merge_intent_does_not_reserve_future_inventory() {
        let store = InMemoryRewardBotStore::new();
        store
            .create_merge_intent_if_absent(&merge_intent(
                "merge_completed",
                RewardMergeIntentStatus::Completed,
            ))
            .await
            .expect("insert completed merge intent");

        assert_eq!(
            store
                .active_merge_intent_size("reward_live", "cond_merge")
                .await
                .expect("active merge size"),
            Decimal::ZERO
        );
    }

    #[tokio::test]
    async fn merge_broadcast_fence_is_one_way_and_not_executable() {
        let store = InMemoryRewardBotStore::new();
        let intent = merge_intent("merge_broadcast", RewardMergeIntentStatus::Pending);
        store
            .create_merge_intent_if_absent(&intent)
            .await
            .expect("insert merge intent");
        let now = OffsetDateTime::now_utc();
        store
            .mark_merge_intent_broadcasting(&intent.id, now, "broadcast fenced")
            .await
            .expect("fence broadcast");

        assert!(
            store
                .list_executable_merge_intents("reward_live", 10)
                .await
                .expect("list executable intents")
                .is_empty()
        );
        assert!(
            store
                .mark_merge_intent_broadcasting(&intent.id, now, "duplicate")
                .await
                .is_err()
        );
        store
            .mark_merge_intent_submitted(&intent.id, "0xtx", now, "submitted")
            .await
            .expect("broadcasting intent becomes submitted");
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
    async fn quote_plans_keep_distinct_strategy_profiles_for_same_condition() {
        let store = InMemoryRewardBotStore::new();
        let standard = quote_plan_for_profile(RewardStrategyProfile::Standard);
        let balanced_merge = quote_plan_for_profile(RewardStrategyProfile::BalancedMerge);

        store
            .save_quote_plans(&[standard, balanced_merge])
            .await
            .expect("save quote plans");

        let plans = store
            .list_all_quote_plans()
            .await
            .expect("list all quote plans");
        let profiles = plans
            .iter()
            .map(|plan| plan.strategy_profile)
            .collect::<HashSet<_>>();
        assert_eq!(plans.len(), 2);
        assert!(profiles.contains(&RewardStrategyProfile::Standard));
        assert!(profiles.contains(&RewardStrategyProfile::BalancedMerge));

        let page = store
            .list_quote_plans_page(&RewardQuotePlanListQuery::default())
            .await
            .expect("list quote plan page");
        assert_eq!(page.page.total_items, 2);
    }

    #[tokio::test]
    async fn external_open_order_count_excludes_local_intents() {
        let store = InMemoryRewardBotStore::new();
        let mut awaiting_reconciliation = open_order("awaiting_reconciliation", Some("pm_cancelled"));
        awaiting_reconciliation.reason =
            "cancel accepted; awaiting final reconciliation".to_string();
        let internal_id = open_order("internal_id", Some("rewlive_local"));
        store.orders.write().await.extend([
            open_order("submitted", Some("pm_order")),
            open_order("local_intent", None),
            awaiting_reconciliation,
            internal_id,
        ]);

        assert_eq!(
            store
                .count_open_orders("reward_live")
                .await
                .expect("count all open-like orders"),
            4
        );
        assert_eq!(
            store
                .count_external_open_orders("reward_live")
                .await
                .expect("count externally live open orders"),
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
    async fn in_memory_candidate_filter_accepts_either_market_activity_signal() {
        let store = InMemoryRewardBotStore::new();
        let filter = RewardBotConfig::default().candidate_filter();

        let mut liquidity_only = candidate_market();
        liquidity_only.condition_id = "liquidity_only".to_string();
        liquidity_only.volume_24h_usd = filter.min_market_volume_24h_usd - Decimal::ONE;

        let mut volume_only = candidate_market();
        volume_only.condition_id = "volume_only".to_string();
        volume_only.liquidity_usd = filter.min_market_liquidity_usd - Decimal::ONE;

        let mut inactive_market = candidate_market();
        inactive_market.condition_id = "inactive_market".to_string();
        inactive_market.liquidity_usd = filter.min_market_liquidity_usd - Decimal::ONE;
        inactive_market.volume_24h_usd = filter.min_market_volume_24h_usd - Decimal::ONE;

        store
            .upsert_markets(&[liquidity_only, volume_only, inactive_market])
            .await
            .expect("seed candidate markets");

        let candidates = store
            .list_candidate_markets(&filter, 100)
            .await
            .expect("list candidates");

        assert_eq!(candidates.len(), 2);
        assert!(candidates
            .iter()
            .any(|candidate| candidate.condition_id == "liquidity_only"));
        assert!(candidates
            .iter()
            .any(|candidate| candidate.condition_id == "volume_only"));
        assert!(!candidates
            .iter()
            .any(|candidate| candidate.condition_id == "inactive_market"));
    }

    #[tokio::test]
    async fn in_memory_candidate_filter_keeps_high_min_size_markets_for_live_balance_check() {
        let store = InMemoryRewardBotStore::new();
        let filter = RewardBotConfig {
            quote_mode: RewardQuoteMode::Auto,
            selection_mode: RewardSelectionMode::Enforce,
            dominant_single_side_enabled: true,
            maker_market_budget_usd: Decimal::from(20),
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

    #[tokio::test]
    async fn in_memory_strategy_action_claim_is_account_scoped_and_skips_unsafe_retries() {
        let store = InMemoryRewardBotStore::new();
        let now = OffsetDateTime::now_utc();
        let actions = vec![
            strategy_action(
                "planned",
                "reward_live",
                RewardStrategyActionStatus::Planned,
                now,
            ),
            strategy_action(
                "other_account",
                "other",
                RewardStrategyActionStatus::Planned,
                now,
            ),
            strategy_action(
                "unknown",
                "reward_live",
                RewardStrategyActionStatus::Unknown,
                now,
            ),
            strategy_action(
                "unleased_executing",
                "reward_live",
                RewardStrategyActionStatus::Executing,
                now,
            ),
        ];
        store
            .record_strategy_actions(&actions)
            .await
            .expect("seed strategy actions");

        let claimed = store
            .claim_strategy_actions(
                "reward_live",
                "executor-a",
                now,
                now + Duration::seconds(30),
                10,
            )
            .await
            .expect("claim strategy actions");

        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].idempotency_key, "planned");
        assert_eq!(claimed[0].status, RewardStrategyActionStatus::Executing);
        assert_eq!(claimed[0].lease_owner.as_deref(), Some("executor-a"));
        assert_eq!(claimed[0].execution_attempts, 1);
    }

    #[tokio::test]
    async fn in_memory_strategy_action_expired_lease_can_be_recovered_and_renewed_by_owner() {
        let store = InMemoryRewardBotStore::new();
        let now = OffsetDateTime::now_utc();
        let mut action = strategy_action(
            "recoverable",
            "reward_live",
            RewardStrategyActionStatus::Executing,
            now - Duration::minutes(1),
        );
        action.lease_owner = Some("executor-a".to_string());
        action.lease_expires_at = Some(now - Duration::seconds(1));
        action.execution_attempts = 1;
        store
            .record_strategy_actions(&[action])
            .await
            .expect("seed expired strategy action");

        let claimed = store
            .claim_strategy_actions(
                "reward_live",
                "executor-b",
                now,
                now + Duration::seconds(30),
                1,
            )
            .await
            .expect("recover expired action");
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].lease_owner.as_deref(), Some("executor-b"));
        assert_eq!(claimed[0].execution_attempts, 2);

        assert!(!store
            .renew_strategy_action_lease(
                claimed[0].action_id,
                "executor-a",
                now,
                now + Duration::minutes(1),
            )
            .await
            .expect("reject stale owner renewal"));
        assert!(store
            .renew_strategy_action_lease(
                claimed[0].action_id,
                "executor-b",
                now,
                now + Duration::minutes(1),
            )
            .await
            .expect("renew current owner lease"));
    }

    #[tokio::test]
    async fn in_memory_strategy_action_resolution_and_release_are_owner_fenced() {
        let store = InMemoryRewardBotStore::new();
        let now = OffsetDateTime::now_utc();
        store
            .record_strategy_actions(&[strategy_action(
                "owner_fenced",
                "reward_live",
                RewardStrategyActionStatus::Planned,
                now,
            )])
            .await
            .expect("seed owner-fenced action");

        let claimed = store
            .claim_strategy_actions(
                "reward_live",
                "executor-a",
                now,
                now + Duration::minutes(1),
                1,
            )
            .await
            .expect("claim owner-fenced action");
        let action_id = claimed[0].action_id;
        assert_eq!(
            store
                .get_strategy_action(action_id)
                .await
                .expect("get claimed action")
                .expect("claimed action exists")
                .lease_owner
                .as_deref(),
            Some("executor-a")
        );

        assert!(!store
            .release_strategy_action_lease(
                action_id,
                "executor-b",
                "retry",
                "wrong owner",
                json!({ "retry": true }),
                now + Duration::seconds(1),
            )
            .await
            .expect("reject stale owner release"));
        assert!(store
            .release_strategy_action_lease(
                action_id,
                "executor-a",
                "retry",
                "safe to retry",
                json!({ "retry": true }),
                now + Duration::seconds(1),
            )
            .await
            .expect("release current owner lease"));

        let released = store
            .get_strategy_action(action_id)
            .await
            .expect("get released action")
            .expect("released action exists");
        assert_eq!(released.status, RewardStrategyActionStatus::Planned);
        assert_eq!(released.lease_owner, None);
        assert_eq!(released.result_json["status"], "planned");

        let reclaimed = store
            .claim_strategy_actions(
                "reward_live",
                "executor-b",
                now + Duration::seconds(2),
                now + Duration::minutes(1),
                1,
            )
            .await
            .expect("reclaim released action");
        assert_eq!(reclaimed[0].execution_attempts, 2);

        let mut terminal = reclaimed[0].clone();
        terminal.status = RewardStrategyActionStatus::Unknown;
        terminal.reason_code = "connector_result_unknown".to_string();
        terminal.reason = "connector result could not be confirmed".to_string();
        terminal.updated_at = now + Duration::seconds(3);
        terminal.result_json = json!({ "status": "unknown" });
        assert!(!store
            .finalize_strategy_action_lease(&terminal, "executor-a")
            .await
            .expect("reject previous owner finalize"));
        assert!(store
            .finalize_strategy_action_lease(&terminal, "executor-b")
            .await
            .expect("finalize current owner action"));

        let resolved = store
            .get_strategy_action(action_id)
            .await
            .expect("get resolved action")
            .expect("resolved action exists");
        assert_eq!(resolved.status, RewardStrategyActionStatus::Unknown);
        assert_eq!(resolved.lease_owner, None);
        assert_eq!(resolved.execution_attempts, 2);
    }

    #[tokio::test]
    async fn in_memory_strategy_action_expired_owner_cannot_renew_finalize_or_release() {
        let store = InMemoryRewardBotStore::new();
        let now = OffsetDateTime::now_utc();
        let mut action = strategy_action(
            "expired_fence",
            "reward_live",
            RewardStrategyActionStatus::Executing,
            now - Duration::minutes(1),
        );
        action.lease_owner = Some("executor-a".to_string());
        action.lease_expires_at = Some(now - Duration::seconds(1));
        store
            .record_strategy_actions(&[action.clone()])
            .await
            .expect("seed expired action");
        let action_id = store
            .strategy_actions
            .read()
            .await
            .first()
            .expect("stored action")
            .action_id;

        assert!(!store
            .renew_strategy_action_lease(
                action_id,
                "executor-a",
                now,
                now + Duration::minutes(1),
            )
            .await
            .expect("reject expired renewal"));
        action.action_id = action_id;
        action.status = RewardStrategyActionStatus::Failed;
        action.updated_at = now;
        assert!(!store
            .finalize_strategy_action_lease(&action, "executor-a")
            .await
            .expect("reject expired finalize"));
        assert!(!store
            .release_strategy_action_lease(
                action_id,
                "executor-a",
                "retry",
                "expired",
                json!({}),
                now,
            )
            .await
            .expect("reject expired release"));
    }

    #[tokio::test]
    async fn postgres_strategy_action_release_and_finalize_are_owner_fenced()
    -> std::result::Result<(), Box<dyn Error>> {
        let Some(database_url) = std::env::var("POLYEDGE_TEST_DATABASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
        else {
            return Ok(());
        };

        let schema = format!("polyedge_reward_test_{}", Uuid::now_v7().simple());
        let quoted_schema = quote_pg_ident(&schema);
        let admin_pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await?;
        admin_pool
            .execute(format!("CREATE SCHEMA {quoted_schema}").as_str())
            .await?;

        let test_result: std::result::Result<(), Box<dyn Error>> = async {
            let search_path_schema = quoted_schema.clone();
            let pool = PgPoolOptions::new()
                .max_connections(2)
                .after_connect(move |connection, _meta| {
                    let search_path_schema = search_path_schema.clone();
                    Box::pin(async move {
                        connection
                            .execute(format!("SET search_path TO {search_path_schema}").as_str())
                            .await?;
                        Ok(())
                    })
                })
                .connect(&database_url)
                .await?;
            REWARD_TEST_MIGRATOR.run(&pool).await?;

            let store = PostgresRewardBotStore::new(pool.clone());
            let now = OffsetDateTime::now_utc();
            let run_id = store
                .start_strategy_run(&RewardStrategyRunStart {
                    account_id: "reward_live".to_string(),
                    trace_id: "trace_postgres_action_fence".to_string(),
                    trigger_type: RewardStrategyRunTrigger::Poll,
                    config_hash: "config-hash".to_string(),
                    config_json: json!({}),
                    input_summary: json!({}),
                    started_at: now,
                })
                .await?;
            let mut action = strategy_action(
                "postgres_owner_fenced",
                "reward_live",
                RewardStrategyActionStatus::Planned,
                now,
            );
            action.run_id = run_id;
            store.record_strategy_actions(&[action]).await?;

            let claimed = store
                .claim_strategy_actions(
                    "reward_live",
                    "executor-a",
                    now,
                    now + Duration::minutes(2),
                    1,
                )
                .await?;
            let action_id = claimed[0].action_id;
            assert!(!store
                .release_strategy_action_lease(
                    action_id,
                    "executor-b",
                    "retry",
                    "wrong owner",
                    json!({}),
                    now + Duration::seconds(1),
                )
                .await?);
            assert!(store
                .release_strategy_action_lease(
                    action_id,
                    "executor-a",
                    "retry",
                    "safe retry",
                    json!({ "retry": true }),
                    now + Duration::seconds(1),
                )
                .await?);

            let reclaimed = store
                .claim_strategy_actions(
                    "reward_live",
                    "executor-b",
                    now + Duration::seconds(2),
                    now + Duration::minutes(2),
                    1,
                )
                .await?;
            let mut terminal = reclaimed[0].clone();
            terminal.status = RewardStrategyActionStatus::Unknown;
            terminal.reason_code = "connector_result_unknown".to_string();
            terminal.reason = "connector result could not be confirmed".to_string();
            terminal.result_json = json!({ "status": "unknown" });
            terminal.updated_at = now + Duration::seconds(3);
            assert!(!store
                .finalize_strategy_action_lease(&terminal, "executor-a")
                .await?);
            assert!(store
                .finalize_strategy_action_lease(&terminal, "executor-b")
                .await?);

            let resolved = store
                .get_strategy_action(action_id)
                .await?
                .expect("resolved Postgres action exists");
            assert_eq!(resolved.status, RewardStrategyActionStatus::Unknown);
            assert_eq!(resolved.lease_owner, None);
            assert_eq!(resolved.execution_attempts, 2);

            pool.close().await;
            Ok(())
        }
        .await;

        admin_pool
            .execute(format!("DROP SCHEMA IF EXISTS {quoted_schema} CASCADE").as_str())
            .await?;
        admin_pool.close().await;
        test_result
    }

    #[test]
    fn reward_config_key_value_round_trip_preserves_market_maker_v2_fields() {
        let expected = RewardBotConfig {
            maker_market_budget_usd: Decimal::from(37),
            quote_bid_rank: 2,
            quote_max_bid_rank: 3,
            inventory_skew_strength: Decimal::new(63, 2),
            ai_action_min_confidence: Decimal::new(81, 2),
            info_risk_min_confidence: Decimal::new(77, 2),
            maker_max_exit_loss_cents: Decimal::new(15, 1),
            adverse_requote_drift_cents: Decimal::new(4, 1),
            adverse_requote_confirm_sec: 2,
            ..RewardBotConfig::default()
        }
        .normalized();
        let mut decoded = RewardBotConfig::default();

        for (key, value) in reward_config_entries(&expected) {
            apply_reward_config_value(&mut decoded, key, &value).expect("decode config entry");
        }

        assert_eq!(decoded.normalized(), expected);
    }
}
