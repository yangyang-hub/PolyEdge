import type {
  DecimalValue,
  RewardAiProvider,
  RewardAiRequestFormat,
  SmartMoneyMode,
  SmartMoneySide,
  SmartSignalDecisionValue,
  SmartSignalStatus,
  SmartWalletCandidateStatus,
  SmartWalletTier,
} from "./primitives";

export type SmartMoneyConfigDto = {
  enabled: boolean;
  mode: SmartMoneyMode;
  discovery_enabled: boolean;
  wallet_advisory_enabled: boolean;
  signal_advisory_enabled: boolean;
  signal_advisory_provider: RewardAiProvider;
  signal_advisory_request_format: RewardAiRequestFormat;
  signal_advisory_model: string;
  signal_advisory_concurrency_enabled: boolean;
  signal_advisory_max_concurrency: number;
  min_trade_count: number;
  min_settled_trade_count: number;
  min_total_volume_usd: DecimalValue;
  min_copyability_score: DecimalValue;
  max_signal_age_ms: number;
  max_price_slippage_cents: DecimalValue;
  min_orderbook_depth_usd: DecimalValue;
  max_wallet_exposure_usd: DecimalValue;
  max_market_exposure_usd: DecimalValue;
  max_daily_notional_usd: DecimalValue;
};

export type SmartMoneyConfigPatchDto = Partial<SmartMoneyConfigDto>;

export type SmartWalletCandidateDto = {
  id: number;
  wallet_address: string;
  source: string;
  status: SmartWalletCandidateStatus;
  first_seen_at: string;
  last_seen_at: string;
  last_analyzed_at?: string | null;
  promoted_at?: string | null;
  rejected_at?: string | null;
  reason?: string | null;
  raw: unknown;
};

export type SmartWalletCandidateStatusUpdateDto = {
  wallet_address: string;
  source?: string | null;
  status: SmartWalletCandidateStatus;
  reason?: string | null;
};

export type SmartWalletProfileDto = {
  wallet_address: string;
  trade_count: number;
  settled_trade_count: number;
  total_volume_usd: DecimalValue;
  realized_pnl_usd: DecimalValue;
  roi: DecimalValue;
  win_rate: DecimalValue;
  max_drawdown_usd: DecimalValue;
  avg_trade_usd: DecimalValue;
  median_trade_usd: DecimalValue;
  avg_hold_secs?: number | null;
  active_days: number;
  markets_traded: number;
  category_concentration_score: DecimalValue;
  market_concentration_score: DecimalValue;
  low_liquidity_trade_ratio: DecimalValue;
  stale_copy_window_ratio: DecimalValue;
  last_trade_at?: string | null;
  updated_at: string;
};

export type SmartWalletScoreDto = {
  wallet_address: string;
  total_score: DecimalValue;
  profit_score: DecimalValue;
  consistency_score: DecimalValue;
  risk_score: DecimalValue;
  liquidity_score: DecimalValue;
  recency_score: DecimalValue;
  copyability_score: DecimalValue;
  tier: SmartWalletTier;
  explanation: unknown;
  scoring_version: string;
  updated_at: string;
};

export type SmartWalletTradeDto = {
  id: string;
  wallet_address: string;
  source: string;
  condition_id: string;
  token_id?: string | null;
  side: SmartMoneySide;
  outcome?: string | null;
  price: DecimalValue;
  size: DecimalValue;
  notional_usd: DecimalValue;
  tx_hash?: string | null;
  source_timestamp: string;
  discovered_at: string;
  raw: unknown;
};

export type SmartSignalDto = {
  id: number;
  source_trade_id: string;
  wallet_address: string;
  condition_id: string;
  token_id?: string | null;
  side: SmartMoneySide;
  source_price: DecimalValue;
  current_price?: DecimalValue | null;
  price_slippage_cents?: DecimalValue | null;
  latency_ms?: number | null;
  source_notional_usd: DecimalValue;
  consensus_wallet_count: number;
  score: DecimalValue;
  status: SmartSignalStatus;
  reason?: string | null;
  created_at: string;
  updated_at: string;
};

export type SmartSignalDecisionDto = {
  id: number;
  signal_id: number;
  decision: SmartSignalDecisionValue;
  stage: string;
  mode: SmartMoneyMode;
  rejection_reason?: string | null;
  risk_checks: unknown;
  decided_at: string;
};

export type SmartSignalAdvisoryDto = {
  id: number;
  signal_id: number;
  provider: string;
  request_format: string;
  model: string;
  input_hash: string;
  recommendation: SmartSignalDecisionValue;
  confidence: DecimalValue;
  risk_tags: string[];
  summary: string;
  reasons: string[];
  raw_output: unknown;
  expires_at: string;
  created_at: string;
};

export type SmartMoneyStatusDto = {
  enabled: boolean;
  mode: SmartMoneyMode;
  candidates: number;
  watch_wallets: number;
  tracked_wallets: number;
  blocked_wallets: number;
  profiles: number;
  scored_wallets: number;
  recent_trades: number;
  recent_signals: number;
  recent_decisions: number;
  recent_signal_advisories: number;
  last_trade_at?: string | null;
};

export type SmartMoneySnapshotDto = {
  status: SmartMoneyStatusDto;
  config: SmartMoneyConfigDto;
  candidates: SmartWalletCandidateDto[];
  profiles: SmartWalletProfileDto[];
  scores: SmartWalletScoreDto[];
  recent_trades: SmartWalletTradeDto[];
  recent_signals: SmartSignalDto[];
  recent_decisions: SmartSignalDecisionDto[];
  recent_signal_advisories: SmartSignalAdvisoryDto[];
};
