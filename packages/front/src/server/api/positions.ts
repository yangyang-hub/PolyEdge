import "server-only";

import type { ApiListResponse, ContractListQuery } from "@/lib/contracts/api";
import type { PositionDto } from "@/lib/contracts/dto";
import { positionFixtures } from "@/lib/server/polyedge-mock-data";
import { buildQueryString, createListResponse, fetchContract } from "@/server/api/base";

export async function listPositions(query?: ContractListQuery): Promise<ApiListResponse<PositionDto>> {
  return fetchContract(
    `/api/positions${buildQueryString(query)}`,
    createListResponse("positions", positionFixtures, query?.limit),
  );
}
