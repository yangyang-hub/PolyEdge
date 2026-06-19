#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemMode {
    LiveAuto,
    KillSwitchLocked,
}

impl SystemMode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MarketSortField {
    #[default]
    UpdatedAt,
    Volume24h,
}

impl MarketSortField {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UpdatedAt => "updated_at",
            Self::Volume24h => "volume_24h",
        }
    }
}

impl FromStr for MarketSortField {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "updated_at" => Ok(Self::UpdatedAt),
            "volume_24h" => Ok(Self::Volume24h),
            _ => Err(AppError::invalid_input(
                "DOMAIN_MARKET_SORT_FIELD_INVALID",
                format!("unknown market sort field: {value}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    Asc,
    #[default]
    Desc,
}

impl SortOrder {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }
}

impl FromStr for SortOrder {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "asc" => Ok(Self::Asc),
            "desc" => Ok(Self::Desc),
            _ => Err(AppError::invalid_input(
                "DOMAIN_SORT_ORDER_INVALID",
                format!("unknown sort order: {value}"),
            )),
        }
    }
}

impl FromStr for SystemMode {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "live_auto" | "research" | "paper_trade" | "manual_confirm" => Ok(Self::LiveAuto),
            "kill_switch_locked" => Ok(Self::KillSwitchLocked),
            _ => Err(AppError::invalid_input(
                "DOMAIN_SYSTEM_MODE_INVALID",
                format!("unknown system mode: {value}"),
            )),
        }
    }
}
