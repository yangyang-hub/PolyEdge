"use client";

import { useEffect, useEffectEvent, useMemo, useState } from "react";

import {
  CHANNEL_EVENT_TYPES,
  type RealtimeChannel,
  type RealtimeMessage,
  type RealtimePayloadByChannel,
} from "@/lib/contracts/realtime";

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

  const eventTypes = useMemo(() => events ?? CHANNEL_EVENT_TYPES[channel], [channel, events]);

  const handleMessage = useEffectEvent((event: MessageEvent<string>) => {
    try {
      const payload = JSON.parse(event.data) as RealtimePayloadByChannel[TChannel];

      setLastEvent({
        id: event.lastEventId,
        type: event.type,
        data: payload,
        receivedAt: Date.now(),
      });
    } catch {
      setLastErrorAt(Date.now());
      setConnectionState("error");
    }
  });

  useEffect(() => {
    if (!enabled) {
      return;
    }

    const stream = new EventSource(`/api/stream/${channel}`);

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
