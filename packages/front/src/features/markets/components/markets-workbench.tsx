"use client";

import { startTransition, useDeferredValue, useState } from "react";

import type { getMarketsPageData } from "@/features/markets/loaders/markets-page-data";
import { PageHeader } from "@/components/shared/page-header";
import { StatusPill } from "@/components/shared/status-pill";
import { WorkbenchLayout } from "@/components/shared/workbench-layout";
import { WorkbenchSegmentedControl } from "@/components/shared/workbench-segmented-control";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { useI18n } from "@/lib/i18n/client";
import { isKeyboardSelect } from "@/lib/keyboard";

type MarketsPageData = Awaited<ReturnType<typeof getMarketsPageData>>;
type MarketFilter = "all" | "review_queue" | "watch_only";

export function MarketsWorkbench({ data }: { data: MarketsPageData }) {
  const [selectedId, setSelectedId] = useState(data.selectedMarketId);
  const [filter, setFilter] = useState<MarketFilter>("all");
  const deferredFilter = useDeferredValue(filter);
  const { dictionary } = useI18n();

  const visibleMarkets = data.markets.filter((market) => {
    if (deferredFilter === "review_queue") {
      return market.tradabilityStatus === "manual_review" || market.tradabilityStatus === "blocked";
    }

    if (deferredFilter === "watch_only") {
      return market.tradabilityStatus === "observe_only";
    }

    return true;
  });

  const activeSelectedId =
    visibleMarkets.find((market) => market.id === selectedId)?.id ??
    visibleMarkets[0]?.id ??
    data.marketDetails[0]?.id ??
    "";
  const selectedMarket =
    data.marketDetails.find((market) => market.id === activeSelectedId) ?? data.marketDetails[0];

  if (!selectedMarket) {
    return null;
  }

  const filterButtons: Array<{ key: MarketFilter; label: string }> = [
    { key: "all", label: dictionary.markets.filterAll },
    { key: "review_queue", label: dictionary.markets.filterReview },
    { key: "watch_only", label: dictionary.markets.filterObserve },
  ];

  function selectMarket(marketId: string) {
    startTransition(() => {
      setSelectedId(marketId);
    });
  }

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow={dictionary.markets.eyebrow}
        title={dictionary.markets.title}
        description={dictionary.markets.description}
      />

      <WorkbenchLayout>
        <Card>
          <CardHeader className="flex flex-col gap-4 border-b border-border/70 md:flex-row md:items-center md:justify-between">
            <div>
              <CardTitle className="font-heading text-base">{dictionary.markets.universe}</CardTitle>
              <p className="mt-1 text-sm text-muted-foreground">
                {dictionary.markets.universeHint}
              </p>
            </div>
            <WorkbenchSegmentedControl items={filterButtons} value={filter} onChange={setFilter} />
          </CardHeader>
          <CardContent>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>{dictionary.markets.question}</TableHead>
                  <TableHead>{dictionary.markets.mid}</TableHead>
                  <TableHead>{dictionary.markets.tradability}</TableHead>
                  <TableHead>{dictionary.markets.ambiguity}</TableHead>
                  <TableHead>{dictionary.markets.events}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {visibleMarkets.map((market) => (
                  <TableRow
                    key={market.id}
                    tabIndex={0}
                    onClick={() => selectMarket(market.id)}
                    onKeyDown={(event) => {
                      if (isKeyboardSelect(event)) {
                        event.preventDefault();
                        selectMarket(market.id);
                      }
                    }}
                    className={
                      market.id === activeSelectedId
                        ? "cursor-pointer bg-accent/60 shadow-[inset_2px_0_0_#0066ff]"
                        : "cursor-pointer transition-colors hover:bg-accent/35"
                    }
                  >
                    <TableCell>
                      <div className="space-y-1">
                        <p className="font-medium">{market.question}</p>
                        <p className="text-xs text-muted-foreground">{market.category}</p>
                      </div>
                    </TableCell>
                    <TableCell className="font-mono">{market.midPrice}</TableCell>
                    <TableCell>
                      <StatusPill tone={market.tradabilityTone}>{market.tradabilityLabel}</StatusPill>
                    </TableCell>
                    <TableCell>
                      <StatusPill tone={market.ambiguityTone}>{market.ambiguityLabel}</StatusPill>
                    </TableCell>
                    <TableCell className="font-mono text-xs text-muted-foreground">{market.linkedEventCount}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>

        <div className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle className="font-heading text-base">{dictionary.markets.settlementView}</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <div>
                <p className="text-sm font-medium text-foreground">{selectedMarket.question}</p>
                <p className="mt-1 text-xs uppercase tracking-[0.2em] text-muted-foreground">
                  {selectedMarket.category}
                </p>
              </div>
              <div className="flex flex-wrap gap-2">
                <StatusPill tone={selectedMarket.tradabilityTone}>
                  {selectedMarket.tradabilityLabel}
                </StatusPill>
                <StatusPill tone={selectedMarket.ambiguityTone}>
                  {dictionary.markets.ambiguity} {selectedMarket.ambiguityLabel}
                </StatusPill>
              </div>
              <div className="rounded-sm border border-border/70 bg-card p-3 text-sm text-muted-foreground">
                {dictionary.markets.resolutionSource}: {selectedMarket.resolutionSource}
              </div>
              <div className="space-y-2 text-sm text-muted-foreground">
                {selectedMarket.edgeCaseNotes.map((note) => (
                  <p key={note}>{dictionary.markets.edgeCase}: {note}</p>
                ))}
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="font-heading text-base">{dictionary.markets.linkedEvents}</CardTitle>
            </CardHeader>
            <CardContent className="space-y-3">
              {selectedMarket.linkedEvents.map((event) => (
                <div key={event.id} className="rounded-sm border border-border/70 bg-card p-3">
                  <div className="flex items-center justify-between">
                    <StatusPill tone="primary">{event.source}</StatusPill>
                    <span className="font-mono text-xs text-muted-foreground">{event.relevance}</span>
                  </div>
                  <p className="mt-2 text-sm text-foreground">{event.summary}</p>
                </div>
              ))}
            </CardContent>
          </Card>
        </div>
      </WorkbenchLayout>
    </div>
  );
}
