import "server-only";

import type { ApiListResponse, ContractListQuery } from "@/lib/contracts/api";
import type { NewsSourceHealthDto } from "@/lib/contracts/dto";
import { newsSourceHealthFixtures } from "@/lib/server/polyedge-mock-data";
import { buildQueryString, createListResponse, fetchListContract } from "@/server/api/base";

export async function listNewsSourceHealth(
  query?: Pick<ContractListQuery, "limit" | "source_type">,
): Promise<ApiListResponse<NewsSourceHealthDto>> {
  const applyFilters = (sources: NewsSourceHealthDto[]) =>
    sources.filter((source) => {
      if (query?.source_type && source.source_type !== query.source_type) {
        return false;
      }

      return true;
    });

  return fetchListContract(
    `/api/v1/news/source-health${buildQueryString({
      source_type: query?.source_type,
      limit: query?.limit,
    })}`,
    createListResponse("news_source_health", applyFilters(newsSourceHealthFixtures), query?.limit),
  );
}
