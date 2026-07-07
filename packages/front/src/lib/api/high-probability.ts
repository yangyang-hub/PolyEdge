import type { ApiResponse } from "@/lib/contracts/api";
import type {
  HighProbabilityBacktestReportDto,
  HighProbabilityBacktestRunDto,
  HighProbabilityBacktestTradeDto,
  HighProbabilityBucketStatsDto,
  HighProbabilityConfigDto,
  HighProbabilityFairValueDto,
  HighProbabilityResearchReportDto,
  HighProbabilitySnapshotDto,
} from "@/lib/contracts/dto";
import { buildQueryString, fetchContract } from "@/lib/api/base";

export async function readHighProbabilitySnapshot(): Promise<ApiResponse<HighProbabilitySnapshotDto>> {
  return fetchContract<ApiResponse<HighProbabilitySnapshotDto>>("/api/v1/high-probability");
}

export async function readHighProbabilityConfig(): Promise<ApiResponse<HighProbabilityConfigDto>> {
  return fetchContract<ApiResponse<HighProbabilityConfigDto>>("/api/v1/high-probability/config");
}

export async function readHighProbabilityBuckets(): Promise<ApiResponse<HighProbabilityBucketStatsDto[]>> {
  return fetchContract<ApiResponse<HighProbabilityBucketStatsDto[]>>("/api/v1/high-probability/buckets");
}

export async function readHighProbabilityReport(): Promise<ApiResponse<HighProbabilityResearchReportDto>> {
  return fetchContract<ApiResponse<HighProbabilityResearchReportDto>>("/api/v1/high-probability/report");
}

export async function readHighProbabilityBacktests(): Promise<ApiResponse<HighProbabilityBacktestReportDto>> {
  return fetchContract<ApiResponse<HighProbabilityBacktestReportDto>>("/api/v1/high-probability/backtests");
}

export async function readHighProbabilityBacktestRuns(
  limit = 5,
): Promise<ApiResponse<HighProbabilityBacktestRunDto[]>> {
  const query = buildQueryString({ limit });
  return fetchContract<ApiResponse<HighProbabilityBacktestRunDto[]>>(
    `/api/v1/high-probability/backtest-runs${query}`,
  );
}

export async function readHighProbabilityBacktestTrades(
  runId: number,
  limit = 20,
): Promise<ApiResponse<HighProbabilityBacktestTradeDto[]>> {
  const query = buildQueryString({ limit });
  return fetchContract<ApiResponse<HighProbabilityBacktestTradeDto[]>>(
    `/api/v1/high-probability/backtest-runs/${runId}/trades${query}`,
  );
}

export async function readHighProbabilityFairValues(
  limit = 100,
): Promise<ApiResponse<HighProbabilityFairValueDto[]>> {
  const query = buildQueryString({ limit });
  return fetchContract<ApiResponse<HighProbabilityFairValueDto[]>>(
    `/api/v1/high-probability/fair-values${query}`,
  );
}
