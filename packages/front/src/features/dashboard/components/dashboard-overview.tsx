"use client";

import type { getDashboardPageData } from "@/features/dashboard/loaders/dashboard-page-data";
import { MetricCard } from "@/components/shared/metric-card";
import { PageHeader } from "@/components/shared/page-header";
import { PaginationBar } from "@/components/pagination-bar";
import { TruncateText } from "@/components/shared/truncate-text";
import { StateBanner } from "@/components/shared/state-banner";
import { StatusPill } from "@/components/shared/status-pill";
import { usePagination } from "@/hooks/use-pagination";
import { dictionary } from "@/lib/i18n/dictionaries";

type DashboardPageData = Awaited<ReturnType<typeof getDashboardPageData>>;

export function DashboardOverview({ data }: { data: DashboardPageData }) {
  const marketsPagination = usePagination(data.markets.length, 10);

  return (
    <div className="space-y-4">
      <PageHeader
        eyebrow={dictionary.dashboard.eyebrow}
        title={dictionary.dashboard.title}
        description={dictionary.dashboard.description}
        className="border-none pb-0"
      />

      <section className="grid gap-4 lg:grid-cols-2">
        <StateBanner
          tone="success"
          title={dictionary.dashboard.streamTitle}
          detail={dictionary.dashboard.streamDetail}
          className="animate-in fade-in-0 duration-300"
        />
        <StateBanner
          tone="info"
          title={dictionary.dashboard.newsHealthTitle}
          detail={dictionary.dashboard.newsHealthDetail}
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

      <section className="grid gap-4 xl:grid-cols-2">
        <div className="rounded-lg bg-card/95 p-4 ring-1 ring-white/5">
          <p className="mb-3 font-heading text-sm font-bold uppercase tracking-[0.18em] text-foreground">
            {dictionary.dashboard.hotMarkets}
          </p>
          <div className="space-y-3">
            {data.markets.slice(marketsPagination.start, marketsPagination.end).map((market) => (
              <div key={market.id} className="flex items-start justify-between gap-4 rounded-md bg-accent/35 p-3">
                <div className="min-w-0 flex-1 space-y-1">
                  <TruncateText
                    text={market.question}
                    lines={2}
                    className="block text-sm font-medium text-foreground"
                  />
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
                <TruncateText
                  text={event.summary}
                  lines={2}
                  className="mt-2 block text-sm text-foreground"
                />
              </div>
            ))}
          </div>
        </div>
      </section>

      <section className="rounded-lg bg-card/95 p-4 ring-1 ring-white/5">
        <p className="mb-3 font-heading text-sm font-bold uppercase tracking-[0.18em] text-foreground">
          {dictionary.dashboard.newsSources}
        </p>
        <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
          {data.sourceHealth.map((source) => (
            <div key={source.source} className="rounded-md bg-accent/35 p-3">
              <div className="flex items-center justify-between gap-2">
                <StatusPill tone={source.tone}>{source.healthScore}</StatusPill>
                <span className="text-[11px] text-muted-foreground">{source.typeLabel}</span>
              </div>
              <p className="mt-2 text-sm font-medium text-foreground">{source.source}</p>
              <p className="mt-1 font-mono text-[11px] text-muted-foreground">{source.updatedAtLabel}</p>
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}
