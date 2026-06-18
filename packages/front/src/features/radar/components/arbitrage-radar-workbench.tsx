"use client";

import { startTransition, useDeferredValue, useEffect, useMemo, useState } from "react";
import { ChevronRight, Filter, Radar } from "lucide-react";

import { MetricCard } from "@/components/shared/metric-card";
import { PageHeader } from "@/components/shared/page-header";
import { PaginationBar } from "@/components/pagination-bar";
import { StatusPill } from "@/components/shared/status-pill";
import { WorkbenchDetailPane, WorkbenchLayout } from "@/components/shared/workbench-layout";
import { WorkbenchSegmentedControl } from "@/components/shared/workbench-segmented-control";
import { Button } from "@/components/ui/button";
import { usePagination } from "@/hooks/use-pagination";
import { dictionary, formatMessage } from "@/lib/i18n/dictionaries";
import type { RadarFilter, RadarPageData, RadarView } from "@/features/radar/types";
import { isKeyboardSelect } from "@/lib/keyboard";

import { viewMatches, compareRadarPriority } from "@/features/radar/lib/radar-helpers";
import { OpportunityDetail } from "./opportunity-detail";

type ArbitrageRadarWorkbenchProps = {
  data: RadarPageData;
};

export function ArbitrageRadarWorkbench({ data }: ArbitrageRadarWorkbenchProps) {
  const [filter, setFilter] = useState<RadarFilter>("all");
  const [view, setView] = useState<RadarView>("active");
  const [selectedId, setSelectedId] = useState(data.selectedOpportunityId);
  const deferredFilter = useDeferredValue(filter);

  const filterButtons: Array<{ key: RadarFilter; label: string }> = [
    { key: "all", label: dictionary.radar.all },
    { key: "binary_buy_both", label: dictionary.radar.buyBoth },
    { key: "binary_sell_both", label: dictionary.radar.sellBoth },
  ];
  const viewButtons: Array<{ key: RadarView; label: string }> = [
    { key: "active", label: dictionary.radar.active },
    { key: "validated", label: dictionary.radar.validated },
    { key: "rejected", label: dictionary.radar.rejected },
    { key: "history", label: dictionary.radar.history },
  ];

  const filteredOpportunities = useMemo(() => {
    return data.opportunities
      .filter((opportunity) => viewMatches(view, opportunity))
      .filter((opportunity) => deferredFilter === "all" || opportunity.opportunityType === deferredFilter)
      .slice()
      .sort(compareRadarPriority);
  }, [data.opportunities, deferredFilter, view]);

  const oppsPagination = usePagination(filteredOpportunities.length, 20);
  const scansPagination = usePagination(data.scans.length, 15);
  const { reset: resetOppsPagination } = oppsPagination;

  useEffect(() => {
    resetOppsPagination();
  }, [view, deferredFilter, resetOppsPagination]);

  const selectedOpportunity =
    filteredOpportunities.find((opportunity) => opportunity.id === selectedId) ??
    filteredOpportunities[0] ??
    data.opportunities.find((opportunity) => opportunity.id === selectedId) ??
    data.opportunities[0] ??
    null;

  function selectOpportunity(opportunityId: string) {
    startTransition(() => {
      setSelectedId(opportunityId);
    });
  }

  function cycleFilter() {
    const currentIndex = filterButtons.findIndex((item) => item.key === filter);
    const nextFilter = filterButtons[(currentIndex + 1) % filterButtons.length]?.key ?? "all";
    setFilter(nextFilter);
  }

  return (
    <div className="space-y-4">
      <PageHeader
        eyebrow={dictionary.radar.eyebrow}
        title={dictionary.radar.title}
        description={dictionary.radar.description}
        className="border-none pb-0"
        actions={
          <>
            <StatusPill tone="success">{formatMessage(dictionary.radar.observed, { count: data.opportunities.length })}</StatusPill>
            <StatusPill tone="primary">{formatMessage(dictionary.radar.scans, { count: data.scans.length })}</StatusPill>
          </>
        }
      />

      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        {data.metrics.map((metric) => (
          <MetricCard
            key={metric.title}
            title={metric.title}
            value={metric.value}
            hint={metric.hint}
            accent={metric.accent}
          />
        ))}
      </div>

      <WorkbenchLayout columnsClassName="xl:grid-cols-[1.55fr_0.95fr]">
        <div className="overflow-hidden rounded-lg bg-card/95 ring-1 ring-white/5">
          <div className="flex flex-col gap-4 bg-popover/70 px-5 py-4 xl:flex-row xl:items-center xl:justify-between">
            <div className="flex items-center gap-3">
              <Radar className="size-5 text-primary" />
              <h2 className="font-heading text-xl font-bold tracking-tight text-foreground">
                {dictionary.radar.opportunities}
              </h2>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <WorkbenchSegmentedControl items={viewButtons} value={view} onChange={setView} />
              <WorkbenchSegmentedControl items={filterButtons} value={filter} onChange={setFilter} />
              <Button
                variant="outline"
                size="sm"
                className="rounded-sm border-white/10 bg-accent/40 text-foreground hover:bg-accent"
                onClick={cycleFilter}
              >
                <Filter className="size-3.5" />
                {dictionary.common.filter}
              </Button>
            </div>
          </div>

          {filteredOpportunities.length > 0 ? (
            <>
            <div className="overflow-x-auto">
              <table className="w-full text-left">
                <thead className="bg-sidebar/60">
                  <tr className="text-[10px] font-bold uppercase tracking-[0.2em] text-muted-foreground">
                    <th className="px-5 py-3">{dictionary.radar.market}</th>
                    <th className="px-4 py-3">{dictionary.radar.type}</th>
                    <th className="px-4 py-3 text-right">{dictionary.radar.sum}</th>
                    <th className="px-4 py-3 text-right">{dictionary.radar.edge}</th>
                    <th className="px-4 py-3 text-right">{dictionary.radar.net}</th>
                    <th className="px-4 py-3 text-right">{dictionary.radar.capacity}</th>
                    <th className="px-4 py-3">{dictionary.radar.observedAt}</th>
                    <th className="px-4 py-3">{dictionary.radar.status}</th>
                    <th className="px-4 py-3">{dictionary.radar.validation}</th>
                    <th className="px-4 py-3">{dictionary.radar.candidate}</th>
                    <th className="px-5 py-3 text-right">{dictionary.radar.open}</th>
                  </tr>
                </thead>
                <tbody className="text-sm">
                  {filteredOpportunities.slice(oppsPagination.start, oppsPagination.end).map((opportunity) => (
                    <tr
                      key={opportunity.id}
                      tabIndex={0}
                      onClick={() => selectOpportunity(opportunity.id)}
                      onKeyDown={(event) => {
                        if (isKeyboardSelect(event)) {
                          event.preventDefault();
                          selectOpportunity(opportunity.id);
                        }
                      }}
                      className={
                        opportunity.id === selectedOpportunity?.id
                          ? "cursor-pointer bg-accent/45 shadow-[inset_2px_0_0_var(--sidebar-primary)] outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/50"
                          : "cursor-pointer outline-none transition-colors hover:bg-accent/35 focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/50"
                      }
                    >
                      <td className="px-5 py-3">
                        <div className="space-y-1">
                          <p className="max-w-[28rem] font-semibold text-foreground">
                            {opportunity.marketQuestion}
                          </p>
                          <p className="text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
                            {opportunity.contextLabel}
                          </p>
                        </div>
                      </td>
                      <td className="px-4 py-3">
                        <StatusPill tone={opportunity.typeTone}>{opportunity.typeLabel}</StatusPill>
                      </td>
                      <td className="px-4 py-3 text-right font-mono text-foreground">
                        {opportunity.priceSum}
                      </td>
                      <td className="px-4 py-3 text-right font-mono text-secondary">
                        {opportunity.grossEdge}
                      </td>
                      <td className="px-4 py-3 text-right font-mono text-secondary">
                        {opportunity.netEdge}
                      </td>
                      <td className="px-4 py-3 text-right font-mono">{opportunity.capacity}</td>
                      <td className="px-4 py-3 font-mono text-muted-foreground">
                        {opportunity.observedClock}
                      </td>
                      <td className="px-4 py-3">
                        <StatusPill tone={opportunity.statusTone}>{opportunity.statusLabel}</StatusPill>
                      </td>
                      <td className="px-4 py-3">
                        <StatusPill tone={opportunity.validationTone}>
                          {opportunity.validationLabel}
                        </StatusPill>
                      </td>
                      <td className="px-4 py-3">
                        <StatusPill tone={opportunity.candidateTone}>
                          {opportunity.candidateLabel}
                        </StatusPill>
                      </td>
                      <td className="px-5 py-3 text-right">
                        <button
                          type="button"
                          className="rounded-sm p-1 text-primary transition-colors hover:bg-primary/10"
                          onClick={(event) => {
                            event.stopPropagation();
                            selectOpportunity(opportunity.id);
                          }}
                        >
                          <ChevronRight className="ml-auto size-4" />
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <PaginationBar pagination={oppsPagination} totalItems={filteredOpportunities.length} className="flex items-center justify-between border-t border-border/70 px-5 pt-3 pb-4" />
            </>
          ) : (
            <div className="px-5 py-10 text-center">
              <p className="font-heading text-lg font-bold text-foreground">{dictionary.radar.noOpportunityTitle}</p>
              <p className="mt-2 text-sm text-muted-foreground">
                {dictionary.radar.noOpportunityDetail}
              </p>
            </div>
          )}
        </div>

        <WorkbenchDetailPane className="space-y-5">
          <OpportunityDetail opportunity={selectedOpportunity} />

          {data.analysis ? (
            <div className="space-y-4 rounded-md bg-popover/70 p-4">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <div>
                  <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
                    {dictionary.radar.analysis}
                  </p>
                  <p className="mt-1 text-sm text-foreground">
                    {data.analysis.generatedClock} / {data.analysis.lookbackHours}
                  </p>
                </div>
                <StatusPill tone="primary">{formatMessage(dictionary.metricHints.markets, { count: data.analysis.marketCount })}</StatusPill>
              </div>

              <div className="grid grid-cols-2 gap-3">
                {data.analysis.typeCounts.map((count) => (
                  <div key={count.typeLabel} className="rounded-md bg-accent/45 p-3">
                    <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
                      {count.typeLabel}
                    </p>
                    <p className="mt-2 font-mono text-lg text-foreground">{count.count}</p>
                  </div>
                ))}
              </div>

              <div className="space-y-3">
                {data.analysis.topMarkets.map((market) => (
                  <div key={market.marketId} className="rounded-md bg-accent/35 p-3">
                    <div className="flex items-start justify-between gap-3">
                      <p className="text-sm font-semibold text-foreground">{market.marketQuestion}</p>
                      <StatusPill tone="success">{market.maxGrossEdge}</StatusPill>
                    </div>
                    <div className="mt-3 grid grid-cols-3 gap-2 text-xs text-muted-foreground">
                      <span>{market.opportunityCount} {dictionary.radar.opps}</span>
                      <span>{market.maxCapacity} {dictionary.radar.cap}</span>
                      <span>{market.duration}</span>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          ) : null}

          <div className="rounded-md bg-popover/70 p-4">
            <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
              {dictionary.radar.scanHistory}
            </p>
            <div className="mt-3 space-y-3">
              {data.scans.slice(scansPagination.start, scansPagination.end).map((scan) => (
                <div key={scan.id} className="rounded-md bg-accent/35 p-3">
                  <div className="flex items-center justify-between gap-3">
                    <p className="font-mono text-xs text-foreground">{scan.startedClock}</p>
                    <StatusPill tone={scan.opportunityCount === "0" ? "neutral" : "success"}>
                      {scan.opportunityCount} {dictionary.radar.opps}
                    </StatusPill>
                  </div>
                  <p className="mt-2 text-xs text-muted-foreground">
                    {formatMessage(dictionary.metricHints.markets, { count: scan.marketCount })} / {scan.snapshotCount} {dictionary.radar.snapshots} / {scan.scannerVersion}
                  </p>
                </div>
              ))}
            </div>
            <PaginationBar pagination={scansPagination} totalItems={data.scans.length} className="mt-3 flex items-center justify-between border-t border-border/70 pt-3" />
          </div>
        </WorkbenchDetailPane>
      </WorkbenchLayout>
    </div>
  );
}
