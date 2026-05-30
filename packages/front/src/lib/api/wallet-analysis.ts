import type { ApiResponse } from "@/lib/contracts/api";
import type { WalletAnalysisReportDto } from "@/lib/contracts/dto";
import { fetchWriteContract, randomUUID } from "@/lib/api/base";

export async function analyzeWallet(
  address: string,
): Promise<ApiResponse<WalletAnalysisReportDto>> {
  return fetchWriteContract<ApiResponse<WalletAnalysisReportDto>>("/api/v1/wallet-analysis", {
    method: "POST",
    idempotencyKey: `wallet-analysis-${randomUUID()}`,
    body: { address },
  });
}
