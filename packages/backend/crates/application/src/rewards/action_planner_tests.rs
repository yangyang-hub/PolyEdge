fn planner_now() -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(1_725_000_000).expect("valid timestamp")
}

fn planner_order(
    id: &str,
    side: RewardOrderSide,
    status: ManagedRewardOrderStatus,
) -> ManagedRewardOrder {
    ManagedRewardOrder {
        id: id.to_string(),
        account_id: "acct".to_string(),
        condition_id: "cond".to_string(),
        token_id: "token".to_string(),
        outcome: "YES".to_string(),
        side,
        price: decimal("0.42"),
        size: decimal("12.5"),
        strategy_bucket: RewardStrategyBucket::Standard,
        strategy_profile: RewardStrategyProfile::Standard,
        exit_strategy_source: RewardExitStrategySource::Configured,
        exit_strategy_selected: None,
        exit_floor_price: None,
        exit_reselect_count: 0,
        exit_last_reselected_at: None,
        external_order_id: None,
        status,
        scoring: true,
        reason: "planned by test".to_string(),
        filled_size: Decimal::ZERO,
        reward_earned: Decimal::ZERO,
        last_scored_at: None,
        created_at: planner_now(),
        updated_at: planner_now(),
    }
}

fn planner_merge_intent() -> RewardMergeIntent {
    RewardMergeIntent {
        id: "merge-1".to_string(),
        account_id: "acct".to_string(),
        condition_id: "cond".to_string(),
        yes_token_id: "yes".to_string(),
        no_token_id: "no".to_string(),
        merge_size: decimal("4"),
        yes_position_size: decimal("4"),
        no_position_size: decimal("4"),
        yes_avg_price: decimal("0.45"),
        no_avg_price: decimal("0.47"),
        status: RewardMergeIntentStatus::Pending,
        reason: "paired inventory".to_string(),
        source_fill_id: "fill-1".to_string(),
        tx_hash: None,
        submitted_at: None,
        confirmed_at: None,
        failed_reason: None,
        retry_count: 0,
        trace_id: "source-trace".to_string(),
        created_at: planner_now(),
        updated_at: planner_now(),
    }
}

#[test]
fn planned_order_action_uses_outcome_idempotency_key() {
    let order = planner_order("order-1", RewardOrderSide::Buy, ManagedRewardOrderStatus::Planned);
    let context = RewardActionPlannerContext {
        run_id: 7,
        trace_id: "trace",
        now: planner_now(),
    };

    let action = RewardActionPlanner::plan_order_action(
        context,
        RewardOrderActionProposal {
            order: &order,
            intent: RewardOrderActionIntent::PlaceBuy,
            reason: "submit candidate",
            metadata: json!({ "source": "test" }),
        },
    );

    assert_eq!(action.run_id, 7);
    assert_eq!(action.action_type, RewardStrategyActionType::PlaceBuy);
    assert_eq!(action.status, RewardStrategyActionStatus::Planned);
    assert_eq!(action.idempotency_key, "trace:order:order-1");
    assert_eq!(action.request_json["phase"], "planned");
    assert_eq!(action.request_json["metadata"]["source"], "test");
}

#[test]
fn pending_submission_planner_filters_blocked_buys_but_keeps_sells() {
    let buy = planner_order("buy-1", RewardOrderSide::Buy, ManagedRewardOrderStatus::Planned);
    let sell = planner_order(
        "sell-1",
        RewardOrderSide::Sell,
        ManagedRewardOrderStatus::ExitPending,
    );
    let context = RewardActionPlannerContext {
        run_id: 9,
        trace_id: "trace",
        now: planner_now(),
    };

    let actions = RewardActionPlanner::plan_pending_order_submissions(
        context,
        &[buy.clone(), sell],
        false,
    );

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].managed_order_id.as_deref(), Some("sell-1"));
    assert_eq!(
        actions[0].action_type,
        RewardStrategyActionType::SubmitExitSell
    );

    let actions = RewardActionPlanner::plan_pending_order_submissions(context, &[buy], true);
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].managed_order_id.as_deref(), Some("buy-1"));
    assert_eq!(actions[0].action_type, RewardStrategyActionType::PlaceBuy);
}

#[test]
fn merge_execution_result_uses_distinct_execute_key() {
    let intent = planner_merge_intent();
    let context = RewardActionPlannerContext {
        run_id: 11,
        trace_id: "trace",
        now: planner_now(),
    };

    let planned = RewardActionPlanner::plan_merge_action(
        context,
        RewardMergeActionProposal {
            intent: &intent,
            action_type: RewardStrategyActionType::CreateMergeIntent,
            reason: "create merge",
            idempotency_suffix: "",
            metadata: json!({}),
        },
    );
    let executed = RewardActionPlanner::merge_execution_result_action(
        context,
        &intent,
        RewardStrategyActionStatus::Succeeded,
        "submitted merge",
        json!({ "tx_hash": "0x1" }),
    );

    assert_eq!(planned.idempotency_key, "trace:merge:merge-1");
    assert_eq!(executed.idempotency_key, "trace:merge:merge-1:execute");
    assert_eq!(executed.action_type, RewardStrategyActionType::ExecuteMerge);
    assert_eq!(executed.status, RewardStrategyActionStatus::Succeeded);
}

#[test]
fn skipped_pre_submit_buy_keeps_place_buy_action_type() {
    let mut order = planner_order(
        "buy-1",
        RewardOrderSide::Buy,
        ManagedRewardOrderStatus::Cancelled,
    );
    order.reason =
        "local-only order cancelled by live submission last-look: fair value failed".to_string();
    let outcome = RewardTickOutcome {
        account: RewardAccountState::fresh("acct", decimal("100"), planner_now()),
        markets: Vec::new(),
        plans: Vec::new(),
        orders: vec![order],
        positions: Vec::new(),
        fills: Vec::new(),
        merge_intents: Vec::new(),
        events: Vec::new(),
        report: RewardBotRunReport::default(),
    };

    let actions = reward_strategy_actions_from_tick_outcome(13, &outcome, "trace", planner_now());

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].action_type, RewardStrategyActionType::PlaceBuy);
    assert_eq!(actions[0].status, RewardStrategyActionStatus::Skipped);
}
