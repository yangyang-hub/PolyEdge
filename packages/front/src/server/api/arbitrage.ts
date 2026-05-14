import "server-only";

import type { ApiListResponse, ContractListQuery } from "@/lib/contracts/api";
import type {
  ArbitrageAnalysisRunDto,
  ArbitrageOpportunityDto,
  ArbitrageScanDto,
} from "@/lib/contracts/dto";
import {
  arbitrageAnalysisRunFixtures,
  arbitrageOpportunityFixtures,
  arbitrageScanFixtures,
} from "@/lib/server/polyedge-mock-data";
import { buildQueryString, createListResponse, fetchListContract } from "@/server/api/base";

type ArbitrageOpportunityQuery = Pick<
  ContractListQuery,
  "limit" | "market_id" | "opportunity_type" | "observed_after"
>;

function timestamp(value: string): number {
  const parsed = Date.parse(value);
  return Number.isNaN(parsed) ? 0 : parsed;
}

export async function listArbitrageScans(
  query?: Pick<ContractListQuery, "limit">,
): Promise<ApiListResponse<ArbitrageScanDto>> {
  return fetchListContract(
    `/api/v1/arbitrage/scans${buildQueryString({ limit: query?.limit })}`,
    createListResponse("arbitrage_scans", arbitrageScanFixtures, query?.limit),
  );
}

export async function listArbitrageOpportunities(
  query?: ArbitrageOpportunityQuery,
): Promise<ApiListResponse<ArbitrageOpportunityDto>> {
  const filtered = arbitrageOpportunityFixtures.filter((opportunity) => {
    if (query?.market_id && opportunity.market_id !== query.market_id) {
      return false;
    }

    if (query?.opportunity_type && opportunity.opportunity_type !== query.opportunity_type) {
      return false;
    }

    if (query?.observed_after && timestamp(opportunity.observed_at) < timestamp(query.observed_after)) {
      return false;
    }

    return true;
  });

  return fetchListContract(
    `/api/v1/arbitrage/opportunities${buildQueryString({
      limit: query?.limit,
      market_id: query?.market_id,
      opportunity_type: query?.opportunity_type,
      observed_after: query?.observed_after,
    })}`,
    createListResponse("arbitrage_opportunities", filtered, query?.limit),
  );
}

export async function listArbitrageAnalysisRuns(
  query?: Pick<ContractListQuery, "limit">,
): Promise<ApiListResponse<ArbitrageAnalysisRunDto>> {
  return fetchListContract(
    `/api/v1/arbitrage/analysis${buildQueryString({ limit: query?.limit })}`,
    createListResponse("arbitrage_analysis_runs", arbitrageAnalysisRunFixtures, query?.limit),
  );
}
