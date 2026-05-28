import type { ApiMeta } from "@/lib/contracts/api";
import type { MarketDto } from "@/lib/contracts/dto";
import { buildQueryString, fetchContract } from "@/lib/api/base";

export type MarketListParams = {
  limit?: number;
  status?: string;
  tradability_status?: string;
  category?: string;
  sort_by?: string;
  sort_order?: string;
  offset?: number;
};

export type MarketListResult = {
  data: MarketDto[];
  totalCount: number;
  meta: ApiMeta;
};

export async function listMarkets(params?: MarketListParams): Promise<MarketListResult> {
  const query: Record<string, string | number | undefined> = {};
  if (params?.limit != null) query.limit = params.limit;
  if (params?.status) query.status = params.status;
  if (params?.tradability_status) query.tradability_status = params.tradability_status;
  if (params?.category) query.category = params.category;
  if (params?.sort_by) query.sort_by = params.sort_by;
  if (params?.sort_order) query.sort_order = params.sort_order;
  if (params?.offset != null) query.offset = params.offset;

  const response = await fetchContract<{
    data: MarketDto[];
    total_count: number;
    meta: ApiMeta;
  }>(`/api/v1/markets${buildQueryString(query)}`);

  return {
    data: response.data,
    totalCount: response.total_count,
    meta: response.meta,
  };
}
