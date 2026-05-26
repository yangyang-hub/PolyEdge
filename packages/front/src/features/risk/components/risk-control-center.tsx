"use client";

import { startTransition, useEffect, useMemo, useRef, useState, useTransition } from "react";
import { Download, ShieldCheck } from "lucide-react";
import { toast } from "sonner";

import type { RuntimeMode } from "@/lib/contracts/dto";
import type { RiskStreamPayload } from "@/lib/contracts/realtime";
import type { getRiskPageData } from "@/features/risk/loaders/risk-page-data";
import {
  setKillSwitchStateAction,
  triggerRiskReleaseAction,
} from "@/lib/api/actions";
import type { OperationActionResult } from "@/lib/api/actions";
import { ActionDialog } from "@/components/shared/action-dialog";
import { useConsoleRealtimeChannel } from "@/components/shared/console-realtime-provider";
import { Button } from "@/components/ui/button";
import { EmptyPanel } from "@/components/shared/empty-panel";
import { useI18n } from "@/lib/i18n/client";
import { localizeGeneratedCopy } from "@/lib/i18n/generated-copy";
import { normalizeOptionalRuntimeMode, normalizeRuntimeMode } from "@/lib/runtime-mode";
import {
  alertSeverityTone,
  alertStatusTone,
  formatClock,
  formatCurrency,
  formatPercentFromRatio,
} from "@/lib/realtime-formatters";
import { MeterBar } from "@/components/shared/meter-bar";
import { OperationFeedbackBanner } from "@/components/shared/operation-feedback-banner";
import { PageHeader } from "@/components/shared/page-header";
import { StateBanner } from "@/components/shared/state-banner";
import { StatusPill } from "@/components/shared/status-pill";

type RiskPageData = Awaited<ReturnType<typeof getRiskPageData>>;
type RiskDialog = "release" | "kill_switch" | null;
type RiskAlertFilter = "all" | "unresolved" | "watching";

