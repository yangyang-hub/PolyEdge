import type { ApiListResponse, ContractListQuery } from "@/lib/contracts/api";
import type { EventDto, EvidenceDto } from "@/lib/contracts/dto";
import { buildQueryString, fetchListContract } from "@/lib/api/base";

export async function listEvents(query?: ContractListQuery): Promise<ApiListResponse<EventDto>> {
  const liveQuery = {
    limit: query?.limit,
    market_id: query?.market_id,
    status: query?.status?.[0],
  };

  return fetchListContract(`/api/v1/events${buildQueryString(liveQuery)}`);
}

export async function listEvidences(query?: ContractListQuery): Promise<ApiListResponse<EvidenceDto>> {
  const liveQuery = {
    limit: query?.limit,
    event_id: query?.event_id,
    market_id: query?.market_id,
    status: query?.status?.[0],
  };

  return fetchListContract(`/api/v1/evidences${buildQueryString(liveQuery)}`);
}
