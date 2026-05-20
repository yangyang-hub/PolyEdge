import "server-only";

import type { ApiListResponse, ApiResponse, ContractListQuery, WriteResponse } from "@/lib/contracts/api";
import type { SignalDto } from "@/lib/contracts/dto";
import {
  buildQueryString,
  createWriteResponse,
  fetchListContract,
  fetchWriteContract,
} from "@/server/api/base";

type LiveSignalDecisionResponse = ApiResponse<{
  replayed: boolean;
  signal: {
    id: string;
    version: number;
  };
}>;

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

export async function submitSignalDecision(input: {
  signalId: string;
  expectedVersion: number;
  decision: "approved" | "rejected";
  note: string;
  stepUpCode?: string;
}): Promise<WriteResponse> {
  const path = input.decision === "approved"
    ? `/api/v1/signals/${input.signalId}/approve`
    : `/api/v1/signals/${input.signalId}/reject`;
  const status = input.decision === "approved" ? "completed" : "rejected";

  return fetchWriteContract<LiveSignalDecisionResponse, WriteResponse>(
    path,
    {
      method: "POST",
      idempotencyKey: `signal-${input.signalId}-${input.decision}-${crypto.randomUUID()}`,
      body: {
        expected_version: input.expectedVersion,
        reason: input.note,
      },
      stepUpCode: input.stepUpCode,
      stepUpScopes: [input.decision === "approved" ? "signal_approve" : "signal_reject"],
    },
    {
      mapLiveResponse: (payload) =>
        createWriteResponse(
          `signal_${input.decision}_${payload.data.signal.id}_${payload.data.signal.version}`,
          payload.data.signal.id,
          status,
        ),
    },
  );
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
      idempotencyKey: `execution-${input.signalId}-${crypto.randomUUID()}`,
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