function patchMetricValues(
  metrics: RiskPageData["metrics"],
  controls: { mode: RuntimeMode; killSwitch: boolean },
  labels: {
    mode: (mode: RuntimeMode) => string;
    active: string;
    armed: string;
    halted: string;
    readyState: string;
  },
) {
  return metrics.map((metric) => {
    if (metric.key === "mode") {
      return {
        ...metric,
        value: labels.mode(controls.mode),
      };
    }

    if (metric.key === "kill_switch") {
      return {
        ...metric,
        value: controls.killSwitch ? labels.active : labels.armed,
        hint: controls.killSwitch ? labels.halted : labels.readyState,
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
  labels: {
    mode: (mode: RuntimeMode) => string;
    active: string;
    armed: string;
    halted: string;
    readyState: string;
    critical: string;
  },
) {
  return metrics.map((metric) => {
    if (metric.key === "mode") {
      return {
        ...metric,
        value: labels.mode(payload.mode ?? controls.mode),
      };
    }

    if (metric.key === "kill_switch") {
      const killSwitch = payload.kill_switch ?? controls.killSwitch;

      return {
        ...metric,
        value: killSwitch ? labels.active : labels.armed,
        hint: killSwitch ? labels.halted : labels.readyState,
        tone: killSwitch ? ("danger" as const) : ("primary" as const),
      };
    }

    if (metric.key === "daily_loss_usage" && payload.daily_loss_limit && payload.daily_loss_used) {
      const dailyLossUsage = Number.parseFloat(payload.daily_loss_used) / Number.parseFloat(payload.daily_loss_limit);

      return {
        ...metric,
        value: formatPercentFromRatio(dailyLossUsage),
        hint: `${formatCurrency(payload.daily_loss_used)} / ${formatCurrency(payload.daily_loss_limit)}`,
      };
    }

    if (metric.key === "open_alerts" && payload.open_alerts !== undefined) {
      return {
        ...metric,
        value: String(payload.open_alerts),
        hint:
          payload.critical_alerts !== undefined
            ? `${payload.critical_alerts} ${labels.critical}`
            : metric.hint,
      };
    }

    return metric;
  });
}

function patchSummaryFromStream(
  summary: RiskPageData["summary"],
  payload: RiskStreamPayload,
  labels: { deskBias: string },
) {
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
    nextSummary.longBiasLabel = `${labels.deskBias} ${formatPercentFromRatio(payload.net_exposure)}`;
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
  enumLabel: (value: string) => string = (value) => value.replaceAll("_", " "),
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
    status: payload.status,
    statusLabel: enumLabel(payload.status),
    statusTone: alertStatusTone(payload.status),
  };
}

function upsertAlert(
  alerts: RiskPageData["alerts"],
  payload: RiskStreamPayload,
  enumLabel: (value: string) => string,
): RiskPageData["alerts"] {
  const current = alerts.find((alert) => alert.id === payload.alert_id);
  const nextAlert = buildAlertItem(payload, current, enumLabel);

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
  const [alertFilter, setAlertFilter] = useState<RiskAlertFilter>("all");
  const [activeDialog, setActiveDialog] = useState<RiskDialog>(null);
  const [note, setNote] = useState("");
  const [stepUpCode, setStepUpCode] = useState("");
  const [dialogFeedback, setDialogFeedback] = useState<OperationActionResult | null>(null);
  const [lastOperation, setLastOperation] = useState<OperationActionResult | null>(null);
  const [fieldErrors, setFieldErrors] = useState<OperationActionResult["fieldErrors"]>({});
  const [isPending, startActionTransition] = useTransition();
  const auditLogRef = useRef<HTMLElement | null>(null);
  const { lastEvent } = useConsoleRealtimeChannel("risk");
  const { locale, dictionary, enumLabel, format } = useI18n();
  const killSwitchAvailable =
    controls.mode === "live_auto" || controls.mode === "kill_switch_locked" || controls.killSwitch;
  const metricLabels = useMemo(() => ({
    mode: (mode: RuntimeMode) => enumLabel(mode),
    active: dictionary.common.active,
    armed: dictionary.common.armed,
    halted: dictionary.metricHints.halted,
    readyState: dictionary.metricHints.readyState,
    critical: dictionary.common.critical,
  }), [
    dictionary.common.active,
    dictionary.common.armed,
    dictionary.common.critical,
    dictionary.metricHints.halted,
    dictionary.metricHints.readyState,
    enumLabel,
  ]);
  useEffect(() => {
    const streamEvent = lastEvent;

    if (!streamEvent) {
      return;
    }

    startTransition(() => {
      const runtimeMode = normalizeOptionalRuntimeMode(streamEvent.data.mode);

      setControls((currentControls) => {
        const nextControls = {
          ...currentControls,
          mode: runtimeMode ?? currentControls.mode,
          modeLabel: enumLabel(runtimeMode ?? currentControls.mode),
          killSwitch: streamEvent.data.kill_switch ?? currentControls.killSwitch,
          environment: streamEvent.data.environment ?? currentControls.environment,
        };

        setMetrics((currentMetrics) =>
          patchMetricsFromStream(currentMetrics, streamEvent.data, nextControls, metricLabels),
        );
        return nextControls;
      });

      setSummary((currentSummary) =>
        patchSummaryFromStream(currentSummary, streamEvent.data, {
          deskBias: dictionary.metricHints.deskBias,
        }),
      );
      setAlerts((currentAlerts) =>
        upsertAlert(currentAlerts, streamEvent.data, enumLabel).map((alert) => ({
          ...alert,
          reason: localizeGeneratedCopy(locale, dictionary, alert.reason),
          target: localizeGeneratedCopy(locale, dictionary, alert.target),
        })),
      );

    });
  }, [dictionary, dictionary.metricHints.deskBias, enumLabel, lastEvent, locale, metricLabels]);

  function openDialog(dialog: Exclude<RiskDialog, null>) {
    setActiveDialog(dialog);
    setDialogFeedback(null);
    setFieldErrors({});
    setStepUpCode("");

    if (dialog === "release") {
      setNote(dictionary.risk.releaseNote);
      return;
    }

    setNote(
      controls.killSwitch
        ? dictionary.risk.killReleaseNote
        : dictionary.risk.killTriggerNote,
    );
  }

  function closeDialog() {
    setActiveDialog(null);
    setDialogFeedback(null);
    setFieldErrors({});
    setStepUpCode("");
  }

  function applyControls(nextControls: { mode: RuntimeMode; killSwitch: boolean }) {
    const normalizedControls = {
      ...nextControls,
      mode: normalizeRuntimeMode(nextControls.mode),
    };
    const nextState = {
      ...controls,
      mode: normalizedControls.mode,
      modeLabel: enumLabel(normalizedControls.mode),
      killSwitch: normalizedControls.killSwitch,
    };

    setControls(nextState);
    setMetrics((currentMetrics) => patchMetricValues(currentMetrics, normalizedControls, metricLabels));
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
          mode: controls.mode === "kill_switch_locked" ? "paper_trade" : controls.mode,
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
          mode: nextEnabled ? "kill_switch_locked" : controls.mode === "kill_switch_locked" ? "paper_trade" : controls.mode,
          killSwitch: nextEnabled,
        });
        closeDialog();
      }
    });
  }

  const visibleAlerts = alerts.filter((alert) => {
    if (alertFilter === "unresolved") {
      return alert.status === "unresolved";
    }

    if (alertFilter === "watching") {
      return alert.status === "watching";
    }

    return true;
  });
  const visibleMetrics = killSwitchAvailable
    ? metrics
    : metrics.filter((metric) => metric.key !== "kill_switch");

  function scrollToAuditLog() {
    auditLogRef.current?.scrollIntoView({ behavior: "smooth", block: "start" });
  }

  function exportVisibleAlertsCsv() {
    const header = ["id", "severity", "reason", "target", "created_at", "status"];
    const rows = visibleAlerts.map((alert) => [
      alert.id,
      alert.severity,
      alert.reason,
      alert.target,
      alert.createdAt,
      alert.status,
    ]);
    const escapeCell = (value: string) => `"${value.replaceAll('"', '""')}"`;
    const csv = [header, ...rows].map((row) => row.map(escapeCell).join(",")).join("\n");
    const url = URL.createObjectURL(new Blob([csv], { type: "text/csv;charset=utf-8" }));
    const link = document.createElement("a");
    link.href = url;
    link.download = `polyedge-risk-alerts-${new Date().toISOString().slice(0, 10)}.csv`;
    link.click();
    URL.revokeObjectURL(url);
  }

  function manageAlert(alert: RiskPageData["alerts"][number]) {
    if (alert.id === "alt_kill_switch_active" || controls.killSwitch) {
      openDialog("release");
      return;
    }

    openDialog("release");
  }

  return (
    <div className="space-y-4">
      <PageHeader
        eyebrow={dictionary.risk.eyebrow}
        title={dictionary.risk.title}
        description={dictionary.risk.description}
        className="border-none pb-0"
        actions={
          <>
            <StatusPill tone={controls.killSwitch ? "danger" : "warning"}>{controls.modeLabel}</StatusPill>
            <StatusPill tone="primary">{controls.environment}</StatusPill>
            {controls.killSwitch ? (
              <Button
                className="rounded-sm bg-primary text-primary-foreground hover:bg-primary/90"
                onClick={() => openDialog("release")}
              >
                <ShieldCheck className="size-4" />
                {dictionary.risk.releaseControls}
              </Button>
            ) : null}
          </>
        }
      />

      {lastOperation ? <OperationFeedbackBanner feedback={lastOperation} /> : null}

      <section className="grid gap-4 lg:grid-cols-2">
        <StateBanner
          tone="warning"
          title={dictionary.risk.watchModeTitle}
          detail={format(dictionary.risk.watchModeDetail, {
            critical: summary.criticalAlerts,
            warning: summary.warningAlerts,
          })}
          className="animate-in fade-in-0 duration-300"
        />
        {killSwitchAvailable ? (
          <StateBanner
            tone={controls.killSwitch ? "warning" : "info"}
            title={controls.killSwitch ? dictionary.risk.killActiveTitle : dictionary.risk.killArmedTitle}
            detail={dictionary.risk.killSwitchDescription}
            className="animate-in fade-in-0 duration-300"
          />
        ) : null}
      </section>

      <section className="grid gap-4 xl:grid-cols-12">
        <div className="rounded-lg bg-card/95 p-5 ring-1 ring-white/5 xl:col-span-4">
          <div className="mb-4 flex items-start justify-between gap-3">
            <p className="text-[10px] font-bold uppercase tracking-[0.2em] text-muted-foreground">
              {dictionary.risk.dailyLossUsage}
            </p>
            <span className="font-mono text-xs text-destructive">
              {dictionary.common.critical} ({summary.dailyLossUsage})
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
            {visibleMetrics.map((metric) => (
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
          <p className="text-[10px] font-bold uppercase tracking-[0.2em] text-destructive">{dictionary.risk.activeAlerts}</p>
          <div className="mt-3 flex items-end justify-between">
            <span className="font-heading text-5xl font-black leading-none text-destructive">
              {String(summary.criticalAlerts + summary.warningAlerts).padStart(2, "0")}
            </span>
            <div className="text-right text-xs">
              <p className="font-semibold text-foreground">{summary.criticalAlerts} {dictionary.common.critical}</p>
              <p className="text-muted-foreground">{summary.warningAlerts} {dictionary.common.warnings}</p>
            </div>
          </div>
          <Button
            className="mt-4 h-8 w-full rounded-sm bg-destructive text-destructive-foreground hover:bg-destructive/90"
            onClick={scrollToAuditLog}
          >
            {dictionary.risk.viewLog}
          </Button>
        </div>
      </section>

      <section ref={auditLogRef} className="grid scroll-mt-4 gap-4 xl:grid-cols-[1.55fr_0.9fr]">
        <div className="overflow-hidden rounded-lg bg-card/95 ring-1 ring-white/5">
          <div className="flex flex-col gap-3 bg-popover/70 px-5 py-4 md:flex-row md:items-center md:justify-between">
            <p className="font-heading text-sm font-bold uppercase tracking-[0.18em] text-foreground">
              {dictionary.risk.auditLog}
            </p>
            <div className="flex flex-wrap gap-2">
              <Button
                variant="outline"
                size="sm"
                className={
                  alertFilter === "all"
                    ? "rounded-sm border-primary/40 bg-primary/10 text-primary hover:bg-primary/15"
                    : "rounded-sm border-white/10 bg-accent/40 text-foreground hover:bg-accent"
                }
                onClick={() => setAlertFilter("all")}
              >
                {dictionary.risk.filterAll}
              </Button>
              {(["unresolved", "watching"] as const).map((status) => (
                <Button
                  key={status}
                  variant="outline"
                  size="sm"
                  className={
                    alertFilter === status
                      ? "rounded-sm border-primary/40 bg-primary/10 text-primary hover:bg-primary/15"
                      : "rounded-sm border-white/10 bg-accent/40 text-foreground hover:bg-accent"
                  }
                  onClick={() => setAlertFilter(status)}
                >
                  {enumLabel(status)}
                </Button>
              ))}
              <Button
                variant="outline"
                size="sm"
                className="rounded-sm border-white/10 bg-accent/40 text-foreground hover:bg-accent"
                onClick={exportVisibleAlertsCsv}
              >
                <Download className="size-3.5" />
                {dictionary.risk.exportCsv}
              </Button>
            </div>
          </div>

          {visibleAlerts.length > 0 ? (
            <div className="overflow-x-auto">
              <table className="w-full text-left">
                <thead className="bg-sidebar/60">
                  <tr className="text-[10px] font-bold uppercase tracking-[0.2em] text-muted-foreground">
                    <th className="px-5 py-3">{dictionary.risk.severity}</th>
                    <th className="px-5 py-3">{dictionary.risk.reason}</th>
                    <th className="px-5 py-3">{dictionary.risk.marketTheme}</th>
                    <th className="px-5 py-3">{dictionary.risk.timestamp}</th>
                    <th className="px-5 py-3">{dictionary.risk.auditStatus}</th>
                    <th className="px-5 py-3 text-right">{dictionary.risk.actions}</th>
                  </tr>
                </thead>
                <tbody>
                  {visibleAlerts.map((alert) => (
                    <tr key={alert.id} className="transition-colors hover:bg-accent/35">
                      <td className="px-5 py-4">
                        <StatusPill tone={alert.severityTone}>{enumLabel(alert.severity)}</StatusPill>
                      </td>
                      <td className="px-5 py-4 font-mono text-sm text-foreground">{alert.reason}</td>
                      <td className="px-5 py-4 text-sm text-foreground">{alert.target}</td>
                      <td className="px-5 py-4 font-mono text-xs text-muted-foreground">{alert.createdAt}</td>
                      <td className="px-5 py-4">
                        <StatusPill tone={alert.statusTone}>{alert.statusLabel}</StatusPill>
                      </td>
                      <td className="px-5 py-4 text-right">
                        <button
                          type="button"
                          className="text-xs font-bold uppercase tracking-[0.18em] text-primary transition-colors hover:text-primary/80"
                          onClick={() => manageAlert(alert)}
                        >
                          {dictionary.common.manage}
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : (
            <EmptyPanel
              title={dictionary.risk.noAlertsTitle}
              detail={dictionary.risk.noAlertsDetail}
            />
          )}
        </div>

        <div className="space-y-4">
          {killSwitchAvailable ? (
            <div className="rounded-lg bg-card/95 p-5 ring-1 ring-white/5">
              <p className="font-heading text-sm font-bold uppercase tracking-[0.18em] text-foreground">
                {dictionary.risk.globalControls}
              </p>
              <div className="mt-4 space-y-3">
                <div className="rounded-md bg-accent/45 p-4 text-sm text-muted-foreground">
                  {dictionary.risk.globalControlsDetail}
                </div>
                <Button
                  variant="outline"
                  className="h-9 w-full rounded-sm border-white/10 bg-accent/40 text-foreground hover:bg-accent"
                  onClick={() => openDialog("kill_switch")}
                >
                  {controls.killSwitch ? dictionary.risk.releaseKillSwitch : dictionary.risk.triggerKillSwitch}
                </Button>
              </div>
            </div>
          ) : null}

          <div className="rounded-lg bg-card/95 p-5 ring-1 ring-white/5">
            <p className="font-heading text-sm font-bold uppercase tracking-[0.18em] text-foreground">
              {dictionary.risk.riskBuckets}
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
        open={activeDialog === "release"}
        onOpenChange={(open) => {
          if (!open) {
            closeDialog();
          }
        }}
        title={dictionary.risk.releaseTitle}
        description={dictionary.risk.releaseDescription}
        confirmLabel={dictionary.risk.queueRelease}
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
            <p>{dictionary.risk.killSwitch}: {controls.killSwitch ? dictionary.common.active : dictionary.common.armed}</p>
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
        title={controls.killSwitch ? dictionary.risk.releaseKillSwitch : dictionary.risk.triggerKillSwitch}
        description={dictionary.risk.killSwitchDescription}
        confirmLabel={controls.killSwitch ? dictionary.risk.queueRelease : dictionary.risk.queueKillSwitch}
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
            <p>{dictionary.risk.currentMode}: {controls.modeLabel}</p>
            <p>{dictionary.risk.killSwitchStatus}: {controls.killSwitch ? dictionary.common.active : dictionary.common.armed}</p>
          </div>
        }
      />
    </div>
  );
}
