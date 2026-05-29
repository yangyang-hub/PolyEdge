import type {
  CopyOrderSide,
  CopyOrderStatus,
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

export type CopyOrderDto = {
  id: string;
  account_id: string;
  wallet_address: string;
  source_trade_id: string;
  condition_id: string;
  token_id: string;
  outcome: string;
  side: CopyOrderSide;
  price: DecimalValue;
  size: DecimalValue;
  notional_usd: DecimalValue;
  external_order_id?: string | null;
  status: CopyOrderStatus;
  reason: string;
  filled_size?: DecimalValue;
  realized_pnl?: DecimalValue;
  created_at: string;
  updated_at: string;
};

export type CopyPositionDto = {
  account_id: string;
  wallet_address: string;
  condition_id: string;
  token_id: string;
  outcome: string;
  size: DecimalValue;
  avg_price: DecimalValue;
  realized_pnl: DecimalValue;
  updated_at: string;
};

export type CopyAccountStateDto = {
  account_id: string;
  capital_usd: DecimalValue;
  available_usd: DecimalValue;
  reserved_usd: DecimalValue;
  realized_pnl: DecimalValue;
  fees_paid: DecimalValue;
  tick_index: number;
  updated_at: string;
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
  mode: CopyTradeMode;
  account_id: string;
  wallets_tracked: number;
  active_wallets: number;
  open_orders: number;
  positions: number;
  source_trades_detected: number;
  last_scan_at?: string | null;
  last_run_at?: string | null;
  error?: string | null;
};

export type CopyTradeSnapshotDto = {
  config: CopyTradeConfigDto;
  status: CopyTradeStatusDto;
  account: CopyAccountStateDto;
  wallets: TrackedWalletDto[];
  source_trades: SourceTradeDto[];
  orders: CopyOrderDto[];
  positions: CopyPositionDto[];
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
