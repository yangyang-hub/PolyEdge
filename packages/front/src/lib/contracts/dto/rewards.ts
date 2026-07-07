import type {
  DecimalValue,
  ManagedRewardOrderStatus,
  PostFillStrategy,
  RewardAiProvider,
  RewardAiRequestFormat,
  RewardAiSuitability,
  RewardEventTimeConfidence,
  RewardEventWindowStatus,
  RewardFillRole,
  RewardGammaEventDateMode,
  RewardInfoDirectionalRisk,
  RewardInfoRiskLevel,
  RewardInfoRiskType,
  RewardOrderSide,
  RewardPlanQuoteMode,
  RewardQuoteReadiness,
  RewardQuoteMode,
  RewardRiskSeverity,
  RewardSelectionMode,
  RewardStrategyBucket,
  RewardStrategyProfile,
  RewardUnknownEventTimeMode,
} from "./primitives";

export type RewardBotConfigDto = {
  enabled: boolean;
  account_id: string;
  max_markets: number;
  max_open_orders: number;
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
  opportunity_metrics_enabled: boolean;
  opportunity_probe_notional_usd: DecimalValue;
  opportunity_min_reward_per_100_usd_day: DecimalValue;
  opportunity_max_competition_multiple: DecimalValue;
  opportunity_competition_hard_gate_enabled: boolean;
  opportunity_competition_hard_gate_multiple: DecimalValue;
  opportunity_max_account_allocation_bps: number;
  opportunity_max_market_allocation_bps: number;
  opportunity_min_exit_depth_usd: DecimalValue;
  opportunity_min_exit_depth_multiple: DecimalValue;
  opportunity_max_entry_exit_slippage_cents: DecimalValue;
  opportunity_max_bad_fill_recovery_days: DecimalValue;
  opportunity_observation_window_sec: number;
  opportunity_min_book_samples: number;
  opportunity_max_midpoint_range_cents: DecimalValue;
  opportunity_max_top_of_book_flip_count: number;
  opportunity_reward_weight: DecimalValue;
  opportunity_competition_weight: DecimalValue;
  opportunity_exit_weight: DecimalValue;
  opportunity_stability_weight: DecimalValue;
  ai_advisory_enabled: boolean;
  ai_provider: RewardAiProvider;
  ai_request_format: RewardAiRequestFormat;
  ai_advisory_ttl_sec: number;
  ai_provider_concurrency_enabled: boolean;
  ai_provider_primary_max_concurrency: number;
  ai_provider_fallback_max_concurrency: number;
  ai_strategy_hint_enabled: boolean;
  ai_strategy_hint_min_confidence: DecimalValue;
  info_risk_enabled: boolean;
  info_risk_mode: RewardSelectionMode;
  info_risk_avoid_level: RewardInfoRiskLevel;
  info_risk_ttl_sec: number;
  ai_advisory_provider_pending_grace_sec: number;
  info_risk_provider_pending_grace_sec: number;
  event_window_enabled: boolean;
  event_window_min_confidence: RewardEventTimeConfidence;
  event_window_stop_new_quote_before_start_sec: number;
  event_window_cancel_open_buy_before_start_sec: number;
  event_window_resume_after_event_end_sec: number;
  event_window_unknown_event_time_mode: RewardUnknownEventTimeMode;
  event_window_gamma_unreviewed_dates_mode: RewardGammaEventDateMode;
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
  balanced_merge_enabled: boolean;
  balanced_merge_max_markets: number;
  balanced_merge_max_open_orders: number;
  balanced_merge_min_edge_cents: DecimalValue;
  balanced_merge_min_market_score: DecimalValue;
  balanced_merge_min_market_liquidity_usd: DecimalValue;
  balanced_merge_min_market_volume_24h_usd: DecimalValue;
  balanced_merge_max_market_spread_cents: DecimalValue;
  balanced_merge_quote_bid_rank: number;
  balanced_merge_max_unpaired_position_usd: DecimalValue;
  balanced_merge_auto_execute_enabled: boolean;
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

export type RewardOpportunityMetricsDto = {
  planned_notional_usd: DecimalValue;
  probe_notional_usd: DecimalValue;
  qualified_competition_usd: DecimalValue;
  competition_share_bps: DecimalValue;
  competition_multiple: DecimalValue;
  estimated_reward_per_100_usd_day: DecimalValue;
  competition_density: DecimalValue;
  account_effective_available_usd: DecimalValue;
  open_buy_notional_usd: DecimalValue;
  open_buy_notional_usd_after_plan: DecimalValue;
  condition_buy_notional_usd_after_plan: DecimalValue;
  account_allocation_bps: DecimalValue;
  market_allocation_bps: DecimalValue;
  exit_depth_usd: DecimalValue;
  exit_slippage_cents?: DecimalValue | null;
  bad_fill_recovery_days?: DecimalValue | null;
  midpoint_range_cents?: DecimalValue | null;
  top_of_book_flip_count?: number | null;
  sample_count: number;
  reward_score: DecimalValue;
  competition_score: DecimalValue;
  exit_score: DecimalValue;
  stability_score: DecimalValue;
  opportunity_score: DecimalValue;
  score_adjustment: DecimalValue;
  warnings: string[];
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

export type RewardEventWindowAssessmentDto = {
  status: RewardEventWindowStatus;
  reason: string;
  event_start_at?: string | null;
  event_end_at?: string | null;
  source?: string | null;
  confidence?: RewardEventTimeConfidence | null;
  event_type?: string | null;
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
  strategy_profile?: RewardStrategyProfile;
  quote_mode: RewardPlanQuoteMode;
  recommended_quote_mode?: RewardPlanQuoteMode | null;
  book_metrics?: RewardMarketBookMetricsDto | null;
  opportunity_metrics?: RewardOpportunityMetricsDto | null;
  ai_advisory?: RewardMarketAdvisoryDto | null;
  info_risk?: RewardMarketInfoRiskDto | null;
  event_window?: RewardEventWindowAssessmentDto | null;
  midpoint?: DecimalValue | null;
  live_skip_until?: string | null;
  live_skip_reason?: string | null;
  first_quote_observed_at?: string | null;
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
  strategy_profile?: RewardStrategyProfile;
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
