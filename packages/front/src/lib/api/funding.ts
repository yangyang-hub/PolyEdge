import { fetchContract, fetchWriteContract, randomUUID } from "@/lib/api/base";
import type { ApiResponse } from "@/lib/contracts/api";
import type {
  FundingStatusDto,
  FundingTransferDto,
  FundingTransferRequestDto,
} from "@/lib/contracts/dto";

export async function readFundingStatus(): Promise<ApiResponse<FundingStatusDto>> {
  return fetchContract<ApiResponse<FundingStatusDto>>("/api/v1/funding");
}

export async function submitFundingTransfer(input: {
  request: FundingTransferRequestDto;
  stepUpCode: string;
}): Promise<ApiResponse<FundingTransferDto>> {
  return fetchWriteContract<ApiResponse<FundingTransferDto>>("/api/v1/funding/transfer", {
    method: "POST",
    idempotencyKey: `funding-transfer-${randomUUID()}`,
    body: input.request as Record<string, unknown>,
    stepUpCode: input.stepUpCode,
    stepUpScopes: ["funding_transfer"],
  });
}
