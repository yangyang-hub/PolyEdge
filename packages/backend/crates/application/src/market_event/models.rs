#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketView {
    pub id: String,
    pub question: String,
    pub category: String,
    pub status: MarketStatus,
    pub best_bid: Probability,
    pub best_ask: Probability,
    pub mid_price: Probability,
    pub volume_24h: UsdAmount,
    pub ambiguity_level: AmbiguityLevel,
    pub tradability_status: TradabilityStatus,
    pub resolution_source: String,
    pub edge_case_notes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polymarket_condition_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polymarket_yes_asset_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polymarket_no_asset_id: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

impl MarketView {
    #[must_use]
    pub fn polymarket_asset_id_for_side(&self, side: SignalSide) -> Option<&str> {
        match side {
            SignalSide::Yes => self.polymarket_yes_asset_id.as_deref(),
            SignalSide::No => self.polymarket_no_asset_id.as_deref(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventView {
    pub id: String,
    pub source: String,
    pub summary: String,
    pub relevance_score: Probability,
    pub confidence: Probability,
    pub status: EventStatus,
    pub related_market_ids: Vec<String>,
    pub reason_trace: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceView {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalView {
    pub id: String,
    pub market_id: String,
    pub event_id: String,
    pub action: SignalAction,
    pub side: SignalSide,
    pub market_price: Probability,
    pub fair_price: Probability,
    pub edge: Edge,
    pub confidence: Probability,
    pub lifecycle_state: SignalLifecycleState,
    pub reason: String,
    pub risk_decision: String,
    pub evidence_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_by_user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub approved_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejected_by_user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub rejected_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbabilityEstimateView {
    pub id: String,
    pub market_id: String,
    pub event_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal_id: Option<String>,
    pub prior_price: Probability,
    pub posterior_price: Probability,
    pub fair_price: Probability,
    pub market_price: Probability,
    pub edge: Edge,
    pub confidence: Probability,
    pub time_horizon: TimeHorizon,
    pub model_version: String,
    pub reason_codes: Vec<String>,
    pub evidence_count: u32,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalTransitionView {
    pub id: String,
    pub signal_id: String,
    pub from_state: SignalLifecycleState,
    pub to_state: SignalLifecycleState,
    pub trigger_type: String,
    pub trigger_payload: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct MarketListFilters {
    pub status: Option<MarketStatus>,
    pub tradability_status: Option<TradabilityStatus>,
    pub category: Option<String>,
    pub sort_by: MarketSortField,
    pub sort_order: SortOrder,
    pub offset: u32,
    pub limit: u16,
}

impl MarketListFilters {
    pub fn new(
        status: Option<MarketStatus>,
        tradability_status: Option<TradabilityStatus>,
        category: Option<String>,
        sort_by: Option<MarketSortField>,
        sort_order: Option<SortOrder>,
        offset: Option<u32>,
        limit: Option<u16>,
    ) -> Result<Self> {
        Ok(Self {
            status,
            tradability_status,
            category,
            sort_by: sort_by.unwrap_or_default(),
            sort_order: sort_order.unwrap_or_default(),
            offset: offset.unwrap_or(0),
            limit: validate_limit(limit)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct EventListFilters {
    pub status: Option<EventStatus>,
    pub limit: u16,
}

impl EventListFilters {
    pub fn new(status: Option<EventStatus>, limit: Option<u16>) -> Result<Self> {
        Ok(Self {
            status,
            limit: validate_limit(limit)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct EvidenceListFilters {
    pub market_id: Option<String>,
    pub event_id: Option<String>,
    pub status: Option<EvidenceStatus>,
    pub limit: u16,
}

impl EvidenceListFilters {
    pub fn new(
        market_id: Option<String>,
        event_id: Option<String>,
        status: Option<EvidenceStatus>,
        limit: Option<u16>,
    ) -> Result<Self> {
        Ok(Self {
            market_id: validate_optional_id("market_id", market_id)?,
            event_id: validate_optional_id("event_id", event_id)?,
            status,
            limit: validate_limit(limit)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct SignalListFilters {
    pub market_id: Option<String>,
    pub event_id: Option<String>,
    pub lifecycle_state: Option<SignalLifecycleState>,
    pub limit: u16,
}

impl SignalListFilters {
    pub fn new(
        market_id: Option<String>,
        event_id: Option<String>,
        lifecycle_state: Option<SignalLifecycleState>,
        limit: Option<u16>,
    ) -> Result<Self> {
        Ok(Self {
            market_id: validate_optional_id("market_id", market_id)?,
            event_id: validate_optional_id("event_id", event_id)?,
            lifecycle_state,
            limit: validate_limit(limit)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ProbabilityEstimateListFilters {
    pub market_id: Option<String>,
    pub event_id: Option<String>,
    pub signal_id: Option<String>,
    pub limit: u16,
}

impl ProbabilityEstimateListFilters {
    pub fn new(
        market_id: Option<String>,
        event_id: Option<String>,
        signal_id: Option<String>,
        limit: Option<u16>,
    ) -> Result<Self> {
        Ok(Self {
            market_id: validate_optional_id("market_id", market_id)?,
            event_id: validate_optional_id("event_id", event_id)?,
            signal_id: validate_optional_id("signal_id", signal_id)?,
            limit: validate_limit(limit)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct SignalTransitionListFilters {
    pub signal_id: String,
    pub limit: u16,
}

impl SignalTransitionListFilters {
    pub fn new(signal_id: impl Into<String>, limit: Option<u16>) -> Result<Self> {
        let signal_id = signal_id.into();
        if signal_id.trim().is_empty() {
            return Err(AppError::invalid_input(
                "SIGNAL_ID_REQUIRED",
                "signal id must not be empty",
            ));
        }

        Ok(Self {
            signal_id: signal_id.trim().to_string(),
            limit: validate_limit(limit)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureMarketRecord {
    pub id: String,
    pub question: String,
    pub category: String,
    pub status: MarketStatus,
    pub best_bid: Probability,
    pub best_ask: Probability,
    pub mid_price: Probability,
    pub volume_24h: UsdAmount,
    pub ambiguity_level: AmbiguityLevel,
    pub tradability_status: TradabilityStatus,
    pub resolution_source: String,
    pub edge_case_notes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polymarket_condition_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polymarket_yes_asset_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polymarket_no_asset_id: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureEventRecord {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_event_id: Option<String>,
    pub source: String,
    pub summary: String,
    pub relevance_score: Probability,
    pub confidence: Probability,
    pub status: EventStatus,
    pub related_market_ids: Vec<String>,
    pub reason_trace: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureEvidenceRecord {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureSignalRecord {
    pub id: String,
    pub market_id: String,
    pub event_id: String,
    pub action: SignalAction,
    pub side: SignalSide,
    pub market_price: Probability,
    pub fair_price: Probability,
    pub edge: Edge,
    pub confidence: Probability,
    pub lifecycle_state: SignalLifecycleState,
    pub reason: String,
    pub risk_decision: String,
    pub evidence_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_by_user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub approved_at: Option<OffsetDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejected_by_user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub rejected_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FixtureBundle {
    pub markets: Vec<FixtureMarketRecord>,
    pub events: Vec<FixtureEventRecord>,
    pub evidences: Vec<FixtureEvidenceRecord>,
    pub signals: Vec<FixtureSignalRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureIngestionReport {
    pub markets_upserted: usize,
    pub events_upserted: usize,
    pub evidences_upserted: usize,
    pub signals_upserted: usize,
}

#[derive(Debug, Clone)]
pub struct RecomputeSignalCommand {
    pub signal_id: String,
    pub reason: String,
    pub trace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecomputeSignalResult {
    pub signal: SignalView,
    pub estimate: ProbabilityEstimateView,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition: Option<SignalTransitionView>,
}

#[derive(Debug, Clone)]
pub struct RecomputeSignalDraft {
    pub next_signal: SignalView,
    pub estimate: ProbabilityEstimateView,
    pub transition: Option<SignalTransitionDraft>,
}

#[derive(Debug, Clone)]
pub struct SourceHealthAdjustment {
    pub source: String,
    pub health_score: Probability,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalTransitionDraft {
    pub from_state: SignalLifecycleState,
    pub to_state: SignalLifecycleState,
    pub trigger_type: String,
    pub trigger_payload: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}
