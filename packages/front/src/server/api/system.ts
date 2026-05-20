import "server-only";

import type { ApiListResponse, ContractListQuery, WriteResponse } from "@/lib/contracts/api";
import type { ApprovalDto } from "@/lib/contracts/dto";
import {
  buildQueryString,
  createWriteResponse,
  fetchListContract,
  fetchWriteContract,
} from "@/server/api/base";

type LiveSignalDecisionResponse = {
  data: {
    replayed: boolean;
    signal: {
      id: string;
      version: number;
    };
  };
};

export async function listApprovals(query?: ContractListQuery): Promise<ApiListResponse<ApprovalDto>> {
  return fetchListContract(
    `/api/v1/approvals${buildQueryString({
      status: query?.status?.[0],
      limit: query?.limit,
    })}`,
  );
}

export async function submitApprovalDecision(input: {
  approvalId: string;
  resourceId: string;
  expectedVersion: number;
  decision: "approved" | "rejected";
  note: string;
  stepUpCode?: string;
}): Promise<WriteResponse> {
  const isSignalDecision = input.resourceId.startsWith("sig_");

  if (!isSignalDecision) {
    throw new Error("Live approval decisions are only wired for signal resources. Use the risk controls endpoints for mode and kill-switch changes.");
  }

  const path = input.decision === "approved"
    ? `/api/v1/signals/${input.resourceId}/approve`
    : `/api/v1/signals/${input.resourceId}/reject`;
  const stepUpScope = input.decision === "approved" ? "signal_approve" : "signal_reject";

  return fetchWriteContract(
    path,
    {
      method: "POST",
      idempotencyKey: `approval-${input.approvalId}-${input.decision}-${crypto.randomUUID()}`,
      body: {
        expected_version: input.expectedVersion,
        reason: input.note,
      },
      stepUpCode: input.stepUpCode,
      stepUpScopes: [stepUpScope],
    },
    {
      mapLiveResponse: (payload: LiveSignalDecisionResponse) =>
        createWriteResponse(
          `signal_${input.decision}_${payload.data.signal.id}_${payload.data.signal.version}`,
          payload.data.signal.id,
          input.decision === "approved" ? "completed" : "rejected",
        ),
    },
  );
}
