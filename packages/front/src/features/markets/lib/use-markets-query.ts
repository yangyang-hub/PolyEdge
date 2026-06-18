"use client";

import { startTransition, useCallback, useEffect, useRef, useState } from "react";

import type { EventDto } from "@/lib/contracts/dto";
import { listMarkets } from "@/lib/api/markets";
import { dictionary } from "@/lib/i18n/dictionaries";

import type { MarketDetailViewModel, MarketFilter, MarketViewModel, SortDir } from "../types";
import { mapMarkets, mapMarketDetails } from "./markets-mappers";
import { buildMarketListParams } from "./markets-query";

export interface MarketsQueryInitial {
  markets: MarketViewModel[];
  marketDetails: MarketDetailViewModel[];
  /** 首屏由 loader 拉取的事件列表；客户端筛选/翻页时复用，不再重复请求 events。 */
  events: EventDto[];
  totalCount: number;
  selectedId: string;
}

/**
 * markets 工作台的数据与交互状态。events 只取一次（loader 注入），
 * 筛选/分类/排序/翻页只重新拉 markets 并用缓存的 events 重新映射，
 * 避免 high-frequency 表格每次交互都额外打一次 events 请求。
 */
export function useMarketsQuery(initial: MarketsQueryInitial) {
  const [markets, setMarkets] = useState(initial.markets);
  const [marketDetails, setMarketDetails] = useState(initial.marketDetails);
  const [totalCount, setTotalCount] = useState(initial.totalCount);
  const [events] = useState(initial.events);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState(initial.selectedId);

  const [filter, setFilterState] = useState<MarketFilter>("all");
  const [category, setCategoryState] = useState("all");
  const [sortDir, setSortDir] = useState<SortDir>("none");
  const [page, setPage] = useState(1);

  const abortRef = useRef<AbortController | null>(null);
  const selectedIdRef = useRef(initial.selectedId);
  useEffect(() => {
    selectedIdRef.current = selectedId;
  }, [selectedId]);

  const refetch = useCallback(
    async (
      nextFilter: MarketFilter,
      nextCategory: string,
      nextSortDir: SortDir,
      nextPage: number,
    ) => {
      abortRef.current?.abort();
      const controller = new AbortController();
      abortRef.current = controller;

      setLoading(true);
      setError(null);
      try {
        const { data: newMarkets, totalCount: newTotal } = await listMarkets(
          buildMarketListParams(nextFilter, nextCategory, nextSortDir, nextPage),
        );
        if (controller.signal.aborted) return;

        const mappedMarkets = mapMarkets(newMarkets, events);
        setMarkets(mappedMarkets);
        setMarketDetails(mapMarketDetails(newMarkets, events));
        setTotalCount(newTotal);

        const current = selectedIdRef.current;
        if (mappedMarkets.length > 0 && !mappedMarkets.find((m) => m.id === current)) {
          setSelectedId(mappedMarkets[0].id);
        }
      } catch (cause) {
        if (controller.signal.aborted) return;
        setError(cause instanceof Error ? cause.message : dictionary.markets.loadFailed);
      } finally {
        if (!controller.signal.aborted) setLoading(false);
      }
    },
    [events],
  );

  useEffect(() => {
    startTransition(() => {
      void refetch(filter, category, sortDir, page);
    });
    return () => {
      abortRef.current?.abort();
    };
  }, [filter, category, sortDir, page, refetch]);

  const selectMarket = useCallback((marketId: string) => {
    startTransition(() => setSelectedId(marketId));
  }, []);

  const setFilter = useCallback((next: MarketFilter) => {
    setFilterState(next);
    setPage(1);
  }, []);

  const setCategory = useCallback((next: string) => {
    setCategoryState(next);
    setPage(1);
  }, []);

  const cycleSort = useCallback(() => {
    setSortDir((prev) => (prev === "none" ? "desc" : prev === "desc" ? "asc" : "none"));
    setPage(1);
  }, []);

  return {
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
  };
}
