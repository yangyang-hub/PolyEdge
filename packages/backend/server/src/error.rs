use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use polyedge_contracts::{ApiError, ApiErrorMeta, ApiErrorResponse};
use polyedge_domain::{AppError, ErrorKind};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("configuration error: {0}")]
    Configuration(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("not found: {0}")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("dependency unavailable: {0}")]
    Dependency(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl ServerError {
    #[must_use]
    pub fn status(&self) -> StatusCode {
        match self {
            Self::InvalidInput(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::Dependency(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::Configuration(_) | Self::Database(_) | Self::Internal(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }

    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Configuration(_) => "SERVER_CONFIGURATION_INVALID",
            Self::InvalidInput(_) => "REQUEST_INVALID",
            Self::Unauthorized => "AUTH_UNAUTHORIZED",
            Self::Forbidden => "AUTH_FORBIDDEN",
            Self::NotFound(_) => "RESOURCE_NOT_FOUND",
            Self::Conflict(_) => "RESOURCE_CONFLICT",
            Self::Database(_) => "DATABASE_OPERATION_FAILED",
            Self::Dependency(_) => "DEPENDENCY_UNAVAILABLE",
            Self::Internal(_) => "SERVER_INTERNAL_ERROR",
        }
    }
}

impl From<AppError> for ServerError {
    fn from(error: AppError) -> Self {
        match error.kind() {
            ErrorKind::InvalidInput => Self::InvalidInput(error.message().to_string()),
            ErrorKind::Unauthorized => Self::Unauthorized,
            ErrorKind::Forbidden => Self::Forbidden,
            ErrorKind::NotFound => Self::NotFound(error.message().to_string()),
            ErrorKind::Conflict => Self::Conflict(error.message().to_string()),
            ErrorKind::DependencyUnavailable => Self::Dependency(error.message().to_string()),
            ErrorKind::Internal => Self::Internal(error.message().to_string()),
        }
    }
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let status = self.status();
        let request_id = format!("req_{}", Uuid::now_v7());
        let trace_id = format!("trc_{}", Uuid::now_v7());
        let message = match self {
            Self::Database(_) if status.is_server_error() => {
                "database operation failed".to_string()
            }
            Self::Configuration(_) if status.is_server_error() => {
                "server configuration is invalid".to_string()
            }
            ref error => error.to_string(),
        };
        let body = ApiErrorResponse {
            error: ApiError {
                code: self.code().to_string(),
                message,
                details: None,
                retryable: matches!(status, StatusCode::SERVICE_UNAVAILABLE),
            },
            meta: ApiErrorMeta {
                request_id,
                trace_id,
            },
        };
        (status, Json(body)).into_response()
    }
}

pub type Result<T> = std::result::Result<T, ServerError>;
