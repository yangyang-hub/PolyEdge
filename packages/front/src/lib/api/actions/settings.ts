import { fetchWriteContract } from "@/lib/api/base";
import type { WriteResponse } from "@/lib/contracts/api";
import type { UpdateSystemRuntimeStateRequest } from "@/lib/contracts/dto";
import { dictionary } from "@/lib/i18n/dictionaries";

import {
  actionOperationId,
  apiActionFailure,
  createActionSuccessResult,
  type OperationActionResult,
} from "./shared";

export async function updateSystemRuntimeState(input: {
  request: UpdateSystemRuntimeStateRequest;
  stepUpCode: string;
}): Promise<OperationActionResult> {
  try {
    const result = await fetchWriteContract<WriteResponse>("/api/v1/system/runtime-state", {
      method: "PATCH",
      body: input.request as unknown as Record<string, unknown>,
      idempotencyKey: actionOperationId("runtime-state"),
      stepUpCode: input.stepUpCode,
      stepUpScopes: [
        input.request.kill_switch_locked
          ? "system_kill_switch_trigger"
          : "system_kill_switch_release",
      ],
    });
    return createActionSuccessResult(dictionary.actionMessages.runtimeUpdated, {
      requestId: result.meta.request_id,
      traceId: result.meta.trace_id,
      operationId: result.data.operation_id,
      status: result.data.status,
    });
  } catch (error) {
    return apiActionFailure(error, dictionary.actionMessages.runtimeUpdateFailed);
  }
}
