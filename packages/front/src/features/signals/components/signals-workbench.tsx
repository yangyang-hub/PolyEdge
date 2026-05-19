"use client";

import { startTransition, useDeferredValue, useEffect, useState, useTransition } from "react";
import { Check, ChevronRight, Filter, Send, X } from "lucide-react";
import { toast } from "sonner";

import { PageHeader } from "@/components/shared/page-header";
import { ActionDialog } from "@/components/shared/action-dialog";
import { useConsoleRealtimeChannel } from "@/components/shared/console-realtime-provider";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from "@/components/ui/sheet";
import { MeterBar } from "@/components/shared/meter-bar";
import { OperationFeedbackBanner } from "@/components/shared/operation-feedback-banner";
import { StatusPill } from "@/components/shared/status-pill";
import { WorkbenchDetailPane, WorkbenchLayout } from "@/components/shared/workbench-layout";
import { WorkbenchSegmentedControl } from "@/components/shared/workbench-segmented-control";
import { useI18n } from "@/lib/i18n/client";
import { localizeGeneratedCopy } from "@/lib/i18n/generated-copy";
import type { SignalStreamPayload } from "@/lib/contracts/realtime";
import { isKeyboardSelect } from "@/lib/keyboard";
import {
  formatPercentFromRatio,
  formatSignedFixed,
  signalStateTone,
  type RealtimeTone,
  uppercaseEnum,
} from "@/lib/realtime-formatters";
import { patchApprovalField, upsertStreamedItem } from "@/lib/signal-stream-utils";
import {
  submitSignalDecisionAction,
  submitSignalExecutionAction,
} from "@/server/actions/signal-actions";
import type { OperationActionResult } from "@/server/actions/action-result";
import type { RuntimeMode, SignalLifecycleState } from "@/lib/contracts/dto";

type SignalTone = RealtimeTone;

type SignalItem = {
  id: string;
  version: number;
  lifecycleState: SignalLifecycleState;
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
  approvedAt: string | null;
  rejectedAt: string | null;
  reason: string;
  riskDecision: string;
  evidenceLines: string[];
  isSelected: boolean;
};

type SelectedSignal = {
  id: string;
  version: number;
  lifecycleState: SignalLifecycleState;
  marketQuestion: string;
  confidence: string;
  marketPrice: string;
  fairPrice: string;
  edge: string;
  stateLabel: string;
  stateTone: SignalTone;
  requiresReview: boolean;
  approvedAt: string | null;
  rejectedAt: string | null;
  reason: string;
  riskDecision: string;
  evidenceLines: string[];
};

type SignalsWorkbenchProps = {
  activeCount: number;
  approvalCount: number;
  runtimeControls: RuntimeControls;
  signals: SignalItem[];
  selectedSignal: SelectedSignal;
};

type RuntimeControls = {
  mode: RuntimeMode;
  modeLabel: string;
  killSwitch: boolean;
};

type SignalFilter = "all" | "high_confidence" | "needs_review";
type SignalActionDialog = "approved" | "rejected" | "execution" | null;

function buildSignalItem(
  payload: SignalStreamPayload,
  current: SignalItem | undefined,
  dictionary: ReturnType<typeof useI18n>["dictionary"],
  enumLabel: (value: string) => string,
): SignalItem {
  const confidenceValue = payload.confidence
    ? Number.parseFloat(payload.confidence)
    : current?.confidenceValue ?? 0;

  return {
    id: payload.signal_id,
    version: payload.version,
    lifecycleState: payload.lifecycle_state,
    marketQuestion: payload.market_question ?? current?.marketQuestion ?? payload.market_id,
    contextLabel: payload.context_label ?? current?.contextLabel ?? dictionary.signals.liveContextFallback,
    confidenceValue,
    side: payload.side ? uppercaseEnum(payload.side) : current?.side ?? "YES",
    fairPrice: payload.fair_price ?? current?.fairPrice ?? "0.00",
    marketPrice: payload.market_price ?? current?.marketPrice ?? "0.00",
    edge: payload.edge ? formatSignedFixed(payload.edge) : current?.edge ?? "0.00",
    confidence: payload.confidence ? formatPercentFromRatio(payload.confidence) : current?.confidence ?? "0%",
    confidenceWidth: payload.confidence
      ? formatPercentFromRatio(payload.confidence)
      : current?.confidenceWidth ?? "0%",
    stateLabel: enumLabel(payload.lifecycle_state),
    stateTone: signalStateTone(payload.lifecycle_state),
    requiresReview: payload.requires_review ?? current?.requiresReview ?? false,
    approvedAt: current?.approvedAt ?? null,
    rejectedAt: current?.rejectedAt ?? null,
    reason: payload.reason ?? current?.reason ?? dictionary.signals.reasonFallback,
    riskDecision: payload.risk_decision ?? current?.riskDecision ?? dictionary.signals.riskFallback,
    evidenceLines: payload.evidence_lines ?? current?.evidenceLines ?? [],
    isSelected: current?.isSelected ?? false,
  };
}

