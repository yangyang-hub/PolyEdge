use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, str::FromStr};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Probability(Decimal);

impl Probability {
    pub const SCALE: u32 = 6;

    pub fn new(value: Decimal) -> Result<Self> {
        if value < Decimal::ZERO || value > Decimal::ONE {
            return Err(AppError::invalid_input(
                "DOMAIN_PROBABILITY_OUT_OF_RANGE",
                "probability must be within [0, 1]",
            ));
        }

        Ok(Self(value))
    }

    #[must_use]
    pub fn value(self) -> Decimal {
        self.0
    }

    #[must_use]
    pub fn api_string(self) -> String {
        format_decimal(self.0, Self::SCALE)
    }
}

impl Serialize for Probability {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.api_string())
    }
}

impl<'de> Deserialize<'de> for Probability {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(deserialize_decimal_str(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for Probability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.api_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Edge(Decimal);

impl Edge {
    pub const SCALE: u32 = 6;

    pub fn new(value: Decimal) -> Result<Self> {
        if value < -Decimal::ONE || value > Decimal::ONE {
            return Err(AppError::invalid_input(
                "DOMAIN_EDGE_OUT_OF_RANGE",
                "edge must be within [-1, 1]",
            ));
        }

        Ok(Self(value))
    }

    #[must_use]
    pub fn value(self) -> Decimal {
        self.0
    }

    #[must_use]
    pub fn api_string(self) -> String {
        format_decimal(self.0, Self::SCALE)
    }
}

impl Serialize for Edge {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.api_string())
    }
}

impl<'de> Deserialize<'de> for Edge {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(deserialize_decimal_str(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for Edge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.api_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExposureRatio(Decimal);

impl ExposureRatio {
    pub const SCALE: u32 = 6;
    const MAX: Decimal = Decimal::from_parts(10, 0, 0, false, 0);

    pub fn new(value: Decimal) -> Result<Self> {
        if value < Decimal::ZERO || value > Self::MAX {
            return Err(AppError::invalid_input(
                "DOMAIN_EXPOSURE_RATIO_OUT_OF_RANGE",
                "exposure ratio must be within [0, 10]",
            ));
        }

        Ok(Self(value))
    }

    #[must_use]
    pub fn value(self) -> Decimal {
        self.0
    }

    #[must_use]
    pub fn api_string(self) -> String {
        format_decimal(self.0, Self::SCALE)
    }
}

impl Serialize for ExposureRatio {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.api_string())
    }
}

impl<'de> Deserialize<'de> for ExposureRatio {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(deserialize_decimal_str(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for ExposureRatio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.api_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Quantity(Decimal);

impl Quantity {
    pub const SCALE: u32 = 8;

    pub fn new(value: Decimal) -> Result<Self> {
        if value < Decimal::ZERO {
            return Err(AppError::invalid_input(
                "DOMAIN_QUANTITY_OUT_OF_RANGE",
                "quantity must be non-negative",
            ));
        }

        Ok(Self(value))
    }

    #[must_use]
    pub fn value(self) -> Decimal {
        self.0
    }

    #[must_use]
    pub fn api_string(self) -> String {
        format_decimal(self.0, Self::SCALE)
    }
}

impl Serialize for Quantity {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.api_string())
    }
}

impl<'de> Deserialize<'de> for Quantity {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(deserialize_decimal_str(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.api_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UsdAmount(Decimal);

impl UsdAmount {
    pub const SCALE: u32 = 2;

    pub fn new(value: Decimal) -> Result<Self> {
        if value < Decimal::ZERO {
            return Err(AppError::invalid_input(
                "DOMAIN_USD_AMOUNT_OUT_OF_RANGE",
                "usd amount must be non-negative",
            ));
        }

        Ok(Self(value))
    }

    #[must_use]
    pub fn value(self) -> Decimal {
        self.0
    }

    #[must_use]
    pub fn api_string(self) -> String {
        format_decimal(self.0, Self::SCALE)
    }
}

impl Serialize for UsdAmount {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.api_string())
    }
}

impl<'de> Deserialize<'de> for UsdAmount {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(deserialize_decimal_str(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for UsdAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.api_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SignedUsdAmount(Decimal);

impl SignedUsdAmount {
    pub const SCALE: u32 = 2;

    pub fn new(value: Decimal) -> Result<Self> {
        Ok(Self(value))
    }

    #[must_use]
    pub fn value(self) -> Decimal {
        self.0
    }

    #[must_use]
    pub fn api_string(self) -> String {
        format_decimal(self.0, Self::SCALE)
    }
}

impl Serialize for SignedUsdAmount {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.api_string())
    }
}

impl<'de> Deserialize<'de> for SignedUsdAmount {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(deserialize_decimal_str(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for SignedUsdAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.api_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemMode {
    Research,
    PaperTrade,
    ManualConfirm,
    LiveAuto,
    KillSwitchLocked,
}

impl SystemMode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Research => "research",
            Self::PaperTrade => "paper_trade",
            Self::ManualConfirm => "manual_confirm",
            Self::LiveAuto => "live_auto",
            Self::KillSwitchLocked => "kill_switch_locked",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketStatus {
    Open,
    Closed,
    Resolved,
}

impl MarketStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
            Self::Resolved => "resolved",
        }
    }
}

impl FromStr for MarketStatus {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "open" => Ok(Self::Open),
            "closed" => Ok(Self::Closed),
            "resolved" => Ok(Self::Resolved),
            _ => Err(AppError::invalid_input(
                "DOMAIN_MARKET_STATUS_INVALID",
                format!("unknown market status: {value}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmbiguityLevel {
    Low,
    Medium,
    High,
}

impl AmbiguityLevel {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

impl FromStr for AmbiguityLevel {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            _ => Err(AppError::invalid_input(
                "DOMAIN_AMBIGUITY_LEVEL_INVALID",
                format!("unknown ambiguity level: {value}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TradabilityStatus {
    Tradable,
    ManualReview,
    ObserveOnly,
    Blocked,
}

impl TradabilityStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tradable => "tradable",
            Self::ManualReview => "manual_review",
            Self::ObserveOnly => "observe_only",
            Self::Blocked => "blocked",
        }
    }
}

impl FromStr for TradabilityStatus {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "tradable" => Ok(Self::Tradable),
            "manual_review" => Ok(Self::ManualReview),
            "observe_only" => Ok(Self::ObserveOnly),
            "blocked" => Ok(Self::Blocked),
            _ => Err(AppError::invalid_input(
                "DOMAIN_TRADABILITY_STATUS_INVALID",
                format!("unknown tradability status: {value}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventStatus {
    Active,
    Expired,
    Invalidated,
    Superseded,
}

impl EventStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Expired => "expired",
            Self::Invalidated => "invalidated",
            Self::Superseded => "superseded",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceDirection {
    SupportsYes,
    SupportsNo,
    Background,
}

impl EvidenceDirection {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SupportsYes => "supports_yes",
            Self::SupportsNo => "supports_no",
            Self::Background => "background",
        }
    }
}

impl FromStr for EvidenceDirection {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "supports_yes" => Ok(Self::SupportsYes),
            "supports_no" => Ok(Self::SupportsNo),
            "background" => Ok(Self::Background),
            _ => Err(AppError::invalid_input(
                "DOMAIN_EVIDENCE_DIRECTION_INVALID",
                format!("unknown evidence direction: {value}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStatus {
    Active,
    Expired,
    Invalidated,
}

impl EvidenceStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Expired => "expired",
            Self::Invalidated => "invalidated",
        }
    }
}

impl FromStr for EvidenceStatus {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "active" => Ok(Self::Active),
            "expired" => Ok(Self::Expired),
            "invalidated" => Ok(Self::Invalidated),
            _ => Err(AppError::invalid_input(
                "DOMAIN_EVIDENCE_STATUS_INVALID",
                format!("unknown evidence status: {value}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalAction {
    Buy,
    Sell,
}

impl SignalAction {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
        }
    }
}

impl FromStr for SignalAction {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "buy" => Ok(Self::Buy),
            "sell" => Ok(Self::Sell),
            _ => Err(AppError::invalid_input(
                "DOMAIN_SIGNAL_ACTION_INVALID",
                format!("unknown signal action: {value}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalSide {
    Yes,
    No,
}

impl SignalSide {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Yes => "yes",
            Self::No => "no",
        }
    }
}

impl FromStr for SignalSide {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "yes" => Ok(Self::Yes),
            "no" => Ok(Self::No),
            _ => Err(AppError::invalid_input(
                "DOMAIN_SIGNAL_SIDE_INVALID",
                format!("unknown signal side: {value}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalLifecycleState {
    New,
    Active,
    Weakened,
    Executed,
    Invalidated,
    Reversed,
    Expired,
}

impl SignalLifecycleState {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Active => "active",
            Self::Weakened => "weakened",
            Self::Executed => "executed",
            Self::Invalidated => "invalidated",
            Self::Reversed => "reversed",
            Self::Expired => "expired",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderDraftStatus {
    Queued,
    Submitted,
    Rejected,
    Canceled,
}

impl OrderDraftStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Submitted => "submitted",
            Self::Rejected => "rejected",
            Self::Canceled => "canceled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionRequestStatus {
    Queued,
    Submitted,
    Failed,
    Canceled,
}

impl ExecutionRequestStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Submitted => "submitted",
            Self::Failed => "failed",
            Self::Canceled => "canceled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    New,
    Submitted,
    Open,
    PartiallyFilled,
    Filled,
    Canceled,
    Expired,
    Rejected,
}

impl OrderStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Submitted => "submitted",
            Self::Open => "open",
            Self::PartiallyFilled => "partially_filled",
            Self::Filled => "filled",
            Self::Canceled => "canceled",
            Self::Expired => "expired",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeHorizon {
    Short,
    Medium,
    Long,
}

impl TimeHorizon {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Short => "short",
            Self::Medium => "medium",
            Self::Long => "long",
        }
    }
}

impl FromStr for TimeHorizon {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "short" => Ok(Self::Short),
            "medium" => Ok(Self::Medium),
            "long" => Ok(Self::Long),
            _ => Err(AppError::invalid_input(
                "DOMAIN_TIME_HORIZON_INVALID",
                format!("unknown time horizon: {value}"),
            )),
        }
    }
}

impl FromStr for SignalLifecycleState {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "new" => Ok(Self::New),
            "active" => Ok(Self::Active),
            "weakened" => Ok(Self::Weakened),
            "executed" => Ok(Self::Executed),
            "invalidated" => Ok(Self::Invalidated),
            "reversed" => Ok(Self::Reversed),
            "expired" => Ok(Self::Expired),
            _ => Err(AppError::invalid_input(
                "DOMAIN_SIGNAL_LIFECYCLE_STATE_INVALID",
                format!("unknown signal lifecycle state: {value}"),
            )),
        }
    }
}

impl FromStr for OrderDraftStatus {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "queued" => Ok(Self::Queued),
            "submitted" => Ok(Self::Submitted),
            "rejected" => Ok(Self::Rejected),
            "canceled" => Ok(Self::Canceled),
            _ => Err(AppError::invalid_input(
                "DOMAIN_ORDER_DRAFT_STATUS_INVALID",
                format!("unknown order draft status: {value}"),
            )),
        }
    }
}

impl FromStr for ExecutionRequestStatus {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "queued" => Ok(Self::Queued),
            "submitted" => Ok(Self::Submitted),
            "failed" => Ok(Self::Failed),
            "canceled" => Ok(Self::Canceled),
            _ => Err(AppError::invalid_input(
                "DOMAIN_EXECUTION_REQUEST_STATUS_INVALID",
                format!("unknown execution request status: {value}"),
            )),
        }
    }
}

impl FromStr for OrderStatus {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "new" => Ok(Self::New),
            "submitted" => Ok(Self::Submitted),
            "open" => Ok(Self::Open),
            "partially_filled" => Ok(Self::PartiallyFilled),
            "filled" => Ok(Self::Filled),
            "canceled" => Ok(Self::Canceled),
            "expired" => Ok(Self::Expired),
            "rejected" => Ok(Self::Rejected),
            _ => Err(AppError::invalid_input(
                "DOMAIN_ORDER_STATUS_INVALID",
                format!("unknown order status: {value}"),
            )),
        }
    }
}

impl FromStr for EventStatus {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "active" => Ok(Self::Active),
            "expired" => Ok(Self::Expired),
            "invalidated" => Ok(Self::Invalidated),
            "superseded" => Ok(Self::Superseded),
            _ => Err(AppError::invalid_input(
                "DOMAIN_EVENT_STATUS_INVALID",
                format!("unknown event status: {value}"),
            )),
        }
    }
}

