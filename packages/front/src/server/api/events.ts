import "server-only";

import type { ApiListResponse, ContractListQuery } from "@/lib/contracts/api";
import type { EventDto, EvidenceDto } from "@/lib/contracts/dto";
import { eventFixtures, evidenceFixtures } from "@/lib/server/polyedge-mock-data";
import { buildQueryString, createListResponse, fetchListContract } from "@/server/api/base";

export async function listEvents(query?: ContractListQuery): Promise<ApiListResponse<EventDto>> {
  const filtered = eventFixtures.filter((event) => {
    if (query?.market_id && !event.related_market_ids.includes(query.market_id)) {
      return false;
    }

    if (query?.status && !query.status.includes(event.status)) {
      return false;
    }

    return true;
  });

  const liveQuery = {
    limit: query?.limit,
    status: query?.status?.[0],
  };

  return fetchListContract(
    `/api/v1/events${buildQueryString(liveQuery)}`,
    createListResponse("events", filtered, query?.limit),
  );
}

export async function listEvidences(query?: ContractListQuery): Promise<ApiListResponse<EvidenceDto>> {
  const filtered = evidenceFixtures.filter((evidence) => {
    if (query?.market_id && evidence.market_id !== query.market_id) {
      return false;
    }

    if (query?.event_id && evidence.event_id !== query.event_id) {
      return false;
    }

    if (query?.status && !query.status.includes(evidence.status)) {
      return false;
    }

    return true;
  });

  const liveQuery = {
    limit: query?.limit,
    event_id: query?.event_id,
    market_id: query?.market_id,
    status: query?.status?.[0],
  };

  return fetchListContract(
    `/api/v1/evidences${buildQueryString(liveQuery)}`,
    createListResponse("evidences", filtered, query?.limit),
  );
}
