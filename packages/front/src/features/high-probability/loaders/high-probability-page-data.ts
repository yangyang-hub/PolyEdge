import type {
  HighProbabilityBacktestReportDto,
  HighProbabilityBacktestRunDto,
  HighProbabilityBacktestTradeDto,
  HighProbabilityFairValueDto,
  HighProbabilityResearchReportDto,
  HighProbabilitySnapshotDto,
} from "@/lib/contracts/dto";
import {
  readHighProbabilityBacktestRuns,
  readHighProbabilityBacktestTrades,
  readHighProbabilityBacktests,
  readHighProbabilityFairValues,
  readHighProbabilityReport,
  readHighProbabilitySnapshot,
} from "@/lib/api/high-probability";

export type HighProbabilityPageData = {
  snapshot: HighProbabilitySnapshotDto;
  report: HighProbabilityResearchReportDto;
  backtest: HighProbabilityBacktestReportDto;
  backtestRuns: HighProbabilityBacktestRunDto[];
  backtestTrades: HighProbabilityBacktestTradeDto[];
  fairValues: HighProbabilityFairValueDto[];
  requestId: string;
  traceId: string;
};

export async function getHighProbabilityPageData(): Promise<HighProbabilityPageData> {
  const [
    snapshotResponse,
    reportResponse,
    backtestResponse,
    backtestRunsResponse,
    fairValuesResponse,
  ] = await Promise.all([
    readHighProbabilitySnapshot(),
    readHighProbabilityReport(),
    readHighProbabilityBacktests(),
    readHighProbabilityBacktestRuns(),
    readHighProbabilityFairValues(),
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
    fairValues: fairValuesResponse.data,
    requestId: snapshotResponse.meta.request_id,
    traceId: snapshotResponse.meta.trace_id,
  };
}
