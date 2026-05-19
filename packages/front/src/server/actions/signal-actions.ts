"use server";

import { z } from "zod";

import { assertConsoleRole } from "@/server/auth/console-session";
import {
  submitSignalDecision,
  submitSignalExecutionRequest,
} from "@/server/api/signals";
import {
  createActionFailureResult,
  createActionSuccessResult,
  type OperationActionResult,
} from "@/server/actions/action-result";
import { PolyEdgeApiError } from "@/server/api/base";

const DECIMAL_STRING_PATTERN = /^(?:\d+|\d+\.\d+|\.\d+)$/;

const decimalString = (label: string) =>
  z
    .string()
    .trim()
    .min(1, `${label} is required.`)
    .refine(
      (value) => DECIMAL_STRING_PATTERN.test(value) && Number.isFinite(Number(value)),
      `${label} must be numeric.`,
    );

const signalDecisionSchema = z.object({
  signalId: z.string().min(1),
  expectedVersion: z.number().int().positive(),
  decision: z.enum(["approved", "rejected"]),
  note: z.string().trim().min(16, "Operator note must be at least 16 characters."),
  stepUpCode: z.string().trim().min(6, "Step-up code is required for signal decisions."),
});

const signalExecutionSchema = z.object({
  signalId: z.string().min(1),
  expectedVersion: z.number().int().positive(),
  limitPrice: decimalString("Limit price").refine((value) => {
    const parsed = Number(value);
    return parsed > 0 && parsed <= 1;
  }, "Limit price must be greater than 0 and no more than 1."),
  quantity: decimalString("Quantity").refine((value) => Number(value) > 0, {
    message: "Quantity must be greater than 0.",
  }),
  connectorName: z.string().trim().optional().default(""),
  note: z.string().trim().min(16, "Execution note must be at least 16 characters."),
  stepUpCode: z.string().trim().min(6, "Step-up code is required for execution submission."),
});

export async function submitSignalDecisionAction(input: {
  signalId: string;
  expectedVersion: number;
  decision: "approved" | "rejected";
  note: string;
  stepUpCode: string;
}): Promise<OperationActionResult> {
  try {
    await assertConsoleRole("operator");

    const parsed = signalDecisionSchema.safeParse(input);
    if (!parsed.success) {
      const flattened = parsed.error.flatten().fieldErrors;
      return createActionFailureResult("Signal decision request is invalid.", {
        fieldErrors: {
          note: flattened.note?.[0],
          stepUpCode: flattened.stepUpCode?.[0],
        },
      });
    }

    const response = await submitSignalDecision(parsed.data);

    return createActionSuccessResult(
      parsed.data.decision === "approved"
        ? "Signal approval accepted by the backend."
        : "Signal rejection accepted by the backend.",
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
      error instanceof Error ? error.message : "Signal decision failed unexpectedly.",
    );
  }
}

export async function submitSignalExecutionAction(input: {
  signalId: string;
  expectedVersion: number;
  limitPrice: string;
  quantity: string;
  connectorName?: string;
  note: string;
  stepUpCode: string;
}): Promise<OperationActionResult> {
  try {
    await assertConsoleRole("operator");

    const parsed = signalExecutionSchema.safeParse(input);
    if (!parsed.success) {
      const flattened = parsed.error.flatten().fieldErrors;
      return createActionFailureResult("Execution request is invalid.", {
        fieldErrors: {
          limitPrice: flattened.limitPrice?.[0],
          quantity: flattened.quantity?.[0],
          connectorName: flattened.connectorName?.[0],
          note: flattened.note?.[0],
          stepUpCode: flattened.stepUpCode?.[0],
        },
      });
    }

    const response = await submitSignalExecutionRequest(parsed.data);

    return createActionSuccessResult("Execution request accepted by the backend.", {
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
      error instanceof Error ? error.message : "Execution request failed unexpectedly.",
    );
  }
}
