"use client";

import { createContext, useContext } from "react";

import { useSseStream, type StreamConnectionState, type StreamEvent } from "@/hooks/use-sse-stream";
import type { RealtimeChannel, RealtimePayloadByChannel } from "@/lib/contracts/realtime";

type ChannelState<TChannel extends RealtimeChannel> = {
  connection: StreamConnectionState;
  lastEvent: StreamEvent<RealtimePayloadByChannel[TChannel]> | null;
  lastErrorAt: number | null;
};

type ConsoleRealtimeContextValue = {
  signals: ChannelState<"signals">;
  risk: ChannelState<"risk">;
  events: ChannelState<"events">;
  arbitrage: ChannelState<"arbitrage">;
};

const ConsoleRealtimeContext = createContext<ConsoleRealtimeContextValue | null>(null);

export function ConsoleRealtimeProvider({ children }: { children: React.ReactNode }) {
  const signals = useSseStream({ channel: "signals" });
  const risk = useSseStream({ channel: "risk" });
  const events = useSseStream({ channel: "events" });
  const arbitrage = useSseStream({ channel: "arbitrage" });

  return (
    <ConsoleRealtimeContext.Provider value={{ signals, risk, events, arbitrage }}>
      {children}
    </ConsoleRealtimeContext.Provider>
  );
}

export function useConsoleRealtime() {
  const context = useContext(ConsoleRealtimeContext);

  if (!context) {
    throw new Error("useConsoleRealtime must be used within ConsoleRealtimeProvider.");
  }

  return context;
}

export function useConsoleRealtimeChannel<TChannel extends RealtimeChannel>(
  channel: TChannel,
): ChannelState<TChannel> {
  const context = useConsoleRealtime();

  return context[channel] as ChannelState<TChannel>;
}
