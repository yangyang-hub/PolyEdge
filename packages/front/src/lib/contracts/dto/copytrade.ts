import type {
  CopyOrderSide,
  CopyEventSeverity,
  CopySizingMode,
  CopyTradeMode,
  DecimalValue,
  TrackedWalletStatus,
} from "./primitives";

export type CopyTradeConfigDto = {
  enabled: boolean;
  mode: CopyTradeMode;
  account_id: string;
  account_capital_usd: DecimalValue;
  sizing_mode: CopySizingMode;
  fixed_usd_per_trade: DecimalValue;
  proportional_factor: DecimalValue;
  capital_ratio: DecimalValue;
  min_source_trade_usd: DecimalValue;
  max_price: DecimalValue;
  min_price: DecimalValue;
  copy_sells: boolean;
  max_position_per_market_usd: DecimalValue;
  per_wallet_max_exposure_usd: DecimalValue;
  max_total_exposure_usd: DecimalValue;
  max_open_copy_orders: number;
  daily_loss_limit_usd: DecimalValue;
  cooldown_secs: number;
  max_slippage_cents: DecimalValue;
  fill_rate_per_tick: DecimalValue;
  max_fill_ratio: DecimalValue;
};

export type CopyTradeConfigPatchDto = Partial<CopyTradeConfigDto>;

export type WalletAnalysisStatsDto = {
  trades_window: number;
  volume_window_usd: DecimalValue;
  realized_pnl_window: DecimalValue;
  win_rate: DecimalValue;
  roi: DecimalValue;
  avg_trade_usd: DecimalValue;
  markets_traded: number;
  last_active_at?: string | null;
  last_analyzed_at?: string | null;
};

export type TrackedWalletDto = {
  address: string;
  label: string;
  status: TrackedWalletStatus;
  sizing_override?: CopySizingMode | null;
  max_exposure_override?: DecimalValue | null;
  added_at: string;
  updated_at: string;
  analysis: WalletAnalysisStatsDto;
};

export type SourceTradeDto = {
  id: string;
  wallet_address: string;
  condition_id: string;
  token_id: string;
  outcome: string;
  side: CopyOrderSide;
  price: DecimalValue;
  size: DecimalValue;
  usd_size: DecimalValue;
  title: string;
  source_tx_hash: string;
  source_timestamp: string;
  observed_at: string;
  copied: boolean;
  decision_reason: string;
};

export type CopyEventDto = {
  id: string;
  wallet_address?: string | null;
  condition_id?: string | null;
  event_type: string;
  severity: CopyEventSeverity;
  message: string;
  metadata: unknown;
  created_at: string;
};

export type CopyTradeStatusDto = {
  enabled: boolean;
  running: boolean;
  wallets_tracked: number;
  active_wallets: number;
  source_trades_detected: number;
  last_scan_at?: string | null;
  error?: string | null;
};

export type CopyTradeSnapshotDto = {
  config: CopyTradeConfigDto;
  status: CopyTradeStatusDto;
  wallets: TrackedWalletDto[];
  source_trades: SourceTradeDto[];
  events: CopyEventDto[];
};

export type AddTrackedWalletInputDto = {
  address: string;
  label?: string;
  sizing_override?: CopySizingMode;
  max_exposure_override?: DecimalValue;
};

export type WalletActionInputDto = {
  address: string;
};
