use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

// ── Input structs (bridging connector data to analysis) ──────────────────────

/// Raw input for a single closed position (from Polymarket Data API).
#[derive(Debug, Clone)]
pub struct ClosedPositionInput {
    pub condition_id: String,
    pub avg_price: Decimal,
    pub total_bought: Decimal,
    pub realized_pnl: Decimal,
    pub cur_price: Decimal,
    pub timestamp: OffsetDateTime,
    pub title: String,
    pub slug: String,
    pub outcome: String,
    pub end_date: String,
}

/// Raw input for a single trade (from Polymarket Data API `/trades`).
#[derive(Debug, Clone)]
pub struct TradeInput {
    pub side: String,
    pub asset: String,
    pub condition_id: String,
    pub size: Decimal,
    pub price: Decimal,
    pub timestamp: OffsetDateTime,
    pub title: String,
    pub slug: String,
    pub outcome: String,
    pub transaction_hash: String,
}

/// Raw input for a current open position (from Polymarket Data API `/positions`).
#[derive(Debug, Clone)]
pub struct OpenPositionInput {
    pub condition_id: String,
    pub outcome: String,
    pub title: String,
    pub slug: String,
    pub size: Decimal,
    pub avg_price: Decimal,
    pub cur_price: Decimal,
    pub realized_pnl: Decimal,
    pub percent_pnl: Decimal,
}

/// Raw input for an activity record (from Polymarket Data API `/activity`).
#[derive(Debug, Clone)]
pub struct ActivityInput {
    pub kind: String,
    pub side: String,
    pub asset: String,
    pub condition_id: String,
    pub outcome: String,
    pub title: String,
    pub price: Decimal,
    pub size: Decimal,
    pub usdc_size: Decimal,
    pub timestamp: OffsetDateTime,
}

// ── Output: Full analysis report ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WalletAnalysisReport {
    pub profile: WalletProfile,
    pub pnl: WalletPnlStats,
    pub activity: WalletActivityStats,
    pub categories: Vec<WalletCategoryItem>,
    pub style: WalletStyleStats,
    pub risk: WalletRiskProfile,
    pub top_markets: Vec<WalletTopMarket>,
    pub recent_trades: Vec<WalletRecentTrade>,
    pub winners: Vec<WalletClosedPositionItem>,
    pub losers: Vec<WalletClosedPositionItem>,
    pub recent_closed: Vec<WalletClosedPositionItem>,
}

// ── Profile ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WalletProfile {
    pub address: String,
    pub name: String,
    pub pseudonym: String,
    pub bio: String,
    pub x_username: String,
    pub profile_image: String,
    pub created_at: String,
    pub verified_badge: bool,
    pub leaderboard_rank: i64,
    pub leaderboard_volume: Decimal,
    pub leaderboard_pnl: Decimal,
    pub portfolio_value: Decimal,
    pub total_markets_traded: i64,
}

// ── P&L Stats ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WalletPnlStats {
    pub total_realized_pnl: Decimal,
    pub total_unrealized_pnl: Decimal,
    pub total_pnl: Decimal,
    pub overall_roi: Decimal,
    pub win_rate_closed: Decimal,
    pub win_rate_open: Decimal,
    pub largest_win: Decimal,
    pub largest_loss: Decimal,
    pub closed_positions_count: i32,
    pub open_positions_count: i32,
}

// ── Activity Stats ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WalletActivityStats {
    pub total_volume_usd: Decimal,
    pub total_trades: i32,
    pub avg_trade_usd: Decimal,
    pub median_trade_usd: Decimal,
    pub first_trade_at: Option<OffsetDateTime>,
    pub last_trade_at: Option<OffsetDateTime>,
    pub trading_days: i32,
    pub avg_trades_per_day: Decimal,
    pub buy_ratio: Decimal,
    pub total_buy_volume: Decimal,
    pub total_sell_volume: Decimal,
}

// ── Category Breakdown ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WalletCategoryItem {
    pub category: String,
    pub trade_count: i32,
    pub volume_usd: Decimal,
    pub pnl: Decimal,
    pub win_count: i32,
    pub loss_count: i32,
}

// ── Trading Style ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WalletStyleStats {
    pub style_label: String,
    pub avg_hold_hours: Decimal,
    pub trade_size_stddev: Decimal,
    pub directional_bias: Decimal,
    pub preferred_price_range_low: Decimal,
    pub preferred_price_range_high: Decimal,
    pub price_concentration: String,
}

// ── Risk Profile ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WalletRiskProfile {
    pub max_single_market_exposure_pct: Decimal,
    pub max_drawdown_estimate: Decimal,
    pub avg_position_size_pct: Decimal,
    pub diversification_score: Decimal,
    pub concentration_label: String,
}

// ── Top Markets ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WalletTopMarket {
    pub condition_id: String,
    pub title: String,
    pub slug: String,
    pub trade_count: i32,
    pub volume_usd: Decimal,
    pub pnl: Decimal,
    pub buy_count: i32,
    pub sell_count: i32,
}

// ── Recent Trade ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WalletRecentTrade {
    pub side: String,
    pub title: String,
    pub slug: String,
    pub outcome: String,
    pub price: Decimal,
    pub size: Decimal,
    pub notional_usd: Decimal,
    pub timestamp: OffsetDateTime,
}

// ── Closed Position Item ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WalletClosedPositionItem {
    pub title: String,
    pub slug: String,
    pub outcome: String,
    pub avg_price: Decimal,
    pub realized_pnl: Decimal,
    pub total_bought: Decimal,
    pub end_date: String,
    pub timestamp: OffsetDateTime,
}
