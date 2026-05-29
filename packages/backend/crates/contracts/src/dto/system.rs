// System mode and runtime-config DTOs.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemModeData {
    pub mode: SystemMode,
    pub environment: String,
    pub version: i64,
    pub replayed: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionSystemModeRequest {
    pub to_mode: SystemMode,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfigEntryData {
    pub key: String,
    pub section: String,
    pub field: String,
    pub label: String,
    pub env_name: String,
    pub value: String,
    pub default_value: String,
    pub value_type: String,
    pub options: Vec<String>,
    pub restart_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRuntimeConfigRequest {
    pub values: BTreeMap<String, String>,
}
