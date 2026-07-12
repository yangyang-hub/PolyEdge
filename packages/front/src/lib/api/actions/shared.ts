import { z } from "zod";

import { PolyEdgeApiError, randomUUID } from "@/lib/api/base";

export type OperationActionResult = {
  ok: boolean;
  message: string;
  requestId?: string;
  traceId?: string;
  operationId?: string;
  status?: "queued" | "completed" | "rejected";
  fieldErrors?: Partial<
    Record<
      | "note"
      | "stepUpCode"
      | "targetMode"
      | "limitPrice"
      | "quantity"
      | "connectorName"
      | "address"
      | "tokenId"
      | "amount"
      | "confirmed"
      | "idempotencyKey"
      | "source"
      | "status"
      | "reason",
      string
    >
  >;
};

export function createActionSuccessResult(
  message: string,
  meta: {
    requestId: string;
    traceId: string;
    operationId: string;
    status: "queued" | "completed" | "rejected";
  },
): OperationActionResult {
  return {
    ok: true,
    message,
    requestId: meta.requestId,
    traceId: meta.traceId,
    operationId: meta.operationId,
    status: meta.status,
  };
}

export function createActionFailureResult(
  message: string,
  options?: {
    requestId?: string;
    traceId?: string;
    fieldErrors?: OperationActionResult["fieldErrors"];
  },
): OperationActionResult {
  return {
    ok: false,
    message,
    requestId: options?.requestId,
    traceId: options?.traceId,
    fieldErrors: options?.fieldErrors,
  };
}

export function apiActionFailure(error: unknown, fallback: string): OperationActionResult {
  if (error instanceof PolyEdgeApiError) {
    return createActionFailureResult(error.message, {
      requestId: error.requestId,
      traceId: error.traceId,
    });
  }

  return createActionFailureResult(error instanceof Error ? error.message : fallback);
}

export function actionOperationId(prefix: string): string {
  return `${prefix}_${randomUUID().slice(0, 8)}`;
}

export const decimalNumber = z.coerce.number().finite();

const DECIMAL_STRING_PATTERN = /^(?:\d+|\d+\.\d+|\.\d+)$/;

export const decimalString = (label: string) =>
  z
    .string()
    .trim()
    .min(1, `${label} is required.`)
    .refine(
      (value) => DECIMAL_STRING_PATTERN.test(value) && Number.isFinite(Number(value)),
      `${label} must be numeric.`,
    );
