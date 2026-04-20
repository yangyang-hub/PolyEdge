"use client";

import { startTransition, useDeferredValue, useEffect, useState } from "react";
import { ChevronRight, Filter } from "lucide-react";

import { PageHeader } from "@/components/shared/page-header";
import { useConsoleRealtimeChannel } from "@/components/shared/console-realtime-provider";
import { Button } from "@/components/ui/button";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from "@/components/ui/sheet";
import { MeterBar } from "@/components/shared/meter-bar";
import { StatusPill } from "@/components/shared/status-pill";
import { WorkbenchDetailPane, WorkbenchLayout } from "@/components/shared/workbench-layout";
import { WorkbenchSegmentedControl } from "@/components/shared/workbench-segmented-control";
import type { RiskStreamPayload, SignalStreamPayload } from "@/lib/contracts/realtime";
import { isKeyboardSelect } from "@/lib/keyboard";
import {
  formatPercentFromRatio,
  formatSignedFixed,
  humanizeSnakeCase,
  signalStateTone,
  type RealtimeTone,
  uppercaseEnum,
} from "@/lib/realtime-formatters";

type SignalTone = RealtimeTone;

type SignalItem = {
  id: string;
  marketQuestion: string;
  contextLabel: string;
  confidenceValue: number;
  side: string;
  fairPrice: string;
  marketPrice: string;
  edge: string;
  confidence: string;
  confidenceWidth: string;
  stateLabel: string;
  stateTone: SignalTone;
  requiresReview: boolean;
  reason: string;
  riskDecision: string;
  evidenceLines: string[];
  isSelected: boolean;
};

type SelectedSignal = {
  marketQuestion: string;
  confidence: string;
  marketPrice: string;
  fairPrice: string;
  edge: string;
  stateLabel: string;
  stateTone: SignalTone;
  requiresReview: boolean;
  reason: string;
  riskDecision: string;
  evidenceLines: string[];
};

type SignalsWorkbenchProps = {
  activeCount: number;
  approvalCount: number;
  signals: SignalItem[];
  selectedSignal: SelectedSignal;
};

type SignalFilter = "all" | "high_confidence" | "needs_review";

function buildSignalItem(payload: SignalStreamPayload, current?: SignalItem): SignalItem {
  const confidenceValue = payload.confidence
    ? Number.parseFloat(payload.confidence)
    : current?.confidenceValue ?? 0;

  return {
    id: payload.signal_id,
    marketQuestion: payload.market_question ?? current?.marketQuestion ?? payload.market_id,
    contextLabel: payload.context_label ?? current?.contextLabel ?? "Live stream / pending enrichment",
    confidenceValue,
    side: payload.side ? uppercaseEnum(payload.side) : current?.side ?? "YES",
    fairPrice: payload.fair_price ?? current?.fairPrice ?? "0.00",
    marketPrice: payload.market_price ?? current?.marketPrice ?? "0.00",
    edge: payload.edge ? formatSignedFixed(payload.edge) : current?.edge ?? "0.00",
    confidence: payload.confidence ? formatPercentFromRatio(payload.confidence) : current?.confidence ?? "0%",
    confidenceWidth: payload.confidence
      ? formatPercentFromRatio(payload.confidence)
      : current?.confidenceWidth ?? "0%",
    stateLabel: humanizeSnakeCase(payload.lifecycle_state),
    stateTone: signalStateTone(payload.lifecycle_state),
    requiresReview: payload.requires_review ?? current?.requiresReview ?? false,
    reason: payload.reason ?? current?.reason ?? "Awaiting realtime hydration.",
    riskDecision: payload.risk_decision ?? current?.riskDecision ?? "Decision update pending.",
    evidenceLines: payload.evidence_lines ?? current?.evidenceLines ?? [],
    isSelected: current?.isSelected ?? false,
  };
}

