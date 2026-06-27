"use client";

import { PaginationBar } from "@/components/pagination-bar";
import { StatusPill } from "@/components/shared/status-pill";
import { TruncateText } from "@/components/shared/truncate-text";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { usePagination } from "@/hooks/use-pagination";
import type { RewardRiskEventDto } from "@/lib/contracts/dto";
import { approvalSeverityTone, formatOptionalClock } from "@/lib/formatters";
import { dictionary } from "@/lib/i18n/dictionaries";

export function EventsTable({ events }: { events: RewardRiskEventDto[] }) {
  const pagination = usePagination(events.length, 15);

  return (
    <div>
      <Table className="min-w-[760px] table-fixed">
        <TableHeader>
          <TableRow>
            <TableHead>{dictionary.rewards.severity}</TableHead>
            <TableHead>{dictionary.rewards.type}</TableHead>
            <TableHead className="w-[50%]">{dictionary.rewards.message}</TableHead>
            <TableHead>{dictionary.common.published}</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {events.slice(pagination.start, pagination.end).map((event) => (
            <TableRow key={event.id}>
              <TableCell>
                <StatusPill tone={approvalSeverityTone(event.severity)}>{event.severity}</StatusPill>
              </TableCell>
              <TableCell className="font-mono text-xs">{event.event_type}</TableCell>
              <TableCell className="leading-5">
                <TruncateText text={event.message} lines={2} />
              </TableCell>
              <TableCell className="font-mono text-xs text-muted-foreground">
                {formatOptionalClock(event.created_at)}
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
      <PaginationBar pagination={pagination} totalItems={events.length} />
    </div>
  );
}
