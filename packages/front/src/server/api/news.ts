import "server-only";

import type { ApiListResponse, ContractListQuery } from "@/lib/contracts/api";
import type { NewsRawEventDto, NewsSourceHealthDto } from "@/lib/contracts/dto";
import { newsRawEventFixtures, newsSourceHealthFixtures } from "@/lib/server/polyedge-mock-data";
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

export async function listNewsRawEvents(
  query?: Pick<ContractListQuery, "limit" | "source_type"> & { source?: string },
): Promise<ApiListResponse<NewsRawEventDto>> {
  const applyFilters = (events: NewsRawEventDto[]) =>
    events.filter((event) => {
      if (query?.source && event.source !== query.source) {
        return false;
      }

      if (query?.source_type && event.source_type !== query.source_type) {
        return false;
      }

      return true;
    });

  return fetchListContract(
    `/api/v1/news/raw-events${buildQueryString({
      source: query?.source,
      source_type: query?.source_type,
      limit: query?.limit,
    })}`,
    createListResponse("news_raw_events", applyFilters(newsRawEventFixtures), query?.limit),
  );
}
