"use client";

import { PaginationBar } from "@/components/pagination-bar";
import { StatusPill } from "@/components/shared/status-pill";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { usePagination } from "@/hooks/use-pagination";
import type { RewardFillDto } from "@/lib/contracts/dto";
import { formatFixed, formatOptionalClock, formatSignedFixed } from "@/lib/formatters";
import { dictionary } from "@/lib/i18n/dictionaries";

export function FillsTable({ fills }: { fills: RewardFillDto[] }) {
  const pagination = usePagination(fills.length, 15);

  if (fills.length === 0) {
    return <p className="py-6 text-center text-sm text-muted-foreground">{dictionary.rewards.none}</p>;
  }

  return (
    <div>
      <Table className="min-w-[700px]">
        <TableHeader>
          <TableRow>
            <TableHead>{dictionary.rewards.outcome}</TableHead>
            <TableHead>{dictionary.rewards.side}</TableHead>
            <TableHead>{dictionary.rewards.role}</TableHead>
            <TableHead>{dictionary.rewards.price}</TableHead>
            <TableHead>{dictionary.rewards.size}</TableHead>
            <TableHead>{dictionary.rewards.pnl}</TableHead>
            <TableHead>{dictionary.rewards.time}</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {fills.slice(pagination.start, pagination.end).map((fill) => (
            <TableRow key={fill.id}>
              <TableCell>{fill.outcome}</TableCell>
              <TableCell>
                <StatusPill tone={fill.side === "buy" ? "success" : "warning"}>{fill.side}</StatusPill>
              </TableCell>
              <TableCell className="font-mono text-xs">{fill.role}</TableCell>
              <TableCell className="font-mono">{formatFixed(fill.price, 2)}</TableCell>
              <TableCell className="font-mono">{formatFixed(fill.size, 2)}</TableCell>
              <TableCell className="font-mono">{formatSignedFixed(fill.realized_pnl, 2)}</TableCell>
              <TableCell className="font-mono text-xs text-muted-foreground">
                {formatOptionalClock(fill.created_at)}
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
      <PaginationBar pagination={pagination} totalItems={fills.length} />
    </div>
  );
}
