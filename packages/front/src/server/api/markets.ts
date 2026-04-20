import "server-only";

import type { ApiListResponse, ContractListQuery } from "@/lib/contracts/api";
import type { MarketDto } from "@/lib/contracts/dto";
import { marketFixtures } from "@/lib/server/polyedge-mock-data";
import { buildQueryString, createListResponse, fetchContract } from "@/server/api/base";

export async function listMarkets(query?: ContractListQuery): Promise<ApiListResponse<MarketDto>> {
  return fetchContract(
    `/api/markets${buildQueryString(query)}`,
    createListResponse("markets", marketFixtures, query?.limit),
  );
}
