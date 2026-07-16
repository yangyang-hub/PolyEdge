import { fetchWriteContract } from "@/lib/api/base";
import type { WriteResponse } from "@/lib/contracts/api";
import type { CreateMarketStrategyRequest } from "@/lib/contracts/dto";
import { dictionary } from "@/lib/i18n/dictionaries";

import {
  actionOperationId,
  apiActionFailure,
  createActionSuccessResult,
  type OperationActionResult,
} from "./shared";

export async function saveStrategy(
  body: CreateMarketStrategyRequest,
): Promise<OperationActionResult> {
  try {
    const result = await fetchWriteContract<WriteResponse>("/api/v1/market-strategies", {
      body: body as unknown as Record<string, unknown>,
      idempotencyKey: actionOperationId("strategy"),
    });
    return createActionSuccessResult(dictionary.actionMessages.strategySaved, {
      requestId: result.meta.request_id,
      traceId: result.meta.trace_id,
      operationId: result.data.operation_id,
      status: result.data.status,
    });
  } catch (error) {
    return apiActionFailure(error, dictionary.actionMessages.strategySaveFailed);
  }
}
