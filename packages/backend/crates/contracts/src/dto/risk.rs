// Risk DTOs retained for connector callback responses.

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