function hasExecutableLifecycle(signal: SignalItem | SelectedSignal): boolean {
  return signal.lifecycleState === "new" || signal.lifecycleState === "active";
}

function canApproveSignal(signal: SignalItem | SelectedSignal, controls: RuntimeControls): boolean {
  return (
    controls.mode === "manual_confirm" &&
    !controls.killSwitch &&
    !signal.approvedAt &&
    !signal.rejectedAt &&
    hasExecutableLifecycle(signal)
  );
}

function canRejectSignal(signal: SignalItem | SelectedSignal, controls: RuntimeControls): boolean {
  return (
    (controls.mode === "manual_confirm" || controls.mode === "kill_switch_locked") &&
    !signal.approvedAt &&
    !signal.rejectedAt &&
    (signal.lifecycleState === "new" ||
      signal.lifecycleState === "active" ||
      signal.lifecycleState === "weakened")
  );
}

function canSubmitExecution(signal: SignalItem | SelectedSignal, controls: RuntimeControls): boolean {
  if (controls.killSwitch || signal.rejectedAt || !hasExecutableLifecycle(signal)) {
    return false;
  }

  if (controls.mode === "paper_trade") {
    return true;
  }

  return controls.mode === "manual_confirm" && Boolean(signal.approvedAt);
}

function SignalsDetailPanel({
  signal,
  runtimeControls,
  onOpenAction,
}: {
  signal: SignalItem | SelectedSignal;
  runtimeControls: RuntimeControls;
  onOpenAction?: (signalId: string, dialog: Exclude<SignalActionDialog, null>) => void;
}) {
  const { dictionary } = useI18n();
  const approveEnabled = canApproveSignal(signal, runtimeControls);
  const rejectEnabled = canRejectSignal(signal, runtimeControls);
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
          {signal.requiresReview ? <StatusPill tone="violet">{dictionary.signals.manualReview}</StatusPill> : null}
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

      <div className="grid gap-2 sm:grid-cols-3">
        <Button
          className="rounded-sm bg-primary text-primary-foreground hover:bg-primary/90"
          disabled={!approveEnabled || !onOpenAction}
          onClick={() => onOpenAction?.(signal.id, "approved")}
        >
          <Check className="size-3.5" />
          {dictionary.signals.approveSignal}
        </Button>
        <Button
          variant="outline"
          className="rounded-sm border-destructive/30 bg-destructive/5 text-destructive hover:bg-destructive/10"
          disabled={!rejectEnabled || !onOpenAction}
          onClick={() => onOpenAction?.(signal.id, "rejected")}
        >
          <X className="size-3.5" />
          {dictionary.signals.reject}
        </Button>
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

export function SignalsWorkbench({
  signals,
  selectedSignal: initialSelectedSignal,
  runtimeControls: initialRuntimeControls,
}: SignalsWorkbenchProps) {
  const [filter, setFilter] = useState<SignalFilter>("all");
  const [liveSignals, setLiveSignals] = useState(signals);
  const [runtimeControls, setRuntimeControls] = useState(initialRuntimeControls);
  const [selectedId, setSelectedId] = useState<string>(
    signals.find((signal) => signal.isSelected)?.id ?? signals[0]?.id ?? "",
  );
  const [activeDialog, setActiveDialog] = useState<SignalActionDialog>(null);
  const [actionSignalId, setActionSignalId] = useState<string>("");
  const [note, setNote] = useState("");
  const [stepUpCode, setStepUpCode] = useState("");
  const [limitPrice, setLimitPrice] = useState("");
  const [quantity, setQuantity] = useState("1");
  const [connectorName, setConnectorName] = useState("paper_executor");
  const [dialogFeedback, setDialogFeedback] = useState<OperationActionResult | null>(null);
  const [lastOperation, setLastOperation] = useState<OperationActionResult | null>(null);
  const [fieldErrors, setFieldErrors] = useState<OperationActionResult["fieldErrors"]>({});
  const [isPending, startActionTransition] = useTransition();
  const deferredFilter = useDeferredValue(filter);
  const { lastEvent } = useConsoleRealtimeChannel("signals");
  const { lastEvent: lastRiskEvent } = useConsoleRealtimeChannel("risk");
  const { locale, dictionary, enumLabel, format } = useI18n();

  useEffect(() => {
    const streamEvent = lastEvent;

    if (!streamEvent) {
      return;
    }

    startTransition(() => {
      setLiveSignals((currentSignals) =>
        upsertStreamedItem(
          currentSignals,
          streamEvent.data,
          (payload, currentSignal) =>
            buildSignalItem(
              {
                ...payload,
                context_label: payload.context_label
                  ? localizeGeneratedCopy(locale, dictionary, payload.context_label)
                  : payload.context_label,
                reason: payload.reason
                  ? localizeGeneratedCopy(locale, dictionary, payload.reason)
                  : payload.reason,
                risk_decision: payload.risk_decision
                  ? localizeGeneratedCopy(locale, dictionary, payload.risk_decision)
                  : payload.risk_decision,
                evidence_lines: payload.evidence_lines?.map((line) =>
                  localizeGeneratedCopy(locale, dictionary, line),
                ),
              },
              currentSignal,
              dictionary,
              enumLabel,
            ),
          streamEvent.type,
        ),
      );
    });
  }, [dictionary, enumLabel, lastEvent, locale]);

  useEffect(() => {
    const streamEvent = lastRiskEvent;

    if (!streamEvent) {
      return;
    }

    startTransition(() => {
      if (streamEvent.data.mode || typeof streamEvent.data.kill_switch === "boolean") {
        setRuntimeControls((currentControls) => ({
          mode: streamEvent.data.mode ?? currentControls.mode,
          modeLabel: streamEvent.data.mode ? enumLabel(streamEvent.data.mode) : currentControls.modeLabel,
          killSwitch: streamEvent.data.kill_switch ?? currentControls.killSwitch,
        }));
      }

      if (streamEvent.type.startsWith("approval.")) {
        setLiveSignals((currentSignals) => patchApprovalField(currentSignals, streamEvent.data, "requiresReview"));
      }
    });
  }, [enumLabel, lastRiskEvent]);

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
  const actionSignal =
    liveSignals.find((signal) => signal.id === actionSignalId) ??
    selectedSignal ??
    initialSelectedSignal;

  const activeCount = liveSignals.filter((signal) => signal.stateTone === "success").length;
  const approvalCount = liveSignals.filter((signal) => signal.requiresReview).length;

  const filterButtons: Array<{ key: SignalFilter; label: string }> = [
    { key: "all", label: dictionary.signals.all },
    { key: "high_confidence", label: dictionary.signals.highConfidence },
    { key: "needs_review", label: dictionary.signals.manualReview },
  ];

  function selectSignal(signalId: string) {
    startTransition(() => {
      setSelectedId(signalId);
    });
  }

  function cycleFilter() {
    const currentIndex = filterButtons.findIndex((item) => item.key === filter);
    const nextFilter = filterButtons[(currentIndex + 1) % filterButtons.length]?.key ?? "all";
    setFilter(nextFilter);
  }

  function openSignalAction(signalId: string, dialog: Exclude<SignalActionDialog, null>) {
    const signal = liveSignals.find((item) => item.id === signalId) ?? selectedSignal;
    if (
      !signal ||
      (dialog === "approved" && !canApproveSignal(signal, runtimeControls)) ||
      (dialog === "rejected" && !canRejectSignal(signal, runtimeControls)) ||
      (dialog === "execution" && !canSubmitExecution(signal, runtimeControls))
    ) {
      return;
    }

    setSelectedId(signalId);
    setActionSignalId(signalId);
    setActiveDialog(dialog);
    setDialogFeedback(null);
    setFieldErrors({});
    setStepUpCode("");
    setLimitPrice(signal?.marketPrice ?? "");
    setQuantity("1");
    setConnectorName("paper_executor");
    setNote(
      dialog === "approved"
        ? dictionary.signals.approveNote
        : dialog === "rejected"
          ? dictionary.signals.rejectNote
          : dictionary.signals.executionNote,
    );
  }

  function closeSignalAction() {
    setActiveDialog(null);
    setActionSignalId("");
    setDialogFeedback(null);
    setFieldErrors({});
    setStepUpCode("");
  }

  function handleActionResult(result: OperationActionResult) {
    setDialogFeedback(result);
    setLastOperation(result);

    if (result.ok) {
      toast.success(result.message, {
        description: [result.requestId, result.traceId].filter(Boolean).join(" · "),
      });
      return;
    }

    toast.error(result.message, {
      description: [result.requestId, result.traceId].filter(Boolean).join(" · "),
    });
  }

  function patchSignalDecision(signalId: string, decision: "approved" | "rejected") {
    setLiveSignals((currentSignals) =>
      currentSignals.map((signal) => {
        if (signal.id !== signalId) {
          return signal;
        }

        return {
          ...signal,
          version: signal.version + 1,
          requiresReview: false,
          approvedAt: decision === "approved" ? new Date().toISOString() : null,
          rejectedAt: decision === "rejected" ? new Date().toISOString() : null,
          riskDecision: decision === "approved" ? dictionary.signals.approveNote : dictionary.signals.rejectNote,
        };
      }),
    );
  }

  function submitSignalDecision(dialog: "approved" | "rejected") {
    if (
      !actionSignal ||
      (dialog === "approved" && !canApproveSignal(actionSignal, runtimeControls)) ||
      (dialog === "rejected" && !canRejectSignal(actionSignal, runtimeControls))
    ) {
      return;
    }

    startActionTransition(async () => {
      const result = await submitSignalDecisionAction({
        signalId: actionSignal.id,
        expectedVersion: actionSignal.version,
        decision: dialog,
        note,
        stepUpCode,
      });

      setFieldErrors(result.fieldErrors ?? {});
      handleActionResult(result);

      if (result.ok) {
        patchSignalDecision(actionSignal.id, dialog);
        closeSignalAction();
      }
    });
  }

  function submitExecutionRequest() {
    if (!actionSignal || !canSubmitExecution(actionSignal, runtimeControls)) {
      return;
    }

    startActionTransition(async () => {
      const result = await submitSignalExecutionAction({
        signalId: actionSignal.id,
        expectedVersion: actionSignal.version,
        limitPrice,
        quantity,
        connectorName,
        note,
        stepUpCode,
      });

      setFieldErrors(result.fieldErrors ?? {});
      handleActionResult(result);

      if (result.ok) {
        closeSignalAction();
      }
    });
  }

  return (
    <div className="space-y-4">
      <PageHeader
        eyebrow={dictionary.signals.eyebrow}
        title={dictionary.signals.title}
        description={dictionary.signals.description}
        className="border-none pb-0"
        actions={
          <>
            <StatusPill tone="success">{format(dictionary.signals.active, { count: activeCount })}</StatusPill>
            <StatusPill tone="violet">{format(dictionary.signals.pendingApproval, { count: approvalCount })}</StatusPill>
          </>
        }
      />
      {lastOperation ? <OperationFeedbackBanner feedback={lastOperation} /> : null}

      <WorkbenchLayout columnsClassName="xl:grid-cols-[1.6fr_0.95fr]">
        <div className="overflow-hidden rounded-lg bg-card/95 ring-1 ring-white/5">
          <div className="flex flex-col gap-4 bg-popover/70 px-5 py-4 xl:flex-row xl:items-center xl:justify-between">
            <div className="flex items-center gap-3">
              <h2 className="font-heading text-xl font-bold tracking-tight text-foreground">{dictionary.signals.liveSignals}</h2>
              <div className="flex flex-wrap gap-2">
                <StatusPill tone="success">{format(dictionary.signals.active, { count: activeCount })}</StatusPill>
                <StatusPill tone="violet">{format(dictionary.signals.approvalReq, { count: approvalCount })}</StatusPill>
              </div>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <WorkbenchSegmentedControl items={filterButtons} value={filter} onChange={setFilter} />
              <Button
                variant="outline"
                size="sm"
                className="rounded-sm border-white/10 bg-accent/40 text-foreground hover:bg-accent"
                onClick={cycleFilter}
              >
                <Filter className="size-3.5" />
                {dictionary.common.filter}
              </Button>
            </div>
          </div>

          {filteredSignals.length > 0 ? (
            <div className="overflow-x-auto">
              <table className="w-full text-left">
                <thead className="bg-sidebar/60">
                  <tr className="text-[10px] font-bold uppercase tracking-[0.2em] text-muted-foreground">
                    <th className="px-5 py-3">{dictionary.signals.market}</th>
                    <th className="px-4 py-3">{dictionary.signals.side}</th>
                    <th className="px-4 py-3">{dictionary.signals.fair}</th>
                    <th className="px-4 py-3">{dictionary.signals.marketPrice}</th>
                    <th className="px-4 py-3 text-right">{dictionary.signals.edge}</th>
                    <th className="px-4 py-3">{dictionary.dashboard.tableConfidence}</th>
                    <th className="px-4 py-3">{dictionary.dashboard.tableState}</th>
                    <th className="px-5 py-3 text-right">{dictionary.signals.action}</th>
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
                          {signal.requiresReview ? <StatusPill tone="violet">{dictionary.signals.manualReview}</StatusPill> : null}
                        </div>
                      </td>
                      <td className="px-5 py-3 text-right">
                        <div className="hidden xl:block">
                          <button
                            type="button"
                            className="rounded-sm p-1 text-primary transition-colors hover:bg-primary/10"
                            onClick={(event) => {
                              event.stopPropagation();
                              selectSignal(signal.id);
                            }}
                          >
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
                                <SheetTitle>{dictionary.signals.signalDetail}</SheetTitle>
                                <SheetDescription>
                                  {dictionary.signals.signalDetailDescription}
                                </SheetDescription>
                              </SheetHeader>
                              <div className="overflow-y-auto px-5 py-5">
                                <SignalsDetailPanel
                                  signal={signal}
                                  runtimeControls={runtimeControls}
                                  onOpenAction={openSignalAction}
                                />
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
              <p className="font-heading text-lg font-bold text-foreground">{dictionary.signals.noFilterTitle}</p>
              <p className="mt-2 text-sm text-muted-foreground">
                {dictionary.signals.noFilterDetail}
              </p>
            </div>
          )}
        </div>

        <WorkbenchDetailPane desktopOnly>
          <SignalsDetailPanel
            signal={selectedSignal ?? initialSelectedSignal}
            runtimeControls={runtimeControls}
            onOpenAction={openSignalAction}
          />
        </WorkbenchDetailPane>

        <ActionDialog
          open={activeDialog === "approved" || activeDialog === "rejected"}
          onOpenChange={(open) => {
            if (!open) {
              closeSignalAction();
            }
          }}
          title={activeDialog === "approved" ? dictionary.signals.approveTitle : dictionary.signals.rejectTitle}
          description={dictionary.signals.decisionDescription}
          confirmLabel={activeDialog === "approved" ? dictionary.signals.queueApproval : dictionary.signals.queueRejection}
          confirmVariant={activeDialog === "rejected" ? "destructive" : "default"}
          isPending={isPending}
          note={note}
          onNoteChange={setNote}
          noteError={fieldErrors?.note}
          stepUpCode={stepUpCode}
          onStepUpCodeChange={setStepUpCode}
          stepUpCodeError={fieldErrors?.stepUpCode}
          requiresStepUp
          confirmDisabled={
            !actionSignal ||
            (activeDialog === "approved" && !canApproveSignal(actionSignal, runtimeControls)) ||
            (activeDialog === "rejected" && !canRejectSignal(actionSignal, runtimeControls))
          }
          onSubmit={() => {
            if (activeDialog === "approved" || activeDialog === "rejected") {
              submitSignalDecision(activeDialog);
            }
          }}
          feedback={dialogFeedback}
          context={
            actionSignal ? (
              <div className="space-y-1">
                <p>{dictionary.signals.market}: {actionSignal.marketQuestion}</p>
                <p>{dictionary.approvals.expectedVersion}: {actionSignal.version}</p>
                <p>{dictionary.dashboard.tableState}: {actionSignal.stateLabel}</p>
                <p>{dictionary.metrics.mode}: {runtimeControls.modeLabel}</p>
              </div>
            ) : null
          }
        />

        <ActionDialog
          open={activeDialog === "execution"}
          onOpenChange={(open) => {
            if (!open) {
              closeSignalAction();
            }
          }}
          title={dictionary.signals.executeTitle}
          description={dictionary.signals.executeDescription}
          confirmLabel={dictionary.signals.queueExecution}
          isPending={isPending}
          note={note}
          onNoteChange={setNote}
          noteError={fieldErrors?.note}
          stepUpCode={stepUpCode}
          onStepUpCodeChange={setStepUpCode}
          stepUpCodeError={fieldErrors?.stepUpCode}
          requiresStepUp
          confirmDisabled={!actionSignal || !canSubmitExecution(actionSignal, runtimeControls)}
          onSubmit={submitExecutionRequest}
          feedback={dialogFeedback}
          context={
            actionSignal ? (
              <div className="space-y-1">
                <p>{dictionary.signals.market}: {actionSignal.marketQuestion}</p>
                <p>{dictionary.approvals.expectedVersion}: {actionSignal.version}</p>
                <p>{dictionary.signals.marketPrice}: {actionSignal.marketPrice}</p>
                <p>{dictionary.metrics.mode}: {runtimeControls.modeLabel}</p>
              </div>
            ) : null
          }
        >
          <div className="grid gap-3 sm:grid-cols-3">
            <div className="space-y-2">
              <label className="text-sm font-medium text-foreground" htmlFor="signal-limit-price">
                {dictionary.signals.limitPrice}
              </label>
              <Input
                id="signal-limit-price"
                value={limitPrice}
                inputMode="decimal"
                onChange={(event) => setLimitPrice(event.target.value)}
                className="h-10 rounded-sm border-white/10 bg-accent/45"
              />
              {fieldErrors?.limitPrice ? <p className="text-xs text-destructive">{fieldErrors.limitPrice}</p> : null}
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium text-foreground" htmlFor="signal-quantity">
                {dictionary.signals.quantity}
              </label>
              <Input
                id="signal-quantity"
                value={quantity}
                inputMode="decimal"
                onChange={(event) => setQuantity(event.target.value)}
                className="h-10 rounded-sm border-white/10 bg-accent/45"
              />
              {fieldErrors?.quantity ? <p className="text-xs text-destructive">{fieldErrors.quantity}</p> : null}
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium text-foreground" htmlFor="signal-connector">
                {dictionary.signals.connectorName}
              </label>
              <Input
                id="signal-connector"
                value={connectorName}
                onChange={(event) => setConnectorName(event.target.value)}
                className="h-10 rounded-sm border-white/10 bg-accent/45"
                placeholder={dictionary.signals.connectorPlaceholder}
              />
              {fieldErrors?.connectorName ? <p className="text-xs text-destructive">{fieldErrors.connectorName}</p> : null}
            </div>
          </div>
        </ActionDialog>
      </WorkbenchLayout>
    </div>
  );
}
