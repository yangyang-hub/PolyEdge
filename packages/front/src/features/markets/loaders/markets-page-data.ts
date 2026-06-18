import { listEvents } from "@/lib/api/events";
import { listMarkets, listMarketCategories, type MarketCategory, type MarketListParams } from "@/lib/api/markets";
import { dictionary } from "@/lib/i18n/dictionaries";
import { selectFirstMatchingItem } from "@/lib/loaders/console-loader-utils";
import { mapMarkets, mapMarketDetails } from "../lib/markets-mappers";

export type { MarketListParams, MarketCategory };

export async function getMarketsPageData(params?: MarketListParams) {
  const [{ data: markets, totalCount }, { data: events }, categories] = await Promise.all([
    listMarkets(params),
    listEvents(),
    listMarketCategories(),
  ]);
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
    totalCount,
    categories,
    /** 原始事件列表，供客户端筛选/翻页时复用，避免每次重新请求 events。 */
    events,
    markets: mapMarkets(markets, events),
    marketDetails: mapMarketDetails(markets, events),
  };
}
