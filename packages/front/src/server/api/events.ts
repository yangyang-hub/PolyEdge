import "server-only";

import type { ApiListResponse, ContractListQuery } from "@/lib/contracts/api";
import type { EventDto, EvidenceDto } from "@/lib/contracts/dto";
import { eventFixtures, evidenceFixtures } from "@/lib/server/polyedge-mock-data";
import { buildQueryString, createListResponse, fetchContract } from "@/server/api/base";

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

  return fetchContract(
    `/api/events${buildQueryString(query)}`,
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

  return fetchContract(
    `/api/evidences${buildQueryString(query)}`,
    createListResponse("evidences", filtered, query?.limit),
  );
}
