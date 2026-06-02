#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArbitrageOpportunityType {
    BinaryBuyBoth,
    BinarySellBoth,
}

impl ArbitrageOpportunityType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BinaryBuyBoth => "binary_buy_both",
            Self::BinarySellBoth => "binary_sell_both",
        }
    }
}

impl FromStr for ArbitrageOpportunityType {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "binary_buy_both" => Ok(Self::BinaryBuyBoth),
            "binary_sell_both" => Ok(Self::BinarySellBoth),
            other => Err(AppError::invalid_input(
                "ARBITRAGE_OPPORTUNITY_TYPE_INVALID",
                format!("unknown arbitrage opportunity_type: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArbitrageOpportunityStatus {
    Observed,
    Expired,
    Repeated,
}

impl ArbitrageOpportunityStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Observed => "observed",
            Self::Expired => "expired",
            Self::Repeated => "repeated",
        }
    }
}

impl FromStr for ArbitrageOpportunityStatus {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "observed" => Ok(Self::Observed),
            "expired" => Ok(Self::Expired),
            "repeated" => Ok(Self::Repeated),
            other => Err(AppError::invalid_input(
                "ARBITRAGE_OPPORTUNITY_STATUS_INVALID",
                format!("unknown arbitrage opportunity status: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArbitrageValidationStatus {
    Unvalidated,
    Valid,
    StaleBook,
    InsufficientDepth,
    PriceMoved,
    FeesExceedEdge,
    BelowThreshold,
    InvalidMarket,
    Error,
}

impl ArbitrageValidationStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unvalidated => "unvalidated",
            Self::Valid => "valid",
            Self::StaleBook => "stale_book",
            Self::InsufficientDepth => "insufficient_depth",
            Self::PriceMoved => "price_moved",
            Self::FeesExceedEdge => "fees_exceed_edge",
            Self::BelowThreshold => "below_threshold",
            Self::InvalidMarket => "invalid_market",
            Self::Error => "error",
        }
    }
}

impl FromStr for ArbitrageValidationStatus {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "unvalidated" => Ok(Self::Unvalidated),
            "valid" => Ok(Self::Valid),
            "stale_book" => Ok(Self::StaleBook),
            "insufficient_depth" => Ok(Self::InsufficientDepth),
            "price_moved" => Ok(Self::PriceMoved),
            "fees_exceed_edge" => Ok(Self::FeesExceedEdge),
            "below_threshold" => Ok(Self::BelowThreshold),
            "invalid_market" => Ok(Self::InvalidMarket),
            "error" => Ok(Self::Error),
            other => Err(AppError::invalid_input(
                "ARBITRAGE_VALIDATION_STATUS_INVALID",
                format!("unknown arbitrage validation status: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArbitrageEventType {
    ScanStarted,
    ScanCompleted,
    OpportunityObserved,
    OpportunityRepeated,
    OpportunityExpired,
    ValidationPassed,
    ValidationFailed,
    AnalysisGenerated,
}

impl ArbitrageEventType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ScanStarted => "arbitrage.scan.started",
            Self::ScanCompleted => "arbitrage.scan.completed",
            Self::OpportunityObserved => "arbitrage.opportunity.observed",
            Self::OpportunityRepeated => "arbitrage.opportunity.repeated",
            Self::OpportunityExpired => "arbitrage.opportunity.expired",
            Self::ValidationPassed => "arbitrage.validation.passed",
            Self::ValidationFailed => "arbitrage.validation.failed",
            Self::AnalysisGenerated => "arbitrage.analysis.generated",
        }
    }
}

