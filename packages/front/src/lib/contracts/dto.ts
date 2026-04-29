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
export type ApprovalType = "signal" | "mode_switch" | "kill_switch";
export type ApprovalSeverity = "info" | "warning" | "critical";
export type ApprovalStatus = "pending" | "approved" | "rejected";
export type AlertSeverity = "warning" | "critical";
export type AlertStatus = "unresolved" | "watching" | "contained";
export type PositionSide = "yes" | "no";
export type BucketStatus = "healthy" | "watch" | "breach";
export type NewsSourceType = "news" | "social" | "official" | "calendar" | "market";
export type ReplayMomentKind = "event_ingested" | "evidence_generated" | "posterior_updated" | "signal_transition";

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

export type ApprovalDto = ResourceVersion & {
  type: ApprovalType;
  severity: ApprovalSeverity;
  owner: string;
  resource_id: string;
  summary: string;
  status: ApprovalStatus;
  requires_step_up_auth: boolean;
  created_at: string;
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
