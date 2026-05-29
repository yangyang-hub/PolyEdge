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
