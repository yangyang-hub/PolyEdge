import "server-only";

import type { RewardBotSnapshotDto } from "@/lib/contracts/dto";
import { readRewardBotSnapshot } from "@/server/api/rewards";

export type RewardsPageData = {
  snapshot: RewardBotSnapshotDto;
  requestId: string;
  traceId: string;
};

export async function getRewardsPageData(): Promise<RewardsPageData> {
  const response = await readRewardBotSnapshot();

  return {
    snapshot: response.data,
    requestId: response.meta.request_id,
    traceId: response.meta.trace_id,
  };
}
