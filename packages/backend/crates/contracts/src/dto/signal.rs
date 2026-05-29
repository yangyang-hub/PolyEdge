// Signal DTOs: snapshots, lifecycle transitions, recompute/approve/reject requests and results.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalData {
    pub id: String,
    pub market_id: String,
    pub event_id: String,
    pub action: SignalAction,
    pub side: SignalSide,
    pub market_price: Probability,
    pub fair_price: Probability,
    pub edge: Edge,
    pub confidence: Probability,
    pub lifecycle_state: SignalLifecycleState,
    pub reason: String,
    pub risk_decision: String,
    pub evidence_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_by_user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub approved_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejected_by_user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub rejected_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalTransitionData {
    pub id: String,
    pub signal_id: String,
    pub from_state: SignalLifecycleState,
    pub to_state: SignalLifecycleState,
    pub trigger_type: String,
    pub trigger_payload: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecomputeSignalRequest {
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApproveSignalRequest {
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_version: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectSignalRequest {
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_version: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecomputeSignalData {
    pub signal: SignalData,
    pub estimate: ProbabilityEstimateData,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition: Option<SignalTransitionData>,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApproveSignalData {
    pub signal: SignalData,
    pub risk_state: RiskStateData,
    pub replayed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectSignalData {
    pub signal: SignalData,
    pub risk_state: RiskStateData,
    pub replayed: bool,
}
