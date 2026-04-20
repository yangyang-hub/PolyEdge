"use client";

import { useConsoleRealtime } from "@/components/shared/console-realtime-provider";
import type { RealtimeTone } from "@/lib/realtime-formatters";
import { formatClock, humanizeSnakeCase } from "@/lib/realtime-formatters";

type StatusBadge = {
  tone: RealtimeTone;
  label: string;
};

function describeStream(
  label: string,
  connection: "connecting" | "open" | "error" | "closed",
  lastEventAt: number | null,
): StatusBadge {
  if (connection === "open") {
    return {
      tone: "success",
      label: lastEventAt ? `${label} ${formatClock(new Date(lastEventAt).toISOString())}` : `${label} live`,
    };
  }

  if (connection === "error") {
    return {
      tone: "warning",
      label: `${label} reconnecting`,
    };
  }

  if (connection === "connecting") {
    return {
      tone: "neutral",
      label: `${label} connecting`,
    };
  }

  return {
    tone: "neutral",
    label: `${label} idle`,
  };
}

export function useLiveStatus() {
  const { signals: signalsStream, risk: riskStream, events: eventsStream } = useConsoleRealtime();

  const connections = [signalsStream.connection, riskStream.connection, eventsStream.connection];
  const allOpen = connections.every((connection) => connection === "open");
  const anyError = connections.some((connection) => connection === "error");
  const anyConnecting = connections.some((connection) => connection === "connecting");

  const apiBadge: StatusBadge = anyError
    ? { tone: "warning", label: "api stream degraded" }
    : anyConnecting && !allOpen
      ? { tone: "neutral", label: "api stream syncing" }
      : { tone: "success", label: "api stream healthy" };

  const riskBadge: StatusBadge =
    riskStream.connection === "open" && riskStream.lastEvent?.data.open_alerts !== undefined
      ? {
          tone:
            riskStream.lastEvent.data.open_alerts > 0
              ? ("warning" as const)
              : ("success" as const),
          label: `risk ${riskStream.lastEvent.data.open_alerts} alerts`,
        }
      : describeStream("risk stream", riskStream.connection, riskStream.lastEvent?.receivedAt ?? null);

  const signalsBadge: StatusBadge =
    signalsStream.connection === "open" && signalsStream.lastEvent?.data.lifecycle_state
      ? {
          tone: "primary",
          label: `signal ${humanizeSnakeCase(signalsStream.lastEvent.data.lifecycle_state)}`,
        }
      : describeStream("market stream", signalsStream.connection, signalsStream.lastEvent?.receivedAt ?? null);

  return {
    badges: [
      apiBadge,
      signalsBadge,
      describeStream("event stream", eventsStream.connection, eventsStream.lastEvent?.receivedAt ?? null),
      riskBadge,
    ] as StatusBadge[],
  };
}
