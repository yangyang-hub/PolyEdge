import "server-only";

import { listRiskAlerts, listRiskBuckets, readRiskState } from "@/server/api/risk";
import { listApprovals } from "@/server/api/system";
import {
  alertSeverityTone,
  alertStatusTone,
  formatBucketWidth,
  formatClock,
  formatCurrency,
  formatPercentFromRatio,
  humanizeSnakeCase,
} from "@/lib/server/console-formatters";

export async function getRiskPageData() {
  const [{ data: riskState }, { data: alerts }, { data: riskBuckets }, { data: approvals }] = await Promise.all([
    readRiskState(),
    listRiskAlerts(),
    listRiskBuckets(),
    listApprovals(),
  ]);

  const dailyLossUsage = (
    Number.parseFloat(riskState.daily_loss_used) / Number.parseFloat(riskState.daily_loss_limit)
  ).toFixed(2);
  const criticalAlerts = alerts.filter((alert) => alert.severity === "critical").length;
  const warningAlerts = alerts.filter((alert) => alert.severity === "warning").length;
  const pendingApprovals = approvals.filter((approval) => approval.status === "pending");

  return {
    controls: {
      mode: riskState.mode,
      modeLabel: humanizeSnakeCase(riskState.mode),
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
      longBiasLabel: `long bias ${formatPercentFromRatio(riskState.net_exposure)}`,
      criticalAlerts,
      warningAlerts,
    },
    metrics: [
      {
        title: "Mode",
        value: humanizeSnakeCase(riskState.mode),
        hint: "active runtime",
        tone: "primary" as const,
      },
      {
        title: "Kill Switch",
        value: riskState.kill_switch ? "active" : "armed",
        hint: riskState.kill_switch ? "halted" : "ready state",
        tone: riskState.kill_switch ? ("danger" as const) : ("primary" as const),
      },
      {
        title: "Daily Loss Usage",
        value: formatPercentFromRatio(dailyLossUsage),
        hint: `${formatCurrency(riskState.daily_loss_used)} / ${formatCurrency(riskState.daily_loss_limit)}`,
        tone: "danger" as const,
      },
      {
        title: "Open Alerts",
        value: String(riskState.open_alerts),
        hint: `${alerts.filter((alert) => alert.severity === "critical").length} critical`,
        tone: "violet" as const,
      },
    ],
    alerts: alerts.map((alert) => ({
      id: alert.id,
      severity: alert.severity,
      severityTone: alertSeverityTone(alert.severity),
      reason: alert.reason,
      target: alert.target,
      createdAt: formatClock(alert.created_at),
      statusLabel: humanizeSnakeCase(alert.status),
      statusTone: alertStatusTone(alert.status),
    })),
    riskBuckets: riskBuckets.map((bucket) => ({
      id: bucket.id,
      name: bucket.name,
      exposure: formatPercentFromRatio(bucket.exposure),
      width: formatBucketWidth(bucket.exposure),
    })),
    approvalCount: pendingApprovals.length,
  };
}
