// Generic response envelopes, error payloads, and health/readiness DTOs shared across endpoints.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiMeta {
    pub request_id: String,
    pub trace_id: String,
    #[serde(with = "time::serde::rfc3339")]
    pub generated_at: OffsetDateTime,
}

impl ApiMeta {
    #[must_use]
    pub fn new(request_id: impl Into<String>, trace_id: impl Into<String>) -> Self {
        Self {
            request_id: request_id.into(),
            trace_id: trace_id.into(),
            generated_at: OffsetDateTime::now_utc(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub data: T,
    pub meta: ApiMeta,
}

impl<T> ApiResponse<T> {
    #[must_use]
    pub fn new(data: T, request_id: impl Into<String>, trace_id: impl Into<String>) -> Self {
        Self {
            data,
            meta: ApiMeta::new(request_id, trace_id),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<BTreeMap<String, String>>,
    pub retryable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorMeta {
    pub request_id: String,
    pub trace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    pub error: ApiError,
    pub meta: ApiErrorMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthData {
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyStatus {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessData {
    pub status: String,
    pub postgres: DependencyStatus,
    pub orderbook: DependencyStatus,
}
