import "server-only";

import { listEvents } from "@/server/api/events";
import { listMarkets } from "@/server/api/markets";
import { readRiskState, listRiskAlerts } from "@/server/api/risk";
import { listSignals } from "@/server/api/signals";
import { listApprovals } from "@/server/api/system";
import { getPendingSignalApprovalIds, indexMarkets } from "@/server/loaders/console-loader-utils";
import {
  alertSeverityTone,
  approvalSeverityTone,
  formatClock,
  formatCurrency,
  formatPercentFromRatio,
  formatSignedFixed,
  humanizeSnakeCase,
  marketTradabilityTone,
  metricToneForPnl,
  signalStateTone,
  uppercaseEnum,
} from "@/lib/server/console-formatters";

export async function getDashboardPageData() {
  const [{ data: markets }, { data: events }, { data: signals }, { data: approvals }, { data: alerts }, { data: riskState }] =
    await Promise.all([
      listMarkets(),
      listEvents(),
      listSignals(),
      listApprovals(),
      listRiskAlerts(),
      readRiskState(),
    ]);

  const marketIndex = indexMarkets(markets);
  const pendingApprovals = approvals.filter((approval) => approval.status === "pending");
  const pendingSignalApprovalIds = getPendingSignalApprovalIds(approvals);

  return {
    modeLabel: humanizeSnakeCase(riskState.mode),
    environmentLabel: riskState.environment,
    metrics: [
      {
        title: "Daily PnL",
        value: formatCurrency(riskState.daily_pnl),
        hint: formatClock(riskState.updated_at),
        tone: metricToneForPnl(riskState.daily_pnl),
      },
      {
        title: "Gross Exposure",
        value: formatPercentFromRatio(riskState.gross_exposure),
        hint: "desk gross",
        tone: "primary" as const,
      },
      {
        title: "Open Alerts",
        value: String(riskState.open_alerts),
        hint: "risk state",
        tone: alerts.some((alert) => alert.severity === "critical") ? ("danger" as const) : ("primary" as const),
      },
      {
        title: "Pending Approvals",
        value: String(pendingApprovals.length),
        hint: `avg ${formatClock(pendingApprovals[0]?.created_at ?? riskState.updated_at)}`,
        tone: "violet" as const,
      },
    ],
    signals: signals.map((signal) => ({
      id: signal.id,
      marketQuestion: marketIndex.get(signal.market_id)?.question ?? signal.market_id,
      side: uppercaseEnum(signal.side),
      edge: formatSignedFixed(signal.edge),
      confidence: formatPercentFromRatio(signal.confidence),
      confidenceWidth: formatPercentFromRatio(signal.confidence),
      stateLabel: humanizeSnakeCase(signal.lifecycle_state),
      stateTone: signalStateTone(signal.lifecycle_state),
      hasPendingApproval: pendingSignalApprovalIds.has(signal.id),
    })),
    alerts: alerts.slice(0, 3).map((alert) => ({
      id: alert.id,
      severity: alert.severity,
      severityTone: alertSeverityTone(alert.severity),
      createdAt: formatClock(alert.created_at),
      reason: alert.reason,
      target: alert.target,
    })),
    approvals: pendingApprovals.slice(0, 3).map((approval) => ({
      id: approval.id,
      typeLabel: humanizeSnakeCase(approval.type),
      severityTone: approvalSeverityTone(approval.severity),
      createdAt: formatClock(approval.created_at),
      summary: approval.summary,
    })),
    markets: markets.map((market) => ({
      id: market.id,
      question: market.question,
      category: market.category,
      midPrice: market.mid_price,
      tradabilityLabel: humanizeSnakeCase(market.tradability_status),
      tradabilityTone: marketTradabilityTone(market.tradability_status),
    })),
    events: events.map((event) => ({
      id: event.id,
      source: event.source,
      confidence: formatPercentFromRatio(event.confidence),
      summary: event.summary,
    })),
  };
}
