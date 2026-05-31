import type { ApiResponse } from "@/lib/contracts/api";
import type {
  RewardBotConfigPatchDto,
  RewardBotSnapshotDto,
} from "@/lib/contracts/dto";
import { buildQueryString, fetchContract, fetchWriteContract, randomUUID } from "@/lib/api/base";

export interface RewardBotSnapshotQuery {
  plans_search?: string;
  plans_eligible?: boolean;
  plans_sort_by?: string;
  plans_sort_order?: string;
  orders_search?: string;
  orders_status?: string;
  orders_sort_by?: string;
  orders_sort_order?: string;
}

export async function readRewardBotSnapshot(
  query?: RewardBotSnapshotQuery,
): Promise<ApiResponse<RewardBotSnapshotDto>> {
  return fetchContract<ApiResponse<RewardBotSnapshotDto>>(
    `/api/v1/rewards-bot${buildQueryString(query as Record<string, string | number | boolean | undefined>)}`,
  );
}

export async function updateRewardBotConfig(
  patch: RewardBotConfigPatchDto,
): Promise<ApiResponse<RewardBotSnapshotDto>> {
  return fetchWriteContract<ApiResponse<RewardBotSnapshotDto>>("/api/v1/rewards-bot/config", {
    method: "POST",
    idempotencyKey: `reward-config-${randomUUID()}`,
    body: patch as Record<string, unknown>,
  });
}

export async function runRewardBotOnce(): Promise<ApiResponse<RewardBotSnapshotDto>> {
  return fetchWriteContract<ApiResponse<RewardBotSnapshotDto>>("/api/v1/rewards-bot/run", {
    method: "POST",
    idempotencyKey: `reward-run-${randomUUID()}`,
    body: {},
  });
}

export async function cancelRewardBotOrders(): Promise<ApiResponse<RewardBotSnapshotDto>> {
  return fetchWriteContract<ApiResponse<RewardBotSnapshotDto>>("/api/v1/rewards-bot/cancel-all", {
    method: "POST",
    idempotencyKey: `reward-cancel-${randomUUID()}`,
    body: {},
  });
}

export async function resetRewardBot(): Promise<ApiResponse<RewardBotSnapshotDto>> {
  return fetchWriteContract<ApiResponse<RewardBotSnapshotDto>>("/api/v1/rewards-bot/reset", {
    method: "POST",
    idempotencyKey: `reward-reset-${randomUUID()}`,
    body: {},
  });
}
