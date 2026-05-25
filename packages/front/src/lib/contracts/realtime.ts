import type {
  AlertSeverity,
  AlertStatus,
  ArbitrageOpportunityStatus,
  ArbitrageOpportunityType,
  ArbitrageValidationStatus,
  RuntimeEnvironment,
  RuntimeMode,
  SignalLifecycleState,
  SignalSide,
} from "@/lib/contracts/dto";

export const REALTIME_CHANNELS = ["signals", "risk", "events", "arbitrage"] as const;

export type RealtimeChannel = (typeof REALTIME_CHANNELS)[number];

export const CHANNEL_EVENT_TYPES = {
  signals: ["signal.created", "signal.updated", "signal.invalidated"],
  risk: ["risk.alerted", "risk.mode_changed"],
  events: ["event.created"],
  arbitrage: [
    "arbitrage.scan.started",
    "arbitrage.scan.completed",
    "arbitrage.opportunity.observed",
    "arbitrage.opportunity.repeated",
    "arbitrage.opportunity.expired",
    "arbitrage.validation.passed",
    "arbitrage.validation.failed",
    "arbitrage.analysis.generated",
  ],
} as const satisfies Record<RealtimeChannel, readonly string[]>;

export type SignalStreamEventType = (typeof CHANNEL_EVENT_TYPES.signals)[number];
export type RiskStreamEventType = (typeof CHANNEL_EVENT_TYPES.risk)[number];
export type ConsoleEventStreamEventType = (typeof CHANNEL_EVENT_TYPES.events)[number];
export type ArbitrageStreamEventType = (typeof CHANNEL_EVENT_TYPES.arbitrage)[number];

export type SignalStreamPayload = {
  signal_id: string;
  market_id: string;
  market_question?: string;
  context_label?: string;
  version: number;
  lifecycle_state: SignalLifecycleState;
  side?: SignalSide;
  fair_price?: string;
  market_price?: string;
  edge?: string;
  confidence?: string;
  reason?: string;
  risk_decision?: string;
  evidence_lines?: string[];
  updated_at?: string;
};

export type RiskStreamPayload = {
  resource_id: string;
  version: number;
  mode?: RuntimeMode;
  environment?: RuntimeEnvironment;
  kill_switch?: boolean;
  daily_pnl?: string;
  gross_exposure?: string;
  net_exposure?: string;
  daily_loss_limit?: string;
  daily_loss_used?: string;
  open_alerts?: number;
  critical_alerts?: number;
  warning_alerts?: number;
  alert_id?: string;
  severity?: AlertSeverity;
  reason?: string;
  target?: string;
  status?: AlertStatus;
  created_at?: string;
  updated_at?: string;
};

export type ConsoleEventStreamPayload = {
  event_id: string;
  source: string;
  summary: string;
  confidence: string;
  created_at?: string;
  version: number;
};

export type ArbitrageStreamPayload = {
  sequence?: number;
  event_id?: string;
  event_type?: ArbitrageStreamEventType;
  resource_type?: string;
  resource_id?: string;
  occurred_at?: string;
  scan_id?: string;
  started_at?: string;
  finished_at?: string | null;
  market_count?: number;
  snapshot_count?: number;
  opportunity_count?: number;
  scanner_version?: string;
  metadata?: unknown;
  opportunity_id?: string;
  market_id?: string;
  opportunity_type?: ArbitrageOpportunityType;
  status?: ArbitrageOpportunityStatus;
  gross_edge?: string;
  price_sum?: string;
  capacity?: string;
  yes_price?: string;
  no_price?: string;
  yes_size?: string;
  no_size?: string;
  observed_at?: string;
  reason_codes?: string[];
  analysis_payload?: unknown;
  validation?: unknown;
  validation_id?: string;
  validation_status?: ArbitrageValidationStatus;
  net_edge?: string;
  fee_estimate?: string;
  slippage_buffer?: string;
  validated_capacity?: string;
  book_age_ms?: number;
  validation_payload?: unknown;
  validated_at?: string;
  analysis_id?: string;
  generated_at?: string;
  lookback_hours?: number;
  summary_payload?: unknown;
  trace_id?: string;
};

export type RealtimePayloadByChannel = {
  signals: SignalStreamPayload;
  risk: RiskStreamPayload;
  events: ConsoleEventStreamPayload;
  arbitrage: ArbitrageStreamPayload;
};

export type RealtimeMessage<T> = {
  id: string;
  type: string;
  data: T;
};
