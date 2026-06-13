// Market and event DTOs, including the market list envelope.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketListResponse {
    pub data: Vec<MarketData>,
    pub total_count: i64,
    pub meta: ApiMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketCategoryData {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketData {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    pub question: String,
    pub category: String,
    pub status: MarketStatus,
    pub best_bid: Probability,
    pub best_ask: Probability,
    pub mid_price: Probability,
    pub volume_24h: UsdAmount,
    pub liquidity_usd: UsdAmount,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "time::serde::rfc3339::option")]
    pub end_at: Option<OffsetDateTime>,
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
pub struct EventData {
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
