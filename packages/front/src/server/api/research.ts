import "server-only";

import type { ApiResponse } from "@/lib/contracts/api";
import type { ReplayRunDto } from "@/lib/contracts/dto";
import { replayRunFixture } from "@/lib/server/polyedge-mock-data";
import { createResponse, fetchContract } from "@/server/api/base";

export async function readReplayRun(runId = replayRunFixture.id): Promise<ApiResponse<ReplayRunDto>> {
  return fetchContract(
    `/api/research/runs/${runId}`,
    createResponse("replay_run", replayRunFixture),
  );
}
