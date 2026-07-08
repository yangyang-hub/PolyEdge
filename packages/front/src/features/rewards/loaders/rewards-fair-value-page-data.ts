import type { RewardBotSnapshotDto } from "@/lib/contracts/dto";
import { readRewardBotSnapshot } from "@/lib/api/rewards";

export type RewardsFairValuePageData = {
  snapshot: RewardBotSnapshotDto;
  requestId: string;
  traceId: string;
};

export async function getRewardsFairValuePageData(): Promise<RewardsFairValuePageData> {
  const response = await readRewardBotSnapshot({
    plans_page: 1,
    plans_page_size: 100,
    plans_sort_by: "selection_score",
    plans_sort_order: "desc",
    orders_page: 1,
    orders_page_size: 5,
    orders_sort_by: "status",
    orders_sort_order: "desc",
  });

  return {
    snapshot: response.data,
    requestId: response.meta.request_id,
    traceId: response.meta.trace_id,
  };
}
