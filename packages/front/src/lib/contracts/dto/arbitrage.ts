import type {
  ArbitrageOpportunityStatus,
  ArbitrageOpportunityType,
  ArbitrageValidationStatus,
} from "./primitives";

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
