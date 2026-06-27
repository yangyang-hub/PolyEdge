import { toFiniteNumber } from "@/lib/formatters";
import type { RewardTokenQuoteDto } from "@/lib/contracts/dto";

export type PositionPnl = {
  /** Total PnL = realized + unrealized, in USD. `null` when no mark price. */
  amount: number | null;
  /** PnL as a ratio of cost basis (amount / (avg_price * size)). `null` when
   * no mark price or zero cost basis. Multiply by 100 for a percentage. */
  percent: number | null;
};

/** Look up the best-effort live quote for a token from the snapshot map. */
export function getPositionQuote(
  tokenQuotes: Record<string, RewardTokenQuoteDto> | null | undefined,
  tokenId: string,
): RewardTokenQuoteDto | undefined {
  return tokenQuotes?.[tokenId];
}

/**
 * Derive position PnL from cost basis + a live mark price. Unrealized PnL is
 * `(mark - avg_price) * size`; total is realized + unrealized; percent is total
 * over the remaining cost basis. Returns `null` fields when no mark price is
 * available (the frontend then renders "—").
 */
export function computePositionPnl(input: {
  size: number | string | null | undefined;
  avg_price: number | string | null | undefined;
  realized_pnl: number | string | null | undefined;
  mark_price: number | string | null | undefined;
}): PositionPnl {
  const size = toFiniteNumber(input.size);
  const avgPrice = toFiniteNumber(input.avg_price);
  const realized = toFiniteNumber(input.realized_pnl);
  if (input.mark_price === null || input.mark_price === undefined) {
    return { amount: null, percent: null };
  }
  const mark = toFiniteNumber(input.mark_price);
  const unrealized = (mark - avgPrice) * size;
  const amount = realized + unrealized;
  const cost = avgPrice * size;
  return { amount, percent: cost > 0 ? amount / cost : null };
}
