"use client";

import { startTransition, useState } from "react";

import type { getReplayPageData } from "@/features/replay/loaders/replay-page-data";
import { MetricCard } from "@/components/shared/metric-card";
import { PageHeader } from "@/components/shared/page-header";
import { StatusPill } from "@/components/shared/status-pill";
import { WorkbenchLayout, WorkbenchDetailPane } from "@/components/shared/workbench-layout";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { useI18n } from "@/lib/i18n/client";
import { isKeyboardSelect } from "@/lib/keyboard";

type ReplayPageData = Awaited<ReturnType<typeof getReplayPageData>>;

function replayMomentTone(
  kind: ReplayPageData["timeline"][number]["kind"],
): "primary" | "success" | "warning" | "violet" {
  if (kind === "event_ingested") {
    return "primary";
  }

  if (kind === "evidence_generated") {
    return "success";
  }

  if (kind === "posterior_updated") {
    return "warning";
  }

  return "violet";
}

export function ReplayWorkbench({ data }: { data: ReplayPageData }) {
  const [selectedId, setSelectedId] = useState(data.selectedMomentId);
  const { dictionary, format } = useI18n();
  const selectedMoment =
    data.timeline.find((moment) => moment.id === selectedId) ??
    data.timeline.find((moment) => moment.id === data.selectedMomentId) ??
    data.timeline[0];

  if (!selectedMoment) {
    return null;
  }

  function selectMoment(momentId: string) {
    startTransition(() => {
      setSelectedId(momentId);
    });
  }

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow={dictionary.replay.eyebrow}
        title={dictionary.replay.title}
        description={dictionary.replay.description}
        actions={
          <>
            <StatusPill tone="primary">{data.runLabel}</StatusPill>
            <StatusPill tone="neutral">{selectedMoment.occurredAt}</StatusPill>
          </>
        }
      />

      <section className="grid gap-4 md:grid-cols-3">
        {data.metrics.map((metric) => (
          <MetricCard
            key={metric.title}
            title={metric.title}
            value={metric.value}
            hint={data.marketQuestion}
            accent={metric.title === dictionary.metrics.brierScore ? "primary" : "success"}
          />
        ))}
      </section>

      <WorkbenchLayout columnsClassName="xl:grid-cols-[1.15fr_1fr]">
        <Card>
          <CardHeader>
            <CardTitle className="font-heading text-base">{dictionary.replay.replayTimeline}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {data.timeline.map((moment) => {
              const active = moment.id === selectedMoment.id;

              return (
                <button
                  key={moment.id}
                  type="button"
                  onClick={() => selectMoment(moment.id)}
                  onKeyDown={(event) => {
                    if (isKeyboardSelect(event)) {
                      event.preventDefault();
                      selectMoment(moment.id);
                    }
                  }}
                  className={
                    active
                      ? "block w-full rounded-sm border border-primary/40 bg-accent/60 p-3 text-left"
                      : "block w-full rounded-sm border border-border/70 bg-card p-3 text-left transition-colors hover:bg-accent/35"
                  }
                >
                  <div className="flex items-center justify-between gap-3">
                    <StatusPill tone={replayMomentTone(moment.kind)}>{moment.kindLabel}</StatusPill>
                    <span className="font-mono text-xs text-muted-foreground">{moment.occurredAt}</span>
                  </div>
                  <p className="mt-2 text-sm text-foreground">{moment.summary}</p>
                </button>
              );
            })}
          </CardContent>
        </Card>

        <div className="space-y-4">
          <WorkbenchDetailPane className="space-y-4">
            <div className="space-y-2">
              <p className="font-heading text-lg font-bold tracking-tight text-foreground">
                {data.snapshot.marketQuestion}
              </p>
              <div className="flex flex-wrap gap-2">
                <StatusPill tone={replayMomentTone(selectedMoment.kind)}>{selectedMoment.kindLabel}</StatusPill>
                <StatusPill tone="warning">{data.snapshot.stateTransition}</StatusPill>
              </div>
            </div>

            <div className="grid grid-cols-3 gap-3">
              <div className="rounded-md bg-accent/45 p-3">
                <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">{dictionary.replay.prior}</p>
                <p className="mt-2 font-mono text-lg text-foreground">{data.snapshot.prior}</p>
              </div>
              <div className="rounded-md bg-accent/45 p-3">
                <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
                  {dictionary.replay.posterior}
                </p>
                <p className="mt-2 font-mono text-lg text-primary">{data.snapshot.posterior}</p>
              </div>
              <div className="rounded-md bg-accent/45 p-3">
                <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">{dictionary.replay.delta}</p>
                <p className="mt-2 font-mono text-lg text-foreground">{data.snapshot.posteriorDelta}</p>
              </div>
            </div>

            <div className="rounded-md bg-popover/70 p-4">
              <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
                {dictionary.replay.selectedMoment}
              </p>
              <p className="mt-3 text-sm text-foreground">{selectedMoment.summary}</p>
              <p className="mt-2 text-xs text-muted-foreground">
                {format(dictionary.replay.runUpdated, {
                  createdAt: data.snapshot.createdAt,
                  updatedAt: data.snapshot.updatedAt,
                })}
              </p>
            </div>
          </WorkbenchDetailPane>

          <Card>
            <CardHeader>
              <CardTitle className="font-heading text-base">{dictionary.replay.relatedSignals}</CardTitle>
            </CardHeader>
            <CardContent className="space-y-3">
              {data.relatedSignals.map((signal) => (
                <div key={signal.id} className="rounded-sm border border-border/70 bg-card p-3">
                  <div className="flex items-center justify-between gap-3">
                    <StatusPill tone={signal.stateTone}>{signal.stateLabel}</StatusPill>
                    <span className="font-mono text-xs text-muted-foreground">
                      {signal.side} / {signal.confidence}
                    </span>
                  </div>
                  <p className="mt-2 text-sm text-foreground">{signal.reason}</p>
                  <p className="mt-2 font-mono text-xs text-muted-foreground">{dictionary.signals.edge} {signal.edge}</p>
                </div>
              ))}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="font-heading text-base">{dictionary.replay.relatedEvents}</CardTitle>
            </CardHeader>
            <CardContent className="space-y-3">
              {data.relatedEvents.map((event) => (
                <div key={event.id} className="rounded-sm border border-border/70 bg-card p-3">
                  <div className="flex items-center justify-between gap-3">
                    <StatusPill tone="primary">{event.source}</StatusPill>
                    <StatusPill tone={event.statusTone}>{event.statusLabel}</StatusPill>
                  </div>
                  <p className="mt-2 text-sm text-foreground">{event.summary}</p>
                  <p className="mt-2 font-mono text-xs text-muted-foreground">
                    {event.createdAt} / {dictionary.common.confidence} {event.confidence}
                  </p>
                </div>
              ))}
            </CardContent>
          </Card>
        </div>
      </WorkbenchLayout>
    </div>
  );
}
