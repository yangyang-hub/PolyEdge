import type {
  DecimalValue,
  HighProbabilityDecision,
  HighProbabilityMode,
} from "./primitives";

export type HighProbabilityConfigDto = {
  enabled: boolean;
  mode: HighProbabilityMode;
  market_scope: string;
  model_version: string;
  min_required_edge: DecimalValue;
  fee_buffer: DecimalValue;
  default_risk_margin: DecimalValue;
  min_confidence: DecimalValue;
  min_bucket_samples: number;
  max_spread_cents: DecimalValue;
  min_depth_usd: DecimalValue;
  max_single_trade_usd: DecimalValue;
  max_single_market_exposure_usd: DecimalValue;
  max_daily_new_notional_usd: DecimalValue;
  conservative_kelly_multiplier: DecimalValue;
  excluded_risk_tags: string[];
};

export type HighProbabilityBucketStatsDto = {
  bucket_key: string;
  bucket_dimensions: unknown;
  sample_count: number;
  win_count: number;
  win_rate: DecimalValue;
  fair_probability: DecimalValue;
  confidence_low?: DecimalValue | null;
  confidence_high?: DecimalValue | null;
  expected_pnl?: DecimalValue | null;
  avg_max_drawdown_cents?: DecimalValue | null;
  break_70_rate?: DecimalValue | null;
  break_60_rate?: DecimalValue | null;
  break_50_rate?: DecimalValue | null;
  avg_hold_seconds?: number | null;
  recommended_max_entry_price?: DecimalValue | null;
  computed_at: string;
};

export type HighProbabilityObservationDto = {
  id: number;
  observed_at: string;
  condition_id: string;
  token_id: string;
  mode: HighProbabilityMode;
  executable_price: DecimalValue;
  fair_probability?: DecimalValue | null;
  net_edge?: DecimalValue | null;
  recommended_size_usd?: DecimalValue | null;
  decision: HighProbabilityDecision;
  reasons: string[];
  model_version?: string | null;
  created_at: string;
};

export type HighProbabilitySnapshotDto = {
  config: HighProbabilityConfigDto;
  bucket_stats: HighProbabilityBucketStatsDto[];
  observations: HighProbabilityObservationDto[];
};

export type HighProbabilityResearchReportDto = {
  generated_at: string;
  model_version: string;
  market_scope: string;
  sample_limit: number;
  samples_scanned: number;
  settled_samples: number;
  win_samples: number;
  loss_samples: number;
  voided_samples: number;
  unknown_samples: number;
  bucket_count: number;
  qualified_bucket_count: number;
  positive_expected_pnl_bucket_count: number;
  weighted_win_rate?: DecimalValue | null;
  weighted_expected_pnl?: DecimalValue | null;
  weighted_break_70_rate?: DecimalValue | null;
  best_bucket?: HighProbabilityBucketStatsDto | null;
  worst_bucket?: HighProbabilityBucketStatsDto | null;
  notes: string[];
};

export type HighProbabilityBacktestReportDto = {
  generated_at: string;
  model_version: string;
  market_scope: string;
  sample_limit: number;
  train_sample_count: number;
  test_sample_count: number;
  candidate_count: number;
  trade_count: number;
  skipped_no_bucket_count: number;
  skipped_no_edge_count: number;
  win_trades: number;
  loss_trades: number;
  win_rate?: DecimalValue | null;
  total_pnl: DecimalValue;
  average_pnl?: DecimalValue | null;
  total_entry_cost: DecimalValue;
  roi?: DecimalValue | null;
  max_drawdown: DecimalValue;
  average_entry_price?: DecimalValue | null;
  train_start_at?: string | null;
  train_end_at?: string | null;
  test_start_at?: string | null;
  test_end_at?: string | null;
  exit_rule_reports: HighProbabilityBacktestExitRuleReportDto[];
  notes: string[];
};

export type HighProbabilityBacktestExitRuleReportDto = {
  rule_key: string;
  trade_count: number;
  win_rate?: DecimalValue | null;
  total_pnl: DecimalValue;
  average_pnl?: DecimalValue | null;
  total_entry_cost: DecimalValue;
  roi?: DecimalValue | null;
  max_drawdown: DecimalValue;
  notes: string[];
};

export type HighProbabilityBacktestRunDto = {
  id: number;
  run_at: string;
  report: HighProbabilityBacktestReportDto;
};

export type HighProbabilityBacktestTradeDto = {
  id: number;
  run_id: number;
  sample_id: number;
  condition_id: string;
  token_id: string;
  sampled_at: string;
  bucket_key: string;
  executable_price: DecimalValue;
  fair_probability: DecimalValue;
  net_edge: DecimalValue;
  recommended_max_entry_price?: DecimalValue | null;
  outcome: "win" | "loss";
  settlement_pnl: DecimalValue;
  cumulative_pnl: DecimalValue;
  drawdown: DecimalValue;
  reasons: string[];
  created_at: string;
};
