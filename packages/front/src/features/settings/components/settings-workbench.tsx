"use client";

import { useCallback, useEffect, useState, useTransition } from "react";

import { ActionDialog } from "@/components/shared/action-dialog";
import { OperationFeedbackBanner } from "@/components/shared/operation-feedback-banner";
import { PageHeader } from "@/components/shared/page-header";
import { StatusPill } from "@/components/shared/status-pill";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { updateSystemRuntimeState, type OperationActionResult } from "@/lib/api/actions";
import { readSystemRuntimeState } from "@/lib/api/settings";
import type { SystemRuntimeStateData } from "@/lib/contracts/dto";
import { dictionary } from "@/lib/i18n/dictionaries";

type RuntimeAction = "lock" | "release" | null;

export function SettingsWorkbench() {
  const d = dictionary.settingsV3;
  const [state, setState] = useState<SystemRuntimeStateData | null>(null);
  const [action, setAction] = useState<RuntimeAction>(null);
  const [enableTrading, setEnableTrading] = useState(false);
  const [reason, setReason] = useState("");
  const [note, setNote] = useState("");
  const [stepUpCode, setStepUpCode] = useState("");
  const [loadError, setLoadError] = useState("");
  const [feedback, setFeedback] = useState<OperationActionResult | null>(null);
  const [isPending, startTransition] = useTransition();

  const reload = useCallback(() => {
    void readSystemRuntimeState()
      .then((response) => {
        setState(response.data);
        setEnableTrading(response.data.trading_enabled);
        setLoadError("");
      })
      .catch(() => setLoadError(d.loadFailed));
  }, [d.loadFailed]);

  useEffect(reload, [reload]);

  const openAction = (nextAction: Exclude<RuntimeAction, null>) => {
    setAction(nextAction);
    setReason("");
    setNote("");
    setStepUpCode("");
    setFeedback(null);
  };

  const submit = () => {
    if (!action) return;
    startTransition(async () => {
      const result = await updateSystemRuntimeState({
        request: {
          kill_switch_locked: action === "lock",
          trading_enabled: action === "release" && enableTrading,
          reason: reason.trim() || undefined,
          operator_note: note.trim() || undefined,
        },
        stepUpCode,
      });
      setFeedback(result);
      if (result.ok) {
        setAction(null);
        reload();
      }
    });
  };

  return (
    <div className="space-y-8">
      <PageHeader eyebrow={d.eyebrow} title={d.title} description={d.description} />
      {feedback ? <OperationFeedbackBanner feedback={feedback} /> : null}
      {loadError ? <p className="text-sm text-destructive">{loadError}</p> : null}

      <Card>
        <CardHeader>
          <CardTitle>{d.runtimeState}</CardTitle>
        </CardHeader>
        <CardContent className="grid gap-4 md:grid-cols-3">
          <StateValue label={d.killSwitch} value={state?.kill_switch_locked ? d.locked : d.released} tone={state?.kill_switch_locked ? "danger" : "success"} />
          <StateValue label={d.trading} value={state?.trading_enabled ? d.enabled : d.disabled} tone={state?.trading_enabled ? "success" : "neutral"} />
          <StateValue label={d.version} value={state ? String(state.version) : dictionary.common.loading} />
          <div className="md:col-span-3 grid gap-2 text-sm text-muted-foreground sm:grid-cols-3">
            <p>{d.reason}: {state?.reason ?? d.none}</p>
            <p>{d.updatedBy}: {state?.updated_by ?? d.none}</p>
            <p>{d.updatedAt}: {state?.updated_at ?? d.none}</p>
          </div>
          <div className="flex flex-wrap gap-2 md:col-span-3">
            <Button variant="destructive" disabled={state?.kill_switch_locked === true} onClick={() => openAction("lock")}>{d.trigger}</Button>
            <Button disabled={!state} onClick={() => openAction("release")}>{d.release}</Button>
            <Button variant="outline" onClick={reload}>{d.refresh}</Button>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader><CardTitle>{d.boundaries}</CardTitle></CardHeader>
        <CardContent className="grid gap-4 text-sm md:grid-cols-3">
          <Boundary title={d.backendMode} detail={d.backendModeDetail} />
          <Boundary title={d.dataSource} detail={d.dataSourceDetail} />
          <Boundary title={d.removedCapabilities} detail={d.removedCapabilitiesDetail} />
        </CardContent>
      </Card>

      <ActionDialog
        open={action !== null}
        onOpenChange={(open) => { if (!open) setAction(null); }}
        title={action === "lock" ? d.lockDialogTitle : d.releaseDialogTitle}
        description={action === "lock" ? d.lockDialogDescription : d.releaseDialogDescription}
        confirmLabel={action === "lock" ? d.lockConfirm : d.releaseConfirm}
        confirmVariant={action === "lock" ? "destructive" : "default"}
        isPending={isPending}
        note={note}
        onNoteChange={setNote}
        stepUpCode={stepUpCode}
        onStepUpCodeChange={setStepUpCode}
        requiresStepUp
        onSubmit={submit}
        confirmDisabled={!stepUpCode.trim()}
      >
        <label className="space-y-2 text-sm">
          <span>{d.reason}</span>
          <Input value={reason} onChange={(event) => setReason(event.target.value)} />
        </label>
        {action === "release" ? (
          <label className="flex items-center gap-3 rounded-md border p-3 text-sm">
            <input type="checkbox" checked={enableTrading} onChange={(event) => setEnableTrading(event.target.checked)} />
            <span>{d.enableTradingOnRelease}</span>
          </label>
        ) : null}
      </ActionDialog>
    </div>
  );
}

function StateValue({ label, value, tone = "neutral" }: { label: string; value: string; tone?: "neutral" | "success" | "danger" }) {
  return <div className="rounded-lg border p-4"><p className="text-xs text-muted-foreground">{label}</p><StatusPill className="mt-2" tone={tone}>{value}</StatusPill></div>;
}

function Boundary({ title, detail }: { title: string; detail: string }) {
  return <div className="rounded-lg border p-4"><p className="font-medium">{title}</p><p className="mt-2 text-muted-foreground">{detail}</p></div>;
}
