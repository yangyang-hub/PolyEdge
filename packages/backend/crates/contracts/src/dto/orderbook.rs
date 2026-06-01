/// Orderbook level price/size pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderbookLevelData {
    pub price: String,
    pub size: String,
}

/// Orderbook snapshot for a single token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderbookData {
    pub token_id: String,
    pub bids: Vec<OrderbookLevelData>,
    pub asks: Vec<OrderbookLevelData>,
    /// Epoch milliseconds when the snapshot was observed.
    pub observed_at: i64,
    /// Data source: "ws" or "poll".
    pub source: String,
}
