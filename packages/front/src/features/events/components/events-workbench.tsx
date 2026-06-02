"use client";

import { startTransition, useEffect, useState } from "react";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { PageHeader } from "@/components/shared/page-header";
import { PaginationBar } from "@/components/pagination-bar";
import { StatusPill } from "@/components/shared/status-pill";
import { useConsoleRealtimeChannel } from "@/components/shared/console-realtime-provider";
import { usePagination } from "@/hooks/use-pagination";
import { dictionary, translateEnum, formatMessage, type Dictionary } from "@/lib/i18n/dictionaries";
import type { ConsoleEventStreamPayload, SignalStreamPayload } from "@/lib/contracts/realtime";
import {
  formatPercentFromRatio,
  formatSignedFixed,
  signalStateTone,
} from "@/lib/realtime-formatters";
import type { getEventsPageData } from "@/features/events/loaders/events-page-data";

type EventsPageData = Awaited<ReturnType<typeof getEventsPageData>>;
type EventItem = EventsPageData["events"][number];

function buildEventItem(
  payload: ConsoleEventStreamPayload,
  current: EventItem | undefined,
  dictionary: Dictionary,
): EventItem {
  return {
    id: payload.event_id,
    source: payload.source,
    summary: payload.summary,
    statusLabel: current?.statusLabel ?? dictionary.common.active,
    statusTone: current?.statusTone ?? "success",
    relevance: current?.relevance ?? formatPercentFromRatio(payload.confidence),
    confidence: formatPercentFromRatio(payload.confidence),
    reasonTrace:
      current?.reasonTrace ?? dictionary.events.realtimeReasonTraceFallback,
    relatedMarketIds: current?.relatedMarketIds ?? [],
    evidence: current?.evidence ?? null,
    linkedSignals: current?.linkedSignals ?? [],
    isSelected: current?.isSelected ?? false,
  };
}

function upsertEvent(
  events: EventItem[],
  payload: ConsoleEventStreamPayload,
  dictionary: Dictionary,
): EventItem[] {
  const current = events.find((event) => event.id === payload.event_id);
  const nextEvent = buildEventItem(payload, current, dictionary);

  if (current) {
    return events.map((event) => (event.id === nextEvent.id ? nextEvent : event));
  }

  return [nextEvent, ...events];
}

function patchLinkedSignals(
  events: EventItem[],
  payload: SignalStreamPayload,
  translateEnum: (value: string) => string,
): EventItem[] {
  const nextSignal = {
    id: payload.signal_id,
    marketId: payload.market_id,
    marketQuestion: payload.market_question ?? payload.market_id,
    edge: payload.edge ? formatSignedFixed(payload.edge) : "0.00",
    stateLabel: translateEnum(payload.lifecycle_state),
    stateTone: signalStateTone(payload.lifecycle_state),
  };

  return events.map((event) => {
    if (!event.relatedMarketIds.includes(payload.market_id)) {
      return event;
    }

    const currentLinkedSignal = event.linkedSignals.find((signal) => signal.id === payload.signal_id);

    if (currentLinkedSignal) {
      return {
        ...event,
        linkedSignals: event.linkedSignals.map((signal) =>
          signal.id === nextSignal.id ? nextSignal : signal,
        ),
      };
    }

    return {
      ...event,
      linkedSignals: [nextSignal, ...event.linkedSignals],
    };
  });
}

export function EventsWorkbench({ data }: { data: EventsPageData }) {
  const [eventItems, setEventItems] = useState(data.events);
  const [selectedId, setSelectedId] = useState(data.selectedEventId);
  const { lastEvent: lastConsoleEvent } = useConsoleRealtimeChannel("events");
  const { lastEvent: lastSignalEvent } = useConsoleRealtimeChannel("signals");

  useEffect(() => {
    const streamEvent = lastConsoleEvent;

    if (!streamEvent) {
      return;
    }

    startTransition(() => {
      setEventItems((currentItems) => upsertEvent(currentItems, streamEvent.data, dictionary));
    });
  }, [dictionary, lastConsoleEvent]);

  useEffect(() => {
    const streamEvent = lastSignalEvent;

    if (!streamEvent) {
      return;
    }

    startTransition(() => {
      setEventItems((currentItems) => patchLinkedSignals(currentItems, streamEvent.data, translateEnum));
    });
  }, [translateEnum, lastSignalEvent]);

  const selectedEvent =
    eventItems.find((event) => event.id === selectedId) ??
    eventItems.find((event) => event.isSelected) ??
    eventItems[0];

  const pagination = usePagination(eventItems.length, 15);

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow={dictionary.events.eyebrow}
        title={dictionary.events.title}
        description={dictionary.events.description}
        actions={
          <>
            <StatusPill tone="primary">{formatMessage(dictionary.events.eventCount, { count: eventItems.length })}</StatusPill>
            <StatusPill tone="success">{dictionary.common.streamSynced}</StatusPill>
          </>
        }
      />

      <section className="grid gap-4 xl:grid-cols-[1fr_1.2fr]">
        <Card>
          <CardHeader>
            <CardTitle className="font-heading text-base">{dictionary.events.timeline}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {eventItems.slice(pagination.start, pagination.end).map((event) => {
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
                  <p className="mt-2 text-sm text-foreground">{event.summary}</p>
                </button>
              );
            })}
            <PaginationBar pagination={pagination} totalItems={eventItems.length} />
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
                <p className="mt-3 text-sm text-muted-foreground">
                  {selectedEvent?.reasonTrace ?? dictionary.events.traceUnavailable}
                </p>
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
