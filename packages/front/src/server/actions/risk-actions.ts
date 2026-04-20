"use server";

import { z } from "zod";

import type { RuntimeMode } from "@/lib/contracts/dto";
import { assertConsoleRole } from "@/server/auth/console-session";
import { requestModeSwitch, releaseRiskControls, setKillSwitchState } from "@/server/api/risk";
import {
  createActionFailureResult,
  createActionSuccessResult,
  type OperationActionResult,
} from "@/server/actions/action-result";
import { PolyEdgeApiError } from "@/server/api/base";

const modeSwitchSchema = z.object({
  currentMode: z.enum(["research", "paper_trade", "manual_confirm", "live_auto", "kill_switch_locked"]),
  targetMode: z.enum(["research", "paper_trade", "manual_confirm", "live_auto", "kill_switch_locked"]),
  note: z.string().trim().min(16, "Mode switch note must be at least 16 characters."),
  stepUpCode: z.string().trim().min(6, "Step-up code is required for mode changes."),
});

const riskControlSchema = z.object({
  note: z.string().trim().min(16, "Operator note must be at least 16 characters."),
  stepUpCode: z.string().trim().min(6, "Step-up code is required for this control."),
});

export async function requestModeSwitchAction(input: {
  currentMode: RuntimeMode;
  targetMode: RuntimeMode;
  note: string;
  stepUpCode: string;
}): Promise<OperationActionResult> {
  try {
    await assertConsoleRole("risk_admin");

    const parsed = modeSwitchSchema.safeParse(input);

    if (!parsed.success) {
      const flattened = parsed.error.flatten().fieldErrors;
      return createActionFailureResult("Mode switch request is invalid.", {
        fieldErrors: {
          note: flattened.note?.[0],
          stepUpCode: flattened.stepUpCode?.[0],
          targetMode: flattened.targetMode?.[0],
        },
      });
    }

    const response = await requestModeSwitch(parsed.data);

    return createActionSuccessResult("Mode switch accepted by the control plane.", {
      requestId: response.meta.request_id,
      traceId: response.meta.trace_id,
      operationId: response.data.operation_id,
      status: response.data.status,
    });
  } catch (error) {
    if (error instanceof PolyEdgeApiError) {
      return createActionFailureResult(error.message, {
        requestId: error.requestId,
        traceId: error.traceId,
      });
    }

    return createActionFailureResult(
      error instanceof Error ? error.message : "Mode switch request failed unexpectedly.",
    );
  }
}

export async function triggerRiskReleaseAction(input: {
  note: string;
  stepUpCode: string;
}): Promise<OperationActionResult> {
  try {
    await assertConsoleRole("risk_admin");

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
    if (error instanceof PolyEdgeApiError) {
      return createActionFailureResult(error.message, {
        requestId: error.requestId,
        traceId: error.traceId,
      });
    }

    return createActionFailureResult(
      error instanceof Error ? error.message : "Risk release request failed unexpectedly.",
    );
  }
}

export async function setKillSwitchStateAction(input: {
  enabled: boolean;
  note: string;
  stepUpCode: string;
}): Promise<OperationActionResult> {
  try {
    await assertConsoleRole("risk_admin");

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
    if (error instanceof PolyEdgeApiError) {
      return createActionFailureResult(error.message, {
        requestId: error.requestId,
        traceId: error.traceId,
      });
    }

    return createActionFailureResult(
      error instanceof Error ? error.message : "Kill switch request failed unexpectedly.",
    );
  }
}
