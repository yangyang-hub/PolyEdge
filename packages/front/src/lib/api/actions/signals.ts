import { z } from "zod";

import { submitSignalExecutionRequest } from "@/lib/api/signals";

import {
  apiActionFailure,
  createActionFailureResult,
  createActionSuccessResult,
  decimalString,
  type OperationActionResult,
} from "./shared";

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
    return apiActionFailure(error, "Execution request failed unexpectedly.");
  }
}
