"use client";

import { ArrowUpDown, ExternalLink } from "lucide-react";

import type { getMarketsPageData } from "../loaders/markets-page-data";
import { EmptyPanel } from "@/components/shared/empty-panel";
import { PageHeader } from "@/components/shared/page-header";
import { StatusPill } from "@/components/shared/status-pill";
import { TruncateText } from "@/components/shared/truncate-text";
import { WorkbenchLayout } from "@/components/shared/workbench-layout";
import { WorkbenchSegmentedControl } from "@/components/shared/workbench-segmented-control";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { dictionary, formatMessage } from "@/lib/i18n/dictionaries";
import { isKeyboardSelect } from "@/lib/keyboard";
import { useMarketsQuery } from "../lib/use-markets-query";
import { MARKETS_PAGE_SIZE } from "../lib/markets-query";
import type { MarketFilter } from "../types";

type MarketsPageData = Awaited<ReturnType<typeof getMarketsPageData>>;

const SKELETON_ROWS = 6;

export function MarketsWorkbench({ data }: { data: MarketsPageData }) {
  const {
    markets,
    marketDetails,
    totalCount,
    loading,
    error,
    selectedId,
    selectMarket,
    filter,
    category,
    sortDir,
    page,
    setFilter,
    setCategory,
    cycleSort,
    setPage,
  } = useMarketsQuery({
    markets: data.markets,
    marketDetails: data.marketDetails,
    events: data.events,
    totalCount: data.totalCount,
    selectedId: data.selectedMarketId,
  });

  const totalPages = Math.max(1, Math.ceil(totalCount / MARKETS_PAGE_SIZE));
  const activeSelectedId =
    markets.find((market) => market.id === selectedId)?.id ??
    markets[0]?.id ??
    marketDetails[0]?.id ??
    "";
  const selectedMarket =
    marketDetails.find((market) => market.id === activeSelectedId) ?? marketDetails[0] ?? null;
  const polymarketUrl = selectedMarket?.slug
    ? `https://polymarket.com/event/${selectedMarket.slug}`
    : null;

  const filterButtons: Array<{ key: MarketFilter; label: string }> = [
    { key: "all", label: dictionary.markets.filterAll },
    { key: "review_queue", label: dictionary.markets.filterReview },
    { key: "watch_only", label: dictionary.markets.filterObserve },
  ];

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow={dictionary.markets.eyebrow}
        title={dictionary.markets.title}
        description={dictionary.markets.description}
      />

      <WorkbenchLayout className="items-start">
        <Card className="max-h-[calc(100vh-14rem)] overflow-y-auto">
          <CardHeader className="flex flex-col gap-4 border-b border-border/70 md:flex-row md:items-center md:justify-between">
            <div>
              <CardTitle className="font-heading text-base">{dictionary.markets.universe}</CardTitle>
              <p className="mt-1 text-sm text-muted-foreground">
                {dictionary.markets.universeHint}
              </p>
            </div>
            <div className="flex flex-wrap items-center gap-3">
              <select
                value={category}
                onChange={(event) => setCategory(event.target.value)}
                className="h-8 rounded-lg border border-input bg-transparent px-2.5 text-sm transition-colors outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 dark:bg-input/30"
              >
                <option value="all">{dictionary.markets.allCategories}</option>
                {data.categories.map((cat) => (
                  <option key={cat.id} value={cat.id}>{cat.label}</option>
                ))}
              </select>
              <WorkbenchSegmentedControl items={filterButtons} value={filter} onChange={setFilter} />
            </div>
          </CardHeader>
          <CardContent>
            <Table className="table-fixed">
              <TableHeader>
                <TableRow>
                  <TableHead className="w-[40%]">{dictionary.markets.question}</TableHead>
                  <TableHead>{dictionary.markets.mid}</TableHead>
                  <TableHead
                    aria-sort={
                      sortDir === "asc" ? "ascending" : sortDir === "desc" ? "descending" : "none"
                    }
                  >
                    <button
                      type="button"
                      onClick={cycleSort}
                      className="inline-flex items-center gap-1 transition-colors hover:text-foreground"
                    >
                      {dictionary.markets.volume}
                      <ArrowUpDown className="size-3" />
                      {sortDir !== "none" && (
                        <span className="text-xs">{sortDir === "desc" ? "↓" : "↑"}</span>
                      )}
                    </button>
                  </TableHead>
                  <TableHead>{dictionary.markets.tradability}</TableHead>
                  <TableHead>{dictionary.markets.ambiguity}</TableHead>
                  <TableHead>{dictionary.markets.events}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {loading ? (
                  Array.from({ length: SKELETON_ROWS }).map((_, index) => (
                    <TableRow key={`market-skeleton-${index}`}>
                      <TableCell colSpan={6} className="py-3">
                        <div className="h-4 animate-pulse rounded bg-accent/40" />
                      </TableCell>
                    </TableRow>
                  ))
                ) : error ? (
                  <TableRow>
                    <TableCell colSpan={6} className="py-8 text-center text-sm text-destructive">
                      {error}
                    </TableCell>
                  </TableRow>
                ) : markets.length === 0 ? (
                  <TableRow>
                    <TableCell colSpan={6} className="py-8 text-center text-sm text-muted-foreground">
                      {dictionary.markets.noResults}
                    </TableCell>
                  </TableRow>
                ) : (
                  markets.map((market) => (
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
                          ? "cursor-pointer bg-accent/60 shadow-[inset_2px_0_0_var(--sidebar-primary)] outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/50"
                          : "cursor-pointer outline-none transition-colors hover:bg-accent/35 focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/50"
                      }
                    >
                      <TableCell className="w-[40%]">
                        <div className="space-y-1">
                          <TruncateText text={market.question} lines={2} className="font-medium" />
                          <p className="text-xs text-muted-foreground">{market.category}</p>
                        </div>
                      </TableCell>
                      <TableCell className="font-mono">{market.midPrice}</TableCell>
                      <TableCell className="font-mono text-xs">
                        {formatVolume(market.volume24h)}
                      </TableCell>
                      <TableCell>
                        <StatusPill tone={market.tradabilityTone}>{market.tradabilityLabel}</StatusPill>
                      </TableCell>
                      <TableCell>
                        <StatusPill tone={market.ambiguityTone}>{market.ambiguityLabel}</StatusPill>
                      </TableCell>
                      <TableCell className="font-mono text-xs text-muted-foreground">{market.linkedEventCount}</TableCell>
                    </TableRow>
                  ))
                )}
              </TableBody>
            </Table>

            <div className="mt-4 flex items-center justify-between border-t border-border/70 pt-3">
              <p className="text-xs text-muted-foreground">
                {formatMessage(dictionary.markets.pageOf, { current: page, total: totalPages })}
              </p>
              <div className="flex items-center gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  disabled={page <= 1 || loading}
                  onClick={() => setPage((p) => Math.max(1, p - 1))}
                >
                  {dictionary.markets.previous}
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  disabled={page >= totalPages || loading}
                  onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
                >
                  {dictionary.markets.next}
                </Button>
              </div>
            </div>
          </CardContent>
        </Card>

        <div className="space-y-4 max-h-[calc(100vh-14rem)] overflow-y-auto">
          {selectedMarket ? (
            <>
              <Card>
                <CardHeader className="py-3">
                  <CardTitle className="font-heading text-sm">{dictionary.markets.settlementView}</CardTitle>
                </CardHeader>
                <CardContent className="space-y-3">
                  <div>
                    <TruncateText text={selectedMarket.question} lines={2} className="text-sm font-medium text-foreground" />
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
                  {polymarketUrl && (
                    <a
                      href={polymarketUrl}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="inline-flex items-center gap-1.5 text-sm text-primary underline-offset-4 hover:underline"
                    >
                      {dictionary.markets.viewOnPolymarket}
                      <ExternalLink className="size-3.5" />
                    </a>
                  )}
                  <div className="line-clamp-2 rounded-sm border border-border/70 bg-card p-2.5 text-xs text-muted-foreground">
                    {dictionary.markets.resolutionSource}: {selectedMarket.resolutionSource}
                  </div>
                  <div className="space-y-1 text-xs text-muted-foreground">
                    {selectedMarket.edgeCaseNotes.map((note) => (
                      <p key={note} className="line-clamp-1">{dictionary.markets.edgeCase}: {note}</p>
                    ))}
                  </div>
                </CardContent>
              </Card>

              <Card>
                <CardHeader className="py-3">
                  <CardTitle className="font-heading text-sm">{dictionary.markets.linkedEvents}</CardTitle>
                </CardHeader>
                <CardContent className="space-y-2">
                  {selectedMarket.linkedEvents.length === 0 ? (
                    <p className="text-xs text-muted-foreground">—</p>
                  ) : (
                    selectedMarket.linkedEvents.map((event) => (
                      <div key={event.id} className="rounded-sm border border-border/70 bg-card px-3 py-2">
                        <div className="flex items-center justify-between gap-2">
                          <StatusPill tone="primary">{event.source}</StatusPill>
                          <span className="shrink-0 font-mono text-xs text-muted-foreground">{event.relevance}</span>
                        </div>
                        <p className="mt-1.5 line-clamp-2 text-xs text-muted-foreground">{event.summary}</p>
                      </div>
                    ))
                  )}
                </CardContent>
              </Card>
            </>
          ) : (
            <EmptyPanel title={dictionary.markets.settlementView} detail={dictionary.markets.emptyDetail} />
          )}
        </div>
      </WorkbenchLayout>
    </div>
  );
}

function formatVolume(value: string): string {
  const num = parseFloat(value);
  if (!num || Number.isNaN(num)) return "—";
  if (num >= 1_000_000) return `$${(num / 1_000_000).toFixed(1)}M`;
  if (num >= 1_000) return `$${(num / 1_000).toFixed(1)}K`;
  return `$${num.toFixed(0)}`;
}
