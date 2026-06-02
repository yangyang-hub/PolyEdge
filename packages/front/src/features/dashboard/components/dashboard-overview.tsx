"use client";

import { startTransition, useEffect, useState } from "react";
import { TriangleAlert } from "lucide-react";

import type { getDashboardPageData } from "@/features/dashboard/loaders/dashboard-page-data";
import { useConsoleRealtime } from "@/components/shared/console-realtime-provider";
import { EmptyPanel } from "@/components/shared/empty-panel";
import { MetricCard } from "@/components/shared/metric-card";
import { MeterBar } from "@/components/shared/meter-bar";
import { PageHeader } from "@/components/shared/page-header";
import { PaginationBar } from "@/components/pagination-bar";
import { StateBanner } from "@/components/shared/state-banner";
import { StatusPill } from "@/components/shared/status-pill";
import { usePagination } from "@/hooks/use-pagination";
import { dictionary, translateEnum, formatMessage } from "@/lib/i18n/dictionaries";
import { localizeGeneratedCopy } from "@/lib/i18n/generated-copy";
import { normalizeOptionalRuntimeMode } from "@/lib/runtime-mode";
import type {
  ConsoleEventStreamPayload,
  RiskStreamPayload,
  SignalStreamPayload,
} from "@/lib/contracts/realtime";
import {
  alertSeverityTone,
  formatClock,
  formatCurrency,
  formatPercentFromRatio,
  formatSignedFixed,
  metricToneForPnl,
  signalStateTone,
  uppercaseEnum,
} from "@/lib/realtime-formatters";
import { upsertStreamedItem } from "@/lib/signal-stream-utils";

type DashboardPageData = Awaited<ReturnType<typeof getDashboardPageData>>;

function buildSignalRow(
  payload: SignalStreamPayload,
  current?: DashboardPageData["signals"][number],
  translateEnum: (value: string) => string = (value) => value.replaceAll("_", " "),
): DashboardPageData["signals"][number] {
  return {
    id: payload.signal_id,
    marketQuestion: payload.market_question ?? current?.marketQuestion ?? payload.market_id,
    side: payload.side ? uppercaseEnum(payload.side) : current?.side ?? "YES",
    edge: payload.edge ? formatSignedFixed(payload.edge) : current?.edge ?? "0.00",
    confidence: payload.confidence ? formatPercentFromRatio(payload.confidence) : current?.confidence ?? "0%",
    confidenceWidth: payload.confidence
      ? formatPercentFromRatio(payload.confidence)
      : current?.confidenceWidth ?? "0%",
    stateLabel: translateEnum(payload.lifecycle_state),
    stateTone: signalStateTone(payload.lifecycle_state),
  };
}

function buildAlertItem(
  payload: RiskStreamPayload,
  current?: DashboardPageData["alerts"][number],
): DashboardPageData["alerts"][number] | null {
  if (!payload.alert_id || !payload.severity || !payload.reason || !payload.target) {
    return current ?? null;
  }

  return {
    id: payload.alert_id,
    severity: payload.severity,
    severityTone: alertSeverityTone(payload.severity),
    createdAt: payload.created_at ? formatClock(payload.created_at) : current?.createdAt ?? "--:--:--",
    reason: payload.reason,
    target: payload.target,
  };
}

function upsertAlert(
  alerts: DashboardPageData["alerts"],
  payload: RiskStreamPayload,
): DashboardPageData["alerts"] {
  const current = alerts.find((alert) => alert.id === payload.alert_id);
  const nextAlert = buildAlertItem(payload, current);

  if (!nextAlert) {
    return alerts;
  }

  if (current) {
    return alerts.map((alert) => (alert.id === nextAlert.id ? nextAlert : alert));
  }

  return [nextAlert, ...alerts].slice(0, 3);
}

function buildEventItem(
  payload: ConsoleEventStreamPayload,
  current: DashboardPageData["events"][number] | undefined,
  fallbackSummary: string,
): DashboardPageData["events"][number] {
  return {
    id: payload.event_id,
    source: payload.source,
    confidence: formatPercentFromRatio(payload.confidence),
    summary: payload.summary ?? current?.summary ?? fallbackSummary,
  };
}

function upsertEvent(
  events: DashboardPageData["events"],
  payload: ConsoleEventStreamPayload,
  fallbackSummary: string,
): DashboardPageData["events"] {
  const current = events.find((event) => event.id === payload.event_id);
  const nextEvent = buildEventItem(payload, current, fallbackSummary);

  if (current) {
    return events.map((event) => (event.id === nextEvent.id ? nextEvent : event));
  }

  return [nextEvent, ...events].slice(0, 4);
}

