import { fetchWriteContract } from "@/lib/api/base";
import type { WriteResponse } from "@/lib/contracts/api";
import type {
  CreateCancellationBatchRequest,
  CreateExecutionBatchRequest,
} from "@/lib/contracts/dto";
import { dictionary } from "@/lib/i18n/dictionaries";

import {
  actionOperationId,
  apiActionFailure,
  createActionSuccessResult,
  type OperationActionResult,
} from "./shared";

type ProtectedMutation<T> = {
  request: T;
};

export async function executeBatch(
  input: ProtectedMutation<CreateExecutionBatchRequest>,
): Promise<OperationActionResult> {
  try {
    const result = await fetchWriteContract<WriteResponse>("/api/v1/execution-batches", {
      body: input.request as unknown as Record<string, unknown>,
      idempotencyKey: actionOperationId("batch"),
    });
    return createActionSuccessResult(dictionary.actionMessages.executionSubmitted, {
      requestId: result.meta.request_id,
      traceId: result.meta.trace_id,
      operationId: result.data.operation_id,
      status: result.data.status,
    });
  } catch (error) {
    return apiActionFailure(error, dictionary.actionMessages.executionFailed);
  }
}

export async function cancelOrders(
  input: ProtectedMutation<CreateCancellationBatchRequest>,
): Promise<OperationActionResult> {
  try {
    const result = await fetchWriteContract<WriteResponse>("/api/v1/cancellation-batches", {
      body: input.request as unknown as Record<string, unknown>,
      idempotencyKey: actionOperationId("cancel"),
    });
    return createActionSuccessResult(dictionary.actionMessages.cancellationSubmitted, {
      requestId: result.meta.request_id,
      traceId: result.meta.trace_id,
      operationId: result.data.operation_id,
      status: result.data.status,
    });
  } catch (error) {
    return apiActionFailure(error, dictionary.actionMessages.cancellationFailed);
  }
}
