// News-source health, raw news events, and evidence DTOs.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsSourceHealthData {
    pub source: String,
    pub source_type: String,
    pub enabled: bool,
    pub reliability: Probability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_success_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_error_at: Option<OffsetDateTime>,
    pub consecutive_failures: u64,
    pub items_fetched: u64,
    pub items_inserted: u64,
    pub items_deduped: u64,
    pub health_score: Probability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsRawEventData {
    pub id: String,
    pub source: String,
    pub source_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub published_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub event_time: OffsetDateTime,
    pub hash: String,
    pub raw_payload: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub ingested_at: OffsetDateTime,
    pub trace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceData {
    pub id: String,
    pub market_id: String,
    pub event_id: String,
    pub direction: EvidenceDirection,
    pub strength: Probability,
    pub source_reliability: Probability,
    pub novelty: Probability,
    pub resolution_relevance: Probability,
    pub status: EvidenceStatus,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}
