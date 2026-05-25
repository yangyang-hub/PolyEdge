import "server-only";

import type { ApiResponse } from "@/lib/contracts/api";
import type {
  RewardBotConfigPatchDto,
  RewardBotSnapshotDto,
} from "@/lib/contracts/dto";
import { fetchContract, fetchWriteContract } from "@/server/api/base";

export async function readRewardBotSnapshot(): Promise<ApiResponse<RewardBotSnapshotDto>> {
  return fetchContract<ApiResponse<RewardBotSnapshotDto>>("/api/v1/rewards-bot");
}

export async function updateRewardBotConfig(
  patch: RewardBotConfigPatchDto,
): Promise<ApiResponse<RewardBotSnapshotDto>> {
  return fetchWriteContract<ApiResponse<RewardBotSnapshotDto>>("/api/v1/rewards-bot/config", {
    method: "POST",
    idempotencyKey: `reward-config-${crypto.randomUUID()}`,
    body: patch as Record<string, unknown>,
  });
}

export async function runRewardBotOnce(): Promise<ApiResponse<RewardBotSnapshotDto>> {
  return fetchWriteContract<ApiResponse<RewardBotSnapshotDto>>("/api/v1/rewards-bot/run", {
    method: "POST",
    idempotencyKey: `reward-run-${crypto.randomUUID()}`,
    body: {},
  });
}

export async function cancelRewardBotOrders(): Promise<ApiResponse<RewardBotSnapshotDto>> {
  return fetchWriteContract<ApiResponse<RewardBotSnapshotDto>>("/api/v1/rewards-bot/cancel-all", {
    method: "POST",
    idempotencyKey: `reward-cancel-${crypto.randomUUID()}`,
    body: {},
  });
}
