"use client";

import { ClientDataBoundary } from "@/components/shared/client-data-boundary";
import { HighProbabilityWorkbench } from "@/features/high-probability/components/high-probability-workbench";
import { getHighProbabilityPageData } from "@/features/high-probability/loaders/high-probability-page-data";

export default function HighProbabilityPage() {
  return (
    <ClientDataBoundary load={getHighProbabilityPageData}>
      {(data) => (
        <HighProbabilityWorkbench
          initialSnapshot={data.snapshot}
          initialReport={data.report}
          initialBacktest={data.backtest}
          initialBacktestRuns={data.backtestRuns}
          initialBacktestTrades={data.backtestTrades}
        />
      )}
    </ClientDataBoundary>
  );
}
