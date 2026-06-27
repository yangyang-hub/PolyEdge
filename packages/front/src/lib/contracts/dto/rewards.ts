import type {
  DecimalValue,
  ManagedRewardOrderStatus,
  PostFillStrategy,
  RewardAiProvider,
  RewardAiRequestFormat,
  RewardAiSuitability,
  RewardFillRole,
  RewardInfoDirectionalRisk,
  RewardInfoRiskLevel,
  RewardInfoRiskType,
  RewardLowCompetitionMode,
  RewardOrderSide,
  RewardPlanQuoteMode,
  RewardQuoteReadiness,
  RewardQuoteMode,
  RewardRiskSeverity,
  RewardSelectionMode,
  RewardStrategyBucket,
} from "./primitives";

export type RewardBotConfigDto = {
  enabled: boolean;
  account_id: string;
  max_markets: number;
  max_open_orders: number;
  per_market_usd: DecimalValue;
  quote_size_usd: DecimalValue;
  min_daily_reward: DecimalValue;
  min_market_liquidity_usd: DecimalValue;
  min_market_volume_24h_usd: DecimalValue;
  min_hours_to_end: number;
  max_market_spread_cents: DecimalValue;
  max_market_data_age_minutes: number;
  min_market_score: DecimalValue;
  max_spread_cents: DecimalValue;
  quote_mode: RewardQuoteMode;
  selection_mode: RewardSelectionMode;
  quote_bid_rank: number;
  dominant_single_side_enabled: boolean;
  dominant_min_probability: DecimalValue;
  dominant_max_probability: DecimalValue;
  dominant_min_exit_depth_usd: DecimalValue;
  max_top1_depth_share: DecimalValue;
  max_top3_depth_share: DecimalValue;
  max_book_hhi: DecimalValue;
  preferred_categories: string[];
  preferred_category_score_bonus: DecimalValue;
  low_competition_mode: RewardLowCompetitionMode;
  low_competition_max_markets: number;
  low_competition_max_open_orders: number;
  low_competition_per_market_usd: DecimalValue;
  low_competition_max_position_usd: DecimalValue;
  low_competition_probe_notional_usd: DecimalValue;
  low_competition_min_competition_share_bps: number;
  low_competition_max_competition_multiple: DecimalValue;
  low_competition_candidate_max_competition_multiple: DecimalValue;
  low_competition_max_account_allocation_bps: number;
  low_competition_max_market_allocation_bps: number;
  low_competition_candidate_liquidity_filter_enabled: boolean;
  low_competition_candidate_volume_filter_enabled: boolean;
  low_competition_min_market_liquidity_usd: DecimalValue;
  low_competition_min_market_volume_24h_usd: DecimalValue;
  low_competition_max_competition_usd: DecimalValue;
  low_competition_min_reward_per_100_usd_day: DecimalValue;
  low_competition_min_exit_depth_usd: DecimalValue;
  low_competition_min_exit_depth_multiple: DecimalValue;
  low_competition_max_entry_exit_slippage_cents: DecimalValue;
  low_competition_max_bad_fill_recovery_days: DecimalValue;
  low_competition_max_midpoint_range_cents: DecimalValue;
  low_competition_max_top_of_book_flip_count: number;
  low_competition_observation_window_sec: number;
  low_competition_min_book_samples: number;
  low_competition_quote_bid_rank: number;
  low_competition_safety_margin_cents: DecimalValue;
  low_competition_max_spread_cents: DecimalValue;
  low_competition_max_market_spread_cents: DecimalValue;
  low_competition_min_market_score: DecimalValue;
  low_competition_require_ai_allow: boolean;
  low_competition_info_risk_avoid_level: RewardInfoRiskLevel;
  low_competition_cancel_confirm_sec: number;
  low_competition_cancel_share_threshold_ratio_bps: number;
  low_competition_cancel_competition_multiple_factor: DecimalValue;
  low_competition_cancel_max_exit_slippage_cents: DecimalValue;
  low_competition_cancel_min_exit_depth_usd: DecimalValue;
  low_competition_cancel_exit_depth_multiple: DecimalValue;
  low_competition_cancel_midpoint_range_floor_cents: DecimalValue;
  low_competition_global_open_order_share_bps: number;
  ai_advisory_enabled: boolean;
  ai_provider: RewardAiProvider;
  ai_request_format: RewardAiRequestFormat;
  ai_advisory_ttl_sec: number;
  ai_advisory_batch_size: number;
  info_risk_enabled: boolean;
  info_risk_mode: RewardSelectionMode;
  info_risk_avoid_level: RewardInfoRiskLevel;
  info_risk_ttl_sec: number;
  info_risk_batch_size: number;
  require_info_risk_before_first_quote: boolean;
  first_quote_quarantine_sec: number;
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
  requote_drift_cents: DecimalValue;
  requote_drift_confirm_sec: number;
  requote_drift_cooldown_sec: number;
  requote_drift_max_cancels_per_cycle: number;
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
  category: string;
  image: string;
  rewards_max_spread: DecimalValue;
  rewards_min_size: DecimalValue;
  total_daily_rate: DecimalValue;
  liquidity_usd: DecimalValue;
  volume_24h_usd: DecimalValue;
  market_spread_cents: DecimalValue;
  end_at?: string | null;
  ambiguity_level: string;
  market_synced_at?: string | null;
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

export type RewardBookSideMetricsDto = {
  top1_depth_share: DecimalValue;
  top3_depth_share: DecimalValue;
  book_hhi: DecimalValue;
  exit_depth_usd: DecimalValue;
};

export type RewardMarketBookMetricsDto = {
  yes_probability: DecimalValue;
  recommended_quote_mode: RewardPlanQuoteMode;
  reason?: string | null;
  yes?: RewardBookSideMetricsDto | null;
  no?: RewardBookSideMetricsDto | null;
};

export type RewardLowCompetitionMetricsDto = {
  planned_notional_usd: DecimalValue;
  competition_probe_notional_usd: DecimalValue;
  qualified_competition_usd: DecimalValue;
  competition_share_bps: DecimalValue;
  competition_multiple: DecimalValue;
  estimated_reward_per_100_usd_day: DecimalValue;
  competition_density: DecimalValue;
  account_effective_available_usd: DecimalValue;
  low_competition_open_buy_notional_usd: DecimalValue;
  low_competition_open_buy_notional_usd_after_plan: DecimalValue;
  condition_buy_notional_usd_after_plan: DecimalValue;
  account_allocation_bps: DecimalValue;
  market_allocation_bps: DecimalValue;
  exit_depth_usd: DecimalValue;
  exit_slippage_cents?: DecimalValue | null;
  bad_fill_recovery_days?: DecimalValue | null;
  midpoint_range_cents?: DecimalValue | null;
  top_of_book_flip_count?: number | null;
  sample_count: number;
  eligible_for_low_competition: boolean;
  rejection_reasons: string[];
  not_low_competition?: boolean;
  not_low_competition_reason?: string | null;
};

export type RewardLowCompetitionShadowReportDto = {
  window_hours: number;
  generated_at: string;
  latest_observed_at?: string | null;
  observations: number;
  unique_markets: number;
  gate_pass_count: number;
  final_pass_count: number;
  sample_insufficient_count: number;
  ai_blocked_count: number;
  info_risk_blocked_count: number;
  standard_overlap_count: number;
  not_low_competition_count: number;
  gate_pass_ratio: DecimalValue;
  final_pass_ratio: DecimalValue;
  sample_insufficient_ratio: DecimalValue;
  ai_blocked_ratio: DecimalValue;
  info_risk_blocked_ratio: DecimalValue;
  standard_overlap_ratio: DecimalValue;
  not_low_competition_ratio: DecimalValue;
  competition_share_bps_median?: DecimalValue | null;
  account_allocation_bps_p90?: DecimalValue | null;
  market_allocation_bps_p90?: DecimalValue | null;
  estimated_reward_per_100_usd_day_median?: DecimalValue | null;
  estimated_reward_per_100_usd_day_p90?: DecimalValue | null;
  exit_depth_multiple_median?: DecimalValue | null;
  midpoint_range_cents_p95?: DecimalValue | null;
  exit_slippage_cents_p95?: DecimalValue | null;
  bad_fill_recovery_days_p95?: DecimalValue | null;
  should_consider_enforce: boolean;
  recommendation_reasons: string[];
};

export type RewardMarketAdvisoryDto = {
  condition_id: string;
  provider: RewardAiProvider;
  request_format: RewardAiRequestFormat;
  model: string;
  input_hash: string;
  suitability: RewardAiSuitability;
  quote_mode: RewardPlanQuoteMode;
  exit_policy: PostFillStrategy;
  confidence: DecimalValue;
  reasons: string[];
  metrics: unknown;
  created_at: string;
  expires_at: string;
};

export type RewardInfoRiskSourceDto = {
  url: string;
  title: string;
  published_at?: string | null;
  snippet?: string | null;
};

export type RewardMarketInfoRiskDto = {
  condition_id: string;
  provider: RewardAiProvider;
  request_format: RewardAiRequestFormat;
  model: string;
  query_hash: string;
  input_hash: string;
  risk_level: RewardInfoRiskLevel;
  risk_type: RewardInfoRiskType;
  directional_risk: RewardInfoDirectionalRisk;
  resolution_imminent: boolean;
  expected_event_at?: string | null;
  confidence: DecimalValue;
  summary: string;
  sources: RewardInfoRiskSourceDto[];
  metrics: unknown;
  created_at: string;
  expires_at: string;
};

export type RewardQuotePlanDto = {
  condition_id: string;
  market_slug: string;
  question: string;
  score: DecimalValue;
  eligible: boolean;
  pre_ai_eligible: boolean;
  quote_readiness?: RewardQuoteReadiness;
  reason: string;
  strategy_bucket: RewardStrategyBucket;
  quote_mode: RewardPlanQuoteMode;
  recommended_quote_mode?: RewardPlanQuoteMode | null;
  book_metrics?: RewardMarketBookMetricsDto | null;
  low_competition_metrics?: RewardLowCompetitionMetricsDto | null;
  ai_advisory?: RewardMarketAdvisoryDto | null;
  info_risk?: RewardMarketInfoRiskDto | null;
  midpoint?: DecimalValue | null;
  live_skip_until?: string | null;
  live_skip_reason?: string | null;
  total_daily_rate: DecimalValue;
  rewards_max_spread: DecimalValue;
  rewards_min_size: DecimalValue;
  orderbook_token_ids?: string[];
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
  strategy_bucket: RewardStrategyBucket;
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
  wallet_address?: string | null;
  capital_usd: DecimalValue;
  available_usd: DecimalValue;
  external_buy_notional: DecimalValue;
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

// Best-effort live quote keyed by token_id, populated by the API snapshot.
// Absent (or the whole map null) when the orderbook service is unavailable.
export type RewardTokenQuoteDto = {
  best_bid?: DecimalValue | null;
  best_ask?: DecimalValue | null;
  mark_price?: DecimalValue | null;
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

export type RewardQuotePlanBlockerCountsDto = {
  waiting_orderbook?: number;
  ai_pending?: number;
  info_risk_pending?: number;
  ai_confidence_low?: number;
  ai_watch?: number;
  ai_avoid?: number;
  info_risk?: number;
  low_competition?: number;
  funding?: number;
  live_validation?: number;
  other?: number;
};

export type RewardBotStatusDto = {
  enabled: boolean;
  running: boolean;
  account_id: string;
  markets_tracked: number;
  eligible_markets: number;
  ready_quote_markets?: number;
  waiting_orderbook_markets?: number;
  provider_pending_markets?: number;
  blocker_counts?: RewardQuotePlanBlockerCountsDto;
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

export type RewardLlmCallDailyStatsDto = {
  day: string;
  ai_advisory_calls: number;
  info_risk_calls: number;
  total_calls: number;
  failed_calls: number;
};

export type RewardBotSnapshotDto = {
  config: RewardBotConfigDto;
  status: RewardBotStatusDto;
  account: RewardAccountStateDto;
  low_competition_report?: RewardLowCompetitionShadowReportDto | null;
  llm_usage?: RewardLlmCallDailyStatsDto[];
  markets: RewardMarketDto[];
  quote_plans: RewardQuotePlanDto[];
  plans_page: RewardListPageDto;
  orders: ManagedRewardOrderDto[];
  orders_page: RewardListPageDto;
  positions: RewardPositionDto[];
  fills: RewardFillDto[];
  events: RewardRiskEventDto[];
  token_quotes?: Record<string, RewardTokenQuoteDto> | null;
};
