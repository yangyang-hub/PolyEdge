#[derive(Debug, Clone, Copy)]
pub struct RewardActionPlannerContext<'a> {
    pub run_id: i64,
    pub trace_id: &'a str,
    pub now: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RewardOrderActionIntent {
    PlaceBuy,
    SubmitExitSell,
    CancelOrder,
    CancelReplaceExit,
}

impl RewardOrderActionIntent {
    #[must_use]
    pub const fn action_type(self) -> RewardStrategyActionType {
        match self {
            Self::PlaceBuy => RewardStrategyActionType::PlaceBuy,
            Self::SubmitExitSell => RewardStrategyActionType::SubmitExitSell,
            Self::CancelOrder => RewardStrategyActionType::CancelOrder,
            Self::CancelReplaceExit => RewardStrategyActionType::CancelReplaceExit,
        }
    }

    #[must_use]
    pub const fn reason_code(self) -> &'static str {
        match self {
            Self::PlaceBuy => "place_buy",
            Self::SubmitExitSell => "submit_exit_sell",
            Self::CancelOrder => "cancel_order",
            Self::CancelReplaceExit => "cancel_replace_exit",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RewardOrderActionProposal<'a> {
    pub order: &'a ManagedRewardOrder,
    pub intent: RewardOrderActionIntent,
    pub reason: &'a str,
    pub metadata: Value,
}

#[derive(Debug, Clone)]
pub struct RewardMergeActionProposal<'a> {
    pub intent: &'a RewardMergeIntent,
    pub action_type: RewardStrategyActionType,
    pub reason: &'a str,
    pub idempotency_suffix: &'a str,
    pub metadata: Value,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RewardActionPlanner;

impl RewardActionPlanner {
    /// Advance already-persisted planned actions immediately before their live
    /// side effects begin. The idempotency key is preserved so stores update
    /// the durable ledger row instead of inserting a second action.
    #[must_use]
    pub fn mark_actions_executing(
        actions: &[RewardStrategyAction],
        now: OffsetDateTime,
    ) -> Vec<RewardStrategyAction> {
        actions
            .iter()
            .map(|action| Self::transition_action(
                action,
                RewardStrategyActionStatus::Executing,
                now,
                action.reason.as_str(),
                json!({
                    "status": RewardStrategyActionStatus::Executing.as_str(),
                    "phase": "executing",
                }),
            ))
            .collect()
    }

    #[must_use]
    pub fn transition_action(
        action: &RewardStrategyAction,
        status: RewardStrategyActionStatus,
        now: OffsetDateTime,
        reason: &str,
        result_json: Value,
    ) -> RewardStrategyAction {
        let mut transitioned = action.clone();
        transitioned.status = status;
        transitioned.reason = reason.to_string();
        transitioned.result_json = result_json;
        transitioned.updated_at = now;
        if status.is_terminal() {
            transitioned.lease_owner = None;
            transitioned.lease_expires_at = None;
        }
        transitioned
    }

    #[must_use]
    pub fn plan_order_action(
        context: RewardActionPlannerContext<'_>,
        proposal: RewardOrderActionProposal<'_>,
    ) -> RewardStrategyAction {
        let action_type = proposal.intent.action_type();
        let request_json = RewardDurableActionEnvelope::order(
            proposal.intent,
            proposal.reason,
            proposal.order,
            proposal.metadata,
        )
        .to_json()
        .unwrap_or_else(|_| json!({}));
        RewardStrategyAction {
            action_id: 0,
            run_id: context.run_id,
            account_id: proposal.order.account_id.clone(),
            condition_id: Some(proposal.order.condition_id.clone()),
            token_id: Some(proposal.order.token_id.clone()),
            managed_order_id: Some(proposal.order.id.clone()),
            external_order_id: proposal.order.external_order_id.clone(),
            action_type,
            status: RewardStrategyActionStatus::Planned,
            reason_code: proposal.intent.reason_code().to_string(),
            reason: proposal.reason.to_string(),
            idempotency_key: reward_order_action_idempotency_key(context.trace_id, proposal.order),
            request_json,
            result_json: json!({ "status": RewardStrategyActionStatus::Planned.as_str() }),
            lease_owner: None,
            lease_expires_at: None,
            execution_attempts: 0,
            created_at: context.now,
            updated_at: context.now,
        }
    }

