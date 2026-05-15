use async_trait::async_trait;
use polyedge_domain::{AppError, Edge, Probability, Quantity, Result};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashSet},
    str::FromStr,
    sync::Arc,
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

const DEFAULT_LIST_LIMIT: u16 = 100;
const MAX_LIST_LIMIT: u16 = 500;
const DEFAULT_REPEAT_WINDOW_SECONDS: i64 = 300;

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
pub struct ArbitrageScanListFilters {
    pub limit: u16,
}

impl ArbitrageScanListFilters {
    pub fn new(limit: Option<u16>) -> Result<Self> {
        Ok(Self {
            limit: validate_limit(limit)?,
        })
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
    pub limit: u16,
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
        limit: Option<u16>,
    ) -> Result<Self> {
        Ok(Self {
            market_id: normalize_optional_id("market_id", market_id)?,
            opportunity_type,
            status,
            validation_status,
            min_net_edge,
            observed_after,
            active_only,
            limit: validate_limit(limit)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ArbitrageAnalysisRunListFilters {
    pub limit: u16,
}

impl ArbitrageAnalysisRunListFilters {
    pub fn new(limit: Option<u16>) -> Result<Self> {
        Ok(Self {
            limit: validate_limit(limit)?,
        })
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
            limit: validate_limit(limit)?,
        })
    }
}

#[async_trait]
pub trait ArbitrageStore: Send + Sync {
    async fn start_arbitrage_scan(&self, scan: &ArbitrageScanView) -> Result<()>;

    async fn complete_arbitrage_scan(
        &self,
        scan_id: &str,
        finished_at: OffsetDateTime,
        market_count: u32,
        snapshot_count: u32,
        opportunity_count: u32,
    ) -> Result<ArbitrageScanView>;

    async fn record_market_book_snapshot(&self, snapshot: &MarketBookSnapshotView) -> Result<()>;

    async fn record_arbitrage_opportunity(
        &self,
        opportunity: &ArbitrageOpportunityView,
    ) -> Result<()>;

    async fn record_arbitrage_opportunity_validation(
        &self,
        validation: &ArbitrageOpportunityValidationView,
    ) -> Result<()>;

    async fn expire_arbitrage_opportunities(
        &self,
        observed_before: OffsetDateTime,
        trace_id: &str,
    ) -> Result<Vec<ArbitrageOpportunityView>>;

    async fn list_arbitrage_scans(
        &self,
        filters: &ArbitrageScanListFilters,
    ) -> Result<Vec<ArbitrageScanView>>;

    async fn list_arbitrage_opportunities(
        &self,
        filters: &ArbitrageOpportunityListFilters,
    ) -> Result<Vec<ArbitrageOpportunityView>>;

    async fn record_arbitrage_analysis_run(
        &self,
        analysis: &ArbitrageAnalysisRunView,
    ) -> Result<()>;

    async fn list_arbitrage_analysis_runs(
        &self,
        filters: &ArbitrageAnalysisRunListFilters,
    ) -> Result<Vec<ArbitrageAnalysisRunView>>;

    async fn record_arbitrage_event(
        &self,
        event: &ArbitrageEventView,
    ) -> Result<ArbitrageEventView>;

    async fn list_arbitrage_events(
        &self,
        filters: &ArbitrageEventListFilters,
    ) -> Result<Vec<ArbitrageEventView>>;

    async fn prune_arbitrage_events(&self, occurred_before: OffsetDateTime) -> Result<u64>;
}

pub struct ArbitrageService {
    store: Arc<dyn ArbitrageStore>,
}

impl ArbitrageService {
    #[must_use]
    pub fn new(store: Arc<dyn ArbitrageStore>) -> Self {
        Self { store }
    }

    pub async fn start_scan(&self, scan: ArbitrageScanView) -> Result<ArbitrageScanView> {
        self.store.start_arbitrage_scan(&scan).await?;
        self.record_event(
            ArbitrageEventType::ScanStarted,
            "scan",
            &scan.id,
            scan_payload(&scan),
            scan.started_at,
            &scan.trace_id,
        )
        .await?;
        Ok(scan)
    }

    pub async fn complete_scan(
        &self,
        scan_id: &str,
        finished_at: OffsetDateTime,
        market_count: u32,
        snapshot_count: u32,
        opportunity_count: u32,
    ) -> Result<ArbitrageScanView> {
        let scan = self
            .store
            .complete_arbitrage_scan(
                scan_id,
                finished_at,
                market_count,
                snapshot_count,
                opportunity_count,
            )
            .await?;
        self.record_event(
            ArbitrageEventType::ScanCompleted,
            "scan",
            &scan.id,
            scan_payload(&scan),
            finished_at,
            &scan.trace_id,
        )
        .await?;
        Ok(scan)
    }

    pub async fn record_snapshot_and_detect(
        &self,
        snapshot: MarketBookSnapshotView,
    ) -> Result<Vec<ArbitrageOpportunityView>> {
        self.store.record_market_book_snapshot(&snapshot).await?;
        let drafts = detect_arbitrage_opportunities(&snapshot)?;
        let mut opportunities = Vec::with_capacity(drafts.len());

        for draft in drafts {
            let repeated = self
                .is_repeated_opportunity(
                    &snapshot.market_id,
                    draft.opportunity_type,
                    snapshot.observed_at,
                )
                .await?;
            let status = if repeated {
                ArbitrageOpportunityStatus::Repeated
            } else {
                ArbitrageOpportunityStatus::Observed
            };
            let opportunity = ArbitrageOpportunityView {
                id: opportunity_id(
                    &snapshot.scan_id,
                    &snapshot.market_id,
                    draft.opportunity_type,
                ),
                scan_id: snapshot.scan_id.clone(),
                market_id: snapshot.market_id.clone(),
                opportunity_type: draft.opportunity_type,
                status,
                gross_edge: draft.gross_edge,
                price_sum: draft.price_sum,
                capacity: draft.capacity,
                yes_price: draft.yes_price,
                no_price: draft.no_price,
                yes_size: draft.yes_size,
                no_size: draft.no_size,
                observed_at: snapshot.observed_at,
                reason_codes: draft.reason_codes,
                analysis_payload: draft.analysis_payload,
                trace_id: snapshot.trace_id.clone(),
                validation: None,
            };
            self.store
                .record_arbitrage_opportunity(&opportunity)
                .await?;
            self.record_event(
                if repeated {
                    ArbitrageEventType::OpportunityRepeated
                } else {
                    ArbitrageEventType::OpportunityObserved
                },
                "opportunity",
                &opportunity.id,
                opportunity_payload(&opportunity),
                opportunity.observed_at,
                &opportunity.trace_id,
            )
            .await?;
            opportunities.push(opportunity);
        }

        Ok(opportunities)
    }

    pub async fn record_book_snapshot(
        &self,
        snapshot: MarketBookSnapshotView,
    ) -> Result<MarketBookSnapshotView> {
        self.store.record_market_book_snapshot(&snapshot).await?;
        Ok(snapshot)
    }

    pub async fn validate_opportunity(
        &self,
        opportunity: &ArbitrageOpportunityView,
        snapshot: &MarketBookSnapshotView,
        config: &ArbitrageValidationConfig,
        validated_at: OffsetDateTime,
    ) -> Result<ArbitrageOpportunityValidationView> {
        let validation = validate_arbitrage_opportunity(
            opportunity,
            snapshot,
            config,
            validated_at,
            &opportunity.trace_id,
        )?;
        self.store
            .record_arbitrage_opportunity_validation(&validation)
            .await?;
        self.record_event(
            if validation.status == ArbitrageValidationStatus::Valid {
                ArbitrageEventType::ValidationPassed
            } else {
                ArbitrageEventType::ValidationFailed
            },
            "validation",
            &validation.opportunity_id,
            validation_payload(&validation),
            validation.validated_at,
            &validation.trace_id,
        )
        .await?;

        Ok(validation)
    }

    pub async fn expire_opportunities(
        &self,
        observed_before: OffsetDateTime,
        trace_id: &str,
    ) -> Result<Vec<ArbitrageOpportunityView>> {
        let expired = self
            .store
            .expire_arbitrage_opportunities(observed_before, trace_id)
            .await?;

        for opportunity in &expired {
            self.record_event(
                ArbitrageEventType::OpportunityExpired,
                "opportunity",
                &opportunity.id,
                opportunity_payload(opportunity),
                OffsetDateTime::now_utc(),
                trace_id,
            )
            .await?;
        }

        Ok(expired)
    }

    pub async fn list_opportunities(
        &self,
        filters: ArbitrageOpportunityListFilters,
    ) -> Result<Vec<ArbitrageOpportunityView>> {
        self.store.list_arbitrage_opportunities(&filters).await
    }

    pub async fn list_scans(
        &self,
        filters: ArbitrageScanListFilters,
    ) -> Result<Vec<ArbitrageScanView>> {
        self.store.list_arbitrage_scans(&filters).await
    }

    pub async fn record_analysis_run(
        &self,
        analysis: ArbitrageAnalysisRunView,
    ) -> Result<ArbitrageAnalysisRunView> {
        self.store.record_arbitrage_analysis_run(&analysis).await?;
        self.record_event(
            ArbitrageEventType::AnalysisGenerated,
            "analysis",
            &analysis.id,
            analysis_payload(&analysis),
            analysis.generated_at,
            &analysis.trace_id,
        )
        .await?;
        Ok(analysis)
    }

    pub async fn list_analysis_runs(
        &self,
        filters: ArbitrageAnalysisRunListFilters,
    ) -> Result<Vec<ArbitrageAnalysisRunView>> {
        self.store.list_arbitrage_analysis_runs(&filters).await
    }

    pub async fn list_events(
        &self,
        filters: ArbitrageEventListFilters,
    ) -> Result<Vec<ArbitrageEventView>> {
        self.store.list_arbitrage_events(&filters).await
    }

    pub async fn prune_events(&self, occurred_before: OffsetDateTime) -> Result<u64> {
        self.store.prune_arbitrage_events(occurred_before).await
    }

    async fn is_repeated_opportunity(
        &self,
        market_id: &str,
        opportunity_type: ArbitrageOpportunityType,
        observed_at: OffsetDateTime,
    ) -> Result<bool> {
        let repeated_after = observed_at - time::Duration::seconds(DEFAULT_REPEAT_WINDOW_SECONDS);
        let recent = self
            .store
            .list_arbitrage_opportunities(&ArbitrageOpportunityListFilters::new(
                Some(market_id.to_string()),
                Some(opportunity_type),
                None,
                None,
                None,
                Some(repeated_after),
                true,
                Some(1),
            )?)
            .await?;

        Ok(recent.into_iter().any(|opportunity| {
            opportunity.status != ArbitrageOpportunityStatus::Expired
                && opportunity.observed_at < observed_at
        }))
    }

    async fn record_event(
        &self,
        event_type: ArbitrageEventType,
        resource_type: &str,
        resource_id: &str,
        payload: Value,
        occurred_at: OffsetDateTime,
        trace_id: &str,
    ) -> Result<ArbitrageEventView> {
        let event = ArbitrageEventView {
            sequence: 0,
            id: arbitrage_event_id(event_type, resource_id, occurred_at),
            event_type,
            resource_type: resource_type.to_string(),
            resource_id: resource_id.to_string(),
            payload,
            occurred_at,
            trace_id: trace_id.to_string(),
        };

        self.store.record_arbitrage_event(&event).await
    }
}

pub fn detect_arbitrage_opportunities(
    snapshot: &MarketBookSnapshotView,
) -> Result<Vec<ArbitrageOpportunityDraft>> {
    let mut opportunities = Vec::new();

    if let (Some(yes_ask), Some(no_ask)) = (snapshot.yes_ask, snapshot.no_ask) {
        let price_sum = yes_ask.value() + no_ask.value();
        let gross_edge = Decimal::ONE - price_sum;
        if gross_edge > Decimal::ZERO {
            opportunities.push(ArbitrageOpportunityDraft {
                opportunity_type: ArbitrageOpportunityType::BinaryBuyBoth,
                gross_edge: Edge::new(gross_edge)?,
                price_sum,
                capacity: min_quantity(snapshot.yes_ask_size, snapshot.no_ask_size),
                yes_price: yes_ask,
                no_price: no_ask,
                yes_size: snapshot.yes_ask_size,
                no_size: snapshot.no_ask_size,
                reason_codes: vec!["yes_ask_plus_no_ask_below_one".to_string()],
                analysis_payload: json!({
                    "formula": "1 - yes_ask - no_ask",
                    "yes_ask": yes_ask,
                    "no_ask": no_ask,
                    "price_sum": price_sum,
                    "gross_edge": gross_edge,
                }),
            });
        }
    }

    if let (Some(yes_bid), Some(no_bid)) = (snapshot.yes_bid, snapshot.no_bid) {
        let price_sum = yes_bid.value() + no_bid.value();
        let gross_edge = price_sum - Decimal::ONE;
        if gross_edge > Decimal::ZERO {
            opportunities.push(ArbitrageOpportunityDraft {
                opportunity_type: ArbitrageOpportunityType::BinarySellBoth,
                gross_edge: Edge::new(gross_edge)?,
                price_sum,
                capacity: min_quantity(snapshot.yes_bid_size, snapshot.no_bid_size),
                yes_price: yes_bid,
                no_price: no_bid,
                yes_size: snapshot.yes_bid_size,
                no_size: snapshot.no_bid_size,
                reason_codes: vec!["yes_bid_plus_no_bid_above_one".to_string()],
                analysis_payload: json!({
                    "formula": "yes_bid + no_bid - 1",
                    "yes_bid": yes_bid,
                    "no_bid": no_bid,
                    "price_sum": price_sum,
                    "gross_edge": gross_edge,
                }),
            });
        }
    }

    Ok(opportunities)
}

pub fn validate_arbitrage_opportunity(
    opportunity: &ArbitrageOpportunityView,
    snapshot: &MarketBookSnapshotView,
    config: &ArbitrageValidationConfig,
    validated_at: OffsetDateTime,
    trace_id: &str,
) -> Result<ArbitrageOpportunityValidationView> {
    let mut status = ArbitrageValidationStatus::Valid;
    let mut reason_codes = Vec::new();
    let book_age_ms = nonnegative_millis(validated_at - snapshot.observed_at);
    let current_draft = detect_arbitrage_opportunities(snapshot)?
        .into_iter()
        .find(|draft| draft.opportunity_type == opportunity.opportunity_type);
    let gross_edge = current_draft
        .as_ref()
        .map_or(Decimal::ZERO, |draft| draft.gross_edge.value());
    let current_capacity = current_draft
        .as_ref()
        .map_or(Quantity::new(Decimal::ZERO)?, |draft| draft.capacity);
    let fee_estimate = config.fee_buffer.value();
    let slippage_buffer = config.slippage_buffer.value();
    let net_edge = clamp_edge(gross_edge - fee_estimate - slippage_buffer);

    if snapshot.market_id != opportunity.market_id || snapshot.scan_id != opportunity.scan_id {
        set_validation_status(
            &mut status,
            ArbitrageValidationStatus::InvalidMarket,
            &mut reason_codes,
            "snapshot_opportunity_mismatch",
        );
    }

    if book_age_ms > config.max_book_age_ms {
        set_validation_status(
            &mut status,
            ArbitrageValidationStatus::StaleBook,
            &mut reason_codes,
            "book_age_exceeds_threshold",
        );
    }

    if current_draft.is_none() {
        set_validation_status(
            &mut status,
            ArbitrageValidationStatus::PriceMoved,
            &mut reason_codes,
            "opportunity_no_longer_present_in_latest_book",
        );
    }

    if gross_edge < config.min_gross_edge.value() {
        set_validation_status(
            &mut status,
            ArbitrageValidationStatus::BelowThreshold,
            &mut reason_codes,
            "gross_edge_below_threshold",
        );
    }

    if current_capacity.value() < config.min_capacity.value() {
        set_validation_status(
            &mut status,
            ArbitrageValidationStatus::InsufficientDepth,
            &mut reason_codes,
            "capacity_below_threshold",
        );
    }

    if net_edge <= Decimal::ZERO {
        set_validation_status(
            &mut status,
            ArbitrageValidationStatus::FeesExceedEdge,
            &mut reason_codes,
            "net_edge_not_positive_after_buffers",
        );
    }

    if status == ArbitrageValidationStatus::Valid {
        reason_codes.push("net_edge_positive_after_buffers".to_string());
    }

    let validated_capacity = if status == ArbitrageValidationStatus::Valid {
        current_capacity
    } else {
        Quantity::new(Decimal::ZERO)?
    };

    Ok(ArbitrageOpportunityValidationView {
        id: arbitrage_validation_id(&opportunity.id, validated_at),
        opportunity_id: opportunity.id.clone(),
        status,
        gross_edge: Edge::new(gross_edge)?,
        net_edge: Edge::new(net_edge)?,
        fee_estimate: config.fee_buffer,
        slippage_buffer: config.slippage_buffer,
        validated_capacity,
        book_age_ms,
        reason_codes,
        validation_payload: json!({
            "max_book_age_ms": config.max_book_age_ms,
            "min_gross_edge": config.min_gross_edge,
            "min_capacity": config.min_capacity,
            "fee_buffer": config.fee_buffer,
            "slippage_buffer": config.slippage_buffer,
            "snapshot_id": snapshot.id,
            "snapshot_observed_at": snapshot.observed_at,
            "discovery_gross_edge": opportunity.gross_edge,
            "discovery_capacity": opportunity.capacity,
            "current_capacity": current_capacity,
            "validated_at": validated_at,
        }),
        validated_at,
        trace_id: trace_id.to_string(),
    })
}

#[must_use]
pub fn build_arbitrage_analysis(
    opportunities: &[ArbitrageOpportunityView],
    lookback_hours: u16,
    generated_at: OffsetDateTime,
) -> ArbitrageAnalysisSummary {
    let mut market_ids = HashSet::new();
    let mut type_counts = BTreeMap::<&'static str, (ArbitrageOpportunityType, u32)>::new();
    let mut market_groups = BTreeMap::<String, Vec<&ArbitrageOpportunityView>>::new();

    for opportunity in opportunities {
        market_ids.insert(opportunity.market_id.clone());
        let entry = type_counts
            .entry(opportunity.opportunity_type.as_str())
            .or_insert((opportunity.opportunity_type, 0));
        entry.1 += 1;
        market_groups
            .entry(opportunity.market_id.clone())
            .or_default()
            .push(opportunity);
    }

    let mut top_markets: Vec<_> = market_groups
        .into_iter()
        .map(|(market_id, items)| market_summary(market_id, &items))
        .collect();
    top_markets.sort_by(|left, right| {
        right
            .opportunity_count
            .cmp(&left.opportunity_count)
            .then_with(|| right.max_gross_edge.cmp(&left.max_gross_edge))
            .then_with(|| left.market_id.cmp(&right.market_id))
    });
    top_markets.truncate(20);

    ArbitrageAnalysisSummary {
        generated_at,
        lookback_hours,
        opportunity_count: u32::try_from(opportunities.len()).unwrap_or(u32::MAX),
        market_count: u32::try_from(market_ids.len()).unwrap_or(u32::MAX),
        type_counts: type_counts
            .into_values()
            .map(|(opportunity_type, count)| ArbitrageTypeCount {
                opportunity_type,
                count,
            })
            .collect(),
        top_markets,
    }
}

#[must_use]
pub fn market_book_snapshot_id(scan_id: &str, market_id: &str) -> String {
    format!(
        "book_{}_{}",
        id_fragment(scan_id).trim_start_matches("scan_"),
        id_fragment(market_id)
    )
}

#[must_use]
pub fn opportunity_id(
    scan_id: &str,
    market_id: &str,
    opportunity_type: ArbitrageOpportunityType,
) -> String {
    format!(
        "arb_{}_{}_{}",
        id_fragment(scan_id).trim_start_matches("scan_"),
        id_fragment(market_id),
        opportunity_type.as_str()
    )
}

fn market_summary(
    market_id: String,
    opportunities: &[&ArbitrageOpportunityView],
) -> ArbitrageMarketSummary {
    let count = Decimal::from(opportunities.len() as u64);
    let first = opportunities
        .iter()
        .map(|opportunity| opportunity.observed_at)
        .min()
        .unwrap_or(OffsetDateTime::UNIX_EPOCH);
    let last = opportunities
        .iter()
        .map(|opportunity| opportunity.observed_at)
        .max()
        .unwrap_or(OffsetDateTime::UNIX_EPOCH);
    let gross_edge_sum = opportunities
        .iter()
        .map(|opportunity| opportunity.gross_edge.value())
        .sum::<Decimal>();
    let capacity_sum = opportunities
        .iter()
        .map(|opportunity| opportunity.capacity.value())
        .sum::<Decimal>();
    let max_gross_edge = opportunities
        .iter()
        .map(|opportunity| opportunity.gross_edge.value())
        .max()
        .unwrap_or(Decimal::ZERO);
    let max_capacity = opportunities
        .iter()
        .map(|opportunity| opportunity.capacity.value())
        .max()
        .unwrap_or(Decimal::ZERO);

    ArbitrageMarketSummary {
        market_id,
        opportunity_count: u32::try_from(opportunities.len()).unwrap_or(u32::MAX),
        first_observed_at: first.to_string(),
        last_observed_at: last.to_string(),
        duration_seconds: (last - first).whole_seconds(),
        max_gross_edge,
        avg_gross_edge: if count > Decimal::ZERO {
            gross_edge_sum / count
        } else {
            Decimal::ZERO
        },
        max_capacity,
        avg_capacity: if count > Decimal::ZERO {
            capacity_sum / count
        } else {
            Decimal::ZERO
        },
    }
}

fn min_quantity(left: Quantity, right: Quantity) -> Quantity {
    if left <= right { left } else { right }
}

fn set_validation_status(
    current: &mut ArbitrageValidationStatus,
    next: ArbitrageValidationStatus,
    reason_codes: &mut Vec<String>,
    reason_code: &str,
) {
    if *current == ArbitrageValidationStatus::Valid {
        *current = next;
    }
    reason_codes.push(reason_code.to_string());
}

fn nonnegative_millis(duration: time::Duration) -> u64 {
    let millis = duration.whole_milliseconds();
    if millis <= 0 {
        0
    } else {
        u64::try_from(millis).unwrap_or(u64::MAX)
    }
}

fn clamp_edge(value: Decimal) -> Decimal {
    value.max(-Decimal::ONE).min(Decimal::ONE)
}

fn arbitrage_validation_id(opportunity_id: &str, validated_at: OffsetDateTime) -> String {
    format!(
        "arb_val_{}_{}",
        id_fragment(opportunity_id).trim_start_matches("arb_"),
        validated_at.unix_timestamp_nanos()
    )
}

fn arbitrage_event_id(
    event_type: ArbitrageEventType,
    resource_id: &str,
    occurred_at: OffsetDateTime,
) -> String {
    format!(
        "arb_evt_{}_{}_{}",
        id_fragment(event_type.as_str()),
        id_fragment(resource_id),
        occurred_at.unix_timestamp_nanos()
    )
}

fn timestamp_payload(timestamp: OffsetDateTime) -> String {
    timestamp
        .format(&Rfc3339)
        .unwrap_or_else(|_| timestamp.to_string())
}

fn scan_payload(scan: &ArbitrageScanView) -> Value {
    json!({
        "scan_id": &scan.id,
        "started_at": timestamp_payload(scan.started_at),
        "finished_at": scan.finished_at.map(timestamp_payload),
        "market_count": scan.market_count,
        "snapshot_count": scan.snapshot_count,
        "opportunity_count": scan.opportunity_count,
        "scanner_version": &scan.scanner_version,
        "metadata": &scan.metadata,
        "trace_id": &scan.trace_id,
    })
}

fn opportunity_payload(opportunity: &ArbitrageOpportunityView) -> Value {
    json!({
        "opportunity_id": &opportunity.id,
        "scan_id": &opportunity.scan_id,
        "market_id": &opportunity.market_id,
        "opportunity_type": opportunity.opportunity_type,
        "status": opportunity.status,
        "gross_edge": opportunity.gross_edge,
        "price_sum": opportunity.price_sum,
        "capacity": opportunity.capacity,
        "yes_price": opportunity.yes_price,
        "no_price": opportunity.no_price,
        "yes_size": opportunity.yes_size,
        "no_size": opportunity.no_size,
        "observed_at": timestamp_payload(opportunity.observed_at),
        "reason_codes": &opportunity.reason_codes,
        "analysis_payload": &opportunity.analysis_payload,
        "validation": &opportunity.validation,
        "trace_id": &opportunity.trace_id,
    })
}

fn validation_payload(validation: &ArbitrageOpportunityValidationView) -> Value {
    json!({
        "validation_id": &validation.id,
        "opportunity_id": &validation.opportunity_id,
        "validation_status": validation.status,
        "gross_edge": validation.gross_edge,
        "net_edge": validation.net_edge,
        "fee_estimate": validation.fee_estimate,
        "slippage_buffer": validation.slippage_buffer,
        "validated_capacity": validation.validated_capacity,
        "book_age_ms": validation.book_age_ms,
        "reason_codes": &validation.reason_codes,
        "validation_payload": &validation.validation_payload,
        "validated_at": timestamp_payload(validation.validated_at),
        "trace_id": &validation.trace_id,
    })
}

fn analysis_payload(analysis: &ArbitrageAnalysisRunView) -> Value {
    json!({
        "analysis_id": &analysis.id,
        "generated_at": timestamp_payload(analysis.generated_at),
        "lookback_hours": analysis.lookback_hours,
        "opportunity_count": analysis.opportunity_count,
        "market_count": analysis.market_count,
        "summary_payload": &analysis.summary_payload,
        "trace_id": &analysis.trace_id,
    })
}

fn validate_limit(limit: Option<u16>) -> Result<u16> {
    let limit = limit.unwrap_or(DEFAULT_LIST_LIMIT);
    if limit == 0 {
        return Err(AppError::invalid_input(
            "ARBITRAGE_LIST_LIMIT_INVALID",
            "arbitrage list limit must be greater than zero",
        ));
    }

    if limit > MAX_LIST_LIMIT {
        return Err(AppError::invalid_input(
            "ARBITRAGE_LIST_LIMIT_INVALID",
            format!("arbitrage list limit must be at most {MAX_LIST_LIMIT}"),
        ));
    }

    Ok(limit)
}

fn normalize_optional_id(field: &'static str, value: Option<String>) -> Result<Option<String>> {
    value
        .map(|raw| {
            let normalized = raw.trim().to_string();
            if normalized.is_empty() {
                Err(AppError::invalid_input(
                    "ARBITRAGE_FILTER_INVALID",
                    format!("{field} filter must not be empty"),
                ))
            } else {
                Ok(normalized)
            }
        })
        .transpose()
}

fn id_fragment(value: &str) -> String {
    let fragment: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = fragment.trim_matches('_');
    if trimmed.is_empty() {
        "id".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ArbitrageAnalysisSummary, ArbitrageOpportunityStatus, ArbitrageOpportunityType,
        ArbitrageValidationConfig, ArbitrageValidationStatus, MarketBookSnapshotView,
        build_arbitrage_analysis, detect_arbitrage_opportunities, validate_arbitrage_opportunity,
    };
    use polyedge_domain::{Edge, Probability, Quantity, Result};
    use rust_decimal::Decimal;
    use serde_json::json;
    use time::{Duration, OffsetDateTime};

    fn probability(units: i64, scale: u32) -> Probability {
        Probability::new(Decimal::new(units, scale)).expect("probability")
    }

    fn quantity(units: i64) -> Quantity {
        Quantity::new(Decimal::from(units)).expect("quantity")
    }

    fn snapshot() -> MarketBookSnapshotView {
        MarketBookSnapshotView {
            id: "book_1".to_string(),
            scan_id: "scan_1".to_string(),
            connector_name: "fixture".to_string(),
            market_id: "mkt_1".to_string(),
            yes_asset_id: Some("yes".to_string()),
            no_asset_id: Some("no".to_string()),
            yes_bid: Some(probability(60, 2)),
            yes_ask: Some(probability(44, 2)),
            yes_bid_size: quantity(7),
            yes_ask_size: quantity(11),
            no_bid: Some(probability(43, 2)),
            no_ask: Some(probability(53, 2)),
            no_bid_size: quantity(9),
            no_ask_size: quantity(13),
            observed_at: OffsetDateTime::UNIX_EPOCH,
            raw_payload: json!({}),
            trace_id: "trc_1".to_string(),
        }
    }

    #[test]
    fn detect_arbitrage_opportunities_finds_buy_and_sell_dislocations() -> Result<()> {
        let opportunities = detect_arbitrage_opportunities(&snapshot())?;

        assert_eq!(opportunities.len(), 2);
        assert_eq!(
            opportunities[0].opportunity_type,
            ArbitrageOpportunityType::BinaryBuyBoth
        );
        assert_eq!(opportunities[0].gross_edge, Edge::new(Decimal::new(3, 2))?);
        assert_eq!(opportunities[0].capacity, quantity(11));
        assert_eq!(
            opportunities[1].opportunity_type,
            ArbitrageOpportunityType::BinarySellBoth
        );
        assert_eq!(opportunities[1].gross_edge, Edge::new(Decimal::new(3, 2))?);
        assert_eq!(opportunities[1].capacity, quantity(7));

        Ok(())
    }

    #[test]
    fn build_arbitrage_analysis_groups_markets_and_types() -> Result<()> {
        let observed_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(60);
        let opportunities = detect_arbitrage_opportunities(&snapshot())?
            .into_iter()
            .map(|draft| super::ArbitrageOpportunityView {
                id: format!("opp_{}", draft.opportunity_type.as_str()),
                scan_id: "scan_1".to_string(),
                market_id: "mkt_1".to_string(),
                opportunity_type: draft.opportunity_type,
                status: ArbitrageOpportunityStatus::Observed,
                gross_edge: draft.gross_edge,
                price_sum: draft.price_sum,
                capacity: draft.capacity,
                yes_price: draft.yes_price,
                no_price: draft.no_price,
                yes_size: draft.yes_size,
                no_size: draft.no_size,
                observed_at,
                reason_codes: draft.reason_codes,
                analysis_payload: draft.analysis_payload,
                trace_id: "trc_1".to_string(),
                validation: None,
            })
            .collect::<Vec<_>>();

        let summary: ArbitrageAnalysisSummary =
            build_arbitrage_analysis(&opportunities, 24, observed_at);

        assert_eq!(summary.opportunity_count, 2);
        assert_eq!(summary.market_count, 1);
        assert_eq!(summary.type_counts.len(), 2);
        assert_eq!(summary.top_markets.len(), 1);
        assert_eq!(summary.top_markets[0].market_id, "mkt_1");
        assert_eq!(summary.top_markets[0].opportunity_count, 2);

        Ok(())
    }

    #[test]
    fn arbitrage_event_payload_timestamps_are_rfc3339_strings() -> Result<()> {
        let started_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(1);
        let finished_at = started_at + Duration::seconds(2);
        let scan = super::ArbitrageScanView {
            id: "scan_1".to_string(),
            started_at,
            finished_at: Some(finished_at),
            market_count: 1,
            snapshot_count: 1,
            opportunity_count: 1,
            scanner_version: "v1".to_string(),
            metadata: json!({ "book_source": "fixture" }),
            trace_id: "trc_1".to_string(),
        };

        let scan_payload = super::scan_payload(&scan);
        assert!(scan_payload["started_at"].is_string());
        assert!(scan_payload["finished_at"].is_string());

        let snapshot = snapshot();
        let draft = detect_arbitrage_opportunities(&snapshot)?
            .into_iter()
            .next()
            .expect("opportunity");
        let opportunity = super::ArbitrageOpportunityView {
            id: "opp_binary_buy_both".to_string(),
            scan_id: snapshot.scan_id.clone(),
            market_id: snapshot.market_id.clone(),
            opportunity_type: draft.opportunity_type,
            status: ArbitrageOpportunityStatus::Observed,
            gross_edge: draft.gross_edge,
            price_sum: draft.price_sum,
            capacity: draft.capacity,
            yes_price: draft.yes_price,
            no_price: draft.no_price,
            yes_size: draft.yes_size,
            no_size: draft.no_size,
            observed_at: snapshot.observed_at,
            reason_codes: draft.reason_codes,
            analysis_payload: draft.analysis_payload,
            trace_id: "trc_1".to_string(),
            validation: None,
        };
        let opportunity_payload = super::opportunity_payload(&opportunity);
        assert!(opportunity_payload["observed_at"].is_string());

        let config = ArbitrageValidationConfig {
            max_book_age_ms: 5_000,
            min_gross_edge: Edge::new(Decimal::new(1, 2))?,
            min_capacity: quantity(5),
            fee_buffer: Edge::new(Decimal::new(5, 3))?,
            slippage_buffer: Edge::new(Decimal::new(5, 3))?,
        };
        let validation = validate_arbitrage_opportunity(
            &opportunity,
            &snapshot,
            &config,
            snapshot.observed_at + Duration::milliseconds(50),
            "trc_1",
        )?;
        let validation_payload = super::validation_payload(&validation);
        assert!(validation_payload["validated_at"].is_string());

        let analysis_payload = super::analysis_payload(&super::ArbitrageAnalysisRunView {
            id: "arb_analysis_1".to_string(),
            generated_at: finished_at,
            lookback_hours: 24,
            opportunity_count: 1,
            market_count: 1,
            summary_payload: json!({ "generated_at": "1970-01-01T00:00:03Z" }),
            trace_id: "trc_1".to_string(),
        });
        assert!(analysis_payload["generated_at"].is_string());

        Ok(())
    }

    #[test]
    fn validate_arbitrage_opportunity_applies_buffers_and_capacity_rules() -> Result<()> {
        let snapshot = snapshot();
        let observed_at = snapshot.observed_at + Duration::seconds(1);
        let draft = detect_arbitrage_opportunities(&snapshot)?
            .into_iter()
            .next()
            .expect("opportunity");
        let opportunity = super::ArbitrageOpportunityView {
            id: "opp_binary_buy_both".to_string(),
            scan_id: snapshot.scan_id.clone(),
            market_id: snapshot.market_id.clone(),
            opportunity_type: draft.opportunity_type,
            status: ArbitrageOpportunityStatus::Observed,
            gross_edge: draft.gross_edge,
            price_sum: draft.price_sum,
            capacity: draft.capacity,
            yes_price: draft.yes_price,
            no_price: draft.no_price,
            yes_size: draft.yes_size,
            no_size: draft.no_size,
            observed_at: snapshot.observed_at,
            reason_codes: draft.reason_codes,
            analysis_payload: draft.analysis_payload,
            trace_id: "trc_1".to_string(),
            validation: None,
        };
        let config = ArbitrageValidationConfig {
            max_book_age_ms: 5_000,
            min_gross_edge: Edge::new(Decimal::new(1, 2))?,
            min_capacity: quantity(5),
            fee_buffer: Edge::new(Decimal::new(5, 3))?,
            slippage_buffer: Edge::new(Decimal::new(5, 3))?,
        };

        let validation =
            validate_arbitrage_opportunity(&opportunity, &snapshot, &config, observed_at, "trc_1")?;

        assert_eq!(validation.status, ArbitrageValidationStatus::Valid);
        assert_eq!(validation.net_edge, Edge::new(Decimal::new(2, 2))?);
        assert_eq!(validation.validated_capacity, quantity(11));

        let stale_validation = validate_arbitrage_opportunity(
            &opportunity,
            &snapshot,
            &config,
            observed_at + Duration::seconds(10),
            "trc_1",
        )?;

        assert_eq!(
            stale_validation.status,
            ArbitrageValidationStatus::StaleBook
        );

        Ok(())
    }

    #[test]
    fn validate_arbitrage_opportunity_marks_price_moved_when_current_book_loses_edge() -> Result<()>
    {
        let discovery_snapshot = snapshot();
        let observed_at = discovery_snapshot.observed_at + Duration::seconds(1);
        let draft = detect_arbitrage_opportunities(&discovery_snapshot)?
            .into_iter()
            .find(|draft| draft.opportunity_type == ArbitrageOpportunityType::BinaryBuyBoth)
            .expect("buy-both opportunity");
        let opportunity = super::ArbitrageOpportunityView {
            id: "opp_binary_buy_both".to_string(),
            scan_id: discovery_snapshot.scan_id.clone(),
            market_id: discovery_snapshot.market_id.clone(),
            opportunity_type: draft.opportunity_type,
            status: ArbitrageOpportunityStatus::Observed,
            gross_edge: draft.gross_edge,
            price_sum: draft.price_sum,
            capacity: draft.capacity,
            yes_price: draft.yes_price,
            no_price: draft.no_price,
            yes_size: draft.yes_size,
            no_size: draft.no_size,
            observed_at: discovery_snapshot.observed_at,
            reason_codes: draft.reason_codes,
            analysis_payload: draft.analysis_payload,
            trace_id: "trc_1".to_string(),
            validation: None,
        };
        let validation_snapshot = MarketBookSnapshotView {
            id: "book_1_validation".to_string(),
            scan_id: discovery_snapshot.scan_id.clone(),
            connector_name: discovery_snapshot.connector_name.clone(),
            market_id: discovery_snapshot.market_id.clone(),
            yes_asset_id: discovery_snapshot.yes_asset_id.clone(),
            no_asset_id: discovery_snapshot.no_asset_id.clone(),
            yes_bid: Some(probability(49, 2)),
            yes_ask: Some(probability(50, 2)),
            yes_bid_size: quantity(7),
            yes_ask_size: quantity(11),
            no_bid: Some(probability(49, 2)),
            no_ask: Some(probability(51, 2)),
            no_bid_size: quantity(9),
            no_ask_size: quantity(13),
            observed_at,
            raw_payload: json!({ "fixture": "price_moved" }),
            trace_id: "trc_1".to_string(),
        };
        let config = ArbitrageValidationConfig {
            max_book_age_ms: 5_000,
            min_gross_edge: Edge::new(Decimal::new(1, 2))?,
            min_capacity: quantity(5),
            fee_buffer: Edge::new(Decimal::new(5, 3))?,
            slippage_buffer: Edge::new(Decimal::new(5, 3))?,
        };

        let validation = validate_arbitrage_opportunity(
            &opportunity,
            &validation_snapshot,
            &config,
            observed_at + Duration::milliseconds(50),
            "trc_1",
        )?;

        assert_eq!(validation.status, ArbitrageValidationStatus::PriceMoved);
        assert_eq!(validation.gross_edge, Edge::new(Decimal::ZERO)?);
        assert_eq!(validation.validated_capacity, quantity(0));
        assert!(
            validation
                .reason_codes
                .contains(&"opportunity_no_longer_present_in_latest_book".to_string())
        );
        assert_eq!(
            validation.validation_payload["snapshot_id"],
            json!("book_1_validation")
        );

        Ok(())
    }
}
