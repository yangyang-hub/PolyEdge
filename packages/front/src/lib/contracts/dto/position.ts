import type { PositionSide, ResourceVersion } from "./primitives";

export type PositionDto = ResourceVersion & {
  market_id: string;
  market_question: string;
  side: PositionSide;
  quantity: string;
  average_cost: string;
  mark_price: string;
  realized_pnl: string;
  unrealized_pnl: string;
  bucket_name: string;
  updated_at: string;
};
