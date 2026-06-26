// Local-dev auth helpers: parse roles/scopes and normalize actor names (local environment only).

fn parse_dev_role(value: &str) -> Result<UserRole> {
    match value {
        "viewer" => Ok(UserRole::Viewer),
        "operator" => Ok(UserRole::Operator),
        "risk_admin" => Ok(UserRole::RiskAdmin),
        "admin" => Ok(UserRole::Admin),
        _ => Err(AppError::unauthorized(
            "AUTH_DEV_ROLE_INVALID",
            format!("invalid local dev role: {value}"),
        )),
    }
}

fn parse_dev_step_up_scopes(value: &str) -> Result<Vec<StepUpScope>> {
    value
        .split(',')
        .filter(|scope| !scope.trim().is_empty())
        .map(|scope| match scope.trim() {
            "signal_approve" => Ok(StepUpScope::SignalApprove),
            "signal_reject" => Ok(StepUpScope::SignalReject),
            "execution_submit" => Ok(StepUpScope::ExecutionSubmit),
            "order_cancel_force" => Ok(StepUpScope::OrderCancelForce),
            "system_mode_switch" => Ok(StepUpScope::SystemModeSwitch),
            "system_kill_switch_trigger" => Ok(StepUpScope::SystemKillSwitchTrigger),
            "system_kill_switch_release" => Ok(StepUpScope::SystemKillSwitchRelease),
            "risk_threshold_update" => Ok(StepUpScope::RiskThresholdUpdate),
            "funding_transfer" => Ok(StepUpScope::FundingTransfer),
            other => Err(AppError::unauthorized(
                "AUTH_DEV_STEP_UP_SCOPE_INVALID",
                format!("invalid local dev step-up scope: {other}"),
            )),
        })
        .collect()
}

fn normalize_dev_actor(value: &str) -> String {
    let normalized: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let normalized = normalized.trim_matches('_');

    if normalized.is_empty() {
        "local_console".to_string()
    } else {
        normalized.to_string()
    }
}
