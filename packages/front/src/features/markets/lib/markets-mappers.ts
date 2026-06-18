import type { EventDto, MarketDto } from "@/lib/contracts/dto";
import { ambiguityTone, formatPercentFromRatio, marketTradabilityTone } from "@/lib/formatters";
import { translateEnum } from "@/lib/i18n/dictionaries";

import type { MarketDetailViewModel, MarketViewModel } from "../types";

/** 把后端 MarketDto + EventDto 映射成表格行视图模型；events 只用来 join 关联事件计数。 */
export function mapMarkets(markets: MarketDto[], events: EventDto[]): MarketViewModel[] {
  return markets.map((market) => ({
    id: market.id,
    question: market.question,
    category: market.category,
    midPrice: market.mid_price,
    volume24h: market.volume_24h,
    tradabilityStatus: market.tradability_status,
    tradabilityLabel: translateEnum(market.tradability_status),
    tradabilityTone: marketTradabilityTone(market.tradability_status),
    ambiguityLabel: translateEnum(market.ambiguity_level),
    ambiguityTone: ambiguityTone(market.ambiguity_level),
    linkedEventCount: String(
      events.filter((event) => event.related_market_ids.includes(market.id)).length,
    ).padStart(2, "0"),
  }));
}

/** 把后端 MarketDto + EventDto 映射成右侧结算/事件详情视图模型。 */
export function mapMarketDetails(
  markets: MarketDto[],
  events: EventDto[],
): MarketDetailViewModel[] {
  return markets.map((market) => ({
    id: market.id,
    question: market.question,
    category: market.category,
    polymarketConditionId: market.polymarket_condition_id ?? null,
    slug: market.slug ?? null,
    tradabilityLabel: translateEnum(market.tradability_status),
    tradabilityTone: marketTradabilityTone(market.tradability_status),
    ambiguityLabel: translateEnum(market.ambiguity_level),
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
  }));
}
