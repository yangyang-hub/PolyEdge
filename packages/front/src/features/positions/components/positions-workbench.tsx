"use client";

import { startTransition, useDeferredValue, useEffect, useState } from "react";

import type { getPositionsPageData } from "@/features/positions/loaders/positions-page-data";
import { MetricCard } from "@/components/shared/metric-card";
import { EmptyPanel } from "@/components/shared/empty-panel";
import { PageHeader } from "@/components/shared/page-header";
import { PaginationBar } from "@/components/pagination-bar";
import { StatusPill } from "@/components/shared/status-pill";
import { WorkbenchLayout, WorkbenchDetailPane } from "@/components/shared/workbench-layout";
import { WorkbenchSegmentedControl } from "@/components/shared/workbench-segmented-control";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { MeterBar } from "@/components/shared/meter-bar";
import { usePagination } from "@/hooks/use-pagination";
import { dictionary, formatMessage } from "@/lib/i18n/dictionaries";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { isKeyboardSelect } from "@/lib/keyboard";

type PositionsPageData = Awaited<ReturnType<typeof getPositionsPageData>>;
type PositionFilter = "all" | "gainers" | "pressure";

export function PositionsWorkbench({ data }: { data: PositionsPageData }) {
  const [filter, setFilter] = useState<PositionFilter>("all");
  const [selectedId, setSelectedId] = useState(data.selectedPositionId);
  const deferredFilter = useDeferredValue(filter);

  const filteredPositions = data.positions.filter((position) => {
    if (deferredFilter === "gainers") {
      return position.pnlValue > 0;
    }

    if (deferredFilter === "pressure") {
      return position.bucketStatus !== "healthy" || position.pnlValue < 0;
    }

    return true;
  });

  const pagination = usePagination(filteredPositions.length, 20);
  const { reset: resetPagination } = pagination;

  useEffect(() => {
    resetPagination();
  }, [deferredFilter, resetPagination]);

  const activeSelectedId =
    filteredPositions.find((position) => position.id === selectedId)?.id ??
    filteredPositions[0]?.id ??
    data.positions[0]?.id ??
    "";
  const selectedPosition =
    data.positions.find((position) => position.id === activeSelectedId) ??
    data.positions[0] ??
    null;
  const pressureCount = data.positions.filter(
    (position) => position.bucketStatus !== "healthy" || position.pnlValue < 0,
  ).length;
  const filterButtons: Array<{ key: PositionFilter; label: string }> = [
    { key: "all", label: dictionary.positions.all },
    { key: "gainers", label: dictionary.positions.positivePnl },
    { key: "pressure", label: dictionary.positions.pressure },
  ];

  function selectPosition(positionId: string) {
    startTransition(() => {
      setSelectedId(positionId);
    });
  }

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow={dictionary.positions.eyebrow}
        title={dictionary.positions.title}
        description={dictionary.positions.description}
        actions={
          <>
            <StatusPill tone="primary">{data.runtimeModeLabel}</StatusPill>
            <StatusPill tone="neutral">{data.runtimeEnvironmentLabel}</StatusPill>
          </>
        }
      />

      <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        {data.metrics.map((metric) => (
          <MetricCard
            key={metric.key}
            title={metric.title}
            value={metric.value}
            hint={metric.hint}
            accent={metric.tone}
          />
        ))}
      </section>

      <WorkbenchLayout columnsClassName="xl:grid-cols-[1.6fr_0.95fr]">
        <Card>
          <CardHeader className="flex flex-col gap-4 border-b border-border/70 md:flex-row md:items-center md:justify-between">
            <div>
              <CardTitle className="font-heading text-base">{dictionary.positions.livePositions}</CardTitle>
              <p className="mt-1 text-sm text-muted-foreground">
                {dictionary.positions.livePositionsHint}
              </p>
            </div>
            <WorkbenchSegmentedControl items={filterButtons} value={filter} onChange={setFilter} />
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex flex-wrap gap-2">
              <StatusPill tone="success">{formatMessage(dictionary.positions.open, { count: data.positions.length })}</StatusPill>
              <StatusPill tone="warning">{formatMessage(dictionary.positions.pressureCount, { count: pressureCount })}</StatusPill>
            </div>

            <div className="overflow-x-auto">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>{dictionary.positions.market}</TableHead>
                    <TableHead>{dictionary.positions.side}</TableHead>
                    <TableHead>{dictionary.positions.qty}</TableHead>
                    <TableHead>{dictionary.positions.pnl}</TableHead>
                    <TableHead>{dictionary.positions.signal}</TableHead>
                    <TableHead>{dictionary.positions.controls}</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {filteredPositions.length === 0 ? (
                    <TableRow>
                      <TableCell colSpan={6} className="py-8 text-center text-sm text-muted-foreground">
                        {data.positions.length === 0 ? dictionary.positions.noPositions : dictionary.positions.noMatchingPositions}
                      </TableCell>
                    </TableRow>
                  ) : filteredPositions.slice(pagination.start, pagination.end).map((position) => (
                    <TableRow
                      key={position.id}
                      tabIndex={0}
                      onClick={() => selectPosition(position.id)}
                      onKeyDown={(event) => {
                        if (isKeyboardSelect(event)) {
                          event.preventDefault();
                          selectPosition(position.id);
                        }
                      }}
                      className={
                        position.id === activeSelectedId
                          ? "cursor-pointer bg-accent/60 shadow-[inset_2px_0_0_var(--sidebar-primary)] outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/50"
                          : "cursor-pointer outline-none transition-colors hover:bg-accent/35 focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/50"
                      }
                    >
                      <TableCell>
                        <div className="space-y-1">
                          <p className="font-medium">{position.marketQuestion}</p>
                          <div className="flex flex-wrap gap-2">
                            <StatusPill tone={position.tradabilityTone}>{position.tradabilityLabel}</StatusPill>
                            <StatusPill tone={position.bucketTone}>{position.bucketName}</StatusPill>
                          </div>
                        </div>
                      </TableCell>
                      <TableCell>{position.side}</TableCell>
                      <TableCell className="font-mono">{position.quantity}</TableCell>
                      <TableCell className="font-mono">{position.pnl}</TableCell>
                      <TableCell>
                        <StatusPill tone={position.signalStateTone}>{position.signalStateLabel}</StatusPill>
                      </TableCell>
                      <TableCell>
                        <div className="flex flex-wrap gap-2">
                          {position.bucketStatus !== "healthy" ? (
                            <StatusPill tone={position.bucketTone}>{position.bucketStatusLabel}</StatusPill>
                          ) : null}
                        </div>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>

            <PaginationBar pagination={pagination} totalItems={filteredPositions.length} />
          </CardContent>
        </Card>

        <div className="space-y-4">
          {selectedPosition ? (
            <>
          <WorkbenchDetailPane className="space-y-4">
            <div className="space-y-2">
              <p className="font-heading text-lg font-bold tracking-tight text-foreground">
                {selectedPosition.marketQuestion}
              </p>
              <div className="flex flex-wrap gap-2">
                <StatusPill tone={selectedPosition.signalStateTone}>
                  {selectedPosition.signalStateLabel}
                </StatusPill>
                <StatusPill tone={selectedPosition.pnlTone}>{selectedPosition.pnl}</StatusPill>
              </div>
            </div>

            <div className="grid grid-cols-2 gap-3">
              <div className="rounded-md bg-accent/45 p-3">
                <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">{dictionary.positions.mark}</p>
                <p className="mt-2 font-mono text-lg text-foreground">{selectedPosition.markPrice}</p>
              </div>
              <div className="rounded-md bg-accent/45 p-3">
                <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
                  {dictionary.positions.posterior}
                </p>
                <p className="mt-2 font-mono text-lg text-primary">{selectedPosition.posterior}</p>
              </div>
              <div className="rounded-md bg-accent/45 p-3">
                <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">{dictionary.positions.avgCost}</p>
                <p className="mt-2 font-mono text-lg text-foreground">{selectedPosition.averageCost}</p>
              </div>
              <div className="rounded-md bg-accent/45 p-3">
                <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
                  {dictionary.positions.confidence}
                </p>
                <p className="mt-2 font-mono text-lg text-foreground">{selectedPosition.confidence}</p>
              </div>
            </div>

            <div className="space-y-3 rounded-md bg-popover/70 p-4">
              <div className="flex items-center justify-between gap-3">
                <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
                  {dictionary.positions.bucketPressure}
                </p>
                <StatusPill tone={selectedPosition.bucketTone}>{selectedPosition.bucketStatusLabel}</StatusPill>
              </div>
              <MeterBar value={selectedPosition.bucketUtilizationWidth} tone={selectedPosition.bucketTone} />
              <div className="flex items-center justify-between text-sm text-muted-foreground">
                <span>{selectedPosition.bucketName}</span>
                <span>{formatMessage(dictionary.positions.utilized, { value: selectedPosition.bucketUtilization })}</span>
              </div>
            </div>

            <div className="rounded-md bg-popover/70 p-4">
              <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
                {dictionary.positions.signalReason}
              </p>
              <p className="mt-3 text-sm text-foreground">{selectedPosition.signalReason}</p>
            </div>

            <div className="rounded-md bg-popover/70 p-4">
              <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
                {dictionary.positions.riskDecision}
              </p>
              <p className="mt-3 text-sm text-muted-foreground">{selectedPosition.riskDecision}</p>
            </div>
          </WorkbenchDetailPane>

          <Card>
            <CardHeader>
              <CardTitle className="font-heading text-base">{dictionary.positions.catalysts}</CardTitle>
            </CardHeader>
            <CardContent className="space-y-3">
              {selectedPosition.linkedEvents.length > 0 ? (
                selectedPosition.linkedEvents.map((event) => (
                  <div key={event.id} className="rounded-sm border border-border/70 bg-card p-3">
                    <div className="flex items-center justify-between gap-3">
                      <StatusPill tone="primary">{event.source}</StatusPill>
                      <span className="font-mono text-xs text-muted-foreground">{event.relevance}</span>
                    </div>
                    <p className="mt-2 text-sm text-foreground">{event.summary}</p>
                  </div>
                ))
              ) : (
                <p className="text-sm text-muted-foreground">{dictionary.positions.noCatalysts}</p>
              )}
            </CardContent>
          </Card>
            </>
          ) : (
            <EmptyPanel title={dictionary.positions.livePositions} detail={dictionary.positions.emptyDetail} />
          )}

          <Card>
            <CardHeader>
              <CardTitle className="font-heading text-base">{dictionary.positions.deskBuckets}</CardTitle>
            </CardHeader>
            <CardContent className="space-y-3">
              {data.riskBuckets.map((bucket) => (
                <div key={bucket.id} className="rounded-sm border border-border/70 bg-card p-3">
                  <div className="flex items-center justify-between gap-3">
                    <p className="text-sm font-medium">{bucket.name}</p>
                    <StatusPill tone={bucket.tone}>{bucket.utilization}</StatusPill>
                  </div>
                  <div className="mt-3 space-y-2">
                    <MeterBar value={bucket.width} tone={bucket.tone} />
                    <div className="flex items-center justify-between text-xs text-muted-foreground">
                      <span>{dictionary.positions.exposure} {bucket.exposure}</span>
                      <span>{dictionary.positions.limit} {bucket.limit}</span>
                    </div>
                  </div>
                </div>
              ))}
            </CardContent>
          </Card>
        </div>
      </WorkbenchLayout>
    </div>
  );
}
