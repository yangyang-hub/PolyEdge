pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    InvalidInput,
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    DependencyUnavailable,
    Internal,
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct AppError {
    kind: ErrorKind,
    code: &'static str,
    message: String,
    retryable: bool,
}

impl AppError {
    #[must_use]
    pub fn invalid_input(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(ErrorKind::InvalidInput, code, message, false)
    }

    #[must_use]
    pub fn unauthorized(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Unauthorized, code, message, false)
    }

    #[must_use]
    pub fn forbidden(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Forbidden, code, message, false)
    }

    #[must_use]
    pub fn not_found(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(ErrorKind::NotFound, code, message, false)
    }

    #[must_use]
    pub fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Conflict, code, message, false)
    }

    #[must_use]
    pub fn dependency_unavailable(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(ErrorKind::DependencyUnavailable, code, message, true)
    }

    #[must_use]
    pub fn internal(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Internal, code, message, true)
    }

    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    #[must_use]
    pub fn code(&self) -> &'static str {
        self.code
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub fn retryable(&self) -> bool {
        self.retryable
    }

    #[must_use]
    fn new(
        kind: ErrorKind,
        code: &'static str,
        message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self {
            kind,
            code,
            message: message.into(),
            retryable,
        }
    }
}

fn round_decimal(value: Decimal, scale: u32) -> Decimal {
    let rounded = value.round_dp_with_strategy(scale, RoundingStrategy::MidpointNearestEven);
    if rounded.is_zero() {
        Decimal::ZERO
    } else {
        rounded.normalize()
    }
}

fn format_decimal(value: Decimal, scale: u32) -> String {
    round_decimal(value, scale).to_string()
}

fn deserialize_decimal_str<'de, D>(deserializer: D) -> std::result::Result<Decimal, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    Decimal::from_str(&raw).map_err(serde::de::Error::custom)
}
