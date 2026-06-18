"use client";

import { startTransition, useState } from "react";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { PageHeader } from "@/components/shared/page-header";
import { PaginationBar } from "@/components/pagination-bar";
import { StatusPill } from "@/components/shared/status-pill";
import { TruncateText } from "@/components/shared/truncate-text";
import { usePagination } from "@/hooks/use-pagination";
import { dictionary, formatMessage } from "@/lib/i18n/dictionaries";
import type { getEventsPageData } from "@/features/events/loaders/events-page-data";

type EventsPageData = Awaited<ReturnType<typeof getEventsPageData>>;

export function EventsWorkbench({ data }: { data: EventsPageData }) {
  const [selectedId, setSelectedId] = useState(data.selectedEventId);

  const selectedEvent =
    data.events.find((event) => event.id === selectedId) ??
    data.events.find((event) => event.isSelected) ??
    data.events[0];

  const pagination = usePagination(data.events.length, 15);

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow={dictionary.events.eyebrow}
        title={dictionary.events.title}
        description={dictionary.events.description}
        actions={
          <>
            <StatusPill tone="primary">{formatMessage(dictionary.events.eventCount, { count: data.events.length })}</StatusPill>
          </>
        }
      />

      <section className="grid gap-4 xl:grid-cols-[1fr_1.2fr]">
        <Card>
          <CardHeader>
            <CardTitle className="font-heading text-base">{dictionary.events.timeline}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {data.events.slice(pagination.start, pagination.end).map((event) => {
              const active = event.id === selectedEvent?.id;

              return (
                <button
                  key={event.id}
                  type="button"
                  onClick={() => {
                    startTransition(() => {
                      setSelectedId(event.id);
                    });
                  }}
                  className={
                    active
                      ? "block w-full rounded-sm border border-primary/40 bg-accent/60 p-3 text-left"
                      : "block w-full rounded-sm border border-border/70 bg-card p-3 text-left transition-colors hover:bg-accent/35"
                  }
                >
                  <div className="flex items-center justify-between gap-3">
                    <StatusPill tone="primary">{event.source}</StatusPill>
                    <StatusPill tone={event.statusTone}>{event.statusLabel}</StatusPill>
                  </div>
                  <TruncateText
                    text={event.summary}
                    lines={2}
                    className="mt-2 block max-w-full text-sm text-foreground"
                  />
                </button>
              );
            })}
            <PaginationBar pagination={pagination} totalItems={data.events.length} />
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="font-heading text-base">{dictionary.events.evidenceMapping}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-5">
            <div className="space-y-2">
              <p className="text-sm font-medium text-foreground">{selectedEvent?.summary}</p>
              <div className="flex flex-wrap gap-2">
                <StatusPill tone="success">
                  {dictionary.events.relevance} {selectedEvent?.relevance ?? dictionary.common.pending}
                </StatusPill>
                <StatusPill tone="primary">
                  {dictionary.common.confidence} {selectedEvent?.confidence ?? dictionary.common.pending}
                </StatusPill>
              </div>
            </div>

            <div className="grid gap-3 md:grid-cols-2">
              <div className="rounded-sm border border-border/70 bg-card p-3">
                <p className="font-mono text-xs uppercase tracking-[0.24em] text-muted-foreground">
                  {dictionary.events.candidateEvidence}
                </p>
                {selectedEvent?.evidence ? (
                  <div className="mt-3 space-y-2 text-sm text-foreground">
                    <p>{dictionary.events.direction}: {selectedEvent.evidence.direction}</p>
                    <p>{dictionary.events.strength}: {selectedEvent.evidence.strength}</p>
                    <p>{dictionary.events.resolutionRelevance}: {selectedEvent.evidence.resolutionRelevance}</p>
                    <p>{dictionary.events.novelty}: {selectedEvent.evidence.novelty}</p>
                    <p>{dictionary.events.sourceReliability}: {selectedEvent.evidence.sourceReliability}</p>
                  </div>
                ) : (
                  <p className="mt-3 text-sm text-muted-foreground">
                    {dictionary.events.evidencePending}
                  </p>
                )}
              </div>
              <div className="rounded-sm border border-border/70 bg-card p-3">
                <p className="font-mono text-xs uppercase tracking-[0.24em] text-muted-foreground">
                  {dictionary.events.reasonTrace}
                </p>
                <TruncateText
                  text={selectedEvent?.reasonTrace ?? dictionary.events.traceUnavailable}
                  lines={5}
                  className="mt-3 block text-sm text-muted-foreground"
                />
              </div>
            </div>

            <div className="space-y-3">
              <p className="font-mono text-xs uppercase tracking-[0.24em] text-muted-foreground">
                {dictionary.events.linkedSignals}
              </p>
              {selectedEvent && selectedEvent.linkedSignals.length > 0 ? (
                selectedEvent.linkedSignals.map((signal) => (
                  <div key={signal.id} className="flex items-center justify-between rounded-sm bg-accent/50 p-3">
                    <div>
                      <p className="text-sm font-medium text-foreground">{signal.marketQuestion}</p>
                      <p className="font-mono text-xs text-muted-foreground">{dictionary.signals.edge} {signal.edge}</p>
                    </div>
                    <StatusPill tone={signal.stateTone}>{signal.stateLabel}</StatusPill>
                  </div>
                ))
              ) : (
                <p className="text-sm text-muted-foreground">
                  {dictionary.events.noLinkedSignals}
                </p>
              )}
            </div>
          </CardContent>
        </Card>
      </section>
    </div>
  );
}
