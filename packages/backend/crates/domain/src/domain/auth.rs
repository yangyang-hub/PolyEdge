#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    Viewer,
    Operator,
    RiskAdmin,
    Admin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepUpScope {
    SignalApprove,
    SignalReject,
    ExecutionSubmit,
    OrderCancelForce,
    SystemModeSwitch,
    SystemKillSwitchTrigger,
    SystemKillSwitchRelease,
    RiskThresholdUpdate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditResult {
    Accepted,
    Succeeded,
    Rejected,
    Failed,
}

impl AuditResult {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Succeeded => "succeeded",
            Self::Rejected => "rejected",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdempotencyStatus {
    Started,
    Completed,
    Failed,
}

impl IdempotencyStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

impl FromStr for IdempotencyStatus {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "started" => Ok(Self::Started),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            _ => Err(AppError::invalid_input(
                "DOMAIN_IDEMPOTENCY_STATUS_INVALID",
                format!("unknown idempotency status: {value}"),
            )),
        }
    }
}
