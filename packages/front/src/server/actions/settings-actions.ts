"use server";

import { revalidatePath } from "next/cache";
import { z } from "zod";

import type {
  RuntimeConfigEntryDto,
  RuntimeConfigUpdateDto,
} from "@/lib/contracts/dto";
import {
  createActionFailureResult,
  createActionSuccessResult,
  type OperationActionResult,
} from "@/server/actions/action-result";
import { updateRuntimeConfig } from "@/server/api/settings";
import { PolyEdgeApiError } from "@/server/api/base";
import { assertConsoleRole } from "@/server/auth/console-session";

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
    await assertConsoleRole("admin");
    const parsed = runtimeConfigSchema.safeParse(input);

    if (!parsed.success) {
      return createActionFailureResult("Runtime configuration is invalid.");
    }

    const response = await updateRuntimeConfig(parsed.data);
    revalidatePath("/settings");

    return {
      ...createActionSuccessResult("Runtime configuration saved. Restart backend processes to apply runtime consumers.", {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: `runtime_config_${crypto.randomUUID().slice(0, 8)}`,
        status: "completed",
      }),
      entries: response.data,
    };
  } catch (error) {
    if (error instanceof PolyEdgeApiError) {
      return createActionFailureResult(error.message, {
        requestId: error.requestId,
        traceId: error.traceId,
      });
    }

    return createActionFailureResult(error instanceof Error ? error.message : "Runtime configuration update failed.");
  }
}
