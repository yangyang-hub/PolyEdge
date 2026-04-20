import type {
  ApprovalSeverity,
  ApprovalStatus,
  ApprovalType,
  AlertSeverity,
  AlertStatus,
  RuntimeEnvironment,
  RuntimeMode,
  SignalLifecycleState,
  SignalSide,
} from "@/lib/contracts/dto";

export const REALTIME_CHANNELS = ["signals", "risk", "events"] as const;

export type RealtimeChannel = (typeof REALTIME_CHANNELS)[number];

export const CHANNEL_EVENT_TYPES = {
  signals: ["signal.created", "signal.updated", "signal.invalidated"],
  risk: ["risk.alerted", "risk.mode_changed", "approval.created", "approval.updated"],
  events: ["event.created"],
} as const satisfies Record<RealtimeChannel, readonly string[]>;

export type SignalStreamEventType = (typeof CHANNEL_EVENT_TYPES.signals)[number];
export type RiskStreamEventType = (typeof CHANNEL_EVENT_TYPES.risk)[number];
export type ConsoleEventStreamEventType = (typeof CHANNEL_EVENT_TYPES.events)[number];

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
  requires_review?: boolean;
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
  approval_count?: number;
  approval_id?: string;
  approval_type?: ApprovalType;
  approval_severity?: ApprovalSeverity;
  approval_status?: ApprovalStatus;
  approval_owner?: string;
  approval_summary?: string;
  approval_resource_id?: string;
  approval_requires_step_up_auth?: boolean;
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

export type RealtimePayloadByChannel = {
  signals: SignalStreamPayload;
  risk: RiskStreamPayload;
  events: ConsoleEventStreamPayload;
};

export type RealtimeMessage<T> = {
  id: string;
  type: string;
  data: T;
};
