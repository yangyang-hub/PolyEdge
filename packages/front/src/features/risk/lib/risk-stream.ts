import type { RuntimeMode } from "@/lib/contracts/dto";
import type { RiskStreamPayload } from "@/lib/contracts/realtime";
import {
  alertSeverityTone,
  alertStatusTone,
  formatClock,
  formatCurrency,
  formatPercentFromRatio,
} from "@/lib/realtime-formatters";

import type { RiskPageData } from "../types";

export function patchMetricValues(
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

export function patchMetricsFromStream(
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

export function patchSummaryFromStream(
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

export function upsertAlert(
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
