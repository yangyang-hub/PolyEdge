export type ResourceVersion = {
  id: string;
  version: number;
};

export type MarketStatus = "open" | "closed" | "resolved";
export type AmbiguityLevel = "low" | "medium" | "high";
export type TradabilityStatus = "tradable" | "manual_review" | "observe_only" | "blocked";
export type EventStatus = "active" | "expired" | "invalidated" | "superseded";
export type EvidenceDirection = "supports_yes" | "supports_no" | "background";
export type EvidenceStatus = "active" | "expired" | "invalidated";
export type SignalAction = "buy" | "sell";
export type SignalSide = "yes" | "no";
export type SignalLifecycleState =
  | "new"
  | "active"
  | "weakened"
  | "executed"
  | "invalidated"
  | "reversed"
  | "expired";
export type RuntimeMode =
  | "research"
  | "paper_trade"
  | "manual_confirm"
  | "live_auto"
  | "kill_switch_locked";
export type RuntimeEnvironment = "local" | "paper" | "staging" | "production";
export type AlertSeverity = "warning" | "critical";
export type AlertStatus = "unresolved" | "watching" | "contained";
export type PositionSide = "yes" | "no";
export type BucketStatus = "healthy" | "watch" | "breach";
export type NewsSourceType = "news" | "social" | "official" | "calendar" | "market";
export type ReplayMomentKind = "event_ingested" | "evidence_generated" | "posterior_updated" | "signal_transition";
export type ArbitrageOpportunityType = "binary_buy_both" | "binary_sell_both";
export type ArbitrageOpportunityStatus = "observed" | "expired" | "repeated";
export type ArbitrageValidationStatus =
  | "unvalidated"
  | "valid"
  | "stale_book"
  | "insufficient_depth"
  | "price_moved"
  | "fees_exceed_edge"
  | "below_threshold"
  | "invalid_market"
  | "error";
export type RewardBotMode = "dry_run" | "live";
export type RewardOrderSide = "buy" | "sell";
export type ManagedRewardOrderStatus =
  | "planned"
  | "open"
  | "cancelled"
  | "filled"
  | "exit_pending"
  | "error";
export type RewardRiskSeverity = "info" | "warning" | "critical";
export type DecimalValue = string | number;

export type MarketDto = ResourceVersion & {
  question: string;
  category: string;
  status: MarketStatus;
  best_bid: string;
  best_ask: string;
  mid_price: string;
  volume_24h: string;
  ambiguity_level: AmbiguityLevel;
  tradability_status: TradabilityStatus;
  resolution_source: string;
  edge_case_notes: string[];
  updated_at: string;
};

export type EventDto = ResourceVersion & {
  source: string;
  summary: string;
  relevance_score: string;
  confidence: string;
  status: EventStatus;
  related_market_ids: string[];
  reason_trace: string;
  created_at: string;
  updated_at: string;
};

export type NewsSourceHealthDto = {
  source: string;
  source_type: NewsSourceType;
  enabled: boolean;
  reliability: string;
  last_success_at?: string | null;
  last_error_at?: string | null;
  consecutive_failures: number;
  items_fetched: number;
  items_inserted: number;
  items_deduped: number;
  health_score: string;
  last_error?: string | null;
  updated_at: string;
};

export type NewsRawEventDto = {
  id: string;
  source: string;
  source_type: NewsSourceType;
  external_id?: string | null;
  title: string;
  url?: string | null;
  author?: string | null;
  published_at?: string | null;
  event_time: string;
  hash: string;
  raw_payload: unknown;
  ingested_at: string;
  trace_id: string;
};

export type EvidenceDto = ResourceVersion & {
  market_id: string;
  event_id: string;
  direction: EvidenceDirection;
  strength: string;
  source_reliability: string;
  novelty: string;
  resolution_relevance: string;
  status: EvidenceStatus;
  expires_at: string;
  created_at: string;
  updated_at: string;
};

export type SignalDto = ResourceVersion & {
  market_id: string;
  event_id: string;
  action: SignalAction;
  side: SignalSide;
  market_price: string;
  fair_price: string;
  edge: string;
  confidence: string;
  lifecycle_state: SignalLifecycleState;
  reason: string;
  risk_decision: string;
  evidence_ids: string[];
  approved_by_user_id?: string | null;
  approved_at?: string | null;
  rejected_by_user_id?: string | null;
  rejected_at?: string | null;
  updated_at: string;
};

export type RiskStateDto = ResourceVersion & {
  mode: RuntimeMode;
  environment: RuntimeEnvironment;
  kill_switch: boolean;
  daily_pnl: string;
  gross_exposure: string;
  net_exposure: string;
  open_alerts: number;
  daily_loss_limit: string;
  daily_loss_used: string;
  updated_at: string;
};

