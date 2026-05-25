import "server-only";

import type { ApiResponse } from "@/lib/contracts/api";
import type {
  RuntimeConfigEntryDto,
  RuntimeConfigUpdateDto,
} from "@/lib/contracts/dto";
import { fetchContract, fetchWriteContract } from "@/server/api/base";

export async function readRuntimeConfig(): Promise<ApiResponse<RuntimeConfigEntryDto[]>> {
  return fetchContract<ApiResponse<RuntimeConfigEntryDto[]>>("/api/v1/runtime-config");
}

export async function updateRuntimeConfig(
  update: RuntimeConfigUpdateDto,
): Promise<ApiResponse<RuntimeConfigEntryDto[]>> {
  return fetchWriteContract<ApiResponse<RuntimeConfigEntryDto[]>>("/api/v1/runtime-config", {
    method: "POST",
    idempotencyKey: `runtime-config-${crypto.randomUUID()}`,
    body: update as unknown as Record<string, unknown>,
  });
}
