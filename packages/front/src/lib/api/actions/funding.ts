import { z } from "zod";

import { submitFundingTransfer } from "@/lib/api/funding";
import type { FundingTransferDto } from "@/lib/contracts/dto";

import {
  apiActionFailure,
  createActionFailureResult,
  createActionSuccessResult,
  decimalString,
  type OperationActionResult,
} from "./shared";

export type FundingTransferActionResult = OperationActionResult & {
  transfer?: FundingTransferDto;
};

const fundingTransferSchema = z.object({
  tokenId: z.string().trim().min(1, "Funding token is required."),
  amount: decimalString("Funding amount").refine((value) => Number(value) > 0, {
    message: "Funding amount must be greater than 0.",
  }).refine((value) => Number(value) <= 10_000, {
    message: "Funding amount cannot exceed 10000.",
  }),
  confirmed: z.boolean().refine((value) => value, "Confirmation is required before transfer."),
});

export async function submitFundingTransferAction(input: {
  tokenId: string;
  amount: string;
  confirmed: boolean;
}): Promise<FundingTransferActionResult> {
  try {
    const parsed = fundingTransferSchema.safeParse(input);
    if (!parsed.success) {
      const flattened = parsed.error.flatten().fieldErrors;
      return createActionFailureResult("Funding transfer is invalid.", {
        fieldErrors: {
          tokenId: flattened.tokenId?.[0],
          amount: flattened.amount?.[0],
          confirmed: flattened.confirmed?.[0],
        },
      });
    }

    const response = await submitFundingTransfer({
      request: {
        token_id: parsed.data.tokenId,
        amount: parsed.data.amount,
        confirmed: parsed.data.confirmed,
      },
    });

    return {
      ...createActionSuccessResult("Funding transfer broadcast on Polygon.", {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: response.data.tx_hash,
        status: "completed",
      }),
      transfer: response.data,
    };
  } catch (error) {
    return apiActionFailure(error, "Funding transfer failed unexpectedly.");
  }
}
