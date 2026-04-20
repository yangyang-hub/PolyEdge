use axum::{
    Json,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use polyedge_contracts::{ApiError, ApiErrorMeta, ApiErrorResponse};
use polyedge_domain::{AppError, ErrorKind, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub struct HttpError {
    error: AppError,
    request_id: String,
    trace_id: String,
}

impl HttpError {
    #[must_use]
    pub fn with_meta(
        error: AppError,
        request_id: impl Into<String>,
        trace_id: impl Into<String>,
    ) -> Self {
        Self {
            error,
            request_id: request_id.into(),
            trace_id: trace_id.into(),
        }
    }
}

impl From<AppError> for HttpError {
    fn from(error: AppError) -> Self {
        Self::with_meta(error, "unknown", new_trace_id())
    }
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        let status = match self.error.kind() {
            ErrorKind::InvalidInput => StatusCode::BAD_REQUEST,
            ErrorKind::Unauthorized => StatusCode::UNAUTHORIZED,
            ErrorKind::Forbidden => StatusCode::FORBIDDEN,
            ErrorKind::NotFound => StatusCode::NOT_FOUND,
            ErrorKind::Conflict => StatusCode::CONFLICT,
            ErrorKind::DependencyUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            ErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let body = ApiErrorResponse {
            error: ApiError {
                code: self.error.code().to_string(),
                message: self.error.message().to_string(),
                details: None,
                retryable: self.error.retryable(),
            },
            meta: ApiErrorMeta {
                request_id: self.request_id,
                trace_id: self.trace_id,
            },
        };

        (status, Json(body)).into_response()
    }
}

#[must_use]
pub fn new_trace_id() -> String {
    format!("trc_{}", Uuid::now_v7())
}

#[must_use]
pub fn request_id_from_headers(headers: &HeaderMap) -> String {
    headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .map(std::borrow::ToOwned::to_owned)
        .unwrap_or_else(|| format!("req_{}", Uuid::now_v7()))
}

pub fn hash_json<T: Serialize>(value: &T) -> Result<String> {
    let bytes = serde_json::to_vec(value).map_err(|error| {
        AppError::internal(
            "HASH_JSON_SERIALIZE_FAILED",
            format!("failed to serialize request body for hashing: {error}"),
        )
    })?;

    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }

    Ok(hex)
}
