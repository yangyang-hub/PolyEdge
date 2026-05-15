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
  | "limit"
  | "market_id"
  | "opportunity_type"
  | "status"
  | "validation_status"
  | "min_net_edge"
  | "observed_after"
  | "active_only"
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

    if (query?.status?.length && !query.status.includes(opportunity.status)) {
      return false;
    }

    if (query?.validation_status === "unvalidated" && opportunity.validation) {
      return false;
    }

    if (
      query?.validation_status &&
      query.validation_status !== "unvalidated" &&
      opportunity.validation?.status !== query.validation_status
    ) {
      return false;
    }

    if (
      query?.min_net_edge &&
      Number.parseFloat(opportunity.validation?.net_edge ?? "0") < Number.parseFloat(query.min_net_edge)
    ) {
      return false;
    }

    if (query?.active_only && opportunity.status === "expired") {
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
      status: query?.status?.[0],
      validation_status: query?.validation_status,
      min_net_edge: query?.min_net_edge,
      observed_after: query?.observed_after,
      active_only: query?.active_only,
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