function patchMetrics(
  metrics: DashboardPageData["metrics"],
  payload: RiskStreamPayload,
  labels: {
    critical: string;
  },
): DashboardPageData["metrics"] {
  return metrics.map((metric) => {
    if (metric.key === "daily_pnl" && payload.daily_pnl) {
      return {
        ...metric,
        value: formatCurrency(payload.daily_pnl),
        hint: payload.updated_at ? formatClock(payload.updated_at) : metric.hint,
        tone: metricToneForPnl(payload.daily_pnl),
      };
    }

    if (metric.key === "gross_exposure" && payload.gross_exposure) {
      return {
        ...metric,
        value: formatPercentFromRatio(payload.gross_exposure),
      };
    }

    if (metric.key === "open_alerts" && payload.open_alerts !== undefined) {
      return {
        ...metric,
        value: String(payload.open_alerts),
        hint:
          payload.critical_alerts !== undefined
            ? `${payload.critical_alerts} ${labels.critical}`
            : metric.hint,
        tone: (payload.critical_alerts ?? 0) > 0 ? ("danger" as const) : ("primary" as const),
      };
    }

    return metric;
  });
}

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
  const [liveData, setLiveData] = useState(data);
  const { signals: signalsStream, risk: riskStream, events: eventsStream } = useConsoleRealtime();
  const format = formatMessage;

  useEffect(() => {
    const streamEvent = signalsStream.lastEvent;

    if (!streamEvent) {
      return;
    }

    startTransition(() => {
      setLiveData((current) => ({
        ...current,
        signals: upsertStreamedItem(
          current.signals,
          streamEvent.data,
          (payload, currentSignal) => buildSignalRow(payload, currentSignal, translateEnum),
          streamEvent.type,
        ),
      }));
    });
  }, [translateEnum, signalsStream.lastEvent]);

  useEffect(() => {
    const streamEvent = riskStream.lastEvent;

    if (!streamEvent) {
      return;
    }

    const runtimeMode = normalizeOptionalRuntimeMode(streamEvent.data.mode);

    startTransition(() => {
      setLiveData((current) => ({
        ...current,
        modeLabel: runtimeMode ? translateEnum(runtimeMode) : current.modeLabel,
        environmentLabel: streamEvent.data.environment ?? current.environmentLabel,
        metrics: patchMetrics(current.metrics, streamEvent.data, {
          critical: dictionary.common.critical,
        }),
        alerts: upsertAlert(current.alerts, streamEvent.data).map((alert) => ({
          ...alert,
          reason: localizeGeneratedCopy(dictionary, alert.reason),
          target: localizeGeneratedCopy(dictionary, alert.target),
        })),
      }));
    });
  }, [dictionary.common.critical, translateEnum, riskStream.lastEvent]);

  useEffect(() => {
    const streamEvent = eventsStream.lastEvent;

    if (!streamEvent) {
      return;
    }

    startTransition(() => {
      setLiveData((current) => ({
        ...current,
        events: upsertEvent(current.events, streamEvent.data, dictionary.events.realtimeSummaryFallback),
      }));
    });
  }, [dictionary.events.realtimeSummaryFallback, eventsStream.lastEvent]);

  const openAlertCount = readMetricCount(liveData.metrics, "open_alerts", liveData.alerts.length);

  const signalsPagination = usePagination(liveData.signals.length, 10);
  const marketsPagination = usePagination(liveData.markets.length, 10);

  return (
    <div className="space-y-4">
      <PageHeader
        eyebrow={dictionary.dashboard.eyebrow}
        title={dictionary.dashboard.title}
        description={dictionary.dashboard.description}
        className="border-none pb-0"
        actions={
          <>
            <StatusPill tone="warning">{liveData.modeLabel}</StatusPill>
            <StatusPill tone="primary">{liveData.environmentLabel}</StatusPill>
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
        {liveData.metrics.map((metric) => (
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
            <span className="font-mono text-[10px] font-bold uppercase tracking-[0.2em] text-secondary">
              {dictionary.dashboard.liveStreaming}
            </span>
          </div>

          {liveData.signals.length > 0 ? (
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
                  {liveData.signals.slice(signalsPagination.start, signalsPagination.end).map((signal) => (
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
            <PaginationBar pagination={signalsPagination} totalItems={liveData.signals.length} className="flex items-center justify-between border-t border-border/70 px-4 pt-3 pb-3" />
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
            {liveData.alerts.length > 0 ? (
              <div className="space-y-3">
                {liveData.alerts.map((alert) => (
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
            {liveData.markets.slice(marketsPagination.start, marketsPagination.end).map((market) => (
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
          <PaginationBar pagination={marketsPagination} totalItems={liveData.markets.length} className="mt-3 flex items-center justify-between border-t border-border/70 pt-3" />
        </div>

        <div className="rounded-lg bg-card/95 p-4 ring-1 ring-white/5">
          <p className="mb-3 font-heading text-sm font-bold uppercase tracking-[0.18em] text-foreground">
            {dictionary.dashboard.latestEvents}
          </p>
          <div className="space-y-3">
            {liveData.events.map((event) => (
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
