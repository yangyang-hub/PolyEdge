// List query-parameter DTOs for all collection endpoints.

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MarketListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<MarketStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tradability_status: Option<TradabilityStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_order: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<EventStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NewsSourceHealthListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NewsRawEventListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EvidenceListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<EvidenceStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SignalListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", alias = "status")]
    pub lifecycle_state: Option<SignalLifecycleState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProbabilityEstimateListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ArbitrageScanListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ArbitrageOpportunityListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opportunity_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_net_edge: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_after: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ArbitrageAnalysisRunListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SignalTransitionListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrderDraftListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connector_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<OrderDraftStatus>,
    /// 1-based page number (default 1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    /// Items per page (default 20, max 200).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecutionRequestListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connector_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<ExecutionRequestStatus>,
    /// 1-based page number (default 1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    /// Items per page (default 20, max 200).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrderListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connector_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<OrderStatus>,
    /// 1-based page number (default 1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    /// Items per page (default 20, max 200).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TradeListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connector_name: Option<String>,
    /// 1-based page number (default 1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    /// Items per page (default 20, max 200).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PositionListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connector_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side: Option<SignalSide>,
    /// 1-based page number (default 1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    /// Items per page (default 20, max 200).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApprovalListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<ApprovalStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RiskAlertListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<AlertStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u16>,
    /// 1-based page number for pagination (default: 1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    /// Number of items per page (default: 20, max: 200).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RiskBucketListQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u16>,
    /// 1-based page number for pagination (default: 1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    /// Number of items per page (default: 20, max: 200).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RewardBotSnapshotQuery {
    /// Text search on quote plan question and reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plans_search: Option<String>,
    /// Filter plans by eligibility: true = eligible only, false = ineligible only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plans_eligible: Option<bool>,
    /// Sort plans by field: "score", "daily_reward", "midpoint".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plans_sort_by: Option<String>,
    /// Sort direction: "asc" or "desc" (default "desc").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plans_sort_order: Option<String>,
    /// Text search on order outcome and condition_id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orders_search: Option<String>,
    /// Filter orders by status: "open", "filled", "cancelled", "exit_pending".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orders_status: Option<String>,
    /// Sort orders by field: "price", "size", "status".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orders_sort_by: Option<String>,
    /// Sort direction: "asc" or "desc" (default "desc").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orders_sort_order: Option<String>,
    /// 1-based orders page number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orders_page: Option<u16>,
    /// Orders page size, clamped by the backend.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orders_page_size: Option<u16>,
    /// 1-based markets page number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markets_page: Option<u16>,
    /// Markets page size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markets_page_size: Option<u16>,
    /// 1-based fills page number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fills_page: Option<u16>,
    /// Fills page size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fills_page_size: Option<u16>,
    /// 1-based positions page number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub positions_page: Option<u16>,
    /// Positions page size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub positions_page_size: Option<u16>,
    /// 1-based events page number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events_page: Option<u16>,
    /// Events page size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events_page_size: Option<u16>,
    /// 1-based plans page number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plans_page: Option<u16>,
    /// Plans page size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plans_page_size: Option<u16>,
}
