"use client";

import { useCallback, useMemo, useState } from "react";

export interface PaginationState {
  /** Current 1-based page number. */
  page: number;
  /** Total number of pages. */
  totalPages: number;
  /** Zero-based start index for `Array.slice`. */
  start: number;
  /** Zero-based end index for `Array.slice`. */
  end: number;
  /** Navigate to a specific 1-based page. */
  setPage: (page: number) => void;
  /** Navigate to the previous page (no-op on first page). */
  goPrevious: () => void;
  /** Navigate to the next page (no-op on last page). */
  goNext: () => void;
  /** Reset to page 1. Call when filters change. */
  reset: () => void;
  /** Whether there is a previous page. */
  hasPrevious: boolean;
  /** Whether there is a next page. */
  hasNext: boolean;
}

export function usePagination(totalItems: number, pageSize: number = 20): PaginationState {
  const [page, setPageRaw] = useState(1);

  const totalPages = Math.max(1, Math.ceil(totalItems / pageSize));

  const clampedPage = Math.min(page, totalPages);

  const setPage = useCallback(
    (p: number) => {
      setPageRaw(Math.max(1, Math.min(totalPages, p)));
    },
    [totalPages],
  );

  const goPrevious = useCallback(() => {
    setPageRaw((p) => Math.max(1, p - 1));
  }, []);

  const goNext = useCallback(() => {
    setPageRaw((p) => Math.min(totalPages, p + 1));
  }, [totalPages]);

  const reset = useCallback(() => {
    setPageRaw(1);
  }, []);

  return useMemo(
    () => ({
      page: clampedPage,
      totalPages,
      start: (clampedPage - 1) * pageSize,
      end: clampedPage * pageSize,
      setPage,
      goPrevious,
      goNext,
      reset,
      hasPrevious: clampedPage > 1,
      hasNext: clampedPage < totalPages,
    }),
    [clampedPage, totalPages, pageSize, setPage, goPrevious, goNext, reset],
  );
}
