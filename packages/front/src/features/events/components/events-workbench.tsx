"use client";

import { startTransition, useEffect, useState } from "react";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { PageHeader } from "@/components/shared/page-header";
import { StatusPill } from "@/components/shared/status-pill";
import { useConsoleRealtimeChannel } from "@/components/shared/console-realtime-provider";
import type { ConsoleEventStreamPayload, SignalStreamPayload } from "@/lib/contracts/realtime";
import {
  formatPercentFromRatio,
  formatSignedFixed,
  humanizeSnakeCase,
  signalStateTone,
} from "@/lib/realtime-formatters";
import type { getEventsPageData } from "@/features/events/loaders/events-page-data";

type EventsPageData = Awaited<ReturnType<typeof getEventsPageData>>;
type EventItem = EventsPageData["events"][number];

function buildEventItem(payload: ConsoleEventStreamPayload, current?: EventItem): EventItem {
  return {
    id: payload.event_id,
    source: payload.source,
    summary: payload.summary,
    statusLabel: current?.statusLabel ?? "active",
    statusTone: current?.statusTone ?? "success",
    relevance: current?.relevance ?? formatPercentFromRatio(payload.confidence),
    confidence: formatPercentFromRatio(payload.confidence),
    reasonTrace:
      current?.reasonTrace ?? "Realtime event ingested. Evidence mapping and causal trace are still catching up.",
    relatedMarketIds: current?.relatedMarketIds ?? [],
    evidence: current?.evidence ?? null,
    linkedSignals: current?.linkedSignals ?? [],
    isSelected: current?.isSelected ?? false,
  };
}

function upsertEvent(events: EventItem[], payload: ConsoleEventStreamPayload): EventItem[] {
  const current = events.find((event) => event.id === payload.event_id);
  const nextEvent = buildEventItem(payload, current);

  if (current) {
    return events.map((event) => (event.id === nextEvent.id ? nextEvent : event));
  }

  return [nextEvent, ...events];
}

function patchLinkedSignals(events: EventItem[], payload: SignalStreamPayload): EventItem[] {
  const nextSignal = {
    id: payload.signal_id,
    marketId: payload.market_id,
    marketQuestion: payload.market_question ?? payload.market_id,
    edge: payload.edge ? formatSignedFixed(payload.edge) : "0.00",
    stateLabel: humanizeSnakeCase(payload.lifecycle_state),
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
      setEventItems((currentItems) => upsertEvent(currentItems, streamEvent.data));
    });
  }, [lastConsoleEvent]);

  useEffect(() => {
    const streamEvent = lastSignalEvent;

    if (!streamEvent) {
      return;
    }

    startTransition(() => {
      setEventItems((currentItems) => patchLinkedSignals(currentItems, streamEvent.data));
    });
  }, [lastSignalEvent]);

  const selectedEvent =
    eventItems.find((event) => event.id === selectedId) ??
    eventItems.find((event) => event.isSelected) ??
    eventItems[0];

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Cognition"
        title="Events"
        description="Track how raw information becomes evidence, then turns into posterior updates and signal candidates."
        actions={
          <>
            <StatusPill tone="primary">{eventItems.length} events</StatusPill>
            <StatusPill tone="success">stream synced</StatusPill>
          </>
        }
      />

      <section className="grid gap-4 xl:grid-cols-[1fr_1.2fr]">
        <Card>
          <CardHeader>
            <CardTitle className="font-heading text-base">Event Timeline</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {eventItems.map((event) => {
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
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="font-heading text-base">Evidence Mapping</CardTitle>
          </CardHeader>
          <CardContent className="space-y-5">
            <div className="space-y-2">
              <p className="text-sm font-medium text-foreground">{selectedEvent?.summary}</p>
              <div className="flex flex-wrap gap-2">
                <StatusPill tone="success">relevance {selectedEvent?.relevance ?? "pending"}</StatusPill>
                <StatusPill tone="primary">confidence {selectedEvent?.confidence ?? "pending"}</StatusPill>
              </div>
            </div>

            <div className="grid gap-3 md:grid-cols-2">
              <div className="rounded-sm border border-border/70 bg-card p-3">
                <p className="font-mono text-xs uppercase tracking-[0.24em] text-muted-foreground">
                  Candidate evidence
                </p>
                {selectedEvent?.evidence ? (
                  <div className="mt-3 space-y-2 text-sm text-foreground">
                    <p>Direction: {selectedEvent.evidence.direction}</p>
                    <p>Strength: {selectedEvent.evidence.strength}</p>
                    <p>Resolution relevance: {selectedEvent.evidence.resolutionRelevance}</p>
                    <p>Novelty: {selectedEvent.evidence.novelty}</p>
                    <p>Source reliability: {selectedEvent.evidence.sourceReliability}</p>
                  </div>
                ) : (
                  <p className="mt-3 text-sm text-muted-foreground">
                    Evidence generation is still pending for this event snapshot.
                  </p>
                )}
              </div>
              <div className="rounded-sm border border-border/70 bg-card p-3">
                <p className="font-mono text-xs uppercase tracking-[0.24em] text-muted-foreground">
                  Reason trace
                </p>
                <p className="mt-3 text-sm text-muted-foreground">
                  {selectedEvent?.reasonTrace ?? "Trace is not available yet."}
                </p>
              </div>
            </div>

            <div className="space-y-3">
              <p className="font-mono text-xs uppercase tracking-[0.24em] text-muted-foreground">
                Linked signals
              </p>
              {selectedEvent && selectedEvent.linkedSignals.length > 0 ? (
                selectedEvent.linkedSignals.map((signal) => (
                  <div key={signal.id} className="flex items-center justify-between rounded-sm bg-accent/50 p-3">
                    <div>
                      <p className="text-sm font-medium text-foreground">{signal.marketQuestion}</p>
                      <p className="font-mono text-xs text-muted-foreground">edge {signal.edge}</p>
                    </div>
                    <StatusPill tone={signal.stateTone}>{signal.stateLabel}</StatusPill>
                  </div>
                ))
              ) : (
                <p className="text-sm text-muted-foreground">
                  No linked signals are associated with this event yet.
                </p>
              )}
            </div>
          </CardContent>
        </Card>
      </section>
    </div>
  );
}
