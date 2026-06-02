"use client";

import { Send } from "lucide-react";

import { MeterBar } from "@/components/shared/meter-bar";
import { StatusPill } from "@/components/shared/status-pill";
import { Button } from "@/components/ui/button";
import { dictionary } from "@/lib/i18n/dictionaries";

import { canSubmitExecution } from "../lib/signals-helpers";
import type { RuntimeControls, SelectedSignal, SignalActionDialog, SignalItem } from "../types";

export function SignalsDetailPanel({
  signal,
  runtimeControls,
  onOpenAction,
}: {
  signal: SignalItem | SelectedSignal;
  runtimeControls: RuntimeControls;
  onOpenAction?: (signalId: string, dialog: Exclude<SignalActionDialog, null>) => void;
}) {
  const executionEnabled = canSubmitExecution(signal, runtimeControls);

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <p className="font-heading text-lg font-bold tracking-tight text-foreground">
          {signal.marketQuestion}
        </p>
        <div className="flex flex-wrap gap-2">
          <StatusPill tone={signal.stateTone}>{signal.stateLabel}</StatusPill>
          <StatusPill tone="primary">{signal.confidence}</StatusPill>
          <StatusPill tone={runtimeControls.killSwitch ? "danger" : "warning"}>{runtimeControls.modeLabel}</StatusPill>
        </div>
      </div>

      <div className="grid grid-cols-3 gap-3">
        <div className="rounded-md bg-accent/45 p-3">
          <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
            {dictionary.signals.marketPrice}
          </p>
          <p className="mt-2 font-mono text-lg text-foreground">{signal.marketPrice}</p>
        </div>
        <div className="rounded-md bg-accent/45 p-3">
          <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
            {dictionary.signals.posterior}
          </p>
          <p className="mt-2 font-mono text-lg text-primary">{signal.fairPrice}</p>
        </div>
        <div className="rounded-md bg-accent/45 p-3">
          <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
            {dictionary.signals.edge}
          </p>
          <p className="mt-2 font-mono text-lg text-foreground">{signal.edge}</p>
        </div>
      </div>

      <div className="rounded-md bg-popover/70 p-4">
        <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
          {dictionary.signals.reasonTrace}
        </p>
        <p className="mt-3 text-sm text-foreground">{signal.reason}</p>
      </div>

      <div className="rounded-md bg-popover/70 p-4">
        <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
          {dictionary.signals.riskDecision}
        </p>
        <p className="mt-3 text-sm text-muted-foreground">{signal.riskDecision}</p>
      </div>

      <div className="rounded-md bg-popover/70 p-4">
        <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
          {dictionary.signals.evidenceStack}
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

      <div className="grid gap-2">
        <Button
          variant="outline"
          className="rounded-sm border-white/10 bg-accent/40 text-foreground hover:bg-accent"
          disabled={!executionEnabled || !onOpenAction}
          onClick={() => onOpenAction?.(signal.id, "execution")}
        >
          <Send className="size-3.5" />
          {dictionary.signals.submitExecution}
        </Button>
      </div>
    </div>
  );
}
