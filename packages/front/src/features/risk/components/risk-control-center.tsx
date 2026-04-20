"use client";

import { startTransition, useEffect, useState, useTransition } from "react";
import Link from "next/link";
import { Download, ShieldCheck, ToggleLeft } from "lucide-react";
import { toast } from "sonner";

import type { RuntimeMode } from "@/lib/contracts/dto";
import type { RiskStreamPayload } from "@/lib/contracts/realtime";
import type { getRiskPageData } from "@/features/risk/loaders/risk-page-data";
import {
  requestModeSwitchAction,
  setKillSwitchStateAction,
  triggerRiskReleaseAction,
} from "@/server/actions/risk-actions";
import type { OperationActionResult } from "@/server/actions/action-result";
import { ActionDialog } from "@/components/shared/action-dialog";
import { useConsoleRealtimeChannel } from "@/components/shared/console-realtime-provider";
import { Button } from "@/components/ui/button";
import { EmptyPanel } from "@/components/shared/empty-panel";
import {
  alertSeverityTone,
  alertStatusTone,
  formatClock,
  formatCurrency,
  formatPercentFromRatio,
  humanizeSnakeCase,
} from "@/lib/realtime-formatters";
import { MeterBar } from "@/components/shared/meter-bar";
import { OperationFeedbackBanner } from "@/components/shared/operation-feedback-banner";
import { PageHeader } from "@/components/shared/page-header";
import { StateBanner } from "@/components/shared/state-banner";
import { StatusPill } from "@/components/shared/status-pill";

type RiskPageData = Awaited<ReturnType<typeof getRiskPageData>>;
type RiskDialog = "mode" | "release" | "kill_switch" | null;

const MODE_OPTIONS: Array<{ value: RuntimeMode; label: string; detail: string }> = [
  { value: "research", label: "research", detail: "No live execution, full operator visibility." },
  { value: "paper_trade", label: "paper trade", detail: "Simulate execution against live inputs." },
  { value: "manual_confirm", label: "manual confirm", detail: "Operator confirmation required before execution." },
  { value: "live_auto", label: "live auto", detail: "Autonomous execution within live controls." },
  { value: "kill_switch_locked", label: "kill switch locked", detail: "Protected mode with execution halted." },
];

function humanizeMode(mode: RuntimeMode): string {
  return mode.replaceAll("_", " ");
}

function patchMetricValues(
  metrics: RiskPageData["metrics"],
  controls: { mode: RuntimeMode; killSwitch: boolean },
) {
  return metrics.map((metric) => {
    if (metric.title === "Mode") {
      return {
        ...metric,
        value: humanizeMode(controls.mode),
      };
    }

    if (metric.title === "Kill Switch") {
      return {
        ...metric,
        value: controls.killSwitch ? "active" : "armed",
        hint: controls.killSwitch ? "halted" : "ready state",
        tone: controls.killSwitch ? ("danger" as const) : ("primary" as const),
      };
    }

    return metric;
  });
}

function patchMetricsFromStream(
  metrics: RiskPageData["metrics"],
  payload: RiskStreamPayload,
  controls: { mode: RuntimeMode; killSwitch: boolean },
) {
  return metrics.map((metric) => {
    if (metric.title === "Mode") {
      return {
        ...metric,
        value: humanizeSnakeCase(payload.mode ?? controls.mode),
      };
    }

    if (metric.title === "Kill Switch") {
      const killSwitch = payload.kill_switch ?? controls.killSwitch;

      return {
        ...metric,
        value: killSwitch ? "active" : "armed",
        hint: killSwitch ? "halted" : "ready state",
        tone: killSwitch ? ("danger" as const) : ("primary" as const),
      };
    }

    if (metric.title === "Daily Loss Usage" && payload.daily_loss_limit && payload.daily_loss_used) {
      const dailyLossUsage = Number.parseFloat(payload.daily_loss_used) / Number.parseFloat(payload.daily_loss_limit);

      return {
        ...metric,
        value: formatPercentFromRatio(dailyLossUsage),
        hint: `${formatCurrency(payload.daily_loss_used)} / ${formatCurrency(payload.daily_loss_limit)}`,
      };
    }

    if (metric.title === "Open Alerts" && payload.open_alerts !== undefined) {
      return {
        ...metric,
        value: String(payload.open_alerts),
        hint:
          payload.critical_alerts !== undefined
            ? `${payload.critical_alerts} critical`
            : metric.hint,
      };
    }

    return metric;
  });
}

