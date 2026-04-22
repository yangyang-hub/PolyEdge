import "server-only";

import type { ApiListResponse, ContractListQuery, WriteResponse } from "@/lib/contracts/api";
import type { ApprovalDto } from "@/lib/contracts/dto";
import { approvalFixtures } from "@/lib/server/polyedge-mock-data";
import {
  createListResponse,
  createWriteResponse,
  fetchWriteContract,
} from "@/server/api/base";
import { readDerivedLiveApprovals } from "@/server/api/live-console-derived";

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
  const applyFilters = (approvals: ApprovalDto[]) => approvals.filter((approval) => {
    if (query?.status && !query.status.includes(approval.status)) {
      return false;
    }

    return true;
  });

  if (!process.env.POLYEDGE_API_BASE_URL) {
    return createListResponse("approvals", applyFilters(approvalFixtures), query?.limit);
  }

  const { data, meta } = await readDerivedLiveApprovals();
  const filtered = applyFilters(data);
  const limited = query?.limit ? filtered.slice(0, query.limit) : filtered;

  return {
    data: limited,
    page: {
      limit: query?.limit ?? limited.length,
      next_cursor: null,
      has_more: filtered.length > limited.length,
    },
    meta,
  };
}

export async function submitApprovalDecision(input: {
  approvalId: string;
  resourceId: string;
  expectedVersion: number;
  decision: "approved" | "rejected";
  note: string;
  stepUpCode?: string;
}): Promise<WriteResponse> {
  const status = input.decision === "approved" ? "queued" : "rejected";
  const isSignalDecision = input.resourceId.startsWith("sig_");

  if (!isSignalDecision && process.env.POLYEDGE_API_BASE_URL) {
    throw new Error("Live approval decisions are only wired for signal resources. Mode-switch and kill-switch queues remain mock-only.");
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
    createWriteResponse(`approval_decision_${input.approvalId}`, input.resourceId, status),
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
