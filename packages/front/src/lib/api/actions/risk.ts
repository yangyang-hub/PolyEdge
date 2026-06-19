import { z } from "zod";

import { releaseRiskControls, setKillSwitchState } from "@/lib/api/risk";

import {
  apiActionFailure,
  createActionFailureResult,
  createActionSuccessResult,
  type OperationActionResult,
} from "./shared";

const riskControlSchema = z.object({
  note: z.string().trim().min(16, "Operator note must be at least 16 characters."),
  stepUpCode: z.string().trim().min(6, "Step-up code is required for this control."),
});

export async function triggerRiskReleaseAction(input: {
  note: string;
  stepUpCode: string;
}): Promise<OperationActionResult> {
  try {
    const parsed = riskControlSchema.safeParse(input);

    if (!parsed.success) {
      const flattened = parsed.error.flatten().fieldErrors;
      return createActionFailureResult("Release request is invalid.", {
        fieldErrors: {
          note: flattened.note?.[0],
          stepUpCode: flattened.stepUpCode?.[0],
        },
      });
    }

    const response = await releaseRiskControls(parsed.data);

    return createActionSuccessResult("Risk controls release queued for audit review.", {
      requestId: response.meta.request_id,
      traceId: response.meta.trace_id,
      operationId: response.data.operation_id,
      status: response.data.status,
    });
  } catch (error) {
    return apiActionFailure(error, "Risk release request failed unexpectedly.");
  }
}

export async function setKillSwitchStateAction(input: {
  enabled: boolean;
  note: string;
  stepUpCode: string;
}): Promise<OperationActionResult> {
  try {
    const parsed = riskControlSchema.safeParse(input);

    if (!parsed.success) {
      const flattened = parsed.error.flatten().fieldErrors;
      return createActionFailureResult("Kill switch request is invalid.", {
        fieldErrors: {
          note: flattened.note?.[0],
          stepUpCode: flattened.stepUpCode?.[0],
        },
      });
    }

    const response = await setKillSwitchState({
      enabled: input.enabled,
      note: parsed.data.note,
      stepUpCode: parsed.data.stepUpCode,
    });

    return createActionSuccessResult(
      input.enabled ? "Kill switch activation has been queued." : "Kill switch release has been queued.",
      {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: response.data.operation_id,
        status: response.data.status,
      },
    );
  } catch (error) {
    return apiActionFailure(error, "Kill switch request failed unexpectedly.");
  }
}
