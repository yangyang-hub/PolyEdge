import type { MarketListParams } from "@/lib/api/markets";

import type { MarketFilter, SortDir } from "../types";

export const MARKETS_PAGE_SIZE = 20;

/** 由当前筛选/分类/排序/分页状态构造后端 listMarkets 查询参数。 */
export function buildMarketListParams(
  filter: MarketFilter,
  category: string,
  sortDir: SortDir,
  page: number,
): MarketListParams {
  const tradabilityStatus =
    filter === "review_queue"
      ? "blocked"
      : filter === "watch_only"
        ? "observe_only"
        : undefined;

  return {
    limit: MARKETS_PAGE_SIZE,
    offset: (page - 1) * MARKETS_PAGE_SIZE,
    tradability_status: tradabilityStatus,
    category: category !== "all" ? category : undefined,
    sort_by: sortDir !== "none" ? "volume_24h" : undefined,
    sort_order: sortDir !== "none" ? sortDir : undefined,
  };
}
