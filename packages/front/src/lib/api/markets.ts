import type { ApiListResponse, ContractListQuery } from "@/lib/contracts/api";
import type { MarketDto } from "@/lib/contracts/dto";
import { buildQueryString, fetchListContract } from "@/lib/api/base";

export async function listMarkets(query?: ContractListQuery): Promise<ApiListResponse<MarketDto>> {
  const liveQuery = {
    limit: query?.limit,
    status: query?.status?.[0],
  };

  return fetchListContract(`/api/v1/markets${buildQueryString(liveQuery)}`);
}
