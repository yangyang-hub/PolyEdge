"use client";

import { startTransition, useCallback, useEffect, useRef, useState } from "react";
import { ArrowUpDown, ExternalLink } from "lucide-react";

import type { getMarketsPageData, MarketListParams } from "@/features/markets/loaders/markets-page-data";
import { PageHeader } from "@/components/shared/page-header";
import { StatusPill } from "@/components/shared/status-pill";
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
import { listMarkets } from "@/lib/api/markets";
import { ambiguityTone, formatPercentFromRatio, marketTradabilityTone } from "@/lib/formatters";
import { useI18n } from "@/lib/i18n/client";
import { isKeyboardSelect } from "@/lib/keyboard";
import { listEvents } from "@/lib/api/events";

type MarketsPageData = Awaited<ReturnType<typeof getMarketsPageData>>;
type MarketFilter = "all" | "review_queue" | "watch_only";
type SortDir = "desc" | "asc" | "none";

const PAGE_SIZE = 20;

export function MarketsWorkbench({ data }: { data: MarketsPageData }) {
  const [markets, setMarkets] = useState(data.markets);
  const [marketDetails, setMarketDetails] = useState(data.marketDetails);
  const [totalCount, setTotalCount] = useState(data.totalCount);
  const [loading, setLoading] = useState(false);
  const [selectedId, setSelectedId] = useState(data.selectedMarketId);

  const [filter, setFilter] = useState<MarketFilter>("all");
  const [category, setCategory] = useState<string>("all");
  const [sortDir, setSortDir] = useState<SortDir>("none");
  const [page, setPage] = useState(1);

  const { dictionary, format, enumLabel } = useI18n();
  const abortRef = useRef<AbortController | null>(null);

  const categories = data.markets.map((m) => m.category).filter(Boolean);
  const uniqueCategories = [...new Set(categories)].sort((a, b) => a.localeCompare(b));

  const fetchMarkets = useCallback(async (params: MarketListParams) => {
    abortRef.current?.abort();
    const controller = new AbortController();
    abortRef.current = controller;

    setLoading(true);
    try {
      const [{ data: newMarkets, totalCount: newTotal }, { data: events }] = await Promise.all([
        listMarkets(params),
        listEvents(),
      ]);

      if (controller.signal.aborted) return;

      const mappedMarkets = newMarkets.map((market) => ({
        id: market.id,
        question: market.question,
        category: market.category,
        midPrice: market.mid_price,
        volume24h: market.volume_24h,
        tradabilityStatus: market.tradability_status,
        tradabilityLabel: enumLabel(market.tradability_status),
        tradabilityTone: marketTradabilityTone(market.tradability_status),
        ambiguityLabel: enumLabel(market.ambiguity_level),
        ambiguityTone: ambiguityTone(market.ambiguity_level),
        linkedEventCount: String(events.filter((event) => event.related_market_ids.includes(market.id)).length).padStart(2, "0"),
      }));

      const mappedDetails = newMarkets.map((market) => ({
        id: market.id,
        question: market.question,
        category: market.category,
        polymarketConditionId: market.polymarket_condition_id ?? null,
        tradabilityLabel: enumLabel(market.tradability_status),
        tradabilityTone: marketTradabilityTone(market.tradability_status),
        ambiguityLabel: enumLabel(market.ambiguity_level),
        ambiguityTone: ambiguityTone(market.ambiguity_level),
        resolutionSource: market.resolution_source,
        edgeCaseNotes: market.edge_case_notes,
        linkedEvents: events
          .filter((event) => event.related_market_ids.includes(market.id))
          .map((event) => ({
            id: event.id,
            source: event.source,
            relevance: formatPercentFromRatio(event.relevance_score),
            summary: event.summary,
          })),
      }));

      setMarkets(mappedMarkets);
      setMarketDetails(mappedDetails);
      setTotalCount(newTotal);
      if (mappedMarkets.length > 0 && !mappedMarkets.find((m) => m.id === selectedId)) {
        setSelectedId(mappedMarkets[0].id);
      }
    } finally {
      if (!controller.signal.aborted) {
        setLoading(false);
      }
    }
  }, [enumLabel, selectedId]);

  useEffect(() => {
    const tradabilityStatus =
      filter === "review_queue" ? "blocked" :
      filter === "watch_only" ? "observe_only" :
      undefined;

    const offset = (page - 1) * PAGE_SIZE;

    fetchMarkets({
      limit: PAGE_SIZE,
      offset,
      tradability_status: tradabilityStatus,
      category: category !== "all" ? category : undefined,
      sort_by: sortDir !== "none" ? "volume_24h" : undefined,
      sort_order: sortDir !== "none" ? sortDir : undefined,
    });
  }, [filter, category, sortDir, page, fetchMarkets]);

  const totalPages = Math.max(1, Math.ceil(totalCount / PAGE_SIZE));

  const activeSelectedId =
    markets.find((market) => market.id === selectedId)?.id ??
    markets[0]?.id ??
    marketDetails[0]?.id ??
    "";
  const selectedMarket =
    marketDetails.find((market) => market.id === activeSelectedId) ?? marketDetails[0];

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

  function cycleSort() {
    setSortDir((prev) => (prev === "none" ? "desc" : prev === "desc" ? "asc" : "none"));
    setPage(1);
  }

  function handleFilterChange(next: MarketFilter) {
    setFilter(next);
    setPage(1);
  }

  function handleCategoryChange(next: string) {
    setCategory(next);
    setPage(1);
  }

  const polymarketUrl = selectedMarket.polymarketConditionId
    ? `https://polymarket.com/event/${selectedMarket.polymarketConditionId}`
    : null;

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
            <div className="flex flex-wrap items-center gap-3">
              <select
                value={category}
                onChange={(e) => handleCategoryChange(e.target.value)}
                className="h-8 rounded-lg border border-input bg-transparent px-2.5 text-sm transition-colors outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 dark:bg-input/30"
              >
                <option value="all">{dictionary.markets.allCategories}</option>
                {uniqueCategories.map((cat) => (
                  <option key={cat} value={cat}>{cat}</option>
                ))}
              </select>
              <WorkbenchSegmentedControl items={filterButtons} value={filter} onChange={handleFilterChange} />
            </div>
          </CardHeader>
          <CardContent>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>{dictionary.markets.question}</TableHead>
                  <TableHead>{dictionary.markets.mid}</TableHead>
                  <TableHead>
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
                  <TableRow>
                    <TableCell colSpan={6} className="py-8 text-center text-sm text-muted-foreground">
                      {dictionary.common.loading}
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
                {format(dictionary.markets.pageOf, { current: page, total: totalPages })}
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

function formatVolume(value: string): string {
  const num = parseFloat(value);
  if (!num || Number.isNaN(num)) return "—";
  if (num >= 1_000_000) return `$${(num / 1_000_000).toFixed(1)}M`;
  if (num >= 1_000) return `$${(num / 1_000).toFixed(1)}K`;
  return `$${num.toFixed(0)}`;
}
