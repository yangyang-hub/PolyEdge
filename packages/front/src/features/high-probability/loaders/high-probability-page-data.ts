import type {
  HighProbabilityBacktestReportDto,
  HighProbabilityBacktestRunDto,
  HighProbabilityBacktestTradeDto,
  HighProbabilityResearchReportDto,
  HighProbabilitySnapshotDto,
} from "@/lib/contracts/dto";
import {
  readHighProbabilityBacktestRuns,
  readHighProbabilityBacktestTrades,
  readHighProbabilityBacktests,
  readHighProbabilityReport,
  readHighProbabilitySnapshot,
} from "@/lib/api/high-probability";

export type HighProbabilityPageData = {
  snapshot: HighProbabilitySnapshotDto;
  report: HighProbabilityResearchReportDto;
  backtest: HighProbabilityBacktestReportDto;
  backtestRuns: HighProbabilityBacktestRunDto[];
  backtestTrades: HighProbabilityBacktestTradeDto[];
  requestId: string;
  traceId: string;
};

export async function getHighProbabilityPageData(): Promise<HighProbabilityPageData> {
  const [snapshotResponse, reportResponse, backtestResponse, backtestRunsResponse] = await Promise.all([
    readHighProbabilitySnapshot(),
    readHighProbabilityReport(),
    readHighProbabilityBacktests(),
    readHighProbabilityBacktestRuns(),
  ]);
  const latestRunId = backtestRunsResponse.data.at(0)?.id;
  const backtestTradesResponse = latestRunId
    ? await readHighProbabilityBacktestTrades(latestRunId)
    : null;

  return {
    snapshot: snapshotResponse.data,
    report: reportResponse.data,
    backtest: backtestResponse.data,
    backtestRuns: backtestRunsResponse.data,
    backtestTrades: backtestTradesResponse?.data ?? [],
    requestId: snapshotResponse.meta.request_id,
    traceId: snapshotResponse.meta.trace_id,
  };
}
