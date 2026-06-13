"use client";

import { useEffect } from "react";
import { ChevronRight } from "lucide-react";

import { MeterBar } from "@/components/shared/meter-bar";
import { PaginationBar } from "@/components/pagination-bar";
import { StatusPill } from "@/components/shared/status-pill";
import { Button } from "@/components/ui/button";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from "@/components/ui/sheet";
import { usePagination } from "@/hooks/use-pagination";
import { dictionary } from "@/lib/i18n/dictionaries";
import { isKeyboardSelect } from "@/lib/keyboard";

import { SignalsDetailPanel } from "./signals-detail-panel";
import type { RuntimeControls, SignalActionDialog, SignalItem } from "../types";

export function SignalsTable({
  signals,
  selectedSignalId,
  runtimeControls,
  onSelect,
  onOpenAction,
}: {
  signals: SignalItem[];
  selectedSignalId?: string;
  runtimeControls: RuntimeControls;
  onSelect: (signalId: string) => void;
  onOpenAction: (signalId: string, dialog: Exclude<SignalActionDialog, null>) => void;
}) {
  const pagination = usePagination(signals.length, 20);
  const { reset: resetPagination } = pagination;

  useEffect(() => {
    resetPagination();
  }, [signals.length, resetPagination]);

  if (signals.length === 0) {
    return (
      <div className="px-5 py-10 text-center">
        <p className="font-heading text-lg font-bold text-foreground">{dictionary.signals.noFilterTitle}</p>
        <p className="mt-2 text-sm text-muted-foreground">
          {dictionary.signals.noFilterDetail}
        </p>
      </div>
    );
  }

  return (
    <div className="overflow-x-auto">
      <table className="w-full text-left">
        <thead className="bg-sidebar/60">
          <tr className="text-[10px] font-bold uppercase tracking-[0.2em] text-muted-foreground">
            <th className="px-5 py-3">{dictionary.signals.market}</th>
            <th className="px-4 py-3">{dictionary.signals.side}</th>
            <th className="px-4 py-3">{dictionary.signals.fair}</th>
            <th className="px-4 py-3">{dictionary.signals.marketPrice}</th>
            <th className="px-4 py-3 text-right">{dictionary.signals.edge}</th>
            <th className="px-4 py-3">{dictionary.dashboard.tableConfidence}</th>
            <th className="px-4 py-3">{dictionary.dashboard.tableState}</th>
            <th className="px-5 py-3 text-right">{dictionary.signals.action}</th>
          </tr>
        </thead>
        <tbody className="text-sm">
          {signals.slice(pagination.start, pagination.end).map((signal) => (
            <tr
              key={signal.id}
              tabIndex={0}
              onClick={() => onSelect(signal.id)}
              onKeyDown={(event) => {
                if (isKeyboardSelect(event)) {
                  event.preventDefault();
                  onSelect(signal.id);
                }
              }}
              className={
                signal.id === selectedSignalId
                  ? "cursor-pointer bg-accent/45 shadow-[inset_2px_0_0_#0066ff]"
                  : "cursor-pointer transition-colors hover:bg-accent/35"
              }
            >
              <td className="px-5 py-3">
                <div className="space-y-1">
                  <p className="font-semibold text-foreground">{signal.marketQuestion}</p>
                  <p className="text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
                    {signal.contextLabel}
                  </p>
                </div>
              </td>
              <td className="px-4 py-3">
                <span
                  className={
                    signal.side === "YES"
                      ? "font-bold uppercase tracking-wide text-secondary"
                      : "font-bold uppercase tracking-wide text-destructive"
                  }
                >
                  {signal.side}
                </span>
              </td>
              <td className="px-4 py-3 font-mono text-primary">{signal.fairPrice}</td>
              <td className="px-4 py-3 font-mono text-foreground">{signal.marketPrice}</td>
              <td className="px-4 py-3 text-right font-mono">{signal.edge}</td>
              <td className="px-4 py-3">
                <div className="w-20 space-y-1">
                  <MeterBar
                    value={signal.confidenceWidth}
                    tone={signal.stateTone === "success" ? "success" : signal.stateTone}
                    trackClassName="h-1 bg-background"
                  />
                  <span className="block text-[10px] text-muted-foreground">{signal.confidence}</span>
                </div>
              </td>
              <td className="px-4 py-3">
                <div className="flex flex-wrap gap-2">
                  <StatusPill tone={signal.stateTone}>{signal.stateLabel}</StatusPill>
                </div>
              </td>
              <td className="px-5 py-3 text-right">
                <div className="hidden xl:block">
                  <button
                    type="button"
                    className="rounded-sm p-1 text-primary transition-colors hover:bg-primary/10"
                    onClick={(event) => {
                      event.stopPropagation();
                      onSelect(signal.id);
                    }}
                  >
                    <ChevronRight className="ml-auto size-4" />
                  </button>
                </div>
                <div className="xl:hidden">
                  <Sheet>
                    <SheetTrigger asChild>
                      <Button
                        variant="ghost"
                        size="icon-sm"
                        className="rounded-sm text-primary hover:bg-primary/10"
                        onClick={() => onSelect(signal.id)}
                      >
                        <ChevronRight className="size-4" />
                      </Button>
                    </SheetTrigger>
                    <SheetContent className="w-full max-w-none border-white/10 bg-card p-0 sm:max-w-md">
                      <SheetHeader className="border-b border-white/8 px-5 py-4">
                        <SheetTitle>{dictionary.signals.signalDetail}</SheetTitle>
                        <SheetDescription>
                          {dictionary.signals.signalDetailDescription}
                        </SheetDescription>
                      </SheetHeader>
                      <div className="overflow-y-auto px-5 py-5">
                        <SignalsDetailPanel
                          signal={signal}
                          runtimeControls={runtimeControls}
                          onOpenAction={onOpenAction}
                        />
                      </div>
                    </SheetContent>
                  </Sheet>
                </div>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <PaginationBar pagination={pagination} totalItems={signals.length} />
    </div>
  );
}
