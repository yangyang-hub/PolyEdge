import { fetchContract, fetchListContract } from "@/lib/api/base";
import type { ApiListResponse, ApiResponse } from "@/lib/contracts/api";
import type { MarketStrategyData } from "@/lib/contracts/dto";

export function listStrategies(): Promise<ApiListResponse<MarketStrategyData>> {
  return fetchListContract<MarketStrategyData>("/api/v1/market-strategies");
}

export function getStrategy(strategyId: number): Promise<ApiResponse<MarketStrategyData>> {
  return fetchContract<ApiResponse<MarketStrategyData>>(
    `/api/v1/market-strategies/${strategyId}`,
  );
}
