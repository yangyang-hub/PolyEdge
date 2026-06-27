import { listEvents } from "@/lib/api/events";
import { listMarkets } from "@/lib/api/markets";
import { listPositions } from "@/lib/api/positions";
import { readRiskState, listRiskBuckets } from "@/lib/api/risk";
import { listSignals } from "@/lib/api/signals";
import { localizeGeneratedCopy } from "@/lib/i18n/generated-copy";
import { dictionary, translateEnum } from "@/lib/i18n/dictionaries";
import { normalizeRuntimeMode } from "@/lib/runtime-mode";
import { sumNumericStrings } from "@/lib/loaders/console-loader-utils";
import {
  bucketTone,
  formatBucketWidth,
  formatCurrency,
  formatInteger,
  formatPercentFromRatio,
  formatSignedPercent,
  metricToneForPnl,
  signalStateTone,
  marketTradabilityTone,
  uppercaseEnum,
} from "@/lib/formatters";
import { indexMarkets, selectFirstMatchingItem } from "@/lib/loaders/console-loader-utils";

function indexLatestSignalsByMarket<T extends { market_id: string; version: number }>(signals: T[]) {
  const signalsByMarket = new Map<string, T>();

  for (const signal of signals) {
    const currentSignal = signalsByMarket.get(signal.market_id);

    if (!currentSignal || signal.version > currentSignal.version) {
      signalsByMarket.set(signal.market_id, signal);
    }
  }

  return signalsByMarket;
}

