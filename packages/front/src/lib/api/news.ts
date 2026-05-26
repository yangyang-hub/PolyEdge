import type { ApiListResponse, ContractListQuery } from "@/lib/contracts/api";
import type { NewsRawEventDto, NewsSourceHealthDto } from "@/lib/contracts/dto";
import { buildQueryString, fetchListContract } from "@/lib/api/base";

export async function listNewsSourceHealth(
  query?: Pick<ContractListQuery, "limit" | "source_type">,
): Promise<ApiListResponse<NewsSourceHealthDto>> {
  return fetchListContract(
    `/api/v1/news/source-health${buildQueryString({
      source_type: query?.source_type,
      limit: query?.limit,
    })}`,
  );
}

export async function listNewsRawEvents(
  query?: Pick<ContractListQuery, "limit" | "source_type"> & { source?: string },
): Promise<ApiListResponse<NewsRawEventDto>> {
  return fetchListContract(
    `/api/v1/news/raw-events${buildQueryString({
      source: query?.source,
      source_type: query?.source_type,
      limit: query?.limit,
    })}`,
  );
}
