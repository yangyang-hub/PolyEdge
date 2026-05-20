import "server-only";

import type { ApiListResponse, ContractListQuery } from "@/lib/contracts/api";
import type { PositionDto } from "@/lib/contracts/dto";
import { buildQueryString, fetchListContract } from "@/server/api/base";

type LivePositionData = {
  id: string;
  market_id: string;
  connector_name: string;
  side: PositionDto["side"];
  net_quantity: string;
  avg_cost: string;
  mark_price: string;
  realized_pnl: string;
  unrealized_pnl: string;
  updated_at: string;
  version: number;
};

export async function listPositions(query?: ContractListQuery): Promise<ApiListResponse<PositionDto>> {
  const liveQuery = {
    limit: query?.limit,
    market_id: query?.market_id,
  };

  return fetchListContract<LivePositionData, PositionDto>(
    `/api/v1/positions${buildQueryString(liveQuery)}`,
    {
      mapItem: (position) => ({
        id: position.id,
        market_id: position.market_id,
        market_question: position.market_id,
        side: position.side,
        quantity: position.net_quantity,
        average_cost: position.avg_cost,
        mark_price: position.mark_price,
        realized_pnl: position.realized_pnl,
        unrealized_pnl: position.unrealized_pnl,
        bucket_name: position.connector_name,
        updated_at: position.updated_at,
        version: position.version,
      }),
    },
  );
}
