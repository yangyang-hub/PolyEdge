// Risk DTOs: risk state, approvals, alerts, buckets, and kill-switch requests/results.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskStateData {
    pub id: String,
    pub mode: SystemMode,
    pub environment: String,
    pub kill_switch: bool,
    pub daily_pnl: SignedUsdAmount,
    pub gross_exposure: ExposureRatio,
    pub net_exposure: ExposureRatio,
    pub open_alerts: u32,
    pub daily_loss_limit: UsdAmount,
    pub daily_loss_used: UsdAmount,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalType {
    Signal,
    ModeSwitch,
    KillSwitch,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalData {
    pub id: String,
    #[serde(rename = "type")]
    pub approval_type: ApprovalType,
    pub severity: ApprovalSeverity,
    pub owner: String,
    pub resource_id: String,
    pub summary: String,
    pub status: ApprovalStatus,
    pub requires_step_up_auth: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlertSeverity {
    Warning,
    Critical,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlertStatus {
    Unresolved,
    Watching,
    Contained,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAlertData {
    pub id: String,
    pub severity: AlertSeverity,
    pub reason: String,
    pub target: String,
    pub status: AlertStatus,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BucketStatus {
    Healthy,
    Watch,
    Breach,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskBucketData {
    pub id: String,
    pub name: String,
    pub exposure: ExposureRatio,
    pub limit: ExposureRatio,
    pub utilization: ExposureRatio,
    pub status: BucketStatus,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerKillSwitchRequest {
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_version: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseKillSwitchRequest {
    pub reason: String,
    pub to_mode: SystemMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_version: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KillSwitchData {
    pub risk_state: RiskStateData,
    pub replayed: bool,
}
