"use client";

import { PaginationBar } from "@/components/pagination-bar";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { usePagination } from "@/hooks/use-pagination";
import type { RewardPositionDto, RewardTokenQuoteDto } from "@/lib/contracts/dto";
import { formatFixed, formatOptionalClock, formatSignedFixed, formatSignedPercent } from "@/lib/formatters";
import { dictionary } from "@/lib/i18n/dictionaries";

import { computePositionPnl, getPositionQuote } from "../lib/position-metrics";

export function PositionsTable({
  positions,
  tokenQuotes,
}: {
  positions: RewardPositionDto[];
  tokenQuotes: Record<string, RewardTokenQuoteDto> | null | undefined;
}) {
  const pagination = usePagination(positions.length, 8);

  if (positions.length === 0) {
    return <p className="py-6 text-center text-sm text-muted-foreground">{dictionary.rewards.none}</p>;
  }

  return (
    <div>
      <Table className="min-w-[1120px]">
        <TableHeader>
          <TableRow>
            <TableHead>{dictionary.rewards.market}</TableHead>
            <TableHead>{dictionary.rewards.outcome}</TableHead>
            <TableHead>{dictionary.rewards.size}</TableHead>
            <TableHead>{dictionary.rewards.avgPrice}</TableHead>
            <TableHead>{dictionary.rewards.bestBid}</TableHead>
            <TableHead>{dictionary.rewards.bestAsk}</TableHead>
            <TableHead>{dictionary.rewards.pnlAmount}</TableHead>
            <TableHead>{dictionary.rewards.pnlPercent}</TableHead>
            <TableHead>{dictionary.rewards.time}</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {positions.slice(pagination.start, pagination.end).map((position) => {
            const quote = getPositionQuote(tokenQuotes, position.token_id);
            const bestBid = quote?.best_bid ?? null;
            const bestAsk = quote?.best_ask ?? null;
            const pnl = computePositionPnl({
              size: position.size,
              avg_price: position.avg_price,
              realized_pnl: position.realized_pnl,
              mark_price: quote?.mark_price ?? null,
            });
            return (
              <TableRow key={`${position.condition_id}:${position.token_id}`}>
                <TableCell className="max-w-[220px] whitespace-normal break-all font-mono text-xs leading-5 text-muted-foreground">
                  {position.condition_id}
                </TableCell>
                <TableCell>{position.outcome}</TableCell>
                <TableCell className="font-mono">{formatFixed(position.size, 2)}</TableCell>
                <TableCell className="font-mono">{formatFixed(position.avg_price, 3)}</TableCell>
                <TableCell className="font-mono">{bestBid != null ? formatFixed(bestBid, 3) : "—"}</TableCell>
                <TableCell className="font-mono">{bestAsk != null ? formatFixed(bestAsk, 3) : "—"}</TableCell>
                <TableCell className="font-mono">
                  {pnl.amount != null ? formatSignedFixed(pnl.amount, 2) : "—"}
                </TableCell>
                <TableCell className="font-mono">
                  {pnl.percent != null ? formatSignedPercent(pnl.percent, 1) : "—"}
                </TableCell>
                <TableCell className="font-mono text-xs text-muted-foreground">
                  {formatOptionalClock(position.updated_at)}
                </TableCell>
              </TableRow>
            );
          })}
        </TableBody>
      </Table>
      <PaginationBar pagination={pagination} totalItems={positions.length} />
    </div>
  );
}