impl FromStr for SystemMode {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "research" => Ok(Self::Research),
            "paper_trade" => Ok(Self::PaperTrade),
            "manual_confirm" => Ok(Self::ManualConfirm),
            "live_auto" => Ok(Self::LiveAuto),
            "kill_switch_locked" => Ok(Self::KillSwitchLocked),
            _ => Err(AppError::invalid_input(
                "DOMAIN_SYSTEM_MODE_INVALID",
                format!("unknown system mode: {value}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    Viewer,
    Operator,
    RiskAdmin,
    Admin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepUpScope {
    SignalApprove,
    SignalReject,
    ExecutionSubmit,
    OrderCancelForce,
    SystemModeSwitch,
    SystemKillSwitchTrigger,
    SystemKillSwitchRelease,
    RiskThresholdUpdate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditResult {
    Accepted,
    Succeeded,
    Rejected,
    Failed,
}

impl AuditResult {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Succeeded => "succeeded",
            Self::Rejected => "rejected",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdempotencyStatus {
    Started,
    Completed,
    Failed,
}

impl IdempotencyStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

impl FromStr for IdempotencyStatus {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "started" => Ok(Self::Started),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            _ => Err(AppError::invalid_input(
                "DOMAIN_IDEMPOTENCY_STATUS_INVALID",
                format!("unknown idempotency status: {value}"),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probability_serializes_as_canonical_string() {
        let value = Probability::new(Decimal::from_str("0.500000").expect("valid decimal"))
            .expect("valid probability");

        let serialized = serde_json::to_string(&value).expect("serialize");
        assert_eq!(serialized, "\"0.5\"");
    }

    #[test]
    fn edge_rejects_out_of_range_value() {
        let edge = Edge::new(Decimal::from_str("1.5").expect("valid decimal"));
        assert!(edge.is_err());
    }

    #[test]
    fn quantity_rejects_negative_value() {
        let quantity = Quantity::new(Decimal::from_str("-1").expect("valid decimal"));
        assert!(quantity.is_err());
    }

    #[test]
    fn usd_amount_serializes_as_two_decimal_string() {
        let amount = UsdAmount::new(Decimal::from_str("125000.00").expect("valid decimal"))
            .expect("valid usd amount");

        let serialized = serde_json::to_string(&amount).expect("serialize");
        assert_eq!(serialized, "\"125000\"");
    }

    #[test]
    fn signed_usd_amount_serializes_negative_values() {
        let amount = SignedUsdAmount::new(Decimal::from_str("-125.50").expect("valid decimal"))
            .expect("valid signed usd amount");

        let serialized = serde_json::to_string(&amount).expect("serialize");
        assert_eq!(serialized, "\"-125.5\"");
    }

    #[test]
    fn tradability_status_parses_from_contract_value() {
        let status = TradabilityStatus::from_str("manual_review").expect("valid status");
        assert_eq!(status.as_str(), "manual_review");
    }

    #[test]
    fn evidence_direction_parses_from_contract_value() {
        let direction =
            EvidenceDirection::from_str("supports_no").expect("valid evidence direction");
        assert_eq!(direction.as_str(), "supports_no");
    }

    #[test]
    fn signal_lifecycle_state_rejects_unknown_value() {
        let state = SignalLifecycleState::from_str("queued");
        assert!(state.is_err());
    }

    #[test]
    fn time_horizon_parses_from_contract_value() {
        let horizon = TimeHorizon::from_str("medium").expect("valid horizon");
        assert_eq!(horizon.as_str(), "medium");
    }
}
