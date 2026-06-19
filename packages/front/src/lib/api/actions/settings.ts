import { z } from "zod";

import { updateRuntimeConfig } from "@/lib/api/settings";
import type { RuntimeConfigEntryDto, RuntimeConfigUpdateDto } from "@/lib/contracts/dto";

import {
  actionOperationId,
  apiActionFailure,
  createActionFailureResult,
  createActionSuccessResult,
  type OperationActionResult,
} from "./shared";

export type RuntimeConfigActionResult = OperationActionResult & {
  entries?: RuntimeConfigEntryDto[];
};

const runtimeConfigSchema = z.object({
  values: z.record(z.string().min(1), z.string().max(20_000)),
});

export async function updateRuntimeConfigAction(
  input: RuntimeConfigUpdateDto,
): Promise<RuntimeConfigActionResult> {
  try {
    const parsed = runtimeConfigSchema.safeParse(input);

    if (!parsed.success) {
      return createActionFailureResult("Runtime configuration is invalid.");
    }

    const response = await updateRuntimeConfig(parsed.data);

    return {
      ...createActionSuccessResult("Runtime configuration saved. Restart backend processes to apply runtime consumers.", {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: actionOperationId("runtime_config"),
        status: "completed",
      }),
      entries: response.data,
    };
  } catch (error) {
    return apiActionFailure(error, "Runtime configuration update failed.");
  }
}
