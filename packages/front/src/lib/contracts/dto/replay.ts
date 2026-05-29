import type { ReplayMomentKind, ResourceVersion, SignalLifecycleState } from "./primitives";

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
