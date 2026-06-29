// Combined rewards provider request/decision. A single LLM call may carry both
// the AI advisory context and the info-risk context for one market. Either
// section is optional: `advisory` is `None` when `ai_advisory_enabled` is false,
// `info_risk` is `None` when `info_risk_enabled` is false; at least one is
// `Some`. The two sub-requests keep their own `input_hash` (and the info-risk
// sub-request its `query_hash`) so the two cache tables
// (`reward_market_advisories`, `reward_market_info_risks`) stay keyed and TTL'd
// independently after the merged call writes both rows.

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RewardProviderRequest {
    pub condition_id: String,
    pub provider: RewardAiProvider,
    pub request_format: RewardAiRequestFormat,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advisory: Option<RewardAiAdvisoryRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info_risk: Option<RewardInfoRiskAssessmentRequest>,
}

impl RewardProviderRequest {
    #[must_use]
    pub fn wants_advisory(&self) -> bool {
        self.advisory.is_some()
    }

    #[must_use]
    pub fn wants_info_risk(&self) -> bool {
        self.info_risk.is_some()
    }
}

/// Combined provider decision parsed from a single response. Each field is
/// `Some` only when the corresponding request section was present and the model
/// returned a parseable object for it.
#[derive(Debug, Clone, PartialEq)]
pub struct RewardProviderDecision {
    pub advisory: Option<RewardAiAdvisoryDecision>,
    pub info_risk: Option<RewardInfoRiskAssessmentDecision>,
}
