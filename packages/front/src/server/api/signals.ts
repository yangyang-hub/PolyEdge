import "server-only";

import type { ApiListResponse, ContractListQuery } from "@/lib/contracts/api";
import type { SignalDto } from "@/lib/contracts/dto";
import { signalFixtures } from "@/lib/server/polyedge-mock-data";
import { buildQueryString, createListResponse, fetchListContract } from "@/server/api/base";

export async function listSignals(query?: ContractListQuery): Promise<ApiListResponse<SignalDto>> {
  const filtered = signalFixtures.filter((signal) => {
    if (query?.market_id && signal.market_id !== query.market_id) {
      return false;
    }

    if (query?.event_id && signal.event_id !== query.event_id) {
      return false;
    }

    if (query?.signal_state && !query.signal_state.includes(signal.lifecycle_state)) {
      return false;
    }

    return true;
  });

  const liveQuery = {
    limit: query?.limit,
    event_id: query?.event_id,
    market_id: query?.market_id,
    status: query?.signal_state?.[0] ?? query?.status?.[0],
  };

  return fetchListContract(
    `/api/v1/signals${buildQueryString(liveQuery)}`,
    createListResponse("signals", filtered, query?.limit),
  );
}
