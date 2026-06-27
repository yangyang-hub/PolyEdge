import type { ApiResponse } from "@/lib/contracts/api";
import type {
  SmartMoneyConfigPatchDto,
  SmartMoneySnapshotDto,
  SmartWalletCandidateStatusUpdateDto,
} from "@/lib/contracts/dto";
import { fetchContract, fetchWriteContract, randomUUID } from "@/lib/api/base";

export async function readSmartMoneySnapshot(): Promise<ApiResponse<SmartMoneySnapshotDto>> {
  return fetchContract<ApiResponse<SmartMoneySnapshotDto>>("/api/v1/smart-money");
}

export async function updateSmartMoneyConfig(
  patch: SmartMoneyConfigPatchDto,
): Promise<ApiResponse<SmartMoneySnapshotDto>> {
  return fetchWriteContract<ApiResponse<SmartMoneySnapshotDto>>("/api/v1/smart-money/config", {
    method: "POST",
    idempotencyKey: `smart-money-config-${randomUUID()}`,
    body: patch as Record<string, unknown>,
  });
}

export async function updateSmartMoneyCandidateStatus(
  input: SmartWalletCandidateStatusUpdateDto,
): Promise<ApiResponse<SmartMoneySnapshotDto>> {
  return fetchWriteContract<ApiResponse<SmartMoneySnapshotDto>>("/api/v1/smart-money/candidates/status", {
    method: "POST",
    idempotencyKey: `smart-money-candidate-status-${randomUUID()}`,
    body: input as Record<string, unknown>,
  });
}
