"use client";

import { Button } from "@/components/ui/button";
import { MeterBar } from "@/components/shared/meter-bar";
import { useI18n } from "@/lib/i18n/client";

import type { RiskPageData } from "../types";

export function RiskMetricsOverview({
  summary,
  visibleMetrics,
  onViewLog,
}: {
  summary: RiskPageData["summary"];
  visibleMetrics: RiskPageData["metrics"];
  onViewLog: () => void;
}) {
  const { dictionary } = useI18n();

  return (
    <section className="grid gap-4 xl:grid-cols-12">
      <div className="rounded-lg bg-card/95 p-5 ring-1 ring-white/5 xl:col-span-4">
        <div className="mb-4 flex items-start justify-between gap-3">
          <p className="text-[10px] font-bold uppercase tracking-[0.2em] text-muted-foreground">
            {dictionary.risk.dailyLossUsage}
          </p>
          <span className="font-mono text-xs text-destructive">
            {dictionary.common.critical} ({summary.dailyLossUsage})
          </span>
        </div>
        <div className="mb-3 flex items-end gap-2">
          <span className="font-heading text-4xl font-black leading-none text-foreground">
            {summary.dailyLossUsed}
          </span>
          <span className="pb-1 font-mono text-xs text-muted-foreground">
            / {summary.dailyLossLimit}
          </span>
        </div>
        <MeterBar
          value={summary.dailyLossWidth}
          tone="danger"
          trackClassName="h-3 bg-background"
          barClassName="bg-gradient-to-r from-primary via-primary to-destructive"
        />
      </div>

      <div className="rounded-lg bg-card/95 p-5 ring-1 ring-white/5 xl:col-span-5">
        <div className="grid gap-4 md:grid-cols-2">
          {visibleMetrics.map((metric) => (
            <div key={metric.title} className="space-y-2 rounded-md bg-accent/35 p-4">
              <p className="text-[10px] font-bold uppercase tracking-[0.2em] text-muted-foreground">
                {metric.title}
              </p>
              <p className="font-heading text-3xl font-black leading-none text-foreground">{metric.value}</p>
              <p className="font-mono text-[11px] text-muted-foreground">{metric.hint}</p>
            </div>
          ))}
        </div>
      </div>

      <div className="rounded-lg border border-destructive/10 bg-destructive/5 p-5 ring-1 ring-destructive/10 xl:col-span-3">
        <p className="text-[10px] font-bold uppercase tracking-[0.2em] text-destructive">{dictionary.risk.activeAlerts}</p>
        <div className="mt-3 flex items-end justify-between">
          <span className="font-heading text-5xl font-black leading-none text-destructive">
            {String(summary.criticalAlerts + summary.warningAlerts).padStart(2, "0")}
          </span>
          <div className="text-right text-xs">
            <p className="font-semibold text-foreground">{summary.criticalAlerts} {dictionary.common.critical}</p>
            <p className="text-muted-foreground">{summary.warningAlerts} {dictionary.common.warnings}</p>
          </div>
        </div>
        <Button
          className="mt-4 h-8 w-full rounded-sm bg-destructive text-destructive-foreground hover:bg-destructive/90"
          onClick={onViewLog}
        >
          {dictionary.risk.viewLog}
        </Button>
      </div>
    </section>
  );
}