impl FromStr for ArbitrageEventType {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "arbitrage.scan.started" => Ok(Self::ScanStarted),
            "arbitrage.scan.completed" => Ok(Self::ScanCompleted),
            "arbitrage.opportunity.observed" => Ok(Self::OpportunityObserved),
            "arbitrage.opportunity.repeated" => Ok(Self::OpportunityRepeated),
            "arbitrage.opportunity.expired" => Ok(Self::OpportunityExpired),
            "arbitrage.validation.passed" => Ok(Self::ValidationPassed),
            "arbitrage.validation.failed" => Ok(Self::ValidationFailed),
            "arbitrage.analysis.generated" => Ok(Self::AnalysisGenerated),
            other => Err(AppError::invalid_input(
                "ARBITRAGE_EVENT_TYPE_INVALID",
                format!("unknown arbitrage event type: {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageScanView {
    pub id: String,
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub finished_at: Option<OffsetDateTime>,
    pub market_count: u32,
    pub snapshot_count: u32,
    pub opportunity_count: u32,
    pub scanner_version: String,
    pub metadata: Value,
    pub trace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketBookSnapshotView {
    pub id: String,
    pub scan_id: String,
    pub connector_name: String,
    pub market_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yes_asset_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_asset_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yes_bid: Option<Probability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yes_ask: Option<Probability>,
    pub yes_bid_size: Quantity,
    pub yes_ask_size: Quantity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_bid: Option<Probability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_ask: Option<Probability>,
    pub no_bid_size: Quantity,
    pub no_ask_size: Quantity,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
    pub raw_payload: Value,
    pub trace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageOpportunityView {
    pub id: String,
    pub scan_id: String,
    pub market_id: String,
    pub opportunity_type: ArbitrageOpportunityType,
    pub status: ArbitrageOpportunityStatus,
    pub gross_edge: Edge,
    pub price_sum: Decimal,
    pub capacity: Quantity,
    pub yes_price: Probability,
    pub no_price: Probability,
    pub yes_size: Quantity,
    pub no_size: Quantity,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
    pub reason_codes: Vec<String>,
    pub analysis_payload: Value,
    pub trace_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<ArbitrageOpportunityValidationView>,
}

#[derive(Debug, Clone)]
pub struct ArbitrageOpportunityDraft {
    pub opportunity_type: ArbitrageOpportunityType,
    pub gross_edge: Edge,
    pub price_sum: Decimal,
    pub capacity: Quantity,
    pub yes_price: Probability,
    pub no_price: Probability,
    pub yes_size: Quantity,
    pub no_size: Quantity,
    pub reason_codes: Vec<String>,
    pub analysis_payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageOpportunityValidationView {
    pub id: String,
    pub opportunity_id: String,
    pub status: ArbitrageValidationStatus,
    pub gross_edge: Edge,
    pub net_edge: Edge,
    pub fee_estimate: Edge,
    pub slippage_buffer: Edge,
    pub validated_capacity: Quantity,
    pub book_age_ms: u64,
    pub reason_codes: Vec<String>,
    pub validation_payload: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub validated_at: OffsetDateTime,
    pub trace_id: String,
}

#[derive(Debug, Clone)]
pub struct ArbitrageValidationConfig {
    pub max_book_age_ms: u64,
    pub min_gross_edge: Edge,
    pub min_capacity: Quantity,
    pub fee_buffer: Edge,
    pub slippage_buffer: Edge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageEventView {
    pub sequence: u64,
    pub id: String,
    pub event_type: ArbitrageEventType,
    pub resource_type: String,
    pub resource_id: String,
    pub payload: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub occurred_at: OffsetDateTime,
    pub trace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageAnalysisRunView {
    pub id: String,
    #[serde(with = "time::serde::rfc3339")]
    pub generated_at: OffsetDateTime,
    pub lookback_hours: u16,
    pub opportunity_count: u32,
    pub market_count: u32,
    pub summary_payload: Value,
    pub trace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageAnalysisSummary {
    #[serde(with = "time::serde::rfc3339")]
    pub generated_at: OffsetDateTime,
    pub lookback_hours: u16,
    pub opportunity_count: u32,
    pub market_count: u32,
    pub type_counts: Vec<ArbitrageTypeCount>,
    pub top_markets: Vec<ArbitrageMarketSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageTypeCount {
    pub opportunity_type: ArbitrageOpportunityType,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageMarketSummary {
    pub market_id: String,
    pub opportunity_count: u32,
    pub first_observed_at: String,
    pub last_observed_at: String,
    pub duration_seconds: i64,
    pub max_gross_edge: Decimal,
    pub avg_gross_edge: Decimal,
    pub max_capacity: Decimal,
    pub avg_capacity: Decimal,
}

#[derive(Debug, Clone)]
pub struct ArbitrageScanListFilters {}

impl ArbitrageScanListFilters {
    pub fn new() -> Result<Self> {
        Ok(Self {})
    }
}

#[derive(Debug, Clone)]
pub struct ArbitrageOpportunityListFilters {
    pub market_id: Option<String>,
    pub opportunity_type: Option<ArbitrageOpportunityType>,
    pub status: Option<ArbitrageOpportunityStatus>,
    pub validation_status: Option<ArbitrageValidationStatus>,
    pub min_net_edge: Option<Edge>,
    pub observed_after: Option<OffsetDateTime>,
    pub active_only: bool,
}

impl ArbitrageOpportunityListFilters {
    pub fn new(
        market_id: Option<String>,
        opportunity_type: Option<ArbitrageOpportunityType>,
        status: Option<ArbitrageOpportunityStatus>,
        validation_status: Option<ArbitrageValidationStatus>,
        min_net_edge: Option<Edge>,
        observed_after: Option<OffsetDateTime>,
        active_only: bool,
    ) -> Result<Self> {
        Ok(Self {
            market_id,
            opportunity_type,
            status,
            validation_status,
            min_net_edge,
            observed_after,
            active_only,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ArbitrageAnalysisRunListFilters {}

impl ArbitrageAnalysisRunListFilters {
    pub fn new() -> Result<Self> {
        Ok(Self {})
    }
}

#[derive(Debug, Clone)]
pub struct ArbitrageEventListFilters {
    pub after_sequence: Option<u64>,
    pub limit: u16,
}

impl ArbitrageEventListFilters {
    pub fn new(after_sequence: Option<u64>, limit: Option<u16>) -> Result<Self> {
        Ok(Self {
            after_sequence,
            limit: limit.unwrap_or(100),
        })
    }
}