function patchSummaryFromStream(summary: RiskPageData["summary"], payload: RiskStreamPayload) {
  const nextSummary = { ...summary };

  if (payload.daily_loss_limit && payload.daily_loss_used) {
    const dailyLossUsage = Number.parseFloat(payload.daily_loss_used) / Number.parseFloat(payload.daily_loss_limit);
    nextSummary.dailyLossUsed = formatCurrency(payload.daily_loss_used);
    nextSummary.dailyLossLimit = formatCurrency(payload.daily_loss_limit);
    nextSummary.dailyLossUsage = formatPercentFromRatio(dailyLossUsage);
    nextSummary.dailyLossWidth = formatPercentFromRatio(dailyLossUsage);
  }

  if (payload.gross_exposure) {
    nextSummary.grossExposure = formatPercentFromRatio(payload.gross_exposure);
  }

  if (payload.net_exposure) {
    nextSummary.netExposure = formatPercentFromRatio(payload.net_exposure);
    nextSummary.longBiasLabel = `long bias ${formatPercentFromRatio(payload.net_exposure)}`;
  }

  if (payload.critical_alerts !== undefined) {
    nextSummary.criticalAlerts = payload.critical_alerts;
  }

  if (payload.warning_alerts !== undefined) {
    nextSummary.warningAlerts = payload.warning_alerts;
  }

  return nextSummary;
}

function buildAlertItem(
  payload: RiskStreamPayload,
  current?: RiskPageData["alerts"][number],
): RiskPageData["alerts"][number] | null {
  if (!payload.alert_id || !payload.severity || !payload.reason || !payload.target || !payload.status) {
    return current ?? null;
  }

  return {
    id: payload.alert_id,
    severity: payload.severity,
    severityTone: alertSeverityTone(payload.severity),
    reason: payload.reason,
    target: payload.target,
    createdAt: payload.created_at ? formatClock(payload.created_at) : current?.createdAt ?? "--:--:--",
    statusLabel: humanizeSnakeCase(payload.status),
    statusTone: alertStatusTone(payload.status),
  };
}

function upsertAlert(
  alerts: RiskPageData["alerts"],
  payload: RiskStreamPayload,
): RiskPageData["alerts"] {
  const current = alerts.find((alert) => alert.id === payload.alert_id);
  const nextAlert = buildAlertItem(payload, current);

  if (!nextAlert) {
    return alerts;
  }

  if (current) {
    return alerts.map((alert) => (alert.id === nextAlert.id ? nextAlert : alert));
  }

  return [nextAlert, ...alerts];
}

