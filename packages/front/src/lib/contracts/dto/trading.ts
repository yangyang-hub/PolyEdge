import type { DecimalValue, QuoteOutcome, TradingOrderSide } from "./primitives";

export type ManagedOrderStatus =
  | "planned"
  | "submitting"
  | "open"
  | "partially_filled"
  | "cancel_pending"
  | "cancelled"
  | "filled"
  | "expired"
  | "rejected"
  | "unknown";

export type ManagedOrderDto = {
  id: number;
  wallet_id: number;
  market_id: number;
  strategy_version_id: number;
  quote_slot_id: number | null;
  token_id: string;
  outcome: QuoteOutcome;
  side: TradingOrderSide;
  price: DecimalValue;
  quantity: DecimalValue;
  filled_quantity: DecimalValue;
  status: ManagedOrderStatus;
  external_order_id: string | null;
  generation: number;
  created_at: string;
  updated_at: string;
};

export type ManagedPositionDto = {
  id: number;
  wallet_id: number;
  market_id: number;
  token_id: string;
  outcome: QuoteOutcome;
  quantity: DecimalValue;
  average_price: DecimalValue;
  realized_pnl: DecimalValue;
  version: number;
  updated_at: string;
};
