import { fetchContract } from "@/lib/api/base";
import type { ApiResponse } from "@/lib/contracts/api";
import type { SystemRuntimeStateData } from "@/lib/contracts/dto";

export function readSystemRuntimeState(): Promise<ApiResponse<SystemRuntimeStateData>> {
  return fetchContract<ApiResponse<SystemRuntimeStateData>>("/api/v1/system/runtime-state");
}
