// ── Wallet Analysis DTOs ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct WalletAnalysisRequest {
    pub address: String,
}

#[derive(Debug, Serialize)]
pub struct WalletAnalysisData {
    pub profile: WalletProfileData,
    pub pnl: WalletPnlData,
    pub activity: WalletActivityData,
    pub categories: Vec<WalletCategoryData>,
    pub style: WalletStyleData,
    pub risk: WalletRiskData,
    pub top_markets: Vec<WalletTopMarketData>,
    pub recent_trades: Vec<WalletRecentTradeData>,
    pub winners: Vec<WalletClosedPositionData>,
    pub losers: Vec<WalletClosedPositionData>,
    pub recent_closed: Vec<WalletClosedPositionData>,
}

#[derive(Debug, Serialize)]
pub struct WalletProfileData {
    pub address: String,
    pub name: String,
    pub pseudonym: String,
    pub bio: String,
    pub x_username: String,
    pub profile_image: String,
    pub created_at: String,
    pub verified_badge: bool,
    pub leaderboard_rank: i64,
    pub leaderboard_volume: String,
    pub leaderboard_pnl: String,
    pub portfolio_value: String,
    pub total_markets_traded: i64,
}

#[derive(Debug, Serialize)]
pub struct WalletPnlData {
    pub total_realized_pnl: String,
    pub total_unrealized_pnl: String,
    pub total_pnl: String,
    pub overall_roi: String,
    pub win_rate_closed: String,
    pub win_rate_open: String,
    pub largest_win: String,
    pub largest_loss: String,
    pub closed_positions_count: i32,
    pub open_positions_count: i32,
}

#[derive(Debug, Serialize)]
pub struct WalletActivityData {
    pub total_volume_usd: String,
    pub total_trades: i32,
    pub avg_trade_usd: String,
    pub median_trade_usd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_trade_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_trade_at: Option<String>,
    pub trading_days: i32,
    pub avg_trades_per_day: String,
    pub buy_ratio: String,
    pub total_buy_volume: String,
    pub total_sell_volume: String,
}

#[derive(Debug, Serialize)]
pub struct WalletCategoryData {
    pub category: String,
    pub trade_count: i32,
    pub volume_usd: String,
    pub pnl: String,
    pub win_count: i32,
    pub loss_count: i32,
}

#[derive(Debug, Serialize)]
pub struct WalletStyleData {
    pub style_label: String,
    pub avg_hold_hours: String,
    pub trade_size_stddev: String,
    pub directional_bias: String,
    pub preferred_price_range_low: String,
    pub preferred_price_range_high: String,
    pub price_concentration: String,
}

#[derive(Debug, Serialize)]
pub struct WalletRiskData {
    pub max_single_market_exposure_pct: String,
    pub max_drawdown_estimate: String,
    pub avg_position_size_pct: String,
    pub diversification_score: String,
    pub concentration_label: String,
}

#[derive(Debug, Serialize)]
pub struct WalletTopMarketData {
    pub condition_id: String,
    pub title: String,
    pub slug: String,
    pub trade_count: i32,
    pub volume_usd: String,
    pub pnl: String,
    pub buy_count: i32,
    pub sell_count: i32,
}

#[derive(Debug, Serialize)]
pub struct WalletRecentTradeData {
    pub side: String,
    pub title: String,
    pub slug: String,
    pub outcome: String,
    pub price: String,
    pub size: String,
    pub notional_usd: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct WalletClosedPositionData {
    pub title: String,
    pub slug: String,
    pub outcome: String,
    pub avg_price: String,
    pub realized_pnl: String,
    pub total_bought: String,
    pub end_date: String,
    pub timestamp: String,
}
