import type { ApiListResponse, ApiResponse, ContractListQuery, WriteResponse } from "@/lib/contracts/api";
import type { SignalDto } from "@/lib/contracts/dto";
import {
  buildQueryString,
  createWriteResponse,
  fetchListContract,
  fetchWriteContract,
  randomUUID,
} from "@/lib/api/base";

type LiveSubmitExecutionResponse = ApiResponse<{
  replayed: boolean;
  execution_request: {
    id: string;
    signal_id: string;
    status: "queued" | "submitted" | "failed" | "canceled";
  };
}>;

export async function listSignals(query?: ContractListQuery): Promise<ApiListResponse<SignalDto>> {
  const liveQuery = {
    limit: query?.limit,
    event_id: query?.event_id,
    market_id: query?.market_id,
    status: query?.signal_state?.[0] ?? query?.status?.[0],
  };

  return fetchListContract(`/api/v1/signals${buildQueryString(liveQuery)}`);
}

export async function submitSignalExecutionRequest(input: {
  signalId: string;
  expectedVersion: number;
  limitPrice: string;
  quantity: string;
  connectorName?: string;
  note: string;
  stepUpCode?: string;
}): Promise<WriteResponse> {
  return fetchWriteContract<LiveSubmitExecutionResponse, WriteResponse>(
    `/api/v1/signals/${input.signalId}/execution-requests`,
    {
      method: "POST",
      idempotencyKey: `execution-${input.signalId}-${randomUUID()}`,
      body: {
        expected_signal_version: input.expectedVersion,
        limit_price: input.limitPrice,
        quantity: input.quantity,
        reason: input.note,
        connector_name: input.connectorName?.trim() || undefined,
      },
      stepUpCode: input.stepUpCode,
      stepUpScopes: ["execution_submit"],
    },
    {
      mapLiveResponse: (payload) =>
        createWriteResponse(
          `execution_${payload.data.execution_request.id}`,
          payload.data.execution_request.signal_id,
          payload.data.execution_request.status === "failed" || payload.data.execution_request.status === "canceled"
            ? "rejected"
            : "queued",
        ),
    },
  );
}
