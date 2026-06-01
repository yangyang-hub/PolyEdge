import type { RewardBotSnapshotDto } from "@/lib/contracts/dto";
import { readRewardBotSnapshot } from "@/lib/api/rewards";

export type RewardsPageData = {
  snapshot: RewardBotSnapshotDto;
  requestId: string;
  traceId: string;
};

const REWARD_ORDERS_PAGE_SIZE = 15;

export async function getRewardsPageData(): Promise<RewardsPageData> {
  const response = await readRewardBotSnapshot({
    orders_page: 1,
    orders_page_size: REWARD_ORDERS_PAGE_SIZE,
    orders_sort_by: "status",
    orders_sort_order: "desc",
  });

  return {
    snapshot: response.data,
    requestId: response.meta.request_id,
    traceId: response.meta.trace_id,
  };
}
