#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopyControlAction {
    RunOnce,
    AnalyzeWallets,
    CancelAll,
    Reset,
}

impl CopyControlAction {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RunOnce => "run_once",
            Self::AnalyzeWallets => "analyze_wallets",
            Self::CancelAll => "cancel_all",
            Self::Reset => "reset",
        }
    }
}

impl FromStr for CopyControlAction {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "run_once" => Ok(Self::RunOnce),
            "analyze_wallets" => Ok(Self::AnalyzeWallets),
            "cancel_all" => Ok(Self::CancelAll),
            "reset" => Ok(Self::Reset),
            other => Err(AppError::invalid_input(
                "COPY_CONTROL_ACTION_INVALID",
                format!("unknown copytrade control action: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopyControlCommandStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

impl CopyControlCommandStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

impl FromStr for CopyControlCommandStatus {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            other => Err(AppError::invalid_input(
                "COPY_CONTROL_STATUS_INVALID",
                format!("unknown copytrade control command status: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CopyControlCommand {
    pub id: String,
    pub action: CopyControlAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    pub reason: String,
    pub status: CopyControlCommandStatus,
    #[serde(with = "time::serde::rfc3339")]
    pub requested_at: OffsetDateTime,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub started_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub completed_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