export type RiskAlertDto = ResourceVersion & {
  severity: AlertSeverity;
  reason: string;
  target: string;
  status: AlertStatus;
  created_at: string;
  updated_at: string;
};

export type PositionDto = ResourceVersion & {
  market_id: string;
  market_question: string;
  side: PositionSide;
  quantity: string;
  average_cost: string;
  mark_price: string;
  realized_pnl: string;
  unrealized_pnl: string;
  bucket_name: string;
  updated_at: string;
};

export type RiskBucketDto = ResourceVersion & {
  name: string;
  exposure: string;
  limit: string;
  utilization: string;
  status: BucketStatus;
  updated_at: string;
};

export type ProbabilityEstimateDto = {
  id: string;
  market_id: string;
  event_id: string;
  signal_id?: string | null;
  prior_price: string;
  posterior_price: string;
  fair_price: string;
  market_price: string;
  edge: string;
  confidence: string;
  time_horizon: string;
  model_version: string;
  reason_codes: string[];
  evidence_count: number;
  created_at: string;
};

export type ArbitrageScanDto = {
  id: string;
  started_at: string;
  finished_at?: string | null;
  market_count: number;
  snapshot_count: number;
  opportunity_count: number;
  scanner_version: string;
  metadata: unknown;
  trace_id: string;
};

export type ArbitrageOpportunityDto = {
  id: string;
  scan_id: string;
  market_id: string;
  opportunity_type: ArbitrageOpportunityType;
  status: ArbitrageOpportunityStatus;
  gross_edge: string;
  price_sum: string;
  capacity: string;
  yes_price: string;
  no_price: string;
  yes_size: string;
  no_size: string;
  observed_at: string;
  reason_codes: string[];
  analysis_payload: unknown;
  trace_id: string;
  validation?: ArbitrageOpportunityValidationDto | null;
};

export type ArbitrageOpportunityValidationDto = {
  id: string;
  opportunity_id: string;
  status: ArbitrageValidationStatus;
  gross_edge: string;
  net_edge: string;
  fee_estimate: string;
  slippage_buffer: string;
  validated_capacity: string;
  book_age_ms: number;
  reason_codes: string[];
  validation_payload: unknown;
  validated_at: string;
  trace_id: string;
};

export type ArbitrageAnalysisRunDto = {
  id: string;
  generated_at: string;
  lookback_hours: number;
  opportunity_count: number;
  market_count: number;
  summary_payload: ArbitrageAnalysisSummaryDto | unknown;
  trace_id: string;
};

export type ArbitrageAnalysisSummaryDto = {
  generated_at: string;
  lookback_hours: number;
  opportunity_count: number;
  market_count: number;
  type_counts: ArbitrageTypeCountDto[];
  top_markets: ArbitrageMarketSummaryDto[];
};

export type ArbitrageTypeCountDto = {
  opportunity_type: ArbitrageOpportunityType;
  count: number;
};

export type ArbitrageMarketSummaryDto = {
  market_id: string;
  opportunity_count: number;
  first_observed_at: string;
  last_observed_at: string;
  duration_seconds: number;
  max_gross_edge: string;
  avg_gross_edge: string;
  max_capacity: string;
  avg_capacity: string;
};

export type RewardBotConfigDto = {
  enabled: boolean;
  mode: RewardBotMode;
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
  created_at: string;
  updated_at: string;
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
  mode: RewardBotMode;
  account_id: string;
  markets_tracked: number;
  eligible_markets: number;
  open_orders: number;
  positions: number;
  last_scan_at?: string | null;
  last_run_at?: string | null;
  error?: string | null;
};

export type RewardBotSnapshotDto = {
  config: RewardBotConfigDto;
  status: RewardBotStatusDto;
  markets: RewardMarketDto[];
  quote_plans: RewardQuotePlanDto[];
  orders: ManagedRewardOrderDto[];
  positions: RewardPositionDto[];
  events: RewardRiskEventDto[];
};

export type SignalTransitionDto = {
  id: string;
  signal_id: string;
  from_state: SignalLifecycleState;
  to_state: SignalLifecycleState;
  trigger_type: string;
  trigger_payload: unknown;
  created_at: string;
};

export type ReplayMomentDto = {
  occurred_at: string;
  kind: ReplayMomentKind;
  summary: string;
};

export type ReplayMetricDto = {
  title: string;
  value: string;
};

export type ReplayRunDto = ResourceVersion & {
  label: string;
  market_id: string;
  market_question: string;
  prior: string;
  posterior: string;
  signal_state_from: SignalLifecycleState;
  signal_state_to: SignalLifecycleState;
  signal_hit_rate: string;
  brier_score: string;
  net_alpha: string;
  metrics?: ReplayMetricDto[];
  timeline: ReplayMomentDto[];
  created_at: string;
  updated_at: string;
};
