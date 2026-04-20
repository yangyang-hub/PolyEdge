"use server";

import { z } from "zod";

import { assertConsoleRole } from "@/server/auth/console-session";
import { submitApprovalDecision } from "@/server/api/system";
import {
  createActionFailureResult,
  createActionSuccessResult,
  type OperationActionResult,
} from "@/server/actions/action-result";
import { PolyEdgeApiError } from "@/server/api/base";

const approvalDecisionSchema = z
  .object({
    approvalId: z.string().min(1),
    resourceId: z.string().min(1),
    expectedVersion: z.number().int().positive(),
    decision: z.enum(["approved", "rejected"]),
    requiresStepUpAuth: z.boolean(),
    note: z.string().trim().min(16, "Operator note must be at least 16 characters."),
    stepUpCode: z.string().trim().optional().default(""),
  })
  .superRefine((value, ctx) => {
    if (value.requiresStepUpAuth && value.stepUpCode.length < 6) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        path: ["stepUpCode"],
        message: "Step-up code is required for this operation.",
      });
    }
  });

export async function submitApprovalDecisionAction(input: {
  approvalId: string;
  resourceId: string;
  expectedVersion: number;
  decision: "approved" | "rejected";
  requiresStepUpAuth: boolean;
  note: string;
  stepUpCode?: string;
}): Promise<OperationActionResult> {
  try {
    await assertConsoleRole("operator");

    const parsed = approvalDecisionSchema.safeParse(input);

    if (!parsed.success) {
      const flattened = parsed.error.flatten().fieldErrors;
      return createActionFailureResult("Approval request is invalid.", {
        fieldErrors: {
          note: flattened.note?.[0],
          stepUpCode: flattened.stepUpCode?.[0],
        },
      });
    }

    const response = await submitApprovalDecision(parsed.data);

    return createActionSuccessResult(
      parsed.data.decision === "approved"
        ? "Approval queued for backend execution."
        : "Rejection queued and audit record updated.",
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
      error instanceof Error ? error.message : "Approval request failed unexpectedly.",
    );
  }
}
