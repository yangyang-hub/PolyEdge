import { listEvents } from "@/lib/api/events";
import { listMarkets } from "@/lib/api/markets";
import { listNewsSourceHealth } from "@/lib/api/news";
import { dictionary, translateEnum } from "@/lib/i18n/dictionaries";
import {
  formatClock,
  formatInteger,
  formatPercentFromRatio,
  marketTradabilityTone,
} from "@/lib/formatters";

export async function getDashboardPageData() {
  const [{ data: markets }, { data: events }, { data: sourceHealth }] =
    await Promise.all([
      listMarkets(),
      listEvents(),
      listNewsSourceHealth({ limit: 10 }),
    ]);

  const tradableMarkets = markets.filter((market) => market.tradability_status === "tradable").length;
  const activeEvents = events.filter((event) => event.status === "active").length;
  const degradedSources = sourceHealth.filter((source) => source.consecutive_failures > 0).length;

  return {
    metrics: [
      {
        key: "markets",
        title: dictionary.metrics.coveredMarkets,
        value: formatInteger(markets.length),
        hint: dictionary.metricHints.trackedMarkets,
        tone: "primary" as const,
      },
      {
        key: "tradable_markets",
        title: dictionary.metrics.tradableMarkets,
        value: formatInteger(tradableMarkets),
        hint: dictionary.metricHints.tradableMarkets,
        tone: "success" as const,
      },
      {
        key: "active_events",
        title: dictionary.metrics.activeEvents,
        value: formatInteger(activeEvents),
        hint: dictionary.metricHints.newsEvents,
        tone: "violet" as const,
      },
      {
        key: "news_sources",
        title: dictionary.metrics.newsSources,
        value: formatInteger(sourceHealth.length),
        hint: degradedSources > 0
          ? `${formatInteger(degradedSources)} ${dictionary.dashboard.degradedSources}`
          : dictionary.common.healthy,
        tone: degradedSources > 0 ? ("danger" as const) : ("success" as const),
      },
    ],
    markets: markets.map((market) => ({
      id: market.id,
      question: market.question,
      category: market.category,
      midPrice: market.mid_price,
      tradabilityLabel: translateEnum(market.tradability_status),
      tradabilityTone: marketTradabilityTone(market.tradability_status),
    })),
    events: events.map((event) => ({
      id: event.id,
      source: event.source,
      confidence: formatPercentFromRatio(event.confidence),
      summary: event.summary,
    })),
    sourceHealth: sourceHealth.map((source) => ({
      source: source.source,
      typeLabel: translateEnum(source.source_type),
      updatedAtLabel: formatClock(source.updated_at),
      healthScore: formatPercentFromRatio(source.health_score),
      tone: source.consecutive_failures > 0 ? ("warning" as const) : ("success" as const),
    })),
  };
}
