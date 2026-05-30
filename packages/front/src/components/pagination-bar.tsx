"use client";

import { Button } from "@/components/ui/button";
import { useI18n } from "@/lib/i18n/client";
import type { PaginationState } from "@/hooks/use-pagination";

interface PaginationBarProps {
  pagination: PaginationState;
  /** Total number of items across all pages. */
  totalItems: number;
  /** Optional class name for the container. */
  className?: string;
}

export function PaginationBar({ pagination, totalItems, className }: PaginationBarProps) {
  const { dictionary, format } = useI18n();
  const t = dictionary.common.pagination;

  if (pagination.totalPages <= 1) {
    return null;
  }

  return (
    <div
      className={
        className ?? "mt-4 flex items-center justify-between border-t border-border/70 pt-3"
      }
    >
      <p className="text-xs text-muted-foreground">
        {format(t.pageOf, { current: pagination.page, total: pagination.totalPages })}
        <span className="ml-2 text-muted-foreground/60">
          ({format(t.totalItems, { count: totalItems })})
        </span>
      </p>
      <div className="flex items-center gap-2">
        <Button
          variant="outline"
          size="sm"
          disabled={!pagination.hasPrevious}
          onClick={pagination.goPrevious}
        >
          {t.previous}
        </Button>
        <Button
          variant="outline"
          size="sm"
          disabled={!pagination.hasNext}
          onClick={pagination.goNext}
        >
          {t.next}
        </Button>
      </div>
    </div>
  );
}