    #[must_use]
    pub fn plan_order_actions(
        context: RewardActionPlannerContext<'_>,
        proposals: &[RewardOrderActionProposal<'_>],
    ) -> Vec<RewardStrategyAction> {
        proposals
            .iter()
            .cloned()
            .map(|proposal| Self::plan_order_action(context, proposal))
            .collect()
    }

    #[must_use]
    pub fn plan_pending_order_submissions(
        context: RewardActionPlannerContext<'_>,
        orders: &[ManagedRewardOrder],
        allow_buy_submit: bool,
    ) -> Vec<RewardStrategyAction> {
        orders
            .iter()
            .filter(|order| order.external_order_id.is_none())
            .filter(|order| {
                (order.side == RewardOrderSide::Buy
                    && order.status == ManagedRewardOrderStatus::Planned
                    && allow_buy_submit)
                    || (order.side == RewardOrderSide::Sell
                        && matches!(
                            order.status,
                            ManagedRewardOrderStatus::Planned
                                | ManagedRewardOrderStatus::ExitPending
                        ))
            })
            .map(|order| {
                let intent = match order.side {
                    RewardOrderSide::Buy => RewardOrderActionIntent::PlaceBuy,
                    RewardOrderSide::Sell => RewardOrderActionIntent::SubmitExitSell,
                };
                Self::plan_order_action(
                    context,
                    RewardOrderActionProposal {
                        order,
                        intent,
                        reason: order.reason.as_str(),
                        metadata: json!({
                            "source": "pending_submission",
                            "allow_buy_submit": allow_buy_submit,
                        }),
                    },
                )
            })
            .collect()
    }

    #[must_use]
    pub fn plan_merge_action(
        context: RewardActionPlannerContext<'_>,
        proposal: RewardMergeActionProposal<'_>,
    ) -> RewardStrategyAction {
        let action_type = proposal.action_type;
        let request_json = RewardDurableActionEnvelope::merge(
            action_type,
            proposal.reason,
            proposal.intent,
            proposal.metadata,
        )
        .and_then(|envelope| envelope.to_json())
        .unwrap_or_else(|_| json!({}));
        let idempotency_key = if proposal.idempotency_suffix.is_empty() {
            reward_merge_action_idempotency_key(context.trace_id, proposal.intent)
        } else {
            format!(
                "{}:{}",
                reward_merge_action_idempotency_key(context.trace_id, proposal.intent),
                proposal.idempotency_suffix
            )
        };
        RewardStrategyAction {
            action_id: 0,
            run_id: context.run_id,
            account_id: proposal.intent.account_id.clone(),
            condition_id: Some(proposal.intent.condition_id.clone()),
            token_id: None,
            managed_order_id: None,
            external_order_id: proposal.intent.tx_hash.clone(),
            action_type,
            status: RewardStrategyActionStatus::Planned,
            reason_code: action_type.as_str().to_string(),
            reason: proposal.reason.to_string(),
            idempotency_key,
            request_json,
            result_json: json!({ "status": RewardStrategyActionStatus::Planned.as_str() }),
            lease_owner: None,
            lease_expires_at: None,
            execution_attempts: 0,
            created_at: context.now,
            updated_at: context.now,
        }
    }

    #[must_use]
    pub fn merge_execution_result_action(
        context: RewardActionPlannerContext<'_>,
        intent: &RewardMergeIntent,
        status: RewardStrategyActionStatus,
        reason: &str,
        result_json: Value,
    ) -> RewardStrategyAction {
        RewardStrategyAction {
            action_id: 0,
            run_id: context.run_id,
            account_id: intent.account_id.clone(),
            condition_id: Some(intent.condition_id.clone()),
            token_id: None,
            managed_order_id: None,
            external_order_id: intent.tx_hash.clone(),
            action_type: RewardStrategyActionType::ExecuteMerge,
            status,
            reason_code: RewardStrategyActionType::ExecuteMerge.as_str().to_string(),
            reason: reason.to_string(),
            idempotency_key: format!(
                "{}:execute",
                reward_merge_action_idempotency_key(context.trace_id, intent)
            ),
            request_json: serde_json::to_value(intent).unwrap_or_else(|_| json!({})),
            result_json,
            lease_owner: None,
            lease_expires_at: None,
            execution_attempts: 0,
            created_at: context.now,
            updated_at: context.now,
        }
    }
}

#[must_use]
pub fn reward_order_action_idempotency_key(
    trace_id: &str,
    order: &ManagedRewardOrder,
) -> String {
    format!("{trace_id}:order:{}", order.id)
}

#[must_use]
pub fn reward_merge_action_idempotency_key(
    trace_id: &str,
    intent: &RewardMergeIntent,
) -> String {
    format!("{trace_id}:merge:{}", intent.id)
}
