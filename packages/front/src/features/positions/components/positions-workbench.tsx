"use client";

import { startTransition, useDeferredValue, useEffect, useState } from "react";

import type { getPositionsPageData } from "@/features/positions/loaders/positions-page-data";
import { MetricCard } from "@/components/shared/metric-card";
import { PageHeader } from "@/components/shared/page-header";
import { StatusPill } from "@/components/shared/status-pill";
import { WorkbenchLayout, WorkbenchDetailPane } from "@/components/shared/workbench-layout";
import { WorkbenchSegmentedControl } from "@/components/shared/workbench-segmented-control";
import { useConsoleRealtimeChannel } from "@/components/shared/console-realtime-provider";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { MeterBar } from "@/components/shared/meter-bar";
import { useI18n } from "@/lib/i18n/client";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type { RiskStreamPayload, SignalStreamPayload } from "@/lib/contracts/realtime";
import { isKeyboardSelect } from "@/lib/keyboard";
import {
  formatClock,
  formatCurrency,
  formatPercentFromRatio,
  metricToneForPnl,
  signalStateTone,
} from "@/lib/realtime-formatters";

type PositionsPageData = Awaited<ReturnType<typeof getPositionsPageData>>;
type PositionItem = PositionsPageData["positions"][number];
type PositionMetric = PositionsPageData["metrics"][number];
type PositionFilter = "all" | "gainers" | "pressure";

function patchPositionSignal(
  position: PositionItem,
  payload: SignalStreamPayload,
  enumLabel: (value: string) => string,
): PositionItem {
  return {
    ...position,
    signalId: payload.signal_id,
    marketQuestion: payload.market_question ?? position.marketQuestion,
    posterior: payload.fair_price ?? position.posterior,
    signalEdge: payload.edge ? formatPercentFromRatio(payload.edge) : position.signalEdge,
    confidence: payload.confidence ? formatPercentFromRatio(payload.confidence) : position.confidence,
    confidenceWidth: payload.confidence ? formatPercentFromRatio(payload.confidence) : position.confidenceWidth,
    signalStateLabel: enumLabel(payload.lifecycle_state),
    signalStateTone: signalStateTone(payload.lifecycle_state),
    signalReason: payload.reason ?? position.signalReason,
    riskDecision: payload.risk_decision ?? position.riskDecision,
    signalUpdatedAt: payload.updated_at ?? position.signalUpdatedAt,
  };
}

function patchPositionsFromSignal(
  positions: PositionItem[],
  payload: SignalStreamPayload,
  enumLabel: (value: string) => string,
): PositionItem[] {
  return positions.map((position) =>
    position.marketId === payload.market_id || position.signalId === payload.signal_id
      ? patchPositionSignal(position, payload, enumLabel)
      : position,
  );
}

function patchMetrics(metrics: PositionMetric[], payload: RiskStreamPayload): PositionMetric[] {
  return metrics.map((metric) => {
    if (metric.key === "daily_pnl" && payload.daily_pnl) {
      return {
        ...metric,
        value: formatCurrency(payload.daily_pnl),
        tone: metricToneForPnl(payload.daily_pnl),
        hint: payload.updated_at ? formatClock(payload.updated_at) : metric.hint,
      };
    }

    if (metric.key === "net_exposure" && payload.net_exposure) {
      return {
        ...metric,
        value: formatPercentFromRatio(payload.net_exposure),
      };
    }

    return metric;
  });
}

export function PositionsWorkbench({ data }: { data: PositionsPageData }) {
  const [filter, setFilter] = useState<PositionFilter>("all");
  const [metrics, setMetrics] = useState(data.metrics);
  const [runtimeModeLabel, setRuntimeModeLabel] = useState(data.runtimeModeLabel);
  const [runtimeEnvironmentLabel, setRuntimeEnvironmentLabel] = useState(data.runtimeEnvironmentLabel);
  const [positionItems, setPositionItems] = useState(data.positions);
  const [selectedId, setSelectedId] = useState(data.selectedPositionId);
  const deferredFilter = useDeferredValue(filter);
  const { lastEvent: lastSignalEvent } = useConsoleRealtimeChannel("signals");
  const { lastEvent: lastRiskEvent } = useConsoleRealtimeChannel("risk");
  const { dictionary, enumLabel, format } = useI18n();

  useEffect(() => {
    const streamEvent = lastSignalEvent;

    if (!streamEvent) {
      return;
    }

    startTransition(() => {
      setPositionItems((currentItems) => patchPositionsFromSignal(currentItems, streamEvent.data, enumLabel));
    });
  }, [enumLabel, lastSignalEvent]);

  useEffect(() => {
    const streamEvent = lastRiskEvent;

    if (!streamEvent) {
      return;
    }

    startTransition(() => {
      setMetrics((currentMetrics) => patchMetrics(currentMetrics, streamEvent.data));

      if (streamEvent.data.mode) {
        setRuntimeModeLabel(enumLabel(streamEvent.data.mode));
      }

      if (streamEvent.data.environment) {
        setRuntimeEnvironmentLabel(streamEvent.data.environment);
      }
    });
  }, [enumLabel, lastRiskEvent]);

  const filteredPositions = positionItems.filter((position) => {
    if (deferredFilter === "gainers") {
      return position.pnlValue > 0;
    }

    if (deferredFilter === "pressure") {
      return position.bucketStatus !== "healthy" || position.pnlValue < 0;
    }

    return true;
  });

  const activeSelectedId =
    filteredPositions.find((position) => position.id === selectedId)?.id ??
    filteredPositions[0]?.id ??
    positionItems[0]?.id ??
    "";
  const selectedPosition =
    positionItems.find((position) => position.id === activeSelectedId) ?? positionItems[0];
  const pressureCount = positionItems.filter(
    (position) => position.bucketStatus !== "healthy" || position.pnlValue < 0,
  ).length;
  const filterButtons: Array<{ key: PositionFilter; label: string }> = [
    { key: "all", label: dictionary.positions.all },
    { key: "gainers", label: dictionary.positions.positivePnl },
    { key: "pressure", label: dictionary.positions.pressure },
  ];

  if (!selectedPosition) {
    return null;
  }

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
            <StatusPill tone="primary">{runtimeModeLabel}</StatusPill>
            <StatusPill tone="neutral">{runtimeEnvironmentLabel}</StatusPill>
          </>
        }
      />

      <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        {metrics.map((metric) => (
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
              <StatusPill tone="success">{format(dictionary.positions.open, { count: positionItems.length })}</StatusPill>
              <StatusPill tone="warning">{format(dictionary.positions.pressureCount, { count: pressureCount })}</StatusPill>
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
                  {filteredPositions.map((position) => (
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
                          ? "cursor-pointer bg-accent/60 shadow-[inset_2px_0_0_#0066ff]"
                          : "cursor-pointer transition-colors hover:bg-accent/35"
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
          </CardContent>
        </Card>

        <div className="space-y-4">
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
                <span>{format(dictionary.positions.utilized, { value: selectedPosition.bucketUtilization })}</span>
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
