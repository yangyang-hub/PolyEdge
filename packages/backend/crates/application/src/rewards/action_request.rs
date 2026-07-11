pub const REWARD_DURABLE_ACTION_SCHEMA_VERSION: u16 = 1;

const REWARD_SENSITIVE_JSON_KEYS: &[&str] = &[
    "accesstoken",
    "apikey",
    "apipassphrase",
    "apisecret",
    "authorization",
    "bearertoken",
    "credentials",
    "password",
    "privatekey",
    "refreshtoken",
    "secretkey",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardDurableActionRecovery {
    Recoverable,
    ReconciliationRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardDurableActionPhase {
    Planned,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RewardDurableOrderActionPayload {
    pub reason: String,
    pub order: ManagedRewardOrder,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RewardDurableMergeActionPayload {
    pub reason: String,
    pub merge_intent: RewardMergeIntent,
    #[serde(default)]
    pub metadata: Value,
}

/// Versioned request persisted in `RewardStrategyAction::request_json`.
///
/// The enum deliberately contains only side effects that a durable executor
/// can reconstruct. Audit-only actions such as record-fill and skip never have
/// an executable durable request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "intent", content = "payload", rename_all = "snake_case")]
pub enum RewardDurableActionRequest {
    PlaceBuy(RewardDurableOrderActionPayload),
    SubmitExitSell(RewardDurableOrderActionPayload),
    CancelOrder(RewardDurableOrderActionPayload),
    CancelReplaceExit(RewardDurableOrderActionPayload),
    CreateMergeIntent(RewardDurableMergeActionPayload),
    ExecuteMerge(RewardDurableMergeActionPayload),
}

impl RewardDurableActionRequest {
    #[must_use]
    pub const fn action_type(&self) -> RewardStrategyActionType {
        match self {
            Self::PlaceBuy(_) => RewardStrategyActionType::PlaceBuy,
            Self::SubmitExitSell(_) => RewardStrategyActionType::SubmitExitSell,
            Self::CancelOrder(_) => RewardStrategyActionType::CancelOrder,
            Self::CancelReplaceExit(_) => RewardStrategyActionType::CancelReplaceExit,
            Self::CreateMergeIntent(_) => RewardStrategyActionType::CreateMergeIntent,
            Self::ExecuteMerge(_) => RewardStrategyActionType::ExecuteMerge,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RewardDurableActionEnvelope {
    pub schema_version: u16,
    pub phase: RewardDurableActionPhase,
    pub request: RewardDurableActionRequest,
}

impl RewardDurableActionEnvelope {
    #[must_use]
    pub fn order(
        intent: RewardOrderActionIntent,
        reason: &str,
        order: &ManagedRewardOrder,
        metadata: Value,
    ) -> Self {
        let payload = RewardDurableOrderActionPayload {
            reason: reason.to_string(),
            order: order.clone(),
            metadata,
        };
        let request = match intent {
            RewardOrderActionIntent::PlaceBuy => RewardDurableActionRequest::PlaceBuy(payload),
            RewardOrderActionIntent::SubmitExitSell => {
                RewardDurableActionRequest::SubmitExitSell(payload)
            }
            RewardOrderActionIntent::CancelOrder => {
                RewardDurableActionRequest::CancelOrder(payload)
            }
            RewardOrderActionIntent::CancelReplaceExit => {
                RewardDurableActionRequest::CancelReplaceExit(payload)
            }
        };
        Self {
            schema_version: REWARD_DURABLE_ACTION_SCHEMA_VERSION,
            phase: RewardDurableActionPhase::Planned,
            request,
        }
    }

    pub fn merge(
        action_type: RewardStrategyActionType,
        reason: &str,
        intent: &RewardMergeIntent,
        metadata: Value,
    ) -> Result<Self> {
        let payload = RewardDurableMergeActionPayload {
            reason: reason.to_string(),
            merge_intent: intent.clone(),
            metadata,
        };
        let request = match action_type {
            RewardStrategyActionType::CreateMergeIntent => {
                RewardDurableActionRequest::CreateMergeIntent(payload)
            }
            RewardStrategyActionType::ExecuteMerge => {
                RewardDurableActionRequest::ExecuteMerge(payload)
            }
            other => {
                return Err(durable_action_error(format!(
                    "action type {} cannot carry a merge request",
                    other.as_str()
                )));
            }
        };
        Ok(Self {
            schema_version: REWARD_DURABLE_ACTION_SCHEMA_VERSION,
            phase: RewardDurableActionPhase::Planned,
            request,
        })
    }

    pub fn to_json(&self) -> Result<Value> {
        let value = serde_json::to_value(self).map_err(|error| {
            durable_action_error(format!("failed to serialize durable action request: {error}"))
        })?;
        reject_sensitive_reward_json_fields(&value, "REWARD_DURABLE_ACTION_SENSITIVE_FIELD")?;
        Ok(value)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RewardValidatedDurableAction {
    pub envelope: RewardDurableActionEnvelope,
    pub recovery: RewardDurableActionRecovery,
}

impl RewardStrategyAction {
    /// Parse and validate an executable request against its durable ledger row.
    /// Legacy unversioned JSON intentionally fails closed and must be resolved
    /// by reconciliation instead of being inferred and replayed.
    pub fn parse_durable_request(&self) -> Result<RewardValidatedDurableAction> {
        reject_sensitive_reward_json_fields(
            &self.request_json,
            "REWARD_DURABLE_ACTION_SENSITIVE_FIELD",
        )?;
        let envelope: RewardDurableActionEnvelope =
            serde_json::from_value(self.request_json.clone()).map_err(|error| {
                durable_action_error(format!("invalid durable action request envelope: {error}"))
            })?;
        validate_durable_action_envelope(self, &envelope)?;
        Ok(RewardValidatedDurableAction {
            recovery: durable_action_recovery(self),
            envelope,
        })
    }
}

fn validate_durable_action_envelope(
    action: &RewardStrategyAction,
    envelope: &RewardDurableActionEnvelope,
) -> Result<()> {
    if envelope.schema_version != REWARD_DURABLE_ACTION_SCHEMA_VERSION {
        return Err(durable_action_error(format!(
            "unsupported durable action schema version {}",
            envelope.schema_version
        )));
    }
    if envelope.request.action_type() != action.action_type {
        return Err(durable_action_error(format!(
            "request intent {} does not match action type {}",
            envelope.request.action_type().as_str(),
            action.action_type.as_str()
        )));
    }
    match &envelope.request {
        RewardDurableActionRequest::PlaceBuy(payload) => {
            validate_order_payload(action, payload, RewardOrderSide::Buy, false)?;
            if payload.order.status != ManagedRewardOrderStatus::Planned {
                return Err(durable_action_error(
                    "place-buy request requires a planned managed order",
                ));
            }
        }
        RewardDurableActionRequest::SubmitExitSell(payload) => {
            validate_order_payload(action, payload, RewardOrderSide::Sell, false)?;
            if !matches!(
                payload.order.status,
                ManagedRewardOrderStatus::Planned | ManagedRewardOrderStatus::ExitPending
            ) {
                return Err(durable_action_error(
                    "exit-sell request requires a planned or exit-pending managed order",
                ));
            }
        }
        RewardDurableActionRequest::CancelOrder(payload) => {
            validate_order_payload(action, payload, payload.order.side, true)?;
        }
        RewardDurableActionRequest::CancelReplaceExit(payload) => {
            validate_order_payload(action, payload, RewardOrderSide::Sell, true)?;
        }
        RewardDurableActionRequest::CreateMergeIntent(payload)
        | RewardDurableActionRequest::ExecuteMerge(payload) => {
            validate_merge_payload(action, payload)?;
        }
    }
    Ok(())
}

fn validate_order_payload(
    action: &RewardStrategyAction,
    payload: &RewardDurableOrderActionPayload,
    expected_side: RewardOrderSide,
    requires_external_order_id: bool,
) -> Result<()> {
    validate_reason_and_metadata(&payload.reason, &payload.metadata)?;
    let order = &payload.order;
    if order.side != expected_side {
        return Err(durable_action_error("managed order side does not match request intent"));
    }
    if order.id.trim().is_empty()
        || order.account_id.trim().is_empty()
        || order.condition_id.trim().is_empty()
        || order.token_id.trim().is_empty()
        || order.price <= Decimal::ZERO
        || order.price >= Decimal::ONE
        || order.size <= Decimal::ZERO
    {
        return Err(durable_action_error(
            "managed order execution fields are incomplete or invalid",
        ));
    }
    if action.account_id != order.account_id
        || action.condition_id.as_deref() != Some(order.condition_id.as_str())
        || action.token_id.as_deref() != Some(order.token_id.as_str())
        || action.managed_order_id.as_deref() != Some(order.id.as_str())
        || action.external_order_id != order.external_order_id
    {
        return Err(durable_action_error(
            "managed order request does not match durable action identity",
        ));
    }
    if requires_external_order_id
        && order
            .external_order_id
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
    {
        return Err(durable_action_error(
            "cancel request requires an external order id",
        ));
    }
    if !requires_external_order_id && order.external_order_id.is_some() {
        return Err(durable_action_error(
            "submission request must not already have an external order id",
        ));
    }
    Ok(())
}

fn validate_merge_payload(
    action: &RewardStrategyAction,
    payload: &RewardDurableMergeActionPayload,
) -> Result<()> {
    validate_reason_and_metadata(&payload.reason, &payload.metadata)?;
    let intent = &payload.merge_intent;
    if intent.id.trim().is_empty()
        || intent.account_id.trim().is_empty()
        || intent.condition_id.trim().is_empty()
        || intent.yes_token_id.trim().is_empty()
        || intent.no_token_id.trim().is_empty()
        || intent.merge_size <= Decimal::ZERO
    {
        return Err(durable_action_error(
            "merge request execution fields are incomplete or invalid",
        ));
    }
    if action.account_id != intent.account_id
        || action.condition_id.as_deref() != Some(intent.condition_id.as_str())
        || action.token_id.is_some()
        || action.managed_order_id.is_some()
        || action.external_order_id != intent.tx_hash
    {
        return Err(durable_action_error(
            "merge request does not match durable action identity",
        ));
    }
    Ok(())
}

fn validate_reason_and_metadata(reason: &str, metadata: &Value) -> Result<()> {
    if reason.trim().is_empty() {
        return Err(durable_action_error("durable action reason is required"));
    }
    if !metadata.is_object() {
        return Err(durable_action_error(
            "durable action metadata must be a JSON object",
        ));
    }
    Ok(())
}

fn durable_action_recovery(action: &RewardStrategyAction) -> RewardDurableActionRecovery {
    match action.status {
        RewardStrategyActionStatus::Planned => RewardDurableActionRecovery::Recoverable,
        // A first, currently leased claim has been fenced but has not yet been
        // dispatched by the executor. It is safe to execute from its payload.
        RewardStrategyActionStatus::Executing
            if action.execution_attempts == 1 && action.lease_owner.is_some() =>
        {
            RewardDurableActionRecovery::Recoverable
        }
        RewardStrategyActionStatus::Executing
            if matches!(
                action.action_type,
                RewardStrategyActionType::CancelOrder
                    | RewardStrategyActionType::CancelReplaceExit
                    | RewardStrategyActionType::CreateMergeIntent
            ) =>
        {
            RewardDurableActionRecovery::Recoverable
        }
        RewardStrategyActionStatus::Executing
        | RewardStrategyActionStatus::Succeeded
        | RewardStrategyActionStatus::Failed
        | RewardStrategyActionStatus::Skipped
        | RewardStrategyActionStatus::Unknown => {
            RewardDurableActionRecovery::ReconciliationRequired
        }
    }
}

fn durable_action_error(message: impl Into<String>) -> AppError {
    AppError::invalid_input("REWARD_DURABLE_ACTION_REQUEST_INVALID", message.into())
}

pub(crate) fn reject_sensitive_reward_json_fields(
    value: &Value,
    code: &'static str,
) -> Result<()> {
    match value {
        Value::Object(object) => {
            for (key, nested) in object {
                let normalized = key
                    .chars()
                    .filter(|character| character.is_ascii_alphanumeric())
                    .flat_map(char::to_lowercase)
                    .collect::<String>();
                if REWARD_SENSITIVE_JSON_KEYS.contains(&normalized.as_str()) {
                    return Err(AppError::invalid_input(
                        code,
                        format!("rewards payload contains forbidden field {key}"),
                    ));
                }
                reject_sensitive_reward_json_fields(nested, code)?;
            }
        }
        Value::Array(values) => {
            for nested in values {
                reject_sensitive_reward_json_fields(nested, code)?;
            }
        }
        _ => {}
    }
    Ok(())
}
