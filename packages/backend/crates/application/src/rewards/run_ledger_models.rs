#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardStrategyRunTrigger {
    Poll,
    RunOnce,
    OrderbookEvent,
    ControlCommand,
    Replay,
}

impl RewardStrategyRunTrigger {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Poll => "poll",
            Self::RunOnce => "run_once",
            Self::OrderbookEvent => "orderbook_event",
            Self::ControlCommand => "control_command",
            Self::Replay => "replay",
        }
    }
}

impl FromStr for RewardStrategyRunTrigger {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "poll" => Ok(Self::Poll),
            "run_once" => Ok(Self::RunOnce),
            "orderbook_event" => Ok(Self::OrderbookEvent),
            "control_command" => Ok(Self::ControlCommand),
            "replay" => Ok(Self::Replay),
            other => Err(AppError::invalid_input(
                "REWARD_STRATEGY_RUN_TRIGGER_INVALID",
                format!("unknown reward strategy run trigger: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardStrategyRunStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl RewardStrategyRunStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

impl FromStr for RewardStrategyRunStatus {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(AppError::invalid_input(
                "REWARD_STRATEGY_RUN_STATUS_INVALID",
                format!("unknown reward strategy run status: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardStrategyActionType {
    PlaceBuy,
    SubmitExitSell,
    CancelOrder,
    CancelReplaceExit,
    RecordFill,
    CreateMergeIntent,
    ExecuteMerge,
    Skip,
}

impl RewardStrategyActionType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PlaceBuy => "place_buy",
            Self::SubmitExitSell => "submit_exit_sell",
            Self::CancelOrder => "cancel_order",
            Self::CancelReplaceExit => "cancel_replace_exit",
            Self::RecordFill => "record_fill",
            Self::CreateMergeIntent => "create_merge_intent",
            Self::ExecuteMerge => "execute_merge",
            Self::Skip => "skip",
        }
    }
}

impl FromStr for RewardStrategyActionType {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "place_buy" => Ok(Self::PlaceBuy),
            "submit_exit_sell" => Ok(Self::SubmitExitSell),
            "cancel_order" => Ok(Self::CancelOrder),
            "cancel_replace_exit" => Ok(Self::CancelReplaceExit),
            "record_fill" => Ok(Self::RecordFill),
            "create_merge_intent" => Ok(Self::CreateMergeIntent),
            "execute_merge" => Ok(Self::ExecuteMerge),
            "skip" => Ok(Self::Skip),
            other => Err(AppError::invalid_input(
                "REWARD_STRATEGY_ACTION_TYPE_INVALID",
                format!("unknown reward strategy action type: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardStrategyActionStatus {
    Planned,
    Executing,
    Succeeded,
    Failed,
    Skipped,
    Unknown,
}

impl RewardStrategyActionStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Executing => "executing",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
            Self::Unknown => "unknown",
        }
    }

    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Skipped | Self::Unknown
        )
    }
}

impl FromStr for RewardStrategyActionStatus {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "planned" => Ok(Self::Planned),
            "executing" => Ok(Self::Executing),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "skipped" => Ok(Self::Skipped),
            "unknown" => Ok(Self::Unknown),
            other => Err(AppError::invalid_input(
                "REWARD_STRATEGY_ACTION_STATUS_INVALID",
                format!("unknown reward strategy action status: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardStrategyRun {
    pub run_id: i64,
    pub account_id: String,
    pub trace_id: String,
    pub trigger_type: RewardStrategyRunTrigger,
    pub status: RewardStrategyRunStatus,
    pub config_hash: String,
    pub config_json: Value,
    pub input_summary: Value,
    pub metrics: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub completed_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardStrategyRunStart {
    pub account_id: String,
    pub trace_id: String,
    pub trigger_type: RewardStrategyRunTrigger,
    pub config_hash: String,
    pub config_json: Value,
    pub input_summary: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardStrategyDecision {
    pub run_id: i64,
    pub condition_id: String,
    pub strategy_profile: RewardStrategyProfile,
    pub decision_rank: i32,
    pub eligible: bool,
    pub quote_readiness: RewardQuoteReadiness,
    pub quote_mode: RewardPlanQuoteMode,
    pub score: Decimal,
    pub selection_score: Decimal,
    pub reason_code: String,
    pub reason: String,
    #[serde(default)]
    pub blocker_codes: Vec<String>,
    pub planned_buy_notional_usd: Decimal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fair_value_passed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fair_value_effective_edge_cents: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opportunity_score: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_window_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info_risk_action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info_risk_level: Option<String>,
    pub decision_json: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardStrategyAction {
    #[serde(default)]
    pub action_id: i64,
    pub run_id: i64,
    pub account_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub managed_order_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_order_id: Option<String>,
    pub action_type: RewardStrategyActionType,
    pub status: RewardStrategyActionStatus,
    pub reason_code: String,
    pub reason: String,
    pub idempotency_key: String,
    pub request_json: Value,
    pub result_json: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease_owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub lease_expires_at: Option<OffsetDateTime>,
    #[serde(default)]
    pub execution_attempts: i32,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardOrderTransition {
    #[serde(default)]
    pub transition_id: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_id: Option<i64>,
    pub managed_order_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_order_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_status: Option<ManagedRewardOrderStatus>,
    pub to_status: ManagedRewardOrderStatus,
    pub reason_code: String,
    pub reason: String,
    pub metadata: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardStrategyRunPage {
    pub items: Vec<RewardStrategyRun>,
    pub page: RewardListPage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardStrategyDecisionPage {
    pub items: Vec<RewardStrategyDecision>,
    pub page: RewardListPage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardStrategyActionPage {
    pub items: Vec<RewardStrategyAction>,
    pub page: RewardListPage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardOrderTransitionPage {
    pub items: Vec<RewardOrderTransition>,
    pub page: RewardListPage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewardStrategyRunListQuery {
    pub account_id: Option<String>,
    pub status: Option<RewardStrategyRunStatus>,
    pub page: usize,
    pub page_size: usize,
}

impl RewardStrategyRunListQuery {
    #[must_use]
    pub fn new(
        account_id: Option<String>,
        status: Option<String>,
        page: Option<u16>,
        page_size: Option<u16>,
    ) -> Self {
        Self {
            account_id: normalize_optional_filter(account_id),
            status: status
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .and_then(|value| RewardStrategyRunStatus::from_str(value).ok()),
            page: usize::from(page.unwrap_or(1).max(1)),
            page_size: usize::from(validate_reward_list_limit(page_size)),
        }
    }

    #[must_use]
    pub fn page_for_total(&self, total_items: usize) -> RewardListPage {
        RewardListPage::new(self.page, self.page_size, total_items)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewardStrategyDecisionListQuery {
    pub search: Option<String>,
    pub eligible: Option<bool>,
    pub page: usize,
    pub page_size: usize,
}

impl RewardStrategyDecisionListQuery {
    #[must_use]
    pub fn new(
        search: Option<String>,
        eligible: Option<bool>,
        page: Option<u16>,
        page_size: Option<u16>,
    ) -> Self {
        Self {
            search: normalize_optional_filter(search).map(|value| value.to_lowercase()),
            eligible,
            page: usize::from(page.unwrap_or(1).max(1)),
            page_size: usize::from(validate_reward_list_limit(page_size)),
        }
    }

    #[must_use]
    pub fn page_for_total(&self, total_items: usize) -> RewardListPage {
        RewardListPage::new(self.page, self.page_size, total_items)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewardStrategyActionListQuery {
    pub status: Option<RewardStrategyActionStatus>,
    pub action_type: Option<RewardStrategyActionType>,
    pub page: usize,
    pub page_size: usize,
}

impl RewardStrategyActionListQuery {
    #[must_use]
    pub fn new(
        status: Option<String>,
        action_type: Option<String>,
        page: Option<u16>,
        page_size: Option<u16>,
    ) -> Self {
        Self {
            status: status
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .and_then(|value| RewardStrategyActionStatus::from_str(value).ok()),
            action_type: action_type
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .and_then(|value| RewardStrategyActionType::from_str(value).ok()),
            page: usize::from(page.unwrap_or(1).max(1)),
            page_size: usize::from(validate_reward_list_limit(page_size)),
        }
    }

    #[must_use]
    pub fn page_for_total(&self, total_items: usize) -> RewardListPage {
        RewardListPage::new(self.page, self.page_size, total_items)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewardOrderTransitionListQuery {
    pub page: usize,
    pub page_size: usize,
}

impl RewardOrderTransitionListQuery {
    #[must_use]
    pub fn new(page: Option<u16>, page_size: Option<u16>) -> Self {
        Self {
            page: usize::from(page.unwrap_or(1).max(1)),
            page_size: usize::from(validate_reward_list_limit(page_size)),
        }
    }

    #[must_use]
    pub fn page_for_total(&self, total_items: usize) -> RewardListPage {
        RewardListPage::new(self.page, self.page_size, total_items)
    }
}

#[must_use]
pub fn reward_config_hash(config: &RewardBotConfig) -> String {
    let mut hasher = Sha256::new();
    match serde_json::to_vec(config) {
        Ok(payload) => hasher.update(payload),
        Err(error) => hasher.update(format!("reward-config-serialization-error:{error}")),
    }
    format!("{:x}", hasher.finalize())
}

#[must_use]
pub fn reward_strategy_decisions_from_plans(
    run_id: i64,
    plans: &[RewardQuotePlan],
    now: OffsetDateTime,
) -> Vec<RewardStrategyDecision> {
    plans
        .iter()
        .enumerate()
        .map(|(index, plan)| reward_strategy_decision_from_plan(run_id, index, plan, now))
        .collect()
}

#[must_use]
pub fn reward_strategy_decision_from_plan(
    run_id: i64,
    index: usize,
    plan: &RewardQuotePlan,
    now: OffsetDateTime,
) -> RewardStrategyDecision {
    let mut plan = plan.clone();
    refresh_reward_quote_plan_readiness(&mut plan);
    let reason_code = reward_quote_plan_reason_code(&plan);
    let blocker_codes = reward_quote_plan_blocker_codes(&plan, &reason_code);
    let fair_value_effective_edge_cents = plan.fair_value.as_ref().and_then(|decision| {
        decision
            .edges
            .iter()
            .map(|edge| edge.effective_edge_cents)
            .max()
    });
    let planned_buy_notional_usd = plan
        .legs
        .iter()
        .filter(|leg| leg.side == RewardOrderSide::Buy)
        .map(|leg| leg.notional_usd)
        .sum::<Decimal>()
        .round_dp(4);

    RewardStrategyDecision {
        run_id,
        condition_id: plan.condition_id.clone(),
        strategy_profile: plan.strategy_profile,
        decision_rank: i32::try_from(index).unwrap_or(i32::MAX),
        eligible: plan.eligible,
        quote_readiness: plan.quote_readiness,
        quote_mode: plan.quote_mode,
        score: plan.score,
        selection_score: plan.selection_score,
        reason_code,
        reason: plan.reason.clone(),
        blocker_codes,
        planned_buy_notional_usd,
        fair_value_passed: plan.fair_value.as_ref().map(|decision| decision.passed),
        fair_value_effective_edge_cents,
        opportunity_score: plan
            .opportunity_metrics
            .as_ref()
            .map(|metrics| metrics.opportunity_score),
        event_window_status: plan
            .event_window
            .as_ref()
            .map(|assessment| assessment.status.as_str().to_string()),
        ai_action: plan
            .ai_advisory
            .as_ref()
            .map(|advisory| advisory.action.as_str().to_string()),
        info_risk_action: plan
            .info_risk
            .as_ref()
            .map(|risk| risk.action.as_str().to_string()),
        info_risk_level: plan
            .info_risk
            .as_ref()
            .map(|risk| risk.risk_level.as_str().to_string()),
        decision_json: serde_json::to_value(&plan).unwrap_or_else(|_| json!({})),
        created_at: now,
    }
}

#[must_use]
pub fn reward_strategy_actions_from_tick_outcome(
    run_id: i64,
    outcome: &RewardTickOutcome,
    trace_id: &str,
    now: OffsetDateTime,
) -> Vec<RewardStrategyAction> {
    let mut actions = Vec::new();
    for order in &outcome.orders {
        actions.push(reward_strategy_action_from_order(run_id, order, trace_id, now));
    }
    for fill in &outcome.fills {
        actions.push(reward_strategy_action_from_fill(run_id, fill, trace_id, now));
    }
    for intent in &outcome.merge_intents {
        actions.push(reward_strategy_action_from_merge_intent(
            run_id, intent, trace_id, now,
        ));
    }
    actions
}

#[must_use]
pub fn reward_order_transition_from_order_change(
    run_id: Option<i64>,
    from_status: Option<ManagedRewardOrderStatus>,
    order: &ManagedRewardOrder,
    now: OffsetDateTime,
) -> RewardOrderTransition {
    RewardOrderTransition {
        transition_id: 0,
        run_id,
        action_id: None,
        managed_order_id: order.id.clone(),
        external_order_id: order.external_order_id.clone(),
        from_status,
        to_status: order.status,
        reason_code: reward_order_transition_reason_code(from_status, order),
        reason: order.reason.clone(),
        metadata: json!({
            "account_id": order.account_id,
            "condition_id": order.condition_id,
            "token_id": order.token_id,
            "outcome": order.outcome,
            "side": order.side.as_str(),
            "price": order.price,
            "size": order.size,
            "filled_size": order.filled_size,
            "reward_earned": order.reward_earned,
        }),
        created_at: now,
    }
}

fn reward_strategy_action_from_order(
    run_id: i64,
    order: &ManagedRewardOrder,
    trace_id: &str,
    now: OffsetDateTime,
) -> RewardStrategyAction {
    let action_type = reward_strategy_action_type_for_order(order);
    RewardStrategyAction {
        action_id: 0,
        run_id,
        account_id: order.account_id.clone(),
        condition_id: Some(order.condition_id.clone()),
        token_id: Some(order.token_id.clone()),
        managed_order_id: Some(order.id.clone()),
        external_order_id: order.external_order_id.clone(),
        action_type,
        status: reward_strategy_action_status_for_order(order),
        reason_code: reward_order_action_reason_code(order),
        reason: order.reason.clone(),
        idempotency_key: format!("{trace_id}:order:{}", order.id),
        request_json: serde_json::to_value(order).unwrap_or_else(|_| json!({})),
        result_json: json!({
            "status": order.status.as_str(),
            "side": order.side.as_str(),
            "external_order_id": order.external_order_id,
            "filled_size": order.filled_size,
            "reward_earned": order.reward_earned,
        }),
        lease_owner: None,
        lease_expires_at: None,
        execution_attempts: 0,
        created_at: now,
        updated_at: now,
    }
}

fn reward_strategy_action_from_fill(
    run_id: i64,
    fill: &RewardFill,
    trace_id: &str,
    now: OffsetDateTime,
) -> RewardStrategyAction {
    RewardStrategyAction {
        action_id: 0,
        run_id,
        account_id: fill.account_id.clone(),
        condition_id: Some(fill.condition_id.clone()),
        token_id: Some(fill.token_id.clone()),
        managed_order_id: Some(fill.order_id.clone()),
        external_order_id: None,
        action_type: RewardStrategyActionType::RecordFill,
        status: RewardStrategyActionStatus::Succeeded,
        reason_code: "record_fill".to_string(),
        reason: fill.reason.clone(),
        idempotency_key: format!("{trace_id}:fill:{}", fill.id),
        request_json: serde_json::to_value(fill).unwrap_or_else(|_| json!({})),
        result_json: json!({
            "fill_id": fill.id,
            "role": fill.role.as_str(),
            "notional_usd": fill.notional_usd,
            "realized_pnl": fill.realized_pnl,
        }),
        lease_owner: None,
        lease_expires_at: None,
        execution_attempts: 0,
        created_at: now,
        updated_at: now,
    }
}

fn reward_strategy_action_from_merge_intent(
    run_id: i64,
    intent: &RewardMergeIntent,
    trace_id: &str,
    now: OffsetDateTime,
) -> RewardStrategyAction {
    let action_type = if matches!(
        intent.status,
        RewardMergeIntentStatus::Submitted | RewardMergeIntentStatus::Completed
    ) {
        RewardStrategyActionType::ExecuteMerge
    } else {
        RewardStrategyActionType::CreateMergeIntent
    };
    let status = match intent.status {
        RewardMergeIntentStatus::Failed => RewardStrategyActionStatus::Failed,
        RewardMergeIntentStatus::Submitted | RewardMergeIntentStatus::Completed => {
            RewardStrategyActionStatus::Succeeded
        }
        // A pending row means the create-intent side effect completed. Execute
        // merge is represented by its own `:execute` action.
        RewardMergeIntentStatus::Pending => RewardStrategyActionStatus::Succeeded,
        RewardMergeIntentStatus::Unsupported => RewardStrategyActionStatus::Skipped,
    };
    RewardStrategyAction {
        action_id: 0,
        run_id,
        account_id: intent.account_id.clone(),
        condition_id: Some(intent.condition_id.clone()),
        token_id: None,
        managed_order_id: None,
        external_order_id: intent.tx_hash.clone(),
        action_type,
        status,
        reason_code: format!("merge_{}", intent.status.as_str()),
        reason: intent.reason.clone(),
        idempotency_key: format!("{trace_id}:merge:{}", intent.id),
        request_json: serde_json::to_value(intent).unwrap_or_else(|_| json!({})),
        result_json: json!({
            "merge_intent_id": intent.id,
            "status": intent.status.as_str(),
            "tx_hash": intent.tx_hash,
            "merge_size": intent.merge_size,
        }),
        lease_owner: None,
        lease_expires_at: None,
        execution_attempts: 0,
        created_at: now,
        updated_at: now,
    }
}

fn reward_strategy_action_type_for_order(order: &ManagedRewardOrder) -> RewardStrategyActionType {
    if reward_order_action_skipped_before_external_submission(order) {
        return match order.side {
            RewardOrderSide::Buy => RewardStrategyActionType::PlaceBuy,
            RewardOrderSide::Sell => RewardStrategyActionType::SubmitExitSell,
        };
    }
    if matches!(order.status, ManagedRewardOrderStatus::Cancelled) {
        return RewardStrategyActionType::CancelOrder;
    }
    let reason = order.reason.to_ascii_lowercase();
    if reason.contains("cancel-replace") || reason.contains("cancel replace") {
        RewardStrategyActionType::CancelReplaceExit
    } else if reason.contains("cancel") && order.side == RewardOrderSide::Buy {
        RewardStrategyActionType::CancelOrder
    } else if order.side == RewardOrderSide::Sell {
        RewardStrategyActionType::SubmitExitSell
    } else {
        RewardStrategyActionType::PlaceBuy
    }
}

fn reward_strategy_action_status_for_order(order: &ManagedRewardOrder) -> RewardStrategyActionStatus {
    let reason = order.reason.to_ascii_lowercase();
    if reward_order_action_skipped_before_external_submission(order) {
        RewardStrategyActionStatus::Skipped
    } else if matches!(order.status, ManagedRewardOrderStatus::Error) {
        RewardStrategyActionStatus::Failed
    } else if reason.contains("unknown") || reason.contains("manual reconciliation required") {
        RewardStrategyActionStatus::Unknown
    } else if matches!(order.status, ManagedRewardOrderStatus::Planned | ManagedRewardOrderStatus::ExitPending) {
        // The durable local order intent has been persisted and external
        // submission is in progress. Keep the action in executing until the
        // submit/last-look outcome supplies a terminal state.
        RewardStrategyActionStatus::Executing
    } else {
        RewardStrategyActionStatus::Succeeded
    }
}

fn reward_order_action_skipped_before_external_submission(order: &ManagedRewardOrder) -> bool {
    if order.external_order_id.is_some()
        || order.status != ManagedRewardOrderStatus::Cancelled
    {
        return false;
    }
    let reason = order.reason.to_ascii_lowercase();
    match order.side {
        RewardOrderSide::Buy => {
            reason.contains("cancelled before live submission")
                || reason.contains("cancelled by live submission last-look")
        }
        RewardOrderSide::Sell => reason.starts_with("sell exit closed because"),
    }
}

fn reward_order_action_reason_code(order: &ManagedRewardOrder) -> String {
    match reward_strategy_action_type_for_order(order) {
        RewardStrategyActionType::PlaceBuy => "place_buy".to_string(),
        RewardStrategyActionType::SubmitExitSell => "submit_exit_sell".to_string(),
        RewardStrategyActionType::CancelOrder => "cancel_order".to_string(),
        RewardStrategyActionType::CancelReplaceExit => "cancel_replace_exit".to_string(),
        RewardStrategyActionType::RecordFill => "record_fill".to_string(),
        RewardStrategyActionType::CreateMergeIntent => "create_merge_intent".to_string(),
        RewardStrategyActionType::ExecuteMerge => "execute_merge".to_string(),
        RewardStrategyActionType::Skip => "skip".to_string(),
    }
}

fn reward_order_transition_reason_code(
    from_status: Option<ManagedRewardOrderStatus>,
    order: &ManagedRewardOrder,
) -> String {
    match (from_status, order.status) {
        (None, ManagedRewardOrderStatus::Planned) => "order_planned".to_string(),
        (None, ManagedRewardOrderStatus::Open) => "order_opened".to_string(),
        (_, ManagedRewardOrderStatus::Cancelled) => "order_cancelled".to_string(),
        (_, ManagedRewardOrderStatus::Filled) => "order_filled".to_string(),
        (_, ManagedRewardOrderStatus::ExitPending) => "exit_pending".to_string(),
        (_, ManagedRewardOrderStatus::Error) => "order_error".to_string(),
        (_, ManagedRewardOrderStatus::Open) => "order_opened".to_string(),
        (_, ManagedRewardOrderStatus::Planned) => "order_planned".to_string(),
    }
}

#[must_use]
pub fn reward_quote_plan_reason_code(plan: &RewardQuotePlan) -> String {
    let reason = plan.reason.to_ascii_lowercase();
    if plan.eligible {
        return "eligible".to_string();
    }
    if reason.starts_with("waiting for fresh orderbook") {
        "waiting_orderbook".to_string()
    } else if reason.starts_with("ai advisory pending:") {
        "ai_pending".to_string()
    } else if reason.starts_with("info risk pending:") {
        "info_risk_pending".to_string()
    } else if reason.starts_with("ai advisory stop_new:") {
        "ai_stop_new".to_string()
    } else if reason.starts_with("provider size adjustment below required rewards quote:") {
        "provider_size".to_string()
    } else if reason.starts_with("info risk ") {
        "info_risk".to_string()
    } else if reason.starts_with("event window") {
        "event_window".to_string()
    } else if reason.starts_with("fair value gate:") {
        "fair_value".to_string()
    } else if reason.starts_with("live funding below rewards minimum:") {
        "funding".to_string()
    } else if reason.starts_with("maker market budget below required rewards quote:") {
        "maker_budget".to_string()
    } else if reason.starts_with("inventory headroom below required rewards quote:") {
        "inventory_headroom".to_string()
    } else if reason.starts_with("live orderbook validation skipped until ") {
        "live_validation".to_string()
    } else if reason.contains("score is below threshold") {
        "score_below_threshold".to_string()
    } else {
        "blocked_other".to_string()
    }
}

#[must_use]
pub fn reward_quote_plan_blocker_codes(
    plan: &RewardQuotePlan,
    primary_reason_code: &str,
) -> Vec<String> {
    let mut codes = Vec::new();
    if !plan.eligible {
        codes.push(primary_reason_code.to_string());
    }
    match plan.quote_readiness {
        RewardQuoteReadiness::ReadyToQuote => {}
        RewardQuoteReadiness::WaitingOrderbook => push_unique_code(&mut codes, "waiting_orderbook"),
        RewardQuoteReadiness::ProviderPending => push_unique_code(&mut codes, "provider_pending"),
        RewardQuoteReadiness::Blocked => push_unique_code(&mut codes, "blocked"),
    }
    if plan
        .fair_value
        .as_ref()
        .is_some_and(|decision| !decision.passed)
    {
        push_unique_code(&mut codes, "fair_value");
    }
    if plan
        .event_window
        .as_ref()
        .is_some_and(|assessment| assessment.status.blocks_new_buy())
    {
        push_unique_code(&mut codes, "event_window");
    }
    if plan
        .ai_advisory
        .as_ref()
        .is_some_and(|advisory| advisory.action != RewardProviderAction::Allow)
    {
        push_unique_code(&mut codes, "ai_advisory");
    }
    if plan
        .info_risk
        .as_ref()
        .is_some_and(|risk| risk.action != RewardProviderAction::Allow)
    {
        push_unique_code(&mut codes, "info_risk");
    }
    codes
}

fn push_unique_code(codes: &mut Vec<String>, code: &str) {
    if !codes.iter().any(|existing| existing == code) {
        codes.push(code.to_string());
    }
}

fn normalize_optional_filter(value: Option<String>) -> Option<String> {
    let value = value?.trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}
