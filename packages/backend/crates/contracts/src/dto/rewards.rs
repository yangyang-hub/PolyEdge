/// Flat rewards configuration update payload. Strategy fields stay compatible
/// with the existing config patch JSON while operator metadata is kept outside
/// the application configuration model.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateRewardBotConfigRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_note: Option<String>,
    #[serde(flatten)]
    pub patch: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct RewardBotControlRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_note: Option<String>,
}
