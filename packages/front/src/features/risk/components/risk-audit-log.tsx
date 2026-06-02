"use client";

import { useEffect } from "react";
import { Download } from "lucide-react";

import { Button } from "@/components/ui/button";
import { EmptyPanel } from "@/components/shared/empty-panel";
import { PaginationBar } from "@/components/pagination-bar";
import { StatusPill } from "@/components/shared/status-pill";
import { usePagination } from "@/hooks/use-pagination";
import { dictionary, translateEnum } from "@/lib/i18n/dictionaries";

import type { RiskAlertFilter, RiskPageData } from "../types";

export function RiskAuditLog({
  visibleAlerts,
  alertFilter,
  onAlertFilterChange,
  onExport,
  onManage,
}: {
  visibleAlerts: RiskPageData["alerts"];
  alertFilter: RiskAlertFilter;
  onAlertFilterChange: (filter: RiskAlertFilter) => void;
  onExport: () => void;
  onManage: (alert: RiskPageData["alerts"][number]) => void;
}) {
  const pagination = usePagination(visibleAlerts.length, 20);

  useEffect(() => {
    pagination.reset();
  }, [alertFilter, pagination.reset]);

  return (
    <div className="overflow-hidden rounded-lg bg-card/95 ring-1 ring-white/5">
      <div className="flex flex-col gap-3 bg-popover/70 px-5 py-4 md:flex-row md:items-center md:justify-between">
        <p className="font-heading text-sm font-bold uppercase tracking-[0.18em] text-foreground">
          {dictionary.risk.auditLog}
        </p>
        <div className="flex flex-wrap gap-2">
          <Button
            variant="outline"
            size="sm"
            className={
              alertFilter === "all"
                ? "rounded-sm border-primary/40 bg-primary/10 text-primary hover:bg-primary/15"
                : "rounded-sm border-white/10 bg-accent/40 text-foreground hover:bg-accent"
            }
            onClick={() => onAlertFilterChange("all")}
          >
            {dictionary.risk.filterAll}
          </Button>
          {(["unresolved", "watching"] as const).map((status) => (
            <Button
              key={status}
              variant="outline"
              size="sm"
              className={
                alertFilter === status
                  ? "rounded-sm border-primary/40 bg-primary/10 text-primary hover:bg-primary/15"
                  : "rounded-sm border-white/10 bg-accent/40 text-foreground hover:bg-accent"
              }
              onClick={() => onAlertFilterChange(status)}
            >
              {translateEnum(status)}
            </Button>
          ))}
          <Button
            variant="outline"
            size="sm"
            className="rounded-sm border-white/10 bg-accent/40 text-foreground hover:bg-accent"
            onClick={onExport}
          >
            <Download className="size-3.5" />
            {dictionary.risk.exportCsv}
          </Button>
        </div>
      </div>

      {visibleAlerts.length > 0 ? (
        <>
        <div className="overflow-x-auto">
          <table className="w-full text-left">
            <thead className="bg-sidebar/60">
              <tr className="text-[10px] font-bold uppercase tracking-[0.2em] text-muted-foreground">
                <th className="px-5 py-3">{dictionary.risk.severity}</th>
                <th className="px-5 py-3">{dictionary.risk.reason}</th>
                <th className="px-5 py-3">{dictionary.risk.marketTheme}</th>
                <th className="px-5 py-3">{dictionary.risk.timestamp}</th>
                <th className="px-5 py-3">{dictionary.risk.auditStatus}</th>
                <th className="px-5 py-3 text-right">{dictionary.risk.actions}</th>
              </tr>
            </thead>
            <tbody>
              {visibleAlerts.slice(pagination.start, pagination.end).map((alert) => (
                <tr key={alert.id} className="transition-colors hover:bg-accent/35">
                  <td className="px-5 py-4">
                    <StatusPill tone={alert.severityTone}>{translateEnum(alert.severity)}</StatusPill>
                  </td>
                  <td className="px-5 py-4 font-mono text-sm text-foreground">{alert.reason}</td>
                  <td className="px-5 py-4 text-sm text-foreground">{alert.target}</td>
                  <td className="px-5 py-4 font-mono text-xs text-muted-foreground">{alert.createdAt}</td>
                  <td className="px-5 py-4">
                    <StatusPill tone={alert.statusTone}>{alert.statusLabel}</StatusPill>
                  </td>
                  <td className="px-5 py-4 text-right">
                    <button
                      type="button"
                      className="text-xs font-bold uppercase tracking-[0.18em] text-primary transition-colors hover:text-primary/80"
                      onClick={() => onManage(alert)}
                    >
                      {dictionary.common.manage}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <PaginationBar pagination={pagination} totalItems={visibleAlerts.length} className="flex items-center justify-between border-t border-border/70 px-5 pt-3 pb-4" />
        </>
      ) : (
        <EmptyPanel
          title={dictionary.risk.noAlertsTitle}
          detail={dictionary.risk.noAlertsDetail}
        />
      )}
    </div>
  );
}
