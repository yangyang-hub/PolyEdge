// Arbitrage and probability-estimate DTOs: scans, opportunities, validations, analysis runs.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbabilityEstimateData {
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
pub struct ArbitrageScanData {
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
pub struct ArbitrageOpportunityData {
    pub id: String,
    pub scan_id: String,
    pub market_id: String,
    pub opportunity_type: String,
    pub status: String,
    pub gross_edge: Edge,
    pub price_sum: String,
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
    pub validation: Option<ArbitrageOpportunityValidationData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageOpportunityValidationData {
    pub id: String,
    pub opportunity_id: String,
    pub status: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageAnalysisRunData {
    pub id: String,
    #[serde(with = "time::serde::rfc3339")]
    pub generated_at: OffsetDateTime,
    pub lookback_hours: u16,
    pub opportunity_count: u32,
    pub market_count: u32,
    pub summary_payload: Value,
    pub trace_id: String,
}
