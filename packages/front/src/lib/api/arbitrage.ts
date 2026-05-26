import type { ApiListResponse, ContractListQuery } from "@/lib/contracts/api";
import type {
  ArbitrageAnalysisRunDto,
  ArbitrageOpportunityDto,
  ArbitrageScanDto,
} from "@/lib/contracts/dto";
import { buildQueryString, fetchListContract } from "@/lib/api/base";

type ArbitrageOpportunityQuery = Pick<
  ContractListQuery,
  | "limit"
  | "market_id"
  | "opportunity_type"
  | "status"
  | "validation_status"
  | "min_net_edge"
  | "observed_after"
  | "active_only"
>;

export async function listArbitrageScans(
  query?: Pick<ContractListQuery, "limit">,
): Promise<ApiListResponse<ArbitrageScanDto>> {
  return fetchListContract(`/api/v1/arbitrage/scans${buildQueryString({ limit: query?.limit })}`);
}

export async function listArbitrageOpportunities(
  query?: ArbitrageOpportunityQuery,
): Promise<ApiListResponse<ArbitrageOpportunityDto>> {
  return fetchListContract(
    `/api/v1/arbitrage/opportunities${buildQueryString({
      limit: query?.limit,
      market_id: query?.market_id,
      opportunity_type: query?.opportunity_type,
      status: query?.status?.[0],
      validation_status: query?.validation_status,
      min_net_edge: query?.min_net_edge,
      observed_after: query?.observed_after,
      active_only: query?.active_only,
    })}`,
  );
}

export async function listArbitrageAnalysisRuns(
  query?: Pick<ContractListQuery, "limit">,
): Promise<ApiListResponse<ArbitrageAnalysisRunDto>> {
  return fetchListContract(`/api/v1/arbitrage/analysis${buildQueryString({ limit: query?.limit })}`);
}
