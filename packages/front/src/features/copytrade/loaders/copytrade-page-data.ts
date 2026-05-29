import type { CopyTradeSnapshotDto } from "@/lib/contracts/dto";
import { readCopyTradeSnapshot } from "@/lib/api/copytrade";

export type CopyTradePageData = {
  snapshot: CopyTradeSnapshotDto;
  requestId: string;
  traceId: string;
};

export async function getCopyTradePageData(): Promise<CopyTradePageData> {
  const response = await readCopyTradeSnapshot();

  return {
    snapshot: response.data,
    requestId: response.meta.request_id,
    traceId: response.meta.trace_id,
  };
}
