"use client";

import { startTransition, useEffect, useMemo, useRef, useState, useTransition } from "react";
import { ShieldCheck } from "lucide-react";
import { toast } from "sonner";

import type { RuntimeMode } from "@/lib/contracts/dto";
import {
  setKillSwitchStateAction,
  triggerRiskReleaseAction,
} from "@/lib/api/actions";
import type { OperationActionResult } from "@/lib/api/actions";
import { useConsoleRealtimeChannel } from "@/components/shared/console-realtime-provider";
import { Button } from "@/components/ui/button";
import { useI18n } from "@/lib/i18n/client";
import { localizeGeneratedCopy } from "@/lib/i18n/generated-copy";
import { normalizeOptionalRuntimeMode, normalizeRuntimeMode } from "@/lib/runtime-mode";
import { OperationFeedbackBanner } from "@/components/shared/operation-feedback-banner";
import { PageHeader } from "@/components/shared/page-header";
import { StateBanner } from "@/components/shared/state-banner";
import { StatusPill } from "@/components/shared/status-pill";

import {
  patchMetricValues,
  patchMetricsFromStream,
  patchSummaryFromStream,
  upsertAlert,
} from "../lib/risk-stream";
import type { RiskAlertFilter, RiskDialog, RiskPageData } from "../types";
import { RiskActionDialogs } from "./risk-action-dialogs";
import { RiskAuditLog } from "./risk-audit-log";
import { RiskControlsSidebar } from "./risk-controls-sidebar";
import { RiskMetricsOverview } from "./risk-metrics-overview";

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

      <RiskMetricsOverview
        summary={summary}
        visibleMetrics={visibleMetrics}
        onViewLog={scrollToAuditLog}
      />

      <section ref={auditLogRef} className="grid scroll-mt-4 gap-4 xl:grid-cols-[1.55fr_0.9fr]">
        <RiskAuditLog
          visibleAlerts={visibleAlerts}
          alertFilter={alertFilter}
          onAlertFilterChange={setAlertFilter}
          onExport={exportVisibleAlertsCsv}
          onManage={manageAlert}
        />

        <RiskControlsSidebar
          killSwitchAvailable={killSwitchAvailable}
          killSwitch={controls.killSwitch}
          onTriggerKillSwitch={() => openDialog("kill_switch")}
          riskBuckets={data.riskBuckets}
        />
      </section>

      <RiskActionDialogs
        activeDialog={activeDialog}
        controls={controls}
        note={note}
        onNoteChange={setNote}
        stepUpCode={stepUpCode}
        onStepUpCodeChange={setStepUpCode}
        fieldErrors={fieldErrors}
        isPending={isPending}
        dialogFeedback={dialogFeedback}
        onClose={closeDialog}
        onSubmitRelease={submitReleaseControls}
        onSubmitKillSwitch={submitKillSwitch}
      />
    </div>
  );
}
