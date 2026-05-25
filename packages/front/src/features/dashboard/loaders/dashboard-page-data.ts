import "server-only";

import { listEvents } from "@/server/api/events";
import { listMarkets } from "@/server/api/markets";
import { readRiskState, listRiskAlerts } from "@/server/api/risk";
import { listSignals } from "@/server/api/signals";
import { localizeGeneratedCopy } from "@/lib/i18n/generated-copy";
import { getServerI18n } from "@/lib/i18n/server";
import { indexMarkets } from "@/server/loaders/console-loader-utils";
import {
  alertSeverityTone,
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
  const [{ data: markets }, { data: events }, { data: signals }, { data: alerts }, { data: riskState }, i18n] =
    await Promise.all([
      listMarkets(),
      listEvents(),
      listSignals(),
      listRiskAlerts(),
      readRiskState(),
      getServerI18n(),
    ]);
  const { locale, dictionary, enumLabel } = i18n;

  const marketIndex = indexMarkets(markets);

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
    })),
    alerts: alerts.slice(0, 3).map((alert) => ({
      id: alert.id,
      severity: alert.severity,
      severityTone: alertSeverityTone(alert.severity),
      createdAt: formatClock(alert.created_at),
      reason: localizeGeneratedCopy(locale, dictionary, alert.reason),
      target: localizeGeneratedCopy(locale, dictionary, alert.target),
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