export async function getPositionsPageData() {
  const [
    { data: positions },
    { data: riskBuckets },
    { data: riskState },
    { data: markets },
    { data: signals },
    { data: events },
  ] = await Promise.all([
    listPositions(),
    listRiskBuckets(),
    readRiskState(),
    listMarkets(),
    listSignals(),
    listEvents(),
  ]);

  const realizedTotal = sumNumericStrings(positions.map((position) => position.realized_pnl));
  const unrealizedTotal = sumNumericStrings(positions.map((position) => position.unrealized_pnl));
  const marketIndex = indexMarkets(markets);
  const latestSignalsByMarket = indexLatestSignalsByMarket(signals);
  const bucketIndex = new Map(riskBuckets.map((bucket) => [bucket.name, bucket]));
  const runtimeMode = normalizeRuntimeMode(riskState.mode);

  const positionItems = positions.map((position) => {
    const market = marketIndex.get(position.market_id);
    const signal = latestSignalsByMarket.get(position.market_id);
    const bucketName = market?.category ?? position.bucket_name;
    const bucket = bucketIndex.get(bucketName);
    const linkedEvents = events.filter((event) => event.related_market_ids.includes(position.market_id));
    const totalPnlValue = (
      Number.parseFloat(position.realized_pnl) + Number.parseFloat(position.unrealized_pnl)
    ).toFixed(2);
    // Best bid/ask come from the joined market; fall back to the mark price when
    // the market has no quoted book. PnL % is total PnL over the cost basis.
    const costBasis =
      Number.parseFloat(position.quantity) * Number.parseFloat(position.average_cost);
    const pnlPercentRatio =
      costBasis > 0 ? Number.parseFloat(totalPnlValue) / costBasis : null;

    return {
      id: position.id,
      marketId: position.market_id,
      signalId: signal?.id ?? null,
      marketQuestion: market?.question ?? position.market_question,
      bucketName,
      side: uppercaseEnum(position.side),
      quantity: formatInteger(position.quantity),
      averageCost: position.average_cost,
      markPrice: position.mark_price,
      bestBid: market?.best_bid ?? position.mark_price,
      bestAsk: market?.best_ask ?? position.mark_price,
      realizedPnl: formatCurrency(position.realized_pnl),
      unrealizedPnl: formatCurrency(position.unrealized_pnl),
      pnl: formatCurrency(totalPnlValue),
      pnlValue: Number.parseFloat(totalPnlValue),
      pnlTone: metricToneForPnl(totalPnlValue),
      pnlAmount: formatCurrency(totalPnlValue),
      pnlPercent: pnlPercentRatio != null ? formatSignedPercent(pnlPercentRatio, 1) : "—",
      pnlPercentTone: metricToneForPnl(totalPnlValue),
      posterior: signal?.fair_price ?? position.mark_price,
      signalEdge: signal ? formatPercentFromRatio(signal.edge) : "0%",
      confidence: signal ? formatPercentFromRatio(signal.confidence) : "n/a",
      confidenceWidth: signal ? formatPercentFromRatio(signal.confidence) : "0%",
      signalStateLabel: signal ? translateEnum(signal.lifecycle_state) : "monitoring",
      signalStateTone: signal ? signalStateTone(signal.lifecycle_state) : ("neutral" as const),
      tradabilityLabel: market ? translateEnum(market.tradability_status) : dictionary.common.unknown,
      tradabilityTone: market ? marketTradabilityTone(market.tradability_status) : ("neutral" as const),
      bucketStatusLabel: bucket ? translateEnum(bucket.status) : dictionary.common.healthy,
      bucketStatus: bucket?.status ?? "healthy",
      bucketTone: bucket ? bucketTone(bucket.status) : ("neutral" as const),
      bucketUtilization: bucket ? formatPercentFromRatio(bucket.utilization) : "0%",
      bucketUtilizationWidth: bucket ? formatPercentFromRatio(bucket.utilization) : "0%",
      signalReason: signal
        ? localizeGeneratedCopy(dictionary, signal.reason)
        : dictionary.positions.signalFallback,
      riskDecision: signal
        ? localizeGeneratedCopy(dictionary, signal.risk_decision)
        : dictionary.positions.riskFallback,
      eventCount: linkedEvents.length,
      linkedEvents: linkedEvents.slice(0, 3).map((event) => ({
        id: event.id,
        source: event.source,
        relevance: formatPercentFromRatio(event.relevance_score),
        summary: event.summary,
      })),
      signalUpdatedAt: signal?.updated_at ?? position.updated_at,
    };
  });
  const selectedPosition = selectFirstMatchingItem(
    positionItems,
    [
      (position) => position.bucketStatus === "breach",
      (position) => position.pnlValue < 0,
    ],
    dictionary.routeStates.positionsDataRequired,
  );

  return {
    runtimeModeLabel: translateEnum(runtimeMode),
    runtimeEnvironmentLabel: riskState.environment,
    metrics: [
      {
        key: "daily_pnl",
        title: dictionary.metrics.dailyPnl,
        value: formatCurrency(riskState.daily_pnl),
        hint: dictionary.metricHints.realizedUnrealized,
        tone: metricToneForPnl(riskState.daily_pnl),
      },
      {
        key: "realized_pnl",
        title: dictionary.metrics.realized,
        value: formatCurrency(realizedTotal),
        hint: dictionary.metricHints.today,
        tone: metricToneForPnl(realizedTotal),
      },
      {
        key: "unrealized_pnl",
        title: dictionary.metrics.unrealized,
        value: formatCurrency(unrealizedTotal),
        hint: dictionary.metricHints.markToMarket,
        tone: metricToneForPnl(unrealizedTotal) === "danger" ? ("danger" as const) : ("violet" as const),
      },
      {
        key: "net_exposure",
        title: dictionary.metrics.netExposure,
        value: formatPercentFromRatio(riskState.net_exposure),
        hint: dictionary.metricHints.deskBias,
        tone: "danger" as const,
      },
    ],
    selectedPositionId: selectedPosition.id,
    positions: positionItems.map((position) => ({
      ...position,
      isSelected: position.id === selectedPosition.id,
    })),
    riskBuckets: riskBuckets.map((bucket) => ({
      id: bucket.id,
      name: bucket.name,
      exposure: formatPercentFromRatio(bucket.exposure),
      limit: formatPercentFromRatio(bucket.limit),
      utilization: formatPercentFromRatio(bucket.utilization),
      statusLabel: translateEnum(bucket.status),
      tone: bucketTone(bucket.status),
      width: formatBucketWidth(bucket.exposure),
    })),
  };
}
