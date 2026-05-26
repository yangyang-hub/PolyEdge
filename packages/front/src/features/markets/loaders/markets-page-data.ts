import { listEvents } from "@/lib/api/events";
import { listMarkets } from "@/lib/api/markets";
import type { I18nRuntime } from "@/lib/i18n/runtime";
import { selectFirstMatchingItem } from "@/lib/loaders/console-loader-utils";
import {
  ambiguityTone,
  formatPercentFromRatio,
  marketTradabilityTone,
} from "@/lib/formatters";

export async function getMarketsPageData(i18n: I18nRuntime) {
  const [{ data: markets }, { data: events }] = await Promise.all([
    listMarkets(),
    listEvents(),
  ]);
  const { dictionary, enumLabel } = i18n;
  const selectedMarket = selectFirstMatchingItem(
    markets,
    [
      (market) => market.tradability_status === "blocked",
      (market) => market.tradability_status === "observe_only",
    ],
    dictionary.routeStates.marketsDataRequired,
  );

  return {
    selectedMarketId: selectedMarket.id,
    markets: markets.map((market) => ({
      id: market.id,
      question: market.question,
      category: market.category,
      midPrice: market.mid_price,
      tradabilityStatus: market.tradability_status,
      tradabilityLabel: enumLabel(market.tradability_status),
      tradabilityTone: marketTradabilityTone(market.tradability_status),
      ambiguityLabel: enumLabel(market.ambiguity_level),
      ambiguityTone: ambiguityTone(market.ambiguity_level),
      linkedEventCount: String(events.filter((event) => event.related_market_ids.includes(market.id)).length).padStart(2, "0"),
    })),
    marketDetails: markets.map((market) => ({
      id: market.id,
      question: market.question,
      category: market.category,
      tradabilityLabel: enumLabel(market.tradability_status),
      tradabilityTone: marketTradabilityTone(market.tradability_status),
      ambiguityLabel: enumLabel(market.ambiguity_level),
      ambiguityTone: ambiguityTone(market.ambiguity_level),
      resolutionSource: market.resolution_source,
      edgeCaseNotes: market.edge_case_notes,
      linkedEvents: events
        .filter((event) => event.related_market_ids.includes(market.id))
        .map((event) => ({
          id: event.id,
          source: event.source,
          relevance: formatPercentFromRatio(event.relevance_score),
          summary: event.summary,
        })),
    })),
  };
}
