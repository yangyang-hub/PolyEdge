"use client";

import { startTransition, useDeferredValue, useState, useTransition } from "react";
import { Filter } from "lucide-react";
import { toast } from "sonner";

import { PageHeader } from "@/components/shared/page-header";
import { ActionDialog } from "@/components/shared/action-dialog";
import { EmptyPanel } from "@/components/shared/empty-panel";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { OperationFeedbackBanner } from "@/components/shared/operation-feedback-banner";
import { StatusPill } from "@/components/shared/status-pill";
import { WorkbenchDetailPane, WorkbenchLayout } from "@/components/shared/workbench-layout";
import { WorkbenchSegmentedControl } from "@/components/shared/workbench-segmented-control";
import { dictionary, formatMessage } from "@/lib/i18n/dictionaries";
import { submitSignalExecutionAction } from "@/lib/api/actions";
import type { OperationActionResult } from "@/lib/api/actions";

import { canSubmitExecution } from "@/features/signals/lib/signals-helpers";
import type {
  SignalActionDialog,
  SignalFilter,
  SignalsWorkbenchProps,
} from "@/features/signals/types";
import { SignalsDetailPanel } from "./signals-detail-panel";
import { SignalsTable } from "./signals-table";

export function SignalsWorkbench({
  signals,
  selectedSignal: initialSelectedSignal,
  runtimeControls: initialRuntimeControls,
}: SignalsWorkbenchProps) {
  const [filter, setFilter] = useState<SignalFilter>("all");
  const [liveSignals] = useState(signals);
  const [runtimeControls] = useState(initialRuntimeControls);
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

  const filteredSignals = liveSignals.filter((signal) => {
    if (deferredFilter === "high_confidence") {
      return signal.confidenceValue >= 0.7;
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
  const filterButtons: Array<{ key: SignalFilter; label: string }> = [
    { key: "all", label: dictionary.signals.all },
    { key: "high_confidence", label: dictionary.signals.highConfidence },
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
    if (!signal || (dialog === "execution" && !canSubmitExecution())) {
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
    setNote(dictionary.signals.executionNote);
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

  function submitExecutionRequest() {
    if (!actionSignal || !canSubmitExecution()) {
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
            <StatusPill tone="success">{formatMessage(dictionary.signals.active, { count: activeCount })}</StatusPill>
          </>
        }
      />
      {lastOperation ? <OperationFeedbackBanner feedback={lastOperation} /> : null}

      {liveSignals.length === 0 ? (
        <EmptyPanel
          title={dictionary.signals.noSignalsTitle}
          detail={dictionary.signals.noSignalsDetail}
        />
      ) : (
      <WorkbenchLayout columnsClassName="xl:grid-cols-[1.6fr_0.95fr]">
        <div className="overflow-hidden rounded-lg bg-card/95 ring-1 ring-white/5">
          <div className="flex flex-col gap-4 bg-popover/70 px-5 py-4 xl:flex-row xl:items-center xl:justify-between">
            <div className="flex items-center gap-3">
              <h2 className="font-heading text-xl font-bold tracking-tight text-foreground">{dictionary.signals.liveSignals}</h2>
              <div className="flex flex-wrap gap-2">
                <StatusPill tone="success">{formatMessage(dictionary.signals.active, { count: activeCount })}</StatusPill>
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

          <SignalsTable
            signals={filteredSignals}
            selectedSignalId={selectedSignal?.id}
            runtimeControls={runtimeControls}
            onSelect={selectSignal}
            onOpenAction={openSignalAction}
          />
        </div>

        <WorkbenchDetailPane desktopOnly>
          <SignalsDetailPanel
            signal={selectedSignal ?? initialSelectedSignal}
            runtimeControls={runtimeControls}
            onOpenAction={openSignalAction}
          />
        </WorkbenchDetailPane>

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
          confirmDisabled={!actionSignal || !canSubmitExecution()}
          onSubmit={submitExecutionRequest}
          feedback={dialogFeedback}
          context={
            actionSignal ? (
              <div className="space-y-1">
                <p>{dictionary.signals.market}: {actionSignal.marketQuestion}</p>
                <p>{dictionary.signals.expectedVersion}: {actionSignal.version}</p>
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
      )}
    </div>
  );
}
