import "server-only";

import { listEvents } from "@/server/api/events";
import { listMarkets } from "@/server/api/markets";
import { readRiskState, listRiskAlerts } from "@/server/api/risk";
import { listSignals } from "@/server/api/signals";
import { listApprovals } from "@/server/api/system";
import { localizeGeneratedCopy } from "@/lib/i18n/generated-copy";
import { getServerI18n } from "@/lib/i18n/server";
import { getPendingSignalApprovalIds, indexMarkets } from "@/server/loaders/console-loader-utils";
import {
  alertSeverityTone,
  approvalSeverityTone,
  formatClock,
  formatCurrency,
  formatPercentFromRatio,
  formatSignedFixed,
  marketTradabilityTone,
  metricToneForPnl,
  signalStateTone,
  uppercaseEnum,
} from "@/lib/server/console-formatters";

export async function getDashboardPageData() {
  const [{ data: markets }, { data: events }, { data: signals }, { data: approvals }, { data: alerts }, { data: riskState }, i18n] =
    await Promise.all([
      listMarkets(),
      listEvents(),
      listSignals(),
      listApprovals(),
      listRiskAlerts(),
      readRiskState(),
      getServerI18n(),
    ]);
  const { locale, dictionary, enumLabel, format } = i18n;

  const marketIndex = indexMarkets(markets);
  const pendingApprovals = approvals.filter((approval) => approval.status === "pending");
  const pendingSignalApprovalIds = getPendingSignalApprovalIds(approvals);

  return {
    modeLabel: enumLabel(riskState.mode),
    environmentLabel: riskState.environment,
    metrics: [
      {
        key: "daily_pnl",
        title: dictionary.metrics.dailyPnl,
        value: formatCurrency(riskState.daily_pnl),
        hint: formatClock(riskState.updated_at),
        tone: metricToneForPnl(riskState.daily_pnl),
      },
      {
        key: "gross_exposure",
        title: dictionary.metrics.grossExposure,
        value: formatPercentFromRatio(riskState.gross_exposure),
        hint: dictionary.metricHints.deskGross,
        tone: "primary" as const,
      },
      {
        key: "open_alerts",
        title: dictionary.metrics.openAlerts,
        value: String(riskState.open_alerts),
        hint: dictionary.metricHints.riskState,
        tone: alerts.some((alert) => alert.severity === "critical") ? ("danger" as const) : ("primary" as const),
      },
      {
        key: "pending_approvals",
        title: dictionary.metrics.pendingApprovals,
        value: String(pendingApprovals.length),
        hint: format(dictionary.metricHints.avg, {
          time: formatClock(pendingApprovals[0]?.created_at ?? riskState.updated_at),
        }),
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
      stateLabel: enumLabel(signal.lifecycle_state),
      stateTone: signalStateTone(signal.lifecycle_state),
      hasPendingApproval: pendingSignalApprovalIds.has(signal.id),
    })),
    alerts: alerts.slice(0, 3).map((alert) => ({
      id: alert.id,
      severity: alert.severity,
      severityTone: alertSeverityTone(alert.severity),
      createdAt: formatClock(alert.created_at),
      reason: localizeGeneratedCopy(locale, dictionary, alert.reason),
      target: localizeGeneratedCopy(locale, dictionary, alert.target),
    })),
    approvals: pendingApprovals.slice(0, 3).map((approval) => ({
      id: approval.id,
      typeLabel: enumLabel(approval.type),
      severityTone: approvalSeverityTone(approval.severity),
      createdAt: formatClock(approval.created_at),
      summary: localizeGeneratedCopy(locale, dictionary, approval.summary),
    })),
    markets: markets.map((market) => ({
      id: market.id,
      question: market.question,
      category: market.category,
      midPrice: market.mid_price,
      tradabilityLabel: enumLabel(market.tradability_status),
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
