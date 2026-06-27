import type { CopyTradeSnapshotDto, SmartMoneySnapshotDto } from "@/lib/contracts/dto";
import { readCopyTradeSnapshot } from "@/lib/api/copytrade";
import { readSmartMoneySnapshot } from "@/lib/api/smart-money";

export type CopyTradePageData = {
  snapshot: CopyTradeSnapshotDto;
  smartMoneySnapshot: SmartMoneySnapshotDto;
  requestId: string;
  traceId: string;
};

export async function getCopyTradePageData(): Promise<CopyTradePageData> {
  const [copytradeResponse, smartMoneyResponse] = await Promise.all([
    readCopyTradeSnapshot(),
    readSmartMoneySnapshot(),
  ]);

  return {
    snapshot: copytradeResponse.data,
    smartMoneySnapshot: smartMoneyResponse.data,
    requestId: copytradeResponse.meta.request_id,
    traceId: copytradeResponse.meta.trace_id,
  };
}
