import { listRiskAlerts, listRiskBuckets, readRiskState } from "@/lib/api/risk";
import { localizeGeneratedCopy } from "@/lib/i18n/generated-copy";
import type { I18nRuntime } from "@/lib/i18n/runtime";
import { normalizeRuntimeMode } from "@/lib/runtime-mode";
import {
  alertSeverityTone,
  alertStatusTone,
  formatBucketWidth,
  formatClock,
  formatCurrency,
  formatPercentFromRatio,
} from "@/lib/formatters";

export async function getRiskPageData(i18n: I18nRuntime) {
  const [{ data: riskState }, { data: alerts }, { data: riskBuckets }] = await Promise.all([
    readRiskState(),
    listRiskAlerts(),
    listRiskBuckets(),
  ]);
  const { locale, dictionary, enumLabel } = i18n;

  const dailyLossUsage = (
    Number.parseFloat(riskState.daily_loss_used) / Number.parseFloat(riskState.daily_loss_limit)
  ).toFixed(2);
  const criticalAlerts = alerts.filter((alert) => alert.severity === "critical").length;
  const warningAlerts = alerts.filter((alert) => alert.severity === "warning").length;
  const runtimeMode = normalizeRuntimeMode(riskState.mode);

  return {
    controls: {
      mode: runtimeMode,
      modeLabel: enumLabel(runtimeMode),
      killSwitch: riskState.kill_switch,
      environment: riskState.environment,
    },
    summary: {
      dailyLossUsed: formatCurrency(riskState.daily_loss_used),
      dailyLossLimit: formatCurrency(riskState.daily_loss_limit),
      dailyLossUsage: formatPercentFromRatio(dailyLossUsage),
      dailyLossWidth: formatBucketWidth(dailyLossUsage),
      grossExposure: formatPercentFromRatio(riskState.gross_exposure),
      netExposure: formatPercentFromRatio(riskState.net_exposure),
      longBiasLabel: `${dictionary.metricHints.deskBias} ${formatPercentFromRatio(riskState.net_exposure)}`,
      criticalAlerts,
      warningAlerts,
    },
    metrics: [
      {
        key: "mode",
        title: dictionary.metrics.mode,
        value: enumLabel(runtimeMode),
        hint: dictionary.metricHints.activeRuntime,
        tone: "primary" as const,
      },
      {
        key: "kill_switch",
        title: dictionary.metrics.killSwitch,
        value: riskState.kill_switch ? dictionary.common.active : dictionary.common.armed,
        hint: riskState.kill_switch ? dictionary.metricHints.halted : dictionary.metricHints.readyState,
        tone: riskState.kill_switch ? ("danger" as const) : ("primary" as const),
      },
      {
        key: "daily_loss_usage",
        title: dictionary.metrics.dailyLossUsage,
        value: formatPercentFromRatio(dailyLossUsage),
        hint: `${formatCurrency(riskState.daily_loss_used)} / ${formatCurrency(riskState.daily_loss_limit)}`,
        tone: "danger" as const,
      },
      {
        key: "open_alerts",
        title: dictionary.metrics.openAlerts,
        value: String(riskState.open_alerts),
        hint: `${alerts.filter((alert) => alert.severity === "critical").length} ${dictionary.common.critical}`,
        tone: "violet" as const,
      },
    ],
    alerts: alerts.map((alert) => ({
      id: alert.id,
      severity: alert.severity,
      severityTone: alertSeverityTone(alert.severity),
      reason: localizeGeneratedCopy(locale, dictionary, alert.reason),
      target: localizeGeneratedCopy(locale, dictionary, alert.target),
      createdAt: formatClock(alert.created_at),
      status: alert.status,
      statusLabel: enumLabel(alert.status),
      statusTone: alertStatusTone(alert.status),
    })),
    riskBuckets: riskBuckets.map((bucket) => ({
      id: bucket.id,
      name: bucket.name,
      exposure: formatPercentFromRatio(bucket.exposure),
      width: formatBucketWidth(bucket.exposure),
    })),
  };
}
