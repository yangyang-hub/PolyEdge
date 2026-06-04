import type {
  DecimalValue,
  ManagedRewardOrderStatus,
  PostFillStrategy,
  RewardFillRole,
  RewardOrderSide,
  RewardRiskSeverity,
} from "./primitives";

export type RewardBotConfigDto = {
  enabled: boolean;
  account_id: string;
  max_markets: number;
  max_open_orders: number;
  per_market_usd: DecimalValue;
  quote_size_usd: DecimalValue;
  min_daily_reward: DecimalValue;
  min_market_score: DecimalValue;
  max_spread_cents: DecimalValue;
  quote_edge_cents: DecimalValue;
  safety_margin_cents: DecimalValue;
  min_midpoint: DecimalValue;
  max_midpoint: DecimalValue;
  stale_book_ms: number;
  min_scoring_check_sec: number;
  max_position_usd: DecimalValue;
  max_global_position_usd: DecimalValue;
  exit_markup_cents: DecimalValue;
  cancel_on_fill: boolean;
  account_capital_usd: DecimalValue;
  reward_competition_factor: DecimalValue;
  single_sided_divisor_c: DecimalValue;
  fill_rate_per_tick: DecimalValue;
  max_fill_ratio: DecimalValue;
  requote_drift_cents: DecimalValue;
  post_fill_strategy: PostFillStrategy;
  // Risk control fields
  min_depth_usd: DecimalValue;
  cancel_bid_rank: number;
  depth_drop_pct: DecimalValue;
  depth_drop_window_sec: number;
  fill_velocity_usd: DecimalValue;
  fill_velocity_window_sec: number;
  mass_cancel_pct: DecimalValue;
  mass_cancel_window_sec: number;
  requote_interval_sec: number;
  requote_jitter_sec: number;
  reconcile_interval_sec: number;
};

export type RewardBotConfigPatchDto = Partial<RewardBotConfigDto>;

export type RewardTokenDto = {
  token_id: string;
  outcome: string;
  price?: DecimalValue | null;
};

export type RewardMarketDto = {
  condition_id: string;
  question: string;
  market_slug: string;
  event_slug: string;
  image: string;
  rewards_max_spread: DecimalValue;
  rewards_min_size: DecimalValue;
  total_daily_rate: DecimalValue;
  tokens: RewardTokenDto[];
  active: boolean;
  updated_at: string;
};

export type RewardQuoteLegDto = {
  token_id: string;
  outcome: string;
  side: RewardOrderSide;
  price: DecimalValue;
  size: DecimalValue;
  notional_usd: DecimalValue;
};

export type RewardQuotePlanDto = {
  condition_id: string;
  market_slug: string;
  question: string;
  score: DecimalValue;
  eligible: boolean;
  reason: string;
  midpoint?: DecimalValue | null;
  total_daily_rate: DecimalValue;
  rewards_max_spread: DecimalValue;
  rewards_min_size: DecimalValue;
  legs: RewardQuoteLegDto[];
  updated_at: string;
};

export type ManagedRewardOrderDto = {
  id: string;
  account_id: string;
  condition_id: string;
  token_id: string;
  outcome: string;
  side: RewardOrderSide;
  price: DecimalValue;
  size: DecimalValue;
  external_order_id?: string | null;
  status: ManagedRewardOrderStatus;
  scoring: boolean;
  reason: string;
  filled_size?: DecimalValue;
  reward_earned?: DecimalValue;
  last_scored_at?: string | null;
  created_at: string;
  updated_at: string;
};

export type RewardAccountStateDto = {
  account_id: string;
  capital_usd: DecimalValue;
  available_usd: DecimalValue;
  reserved_usd: DecimalValue;
  realized_pnl: DecimalValue;
  reward_earned_usd: DecimalValue;
  fees_paid: DecimalValue;
  tick_index: number;
  updated_at: string;
};

export type RewardFillDto = {
  id: string;
  order_id: string;
  account_id: string;
  condition_id: string;
  token_id: string;
  outcome: string;
  side: RewardOrderSide;
  price: DecimalValue;
  size: DecimalValue;
  notional_usd: DecimalValue;
  role: RewardFillRole;
  realized_pnl: DecimalValue;
  reason: string;
  trace_id: string;
  created_at: string;
};

export type RewardPositionDto = {
  account_id: string;
  condition_id: string;
  token_id: string;
  outcome: string;
  size: DecimalValue;
  avg_price: DecimalValue;
  realized_pnl: DecimalValue;
  updated_at: string;
};

export type RewardRiskEventDto = {
  id: string;
  account_id?: string | null;
  condition_id?: string | null;
  external_order_id?: string | null;
  event_type: string;
  severity: RewardRiskSeverity;
  message: string;
  metadata: unknown;
  created_at: string;
};

export type RewardBotStatusDto = {
  enabled: boolean;
  running: boolean;
  account_id: string;
  markets_tracked: number;
  eligible_markets: number;
  plans_total: number;
  open_orders: number;
  positions: number;
  last_scan_at?: string | null;
  last_run_at?: string | null;
  error?: string | null;
};

export type RewardListPageDto = {
  page: number;
  page_size: number;
  total_items: number;
  total_pages: number;
};

export type RewardBotSnapshotDto = {
  config: RewardBotConfigDto;
  status: RewardBotStatusDto;
  account: RewardAccountStateDto;
  markets: RewardMarketDto[];
  quote_plans: RewardQuotePlanDto[];
  plans_page: RewardListPageDto;
  orders: ManagedRewardOrderDto[];
  orders_page: RewardListPageDto;
  positions: RewardPositionDto[];
  fills: RewardFillDto[];
  events: RewardRiskEventDto[];
};
