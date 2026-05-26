import { listEvents } from "@/lib/api/events";
import { listMarkets } from "@/lib/api/markets";
import { readRiskState, listRiskAlerts } from "@/lib/api/risk";
import { listSignals } from "@/lib/api/signals";
import { localizeGeneratedCopy } from "@/lib/i18n/generated-copy";
import type { I18nRuntime } from "@/lib/i18n/runtime";
import { indexMarkets } from "@/lib/loaders/console-loader-utils";
import { normalizeRuntimeMode } from "@/lib/runtime-mode";
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
} from "@/lib/formatters";

export async function getDashboardPageData(i18n: I18nRuntime) {
  const [{ data: markets }, { data: events }, { data: signals }, { data: alerts }, { data: riskState }] =
    await Promise.all([
      listMarkets(),
      listEvents(),
      listSignals(),
      listRiskAlerts(),
      readRiskState(),
    ]);
  const { locale, dictionary, enumLabel } = i18n;

  const marketIndex = indexMarkets(markets);
  const runtimeMode = normalizeRuntimeMode(riskState.mode);

  return {
    modeLabel: enumLabel(runtimeMode),
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
