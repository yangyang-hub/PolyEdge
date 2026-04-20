import "server-only";

import type { ApiListResponse, ContractListQuery } from "@/lib/contracts/api";
import type { SignalDto } from "@/lib/contracts/dto";
import { signalFixtures } from "@/lib/server/polyedge-mock-data";
import { buildQueryString, createListResponse, fetchContract } from "@/server/api/base";

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

  return fetchContract(
    `/api/signals${buildQueryString(query)}`,
    createListResponse("signals", filtered, query?.limit),
  );
}