export function RiskControlCenter({ data }: { data: RiskPageData }) {
  const [controls, setControls] = useState(data.controls);
  const [metrics, setMetrics] = useState(data.metrics);
  const [summary, setSummary] = useState(data.summary);
  const [alerts, setAlerts] = useState(data.alerts);
  const [approvalCount, setApprovalCount] = useState(data.approvalCount);
  const [activeDialog, setActiveDialog] = useState<RiskDialog>(null);
  const [targetMode, setTargetMode] = useState<RuntimeMode>(
    data.controls.mode === "manual_confirm" ? "paper_trade" : "manual_confirm",
  );
  const [note, setNote] = useState("");
  const [stepUpCode, setStepUpCode] = useState("");
  const [dialogFeedback, setDialogFeedback] = useState<OperationActionResult | null>(null);
  const [lastOperation, setLastOperation] = useState<OperationActionResult | null>(null);
  const [fieldErrors, setFieldErrors] = useState<OperationActionResult["fieldErrors"]>({});
  const [isPending, startActionTransition] = useTransition();
  const { lastEvent } = useConsoleRealtimeChannel("risk");

  useEffect(() => {
    const streamEvent = lastEvent;

    if (!streamEvent) {
      return;
    }

    startTransition(() => {
      setControls((currentControls) => {
        const nextControls = {
          ...currentControls,
          mode: streamEvent.data.mode ?? currentControls.mode,
          modeLabel: humanizeMode(streamEvent.data.mode ?? currentControls.mode),
          killSwitch: streamEvent.data.kill_switch ?? currentControls.killSwitch,
          environment: streamEvent.data.environment ?? currentControls.environment,
        };

        setMetrics((currentMetrics) => patchMetricsFromStream(currentMetrics, streamEvent.data, nextControls));
        return nextControls;
      });

      setSummary((currentSummary) => patchSummaryFromStream(currentSummary, streamEvent.data));
      setAlerts((currentAlerts) => upsertAlert(currentAlerts, streamEvent.data));

      if (streamEvent.data.approval_count !== undefined) {
        setApprovalCount(streamEvent.data.approval_count);
      }
    });
  }, [lastEvent]);

  function openDialog(dialog: Exclude<RiskDialog, null>) {
    setActiveDialog(dialog);
    setDialogFeedback(null);
    setFieldErrors({});
    setStepUpCode("");

    if (dialog === "mode") {
      setNote(`Switching runtime from ${controls.modeLabel} after reviewing current alert and approval state.`);
      setTargetMode(controls.mode === "manual_confirm" ? "paper_trade" : "manual_confirm");
      return;
    }

    if (dialog === "release") {
      setNote("Releasing protective controls after operator review of current alerts and exposure.");
      return;
    }

    setNote(
      controls.killSwitch
        ? "Releasing the kill switch after verifying upstream health and desk approvals."
        : "Triggering the kill switch because current risk conditions require an immediate halt.",
    );
  }

  function closeDialog() {
    setActiveDialog(null);
    setDialogFeedback(null);
    setFieldErrors({});
    setStepUpCode("");
  }

  function applyControls(nextControls: { mode: RuntimeMode; killSwitch: boolean }) {
    const nextState = {
      ...controls,
      mode: nextControls.mode,
      modeLabel: humanizeMode(nextControls.mode),
      killSwitch: nextControls.killSwitch,
    };

    setControls(nextState);
    setMetrics((currentMetrics) => patchMetricValues(currentMetrics, nextControls));
  }

  function handleResult(result: OperationActionResult) {
    setDialogFeedback(result);
    setLastOperation(result);

    if (result.ok) {
      toast.success(result.message, {
        description: [result.requestId, result.traceId].filter(Boolean).join(" · "),
      });
    } else {
      toast.error(result.message, {
        description: [result.requestId, result.traceId].filter(Boolean).join(" · "),
      });
    }
  }

  function submitModeSwitch() {
    startActionTransition(async () => {
      const result = await requestModeSwitchAction({
        currentMode: controls.mode,
        targetMode,
        note,
        stepUpCode,
      });

      setFieldErrors(result.fieldErrors ?? {});
      handleResult(result);

      if (result.ok) {
        applyControls({
          mode: targetMode,
          killSwitch: targetMode === "kill_switch_locked" ? true : controls.killSwitch,
        });
        closeDialog();
      }
    });
  }

  function submitReleaseControls() {
    startActionTransition(async () => {
      const result = await triggerRiskReleaseAction({
        note,
        stepUpCode,
      });

      setFieldErrors(result.fieldErrors ?? {});
      handleResult(result);

      if (result.ok) {
        applyControls({
          mode: controls.mode === "kill_switch_locked" ? "manual_confirm" : controls.mode,
          killSwitch: false,
        });
        closeDialog();
      }
    });
  }

  function submitKillSwitch() {
    const nextEnabled = !controls.killSwitch;

    startActionTransition(async () => {
      const result = await setKillSwitchStateAction({
        enabled: nextEnabled,
        note,
        stepUpCode,
      });

      setFieldErrors(result.fieldErrors ?? {});
      handleResult(result);

      if (result.ok) {
        applyControls({
          mode: nextEnabled ? "kill_switch_locked" : controls.mode === "kill_switch_locked" ? "manual_confirm" : controls.mode,
          killSwitch: nextEnabled,
        });
        closeDialog();
      }
    });
  }

  return (
    <div className="space-y-4">
      <PageHeader
        eyebrow="Safety"
        title="Risk Monitoring"
        description="Real-time exposure, alert state and circuit-breaker controls."
        className="border-none pb-0"
        actions={
          <>
            <StatusPill tone={controls.killSwitch ? "danger" : "warning"}>{controls.modeLabel}</StatusPill>
            <StatusPill tone="primary">{controls.environment}</StatusPill>
            <Button
              variant="outline"
              className="rounded-sm border-white/10 bg-accent/40 text-foreground hover:bg-accent"
              onClick={() => openDialog("mode")}
            >
              <ToggleLeft className="size-4" />
              Switch mode
            </Button>
            <Button
              className="rounded-sm bg-primary text-primary-foreground hover:bg-primary/90"
              onClick={() => openDialog("release")}
            >
              <ShieldCheck className="size-4" />
              Release controls
            </Button>
          </>
        }
      />

      {lastOperation ? <OperationFeedbackBanner feedback={lastOperation} /> : null}

      <section className="grid gap-4 lg:grid-cols-2">
        <StateBanner
          tone="warning"
          title="Risk engine in watch mode"
          detail={`${summary.criticalAlerts} critical and ${summary.warningAlerts} warning alerts are influencing runtime controls.`}
          className="animate-in fade-in-0 duration-300"
        />
        <StateBanner
          tone={controls.killSwitch ? "warning" : "info"}
          title={controls.killSwitch ? "Kill switch active" : "Kill switch armed"}
          detail={`Approval queue currently holds ${approvalCount} pending high-risk items.`}
          className="animate-in fade-in-0 duration-300"
        />
      </section>

      <section className="grid gap-4 xl:grid-cols-12">
        <div className="rounded-lg bg-card/95 p-5 ring-1 ring-white/5 xl:col-span-4">
          <div className="mb-4 flex items-start justify-between gap-3">
            <p className="text-[10px] font-bold uppercase tracking-[0.2em] text-muted-foreground">
              Daily Loss Usage
            </p>
            <span className="font-mono text-xs text-destructive">
              critical ({summary.dailyLossUsage})
            </span>
          </div>
          <div className="mb-3 flex items-end gap-2">
            <span className="font-heading text-4xl font-black leading-none text-foreground">
              {summary.dailyLossUsed}
            </span>
            <span className="pb-1 font-mono text-xs text-muted-foreground">
              / {summary.dailyLossLimit}
            </span>
          </div>
          <MeterBar
            value={summary.dailyLossWidth}
            tone="danger"
            trackClassName="h-3 bg-background"
            barClassName="bg-gradient-to-r from-primary via-primary to-destructive"
          />
        </div>

        <div className="rounded-lg bg-card/95 p-5 ring-1 ring-white/5 xl:col-span-5">
          <div className="grid gap-4 md:grid-cols-2">
            {metrics.map((metric) => (
              <div key={metric.title} className="space-y-2 rounded-md bg-accent/35 p-4">
                <p className="text-[10px] font-bold uppercase tracking-[0.2em] text-muted-foreground">
                  {metric.title}
                </p>
                <p className="font-heading text-3xl font-black leading-none text-foreground">{metric.value}</p>
                <p className="font-mono text-[11px] text-muted-foreground">{metric.hint}</p>
              </div>
            ))}
          </div>
        </div>

        <div className="rounded-lg border border-destructive/10 bg-destructive/5 p-5 ring-1 ring-destructive/10 xl:col-span-3">
          <p className="text-[10px] font-bold uppercase tracking-[0.2em] text-destructive">Active Alerts</p>
          <div className="mt-3 flex items-end justify-between">
            <span className="font-heading text-5xl font-black leading-none text-destructive">
              {String(summary.criticalAlerts + summary.warningAlerts).padStart(2, "0")}
            </span>
            <div className="text-right text-xs">
              <p className="font-semibold text-foreground">{summary.criticalAlerts} critical</p>
              <p className="text-muted-foreground">{summary.warningAlerts} warnings</p>
            </div>
          </div>
          <Button className="mt-4 h-8 w-full rounded-sm bg-destructive text-destructive-foreground hover:bg-destructive/90">
            View log
          </Button>
        </div>
      </section>

      <section className="grid gap-4 xl:grid-cols-[1.55fr_0.9fr]">
        <div className="overflow-hidden rounded-lg bg-card/95 ring-1 ring-white/5">
          <div className="flex flex-col gap-3 bg-popover/70 px-5 py-4 md:flex-row md:items-center md:justify-between">
            <p className="font-heading text-sm font-bold uppercase tracking-[0.18em] text-foreground">
              Risk Event Audit Log
            </p>
            <div className="flex flex-wrap gap-2">
              <Button
                variant="outline"
                size="sm"
                className="rounded-sm border-white/10 bg-accent/40 text-foreground hover:bg-accent"
              >
                Filter: all
              </Button>
              <Button
                variant="outline"
                size="sm"
                className="rounded-sm border-white/10 bg-accent/40 text-foreground hover:bg-accent"
              >
                <Download className="size-3.5" />
                Export CSV
              </Button>
            </div>
          </div>

          {alerts.length > 0 ? (
            <div className="overflow-x-auto">
              <table className="w-full text-left">
                <thead className="bg-sidebar/60">
                  <tr className="text-[10px] font-bold uppercase tracking-[0.2em] text-muted-foreground">
                    <th className="px-5 py-3">Severity</th>
                    <th className="px-5 py-3">Reason</th>
                    <th className="px-5 py-3">Market / Theme</th>
                    <th className="px-5 py-3">Timestamp</th>
                    <th className="px-5 py-3">Audit Status</th>
                    <th className="px-5 py-3 text-right">Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {alerts.map((alert) => (
                    <tr key={alert.id} className="transition-colors hover:bg-accent/35">
                      <td className="px-5 py-4">
                        <StatusPill tone={alert.severityTone}>{alert.severity}</StatusPill>
                      </td>
                      <td className="px-5 py-4 font-mono text-sm text-foreground">{alert.reason}</td>
                      <td className="px-5 py-4 text-sm text-foreground">{alert.target}</td>
                      <td className="px-5 py-4 font-mono text-xs text-muted-foreground">{alert.createdAt}</td>
                      <td className="px-5 py-4">
                        <StatusPill tone={alert.statusTone}>{alert.statusLabel}</StatusPill>
                      </td>
                      <td className="px-5 py-4 text-right">
                        <button className="text-xs font-bold uppercase tracking-[0.18em] text-primary transition-colors hover:text-primary/80">
                          Manage
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : (
            <EmptyPanel
              title="No risk alerts"
              detail="When thresholds or data-quality rules fire, the audit log will populate here."
            />
          )}
        </div>

        <div className="space-y-4">
          <div className="rounded-lg bg-card/95 p-5 ring-1 ring-white/5">
            <p className="font-heading text-sm font-bold uppercase tracking-[0.18em] text-foreground">
              Global Controls
            </p>
            <div className="mt-4 space-y-3">
              <div className="rounded-md bg-accent/45 p-4 text-sm text-muted-foreground">
                Mode changes, kill switch actions and release operations require step-up auth.
              </div>
              <Button
                variant="outline"
                className="h-9 w-full rounded-sm border-white/10 bg-accent/40 text-foreground hover:bg-accent"
                onClick={() => openDialog("kill_switch")}
              >
                {controls.killSwitch ? "Release kill switch" : "Trigger kill switch"}
              </Button>
              <Button asChild className="h-9 w-full rounded-sm bg-primary text-primary-foreground hover:bg-primary/90">
                <Link href="/approvals">Review approvals ({approvalCount})</Link>
              </Button>
            </div>
          </div>

          <div className="rounded-lg bg-card/95 p-5 ring-1 ring-white/5">
            <p className="font-heading text-sm font-bold uppercase tracking-[0.18em] text-foreground">
              Risk Buckets
            </p>
            <div className="mt-4 space-y-4">
              {data.riskBuckets.map((bucket, index) => (
                <div key={bucket.id} className="space-y-2">
                  <div className="flex items-center justify-between gap-3">
                    <p className="text-sm font-medium text-foreground">{bucket.name}</p>
                    <span className="font-mono text-xs text-muted-foreground">{bucket.exposure}</span>
                  </div>
                  <MeterBar
                    value={bucket.width}
                    tone={index === 2 ? "danger" : index === 1 ? "warning" : "primary"}
                    trackClassName="h-2 bg-background"
                  />
                </div>
              ))}
            </div>
          </div>
        </div>
      </section>

      <ActionDialog
        open={activeDialog === "mode"}
        onOpenChange={(open) => {
          if (!open) {
            closeDialog();
          }
        }}
        title="Switch runtime mode"
        description="Mode changes require step-up authentication, an audit note and a clear target state."
        confirmLabel="Queue mode switch"
        isPending={isPending}
        note={note}
        onNoteChange={setNote}
        noteError={fieldErrors?.note}
        stepUpCode={stepUpCode}
        onStepUpCodeChange={setStepUpCode}
        stepUpCodeError={fieldErrors?.stepUpCode}
        requiresStepUp
        onSubmit={submitModeSwitch}
        feedback={dialogFeedback}
        context={
          <div className="space-y-1">
            <p>Current mode: {controls.modeLabel}</p>
            <p>Environment: {controls.environment}</p>
          </div>
        }
      >
        <div className="space-y-2">
          <p className="text-sm font-medium text-foreground">Target mode</p>
          <div className="grid gap-2 sm:grid-cols-2">
            {MODE_OPTIONS.map((option) => (
              <button
                key={option.value}
                type="button"
                onClick={() => setTargetMode(option.value)}
                className={
                  targetMode === option.value
                    ? "rounded-md border border-primary/40 bg-primary/10 p-3 text-left"
                    : "rounded-md border border-border/70 bg-accent/35 p-3 text-left hover:bg-accent/55"
                }
              >
                <p className="text-sm font-semibold text-foreground">{option.label}</p>
                <p className="mt-1 text-xs text-muted-foreground">{option.detail}</p>
              </button>
            ))}
          </div>
          {fieldErrors?.targetMode ? <p className="text-xs text-destructive">{fieldErrors.targetMode}</p> : null}
        </div>
      </ActionDialog>

      <ActionDialog
        open={activeDialog === "release"}
        onOpenChange={(open) => {
          if (!open) {
            closeDialog();
          }
        }}
        title="Release protective controls"
        description="This queues a protected operation to release desk controls after operator confirmation."
        confirmLabel="Queue release"
        isPending={isPending}
        note={note}
        onNoteChange={setNote}
        noteError={fieldErrors?.note}
        stepUpCode={stepUpCode}
        onStepUpCodeChange={setStepUpCode}
        stepUpCodeError={fieldErrors?.stepUpCode}
        requiresStepUp
        onSubmit={submitReleaseControls}
        feedback={dialogFeedback}
        context={
          <div className="space-y-1">
            <p>Kill switch: {controls.killSwitch ? "active" : "armed"}</p>
            <p>Pending approvals: {approvalCount}</p>
          </div>
        }
      />

      <ActionDialog
        open={activeDialog === "kill_switch"}
        onOpenChange={(open) => {
          if (!open) {
            closeDialog();
          }
        }}
        title={controls.killSwitch ? "Release kill switch" : "Trigger kill switch"}
        description="Kill switch operations are high-risk controls and always require step-up authentication."
        confirmLabel={controls.killSwitch ? "Queue release" : "Queue kill switch"}
        confirmVariant={controls.killSwitch ? "default" : "destructive"}
        isPending={isPending}
        note={note}
        onNoteChange={setNote}
        noteError={fieldErrors?.note}
        stepUpCode={stepUpCode}
        onStepUpCodeChange={setStepUpCode}
        stepUpCodeError={fieldErrors?.stepUpCode}
        requiresStepUp
        onSubmit={submitKillSwitch}
        feedback={dialogFeedback}
        context={
          <div className="space-y-1">
            <p>Current mode: {controls.modeLabel}</p>
            <p>Kill switch status: {controls.killSwitch ? "active" : "armed"}</p>
          </div>
        }
      />
    </div>
  );
}