function upsertSignal(signals: SignalItem[], payload: SignalStreamPayload, eventType: string): SignalItem[] {
  const current = signals.find((signal) => signal.id === payload.signal_id);
  const nextSignal = buildSignalItem(payload, current);

  if (current) {
    return signals.map((signal) => (signal.id === payload.signal_id ? nextSignal : signal));
  }

  if (eventType === "signal.created") {
    return [nextSignal, ...signals];
  }

  return [...signals, nextSignal];
}

function patchSignalApprovalStatus(signals: SignalItem[], payload: RiskStreamPayload): SignalItem[] {
  if (
    payload.approval_type !== "signal" ||
    !payload.approval_resource_id ||
    !payload.approval_status
  ) {
    return signals;
  }

  return signals.map((signal) =>
    signal.id === payload.approval_resource_id
      ? {
          ...signal,
          requiresReview: payload.approval_status === "pending",
        }
      : signal,
  );
}

function SignalsDetailPanel({
  signal,
}: {
  signal: SignalItem | SelectedSignal;
}) {
  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <p className="font-heading text-lg font-bold tracking-tight text-foreground">
          {signal.marketQuestion}
        </p>
        <div className="flex flex-wrap gap-2">
          <StatusPill tone={signal.stateTone}>{signal.stateLabel}</StatusPill>
          <StatusPill tone="primary">{signal.confidence}</StatusPill>
          {signal.requiresReview ? <StatusPill tone="violet">manual review</StatusPill> : null}
        </div>
      </div>

      <div className="grid grid-cols-3 gap-3">
        <div className="rounded-md bg-accent/45 p-3">
          <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">market</p>
          <p className="mt-2 font-mono text-lg text-foreground">{signal.marketPrice}</p>
        </div>
        <div className="rounded-md bg-accent/45 p-3">
          <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">posterior</p>
          <p className="mt-2 font-mono text-lg text-primary">{signal.fairPrice}</p>
        </div>
        <div className="rounded-md bg-accent/45 p-3">
          <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">edge</p>
          <p className="mt-2 font-mono text-lg text-foreground">{signal.edge}</p>
        </div>
      </div>

      <div className="rounded-md bg-popover/70 p-4">
        <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
          Reason Trace
        </p>
        <p className="mt-3 text-sm text-foreground">{signal.reason}</p>
      </div>

      <div className="rounded-md bg-popover/70 p-4">
        <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
          Risk Decision
        </p>
        <p className="mt-3 text-sm text-muted-foreground">{signal.riskDecision}</p>
      </div>

      <div className="rounded-md bg-popover/70 p-4">
        <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
          Evidence Stack
        </p>
        <ul className="mt-3 space-y-3">
          {signal.evidenceLines.map((line, index) => (
            <li key={line} className="space-y-2">
              <p className="text-sm text-foreground">{line}</p>
              <MeterBar
                value={`${Math.max(30, 85 - index * 18)}%`}
                tone={index === 0 ? "success" : index === 1 ? "warning" : "primary"}
                trackClassName="h-1 bg-background"
              />
            </li>
          ))}
        </ul>
      </div>

      <div className="flex gap-2">
        <Button className="flex-1 rounded-sm bg-primary text-primary-foreground hover:bg-primary/90">
          Approve signal
        </Button>
        <Button
          variant="outline"
          className="flex-1 rounded-sm border-destructive/30 bg-destructive/5 text-destructive hover:bg-destructive/10"
        >
          Reject
        </Button>
      </div>
    </div>
  );
}

