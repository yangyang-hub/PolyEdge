#[derive(Debug, Clone, Copy)]
enum AuditResultMarker {
    Rejected,
    Succeeded,
    Failed,
}

impl From<AuditResultMarker> for polyedge_domain::AuditResult {
    fn from(value: AuditResultMarker) -> Self {
        match value {
            AuditResultMarker::Rejected => Self::Rejected,
            AuditResultMarker::Succeeded => Self::Succeeded,
            AuditResultMarker::Failed => Self::Failed,
        }
    }
}

fn validate_execution_mode(mode: SystemMode, kill_switch: bool) -> Result<()> {
    if kill_switch || mode == SystemMode::KillSwitchLocked {
        return Err(AppError::forbidden(
            "RISK_KILL_SWITCH_ACTIVE",
            "execution submission is blocked while the kill switch is active",
        ));
    }

    match mode {
        SystemMode::ManualConfirm | SystemMode::PaperTrade => Ok(()),
        SystemMode::Research => Err(AppError::conflict(
            "STATE_EXECUTION_MODE_INVALID",
            "execution submission is not available in research mode",
        )),
        SystemMode::LiveAuto => Err(AppError::conflict(
            "STATE_EXECUTION_MODE_NOT_SUPPORTED",
            "live_auto execution submission is not available until the connector is enabled",
        )),
        SystemMode::KillSwitchLocked => Err(AppError::forbidden(
            "RISK_KILL_SWITCH_ACTIVE",
            "execution submission is blocked while the kill switch is active",
        )),
    }
}

fn aggregate_execution_risk_metrics(
    positions: &[PositionView],
    policy: &RiskPolicy,
) -> Result<(SignedUsdAmount, ExposureRatio, ExposureRatio)> {
    let reference_nav = policy.exposure_reference_nav.value();
    if reference_nav <= Decimal::ZERO {
        return Err(AppError::internal(
            "RISK_REFERENCE_NAV_INVALID",
            "risk exposure_reference_nav must be greater than zero",
        ));
    }

    let mut daily_pnl = Decimal::ZERO;
    let mut gross_notional = Decimal::ZERO;
    let mut signed_notional = Decimal::ZERO;

    for position in positions {
        let mark_notional = position.net_quantity.value() * position.mark_price.value();
        gross_notional += mark_notional;
        signed_notional += match position.side {
            SignalSide::Yes => mark_notional,
            SignalSide::No => -mark_notional,
        };
        daily_pnl += position.realized_pnl.value() + position.unrealized_pnl.value();
    }

    let gross_exposure = ExposureRatio::new(gross_notional / reference_nav).map_err(|error| {
        AppError::internal(
            "RISK_GROSS_EXPOSURE_COMPUTE_FAILED",
            format!("failed to compute gross exposure: {error}"),
        )
    })?;
    let net_exposure =
        ExposureRatio::new(signed_notional.abs() / reference_nav).map_err(|error| {
            AppError::internal(
                "RISK_NET_EXPOSURE_COMPUTE_FAILED",
                format!("failed to compute net exposure: {error}"),
            )
        })?;
    let daily_pnl = SignedUsdAmount::new(daily_pnl).map_err(|error| {
        AppError::internal(
            "RISK_DAILY_PNL_COMPUTE_FAILED",
            format!("failed to compute daily pnl: {error}"),
        )
    })?;

    Ok((daily_pnl, gross_exposure, net_exposure))
}

fn normalize_connector_name(connector_name: Option<String>) -> Result<String> {
    let connector_name = connector_name
        .unwrap_or_else(|| DEFAULT_EXECUTION_CONNECTOR.to_string())
        .trim()
        .to_ascii_lowercase();

    if connector_name.is_empty() {
        return Err(AppError::invalid_input(
            "EXECUTION_CONNECTOR_NAME_REQUIRED",
            "connector name must not be empty",
        ));
    }

    Ok(connector_name)
}

fn validate_limit(limit: Option<u16>) -> Result<u16> {
    crate::list_filters::validate_list_limit(
        limit,
        DEFAULT_LIST_LIMIT,
        MAX_LIST_LIMIT,
        "LIST_LIMIT_INVALID",
        format!("limit must be within 1..={MAX_LIST_LIMIT}"),
        "LIST_LIMIT_INVALID",
        format!("limit must be within 1..={MAX_LIST_LIMIT}"),
    )
}

fn validate_optional_id(field_name: &str, value: Option<String>) -> Result<Option<String>> {
    crate::list_filters::normalize_optional_filter_id(
        field_name,
        value,
        "FILTER_ID_INVALID",
        |field_name| format!("{field_name} must not be empty when provided"),
    )
}

fn validate_optional_connector_name(value: Option<String>) -> Result<Option<String>> {
    value
        .map(|raw| normalize_connector_name(Some(raw)))
        .transpose()
}
