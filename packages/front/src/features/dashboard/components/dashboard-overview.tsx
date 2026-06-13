"use client";

import { TriangleAlert } from "lucide-react";

import type { getDashboardPageData } from "@/features/dashboard/loaders/dashboard-page-data";
import { EmptyPanel } from "@/components/shared/empty-panel";
import { MetricCard } from "@/components/shared/metric-card";
import { MeterBar } from "@/components/shared/meter-bar";
import { PageHeader } from "@/components/shared/page-header";
import { PaginationBar } from "@/components/pagination-bar";
import { StateBanner } from "@/components/shared/state-banner";
import { StatusPill } from "@/components/shared/status-pill";
import { usePagination } from "@/hooks/use-pagination";
import { dictionary, translateEnum, formatMessage } from "@/lib/i18n/dictionaries";

type DashboardPageData = Awaited<ReturnType<typeof getDashboardPageData>>;

function readMetricCount(
  metrics: DashboardPageData["metrics"],
  key: string,
  fallback: number,
): number {
  const rawValue = metrics.find((metric) => metric.key === key)?.value;
  const parsedValue = Number.parseInt(rawValue ?? String(fallback), 10);
  return Number.isNaN(parsedValue) ? fallback : parsedValue;
}

export function DashboardOverview({ data }: { data: DashboardPageData }) {
  const openAlertCount = readMetricCount(data.metrics, "open_alerts", data.alerts.length);

  const signalsPagination = usePagination(data.signals.length, 10);
  const marketsPagination = usePagination(data.markets.length, 10);

  return (
    <div className="space-y-4">
      <PageHeader
        eyebrow={dictionary.dashboard.eyebrow}
        title={dictionary.dashboard.title}
        description={dictionary.dashboard.description}
        className="border-none pb-0"
        actions={
          <>
            <StatusPill tone="warning">{data.modeLabel}</StatusPill>
            <StatusPill tone="primary">{data.environmentLabel}</StatusPill>
          </>
        }
      />

      <section className="grid gap-4 lg:grid-cols-2">
        <StateBanner
          tone="success"
          title={dictionary.dashboard.streamTitle}
          detail={dictionary.dashboard.streamDetail}
          className="animate-in fade-in-0 duration-300"
        />
        <StateBanner
          tone={openAlertCount > 0 ? "warning" : "info"}
          title={openAlertCount > 0 ? dictionary.dashboard.reviewActiveTitle : dictionary.dashboard.reviewQuietTitle}
          detail={
            openAlertCount > 0
              ? formatMessage(dictionary.dashboard.reviewActiveDetail, { count: openAlertCount })
              : dictionary.dashboard.reviewQuietDetail
          }
          className="animate-in fade-in-0 duration-300"
        />
      </section>

      <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        {data.metrics.map((metric) => (
          <MetricCard
            key={metric.title}
            title={metric.title}
            value={metric.value}
            hint={metric.hint}
            accent={metric.tone}
          />
        ))}
      </section>

      <section className="grid gap-4 xl:grid-cols-[1.75fr_0.95fr]">
        <div className="overflow-hidden rounded-lg bg-card/95 ring-1 ring-white/5">
          <div className="flex items-center justify-between bg-popover/70 px-4 py-3">
            <p className="font-heading text-sm font-bold uppercase tracking-[0.18em] text-foreground">
              {dictionary.dashboard.realtimeSignals}
            </p>
          </div>

          {data.signals.length > 0 ? (
            <>
            <div className="overflow-x-auto">
              <table className="w-full text-left">
                <thead className="bg-sidebar/60">
                  <tr className="text-[10px] font-bold uppercase tracking-[0.2em] text-muted-foreground">
                    <th className="px-4 py-3">{dictionary.dashboard.tableMarket}</th>
                    <th className="px-4 py-3">{dictionary.dashboard.tableSide}</th>
                    <th className="px-4 py-3">{dictionary.dashboard.tableEdge}</th>
                    <th className="px-4 py-3">{dictionary.dashboard.tableConfidence}</th>
                    <th className="px-4 py-3">{dictionary.dashboard.tableState}</th>
                  </tr>
                </thead>
                <tbody>
                  {data.signals.slice(signalsPagination.start, signalsPagination.end).map((signal) => (
                    <tr
                      key={signal.id}
                      className="transition-colors hover:bg-accent/35"
                    >
                      <td className="px-4 py-3">
                        <div className="space-y-1">
                          <p className="font-medium text-foreground">{signal.marketQuestion}</p>
                          <p className="font-mono text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
                            {dictionary.dashboard.signalPrefix} {signal.id}
                          </p>
                        </div>
                      </td>
                      <td className="px-4 py-3">
                        <span
                          className={
                            signal.side === "YES"
                              ? "text-[10px] font-bold uppercase tracking-wide text-secondary"
                              : "text-[10px] font-bold uppercase tracking-wide text-destructive"
                          }
                        >
                          {signal.side}
                        </span>
                      </td>
                      <td className="px-4 py-3 font-mono text-xs">{signal.edge}</td>
                      <td className="px-4 py-3">
                        <div className="w-24 space-y-1">
                          <MeterBar
                            value={signal.confidenceWidth}
                            tone={signal.stateTone === "success" ? "success" : signal.stateTone}
                            trackClassName="h-1 bg-background"
                          />
                          <span className="block text-[10px] text-muted-foreground">{signal.confidence}</span>
                        </div>
                      </td>
                      <td className="px-4 py-3">
                        <div className="flex items-center gap-2">
                          <StatusPill tone={signal.stateTone}>{signal.stateLabel}</StatusPill>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <PaginationBar pagination={signalsPagination} totalItems={data.signals.length} className="flex items-center justify-between border-t border-border/70 px-4 pt-3 pb-3" />
            </>
          ) : (
            <EmptyPanel
              title={dictionary.dashboard.noLiveSignalsTitle}
              detail={dictionary.dashboard.noLiveSignalsDetail}
            />
          )}
        </div>

        <div className="space-y-4">
          <div className="rounded-lg bg-card/95 p-4 ring-1 ring-white/5">
            <div className="mb-3 flex items-center justify-between">
              <p className="font-heading text-sm font-bold uppercase tracking-[0.18em] text-foreground">
                {dictionary.dashboard.riskAlerts}
              </p>
              <TriangleAlert className="size-4 text-destructive" />
            </div>
            {data.alerts.length > 0 ? (
              <div className="space-y-3">
                {data.alerts.map((alert) => (
                  <div key={alert.id} className="rounded-md bg-accent/45 p-3">
                    <div className="flex items-center justify-between gap-3">
                      <StatusPill tone={alert.severityTone}>{translateEnum(alert.severity)}</StatusPill>
                      <span className="font-mono text-[10px] text-muted-foreground">{alert.createdAt}</span>
                    </div>
                    <p className="mt-2 text-sm font-medium text-foreground">{alert.reason}</p>
                    <p className="mt-1 text-[11px] text-muted-foreground">{alert.target}</p>
                  </div>
                ))}
              </div>
            ) : (
              <EmptyPanel
                title={dictionary.dashboard.noOpenAlertsTitle}
                detail={dictionary.dashboard.noOpenAlertsDetail}
              />
            )}
          </div>
        </div>
      </section>

      <section className="grid gap-4 xl:grid-cols-2">
        <div className="rounded-lg bg-card/95 p-4 ring-1 ring-white/5">
          <p className="mb-3 font-heading text-sm font-bold uppercase tracking-[0.18em] text-foreground">
            {dictionary.dashboard.hotMarkets}
          </p>
          <div className="space-y-3">
            {data.markets.slice(marketsPagination.start, marketsPagination.end).map((market) => (
              <div key={market.id} className="flex items-start justify-between gap-4 rounded-md bg-accent/35 p-3">
                <div className="space-y-1">
                  <p className="text-sm font-medium text-foreground">{market.question}</p>
                  <p className="text-[11px] uppercase tracking-[0.18em] text-muted-foreground">
                    {market.category}
                  </p>
                </div>
                <div className="space-y-2 text-right">
                  <p className="font-mono text-sm text-primary">{market.midPrice}</p>
                  <StatusPill tone={market.tradabilityTone}>{market.tradabilityLabel}</StatusPill>
                </div>
              </div>
            ))}
          </div>
          <PaginationBar pagination={marketsPagination} totalItems={data.markets.length} className="mt-3 flex items-center justify-between border-t border-border/70 pt-3" />
        </div>

        <div className="rounded-lg bg-card/95 p-4 ring-1 ring-white/5">
          <p className="mb-3 font-heading text-sm font-bold uppercase tracking-[0.18em] text-foreground">
            {dictionary.dashboard.latestEvents}
          </p>
          <div className="space-y-3">
            {data.events.map((event) => (
              <div key={event.id} className="rounded-md bg-popover/70 p-3">
                <div className="flex items-center justify-between gap-3">
                  <StatusPill tone="primary">{event.source}</StatusPill>
                  <span className="font-mono text-[10px] text-muted-foreground">
                    {dictionary.common.confidence} {event.confidence}
                  </span>
                </div>
                <p className="mt-2 text-sm text-foreground">{event.summary}</p>
              </div>
            ))}
          </div>
        </div>
      </section>
    </div>
  );
}