export function SignalsWorkbench({
  signals,
  selectedSignal: initialSelectedSignal,
}: SignalsWorkbenchProps) {
  const [filter, setFilter] = useState<SignalFilter>("all");
  const [liveSignals, setLiveSignals] = useState(signals);
  const [selectedId, setSelectedId] = useState<string>(
    signals.find((signal) => signal.isSelected)?.id ?? signals[0]?.id ?? "",
  );
  const deferredFilter = useDeferredValue(filter);
  const { lastEvent } = useConsoleRealtimeChannel("signals");
  const { lastEvent: lastRiskEvent } = useConsoleRealtimeChannel("risk");

  useEffect(() => {
    const streamEvent = lastEvent;

    if (!streamEvent) {
      return;
    }

    startTransition(() => {
      setLiveSignals((currentSignals) => upsertSignal(currentSignals, streamEvent.data, streamEvent.type));
    });
  }, [lastEvent]);

  useEffect(() => {
    const streamEvent = lastRiskEvent;

    if (!streamEvent || !streamEvent.type.startsWith("approval.")) {
      return;
    }

    startTransition(() => {
      setLiveSignals((currentSignals) => patchSignalApprovalStatus(currentSignals, streamEvent.data));
    });
  }, [lastRiskEvent]);

  const filteredSignals = liveSignals.filter((signal) => {
    if (deferredFilter === "high_confidence") {
      return signal.confidenceValue >= 0.7;
    }

    if (deferredFilter === "needs_review") {
      return signal.requiresReview;
    }

    return true;
  });

  const selectedSignal =
    filteredSignals.find((signal) => signal.id === selectedId) ??
    liveSignals.find((signal) => signal.id === selectedId) ??
    filteredSignals[0] ??
    liveSignals[0];

  const activeCount = liveSignals.filter((signal) => signal.stateLabel === "active").length;
  const approvalCount = liveSignals.filter((signal) => signal.requiresReview).length;

  const filterButtons: Array<{ key: SignalFilter; label: string }> = [
    { key: "all", label: "all" },
    { key: "high_confidence", label: "high confidence" },
    { key: "needs_review", label: "manual review" },
  ];

  function selectSignal(signalId: string) {
    startTransition(() => {
      setSelectedId(signalId);
    });
  }

  return (
    <div className="space-y-4">
      <PageHeader
        eyebrow="Decisioning"
        title="Signals"
        description="Inspect posterior, edge and confidence together with approval state and risk reasoning."
        className="border-none pb-0"
        actions={
          <>
            <StatusPill tone="success">{activeCount} active</StatusPill>
            <StatusPill tone="violet">{approvalCount} pending approval</StatusPill>
          </>
        }
      />

      <WorkbenchLayout columnsClassName="xl:grid-cols-[1.6fr_0.95fr]">
        <div className="overflow-hidden rounded-lg bg-card/95 ring-1 ring-white/5">
          <div className="flex flex-col gap-4 bg-popover/70 px-5 py-4 xl:flex-row xl:items-center xl:justify-between">
            <div className="flex items-center gap-3">
              <h2 className="font-heading text-xl font-bold tracking-tight text-foreground">Live Signals</h2>
              <div className="flex flex-wrap gap-2">
                <StatusPill tone="success">{activeCount} active</StatusPill>
                <StatusPill tone="violet">{approvalCount} approval req</StatusPill>
              </div>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <WorkbenchSegmentedControl items={filterButtons} value={filter} onChange={setFilter} />
              <Button
                variant="outline"
                size="sm"
                className="rounded-sm border-white/10 bg-accent/40 text-foreground hover:bg-accent"
              >
                <Filter className="size-3.5" />
                Filter
              </Button>
            </div>
          </div>

          {filteredSignals.length > 0 ? (
            <div className="overflow-x-auto">
              <table className="w-full text-left">
                <thead className="bg-sidebar/60">
                  <tr className="text-[10px] font-bold uppercase tracking-[0.2em] text-muted-foreground">
                    <th className="px-5 py-3">Market</th>
                    <th className="px-4 py-3">Side</th>
                    <th className="px-4 py-3">Fair</th>
                    <th className="px-4 py-3">Market</th>
                    <th className="px-4 py-3 text-right">Edge</th>
                    <th className="px-4 py-3">Confidence</th>
                    <th className="px-4 py-3">State</th>
                    <th className="px-5 py-3 text-right">Action</th>
                  </tr>
                </thead>
                <tbody className="text-sm">
                  {filteredSignals.map((signal) => (
                    <tr
                      key={signal.id}
                      tabIndex={0}
                      onClick={() => selectSignal(signal.id)}
                      onKeyDown={(event) => {
                        if (isKeyboardSelect(event)) {
                          event.preventDefault();
                          selectSignal(signal.id);
                        }
                      }}
                      className={
                        signal.id === selectedSignal?.id
                          ? "cursor-pointer bg-accent/45 shadow-[inset_2px_0_0_#0066ff]"
                          : "cursor-pointer transition-colors hover:bg-accent/35"
                      }
                    >
                      <td className="px-5 py-3">
                        <div className="space-y-1">
                          <p className="font-semibold text-foreground">{signal.marketQuestion}</p>
                          <p className="text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
                            {signal.contextLabel}
                          </p>
                        </div>
                      </td>
                      <td className="px-4 py-3">
                        <span
                          className={
                            signal.side === "YES"
                              ? "font-bold uppercase tracking-wide text-secondary"
                              : "font-bold uppercase tracking-wide text-destructive"
                          }
                        >
                          {signal.side}
                        </span>
                      </td>
                      <td className="px-4 py-3 font-mono text-primary">{signal.fairPrice}</td>
                      <td className="px-4 py-3 font-mono text-foreground">{signal.marketPrice}</td>
                      <td className="px-4 py-3 text-right font-mono">{signal.edge}</td>
                      <td className="px-4 py-3">
                        <div className="w-20 space-y-1">
                          <MeterBar
                            value={signal.confidenceWidth}
                            tone={signal.stateTone === "success" ? "success" : signal.stateTone}
                            trackClassName="h-1 bg-background"
                          />
                          <span className="block text-[10px] text-muted-foreground">{signal.confidence}</span>
                        </div>
                      </td>
                      <td className="px-4 py-3">
                        <div className="flex flex-wrap gap-2">
                          <StatusPill tone={signal.stateTone}>{signal.stateLabel}</StatusPill>
                          {signal.requiresReview ? <StatusPill tone="violet">manual review</StatusPill> : null}
                        </div>
                      </td>
                      <td className="px-5 py-3 text-right">
                        <div className="hidden xl:block">
                          <button className="rounded-sm p-1 text-primary transition-colors hover:bg-primary/10">
                            <ChevronRight className="ml-auto size-4" />
                          </button>
                        </div>
                        <div className="xl:hidden">
                          <Sheet>
                            <SheetTrigger asChild>
                              <Button
                                variant="ghost"
                                size="icon-sm"
                                className="rounded-sm text-primary hover:bg-primary/10"
                                onClick={() => selectSignal(signal.id)}
                              >
                                <ChevronRight className="size-4" />
                              </Button>
                            </SheetTrigger>
                            <SheetContent className="w-full max-w-none border-white/10 bg-card p-0 sm:max-w-md">
                              <SheetHeader className="border-b border-white/8 px-5 py-4">
                                <SheetTitle>Signal Detail</SheetTitle>
                                <SheetDescription>
                                  Posterior, evidence stack and risk decision.
                                </SheetDescription>
                              </SheetHeader>
                              <div className="overflow-y-auto px-5 py-5">
                                <SignalsDetailPanel signal={signal} />
                              </div>
                            </SheetContent>
                          </Sheet>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : (
            <div className="px-5 py-10 text-center">
              <p className="font-heading text-lg font-bold text-foreground">No signals match this filter</p>
              <p className="mt-2 text-sm text-muted-foreground">
                Adjust the filter or wait for the next strategy refresh.
              </p>
            </div>
          )}
        </div>

        <WorkbenchDetailPane desktopOnly>
          <SignalsDetailPanel signal={selectedSignal ?? initialSelectedSignal} />
        </WorkbenchDetailPane>
      </WorkbenchLayout>
    </div>
  );
}
