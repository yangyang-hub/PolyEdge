"use client";

import { useEffect, useEffectEvent, useMemo, useRef, useState } from "react";

import { getApiBaseUrl } from "@/lib/api/base";
import {
  CHANNEL_EVENT_TYPES,
  type RealtimeChannel,
  type RealtimeMessage,
  type RealtimePayloadByChannel,
} from "@/lib/contracts/realtime";

const MAX_SEEN_EVENT_IDS = 500;

function evictOldestIds(set: Set<string>, max: number): void {
  if (set.size <= max) {
    return;
  }

  let toRemove = set.size - max;
  for (const id of set) {
    if (toRemove <= 0) {
      break;
    }
    set.delete(id);
    toRemove -= 1;
  }
}

export type StreamConnectionState = "connecting" | "open" | "error" | "closed";

export type StreamEvent<T> = RealtimeMessage<T> & {
  receivedAt: number;
};

export function useSseStream<TChannel extends RealtimeChannel>({
  channel,
  enabled = true,
  events,
}: {
  channel: TChannel;
  enabled?: boolean;
  events?: readonly string[];
}) {
  const [connectionState, setConnectionState] = useState<StreamConnectionState>(
    enabled ? "connecting" : "closed",
  );
  const [lastEvent, setLastEvent] = useState<StreamEvent<RealtimePayloadByChannel[TChannel]> | null>(null);
  const [lastErrorAt, setLastErrorAt] = useState<number | null>(null);
  const seenEventIdsRef = useRef<Set<string>>(new Set());

  const eventTypes = useMemo(() => events ?? CHANNEL_EVENT_TYPES[channel], [channel, events]);

  const handleMessage = useEffectEvent((event: MessageEvent<string>) => {
    try {
      if (event.lastEventId && seenEventIdsRef.current.has(event.lastEventId)) {
        return;
      }

      const payload = JSON.parse(event.data) as RealtimePayloadByChannel[TChannel];

      if (event.lastEventId) {
        seenEventIdsRef.current.add(event.lastEventId);
        evictOldestIds(seenEventIdsRef.current, MAX_SEEN_EVENT_IDS);
      }

      setLastEvent({
        id: event.lastEventId,
        type: event.type,
        data: payload,
        receivedAt: Date.now(),
      });
      // Recover from a previous data-level error — the connection is still open.
      setConnectionState("open");
    } catch {
      setLastErrorAt(Date.now());
      // Do NOT set connectionState to "error" here: the EventSource is still open.
      // A JSON parse failure is a data-level issue, not a connection-level one.
    }
  });

  useEffect(() => {
    if (!enabled) {
      return;
    }

    seenEventIdsRef.current.clear();
    const stream = new EventSource(`${getApiBaseUrl()}/api/v1/stream/${channel}`);

    stream.onopen = () => {
      setConnectionState("open");
    };

    stream.onerror = () => {
      setLastErrorAt(Date.now());
      setConnectionState("error");
    };

    for (const eventType of eventTypes) {
      stream.addEventListener(eventType, handleMessage as EventListener);
    }

    return () => {
      stream.close();
    };
  }, [channel, enabled, eventTypes]);

  return {
    connection: enabled ? connectionState : "closed",
    lastEvent,
    lastErrorAt,
  };
}
