import "server-only";

import type { ApiListResponse, ContractListQuery, WriteResponse } from "@/lib/contracts/api";
import type { ApprovalDto } from "@/lib/contracts/dto";
import { approvalFixtures } from "@/lib/server/polyedge-mock-data";
import {
  buildQueryString,
  createListResponse,
  createWriteResponse,
  fetchContract,
  fetchWriteContract,
} from "@/server/api/base";

export async function listApprovals(query?: ContractListQuery): Promise<ApiListResponse<ApprovalDto>> {
  const filtered = approvalFixtures.filter((approval) => {
    if (query?.status && !query.status.includes(approval.status)) {
      return false;
    }

    return true;
  });

  return fetchContract(
    `/api/system/approvals${buildQueryString(query)}`,
    createListResponse("approvals", filtered, query?.limit),
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
  const status = input.decision === "approved" ? "queued" : "rejected";

  return fetchWriteContract(
    `/api/system/approvals/${input.approvalId}/decision`,
    {
      method: "POST",
      idempotencyKey: `approval-${input.approvalId}-${input.decision}-${crypto.randomUUID()}`,
      body: {
        resource_id: input.resourceId,
        expected_version: input.expectedVersion,
        decision: input.decision,
        note: input.note,
        step_up_code: input.stepUpCode ?? null,
      },
    },
    createWriteResponse(`approval_decision_${input.approvalId}`, input.resourceId, status),
  );
}
