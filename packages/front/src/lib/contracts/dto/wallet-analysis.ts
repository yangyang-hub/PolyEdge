import type { DecimalValue } from "./primitives";

export type WalletProfileDto = {
  address: string;
  name: string;
  pseudonym: string;
  bio: string;
  x_username: string;
  profile_image: string;
  created_at: string;
  verified_badge: boolean;
  leaderboard_rank: number;
  leaderboard_volume: DecimalValue;
  leaderboard_pnl: DecimalValue;
  portfolio_value: DecimalValue;
  total_markets_traded: number;
};

export type WalletPnlDto = {
  total_realized_pnl: DecimalValue;
  total_unrealized_pnl: DecimalValue;
  total_pnl: DecimalValue;
  overall_roi: DecimalValue;
  win_rate_closed: DecimalValue;
  win_rate_open: DecimalValue;
  largest_win: DecimalValue;
  largest_loss: DecimalValue;
  closed_positions_count: number;
  open_positions_count: number;
};

export type WalletActivityDto = {
  total_volume_usd: DecimalValue;
  total_trades: number;
  avg_trade_usd: DecimalValue;
  median_trade_usd: DecimalValue;
  first_trade_at?: string | null;
  last_trade_at?: string | null;
  trading_days: number;
  avg_trades_per_day: DecimalValue;
  buy_ratio: DecimalValue;
  total_buy_volume: DecimalValue;
  total_sell_volume: DecimalValue;
};

export type WalletCategoryDto = {
  category: string;
  trade_count: number;
  volume_usd: DecimalValue;
  pnl: DecimalValue;
  win_count: number;
  loss_count: number;
};

export type WalletStyleDto = {
  style_label: string;
  avg_hold_hours: DecimalValue;
  trade_size_stddev: DecimalValue;
  directional_bias: DecimalValue;
  preferred_price_range_low: DecimalValue;
  preferred_price_range_high: DecimalValue;
  price_concentration: string;
};

export type WalletRiskDto = {
  max_single_market_exposure_pct: DecimalValue;
  max_drawdown_estimate: DecimalValue;
  avg_position_size_pct: DecimalValue;
  diversification_score: DecimalValue;
  concentration_label: string;
};

export type WalletTopMarketDto = {
  condition_id: string;
  title: string;
  slug: string;
  trade_count: number;
  volume_usd: DecimalValue;
  pnl: DecimalValue;
  buy_count: number;
  sell_count: number;
};

export type WalletRecentTradeDto = {
  side: string;
  title: string;
  slug: string;
  outcome: string;
  price: DecimalValue;
  size: DecimalValue;
  notional_usd: DecimalValue;
  timestamp: string;
};

export type WalletClosedPositionDto = {
  title: string;
  slug: string;
  outcome: string;
  avg_price: DecimalValue;
  realized_pnl: DecimalValue;
  total_bought: DecimalValue;
  end_date: string;
  timestamp: string;
};

export type WalletAnalysisReportDto = {
  profile: WalletProfileDto;
  pnl: WalletPnlDto;
  activity: WalletActivityDto;
  categories: WalletCategoryDto[];
  style: WalletStyleDto;
  risk: WalletRiskDto;
  top_markets: WalletTopMarketDto[];
  recent_trades: WalletRecentTradeDto[];
  winners: WalletClosedPositionDto[];
  losers: WalletClosedPositionDto[];
  recent_closed: WalletClosedPositionDto[];
};
