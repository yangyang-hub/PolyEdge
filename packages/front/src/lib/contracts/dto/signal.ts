import type {
  ResourceVersion,
  SignalAction,
  SignalLifecycleState,
  SignalSide,
} from "./primitives";

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

export type SignalTransitionDto = {
  id: string;
  signal_id: string;
  from_state: SignalLifecycleState;
  to_state: SignalLifecycleState;
  trigger_type: string;
  trigger_payload: unknown;
  created_at: string;
};
