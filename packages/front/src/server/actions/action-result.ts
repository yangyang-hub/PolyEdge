export type OperationActionResult = {
  ok: boolean;
  message: string;
  requestId?: string;
  traceId?: string;
  operationId?: string;
  status?: "queued" | "completed" | "rejected";
  fieldErrors?: Partial<
    Record<"note" | "stepUpCode" | "targetMode" | "limitPrice" | "quantity" | "connectorName", string>
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
