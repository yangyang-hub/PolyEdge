"use client";

import { useConsoleRealtime } from "@/components/shared/console-realtime-provider";
import { useI18n } from "@/lib/i18n/client";
import type { RealtimeTone } from "@/lib/realtime-formatters";
import { formatClock } from "@/lib/realtime-formatters";
import { useMemo } from "react";

type StatusBadge = {
  tone: RealtimeTone;
  label: string;
};

function describeStream(
  label: string,
  connection: "connecting" | "open" | "error" | "closed",
  lastEventAt: number | null,
  dictionary: ReturnType<typeof useI18n>["dictionary"],
): StatusBadge {
  if (connection === "open") {
    return {
      tone: "success",
      label: lastEventAt
        ? `${label} ${formatClock(new Date(lastEventAt).toISOString())}`
        : `${label} ${dictionary.common.live}`,
    };
  }

  if (connection === "error") {
    return {
      tone: "warning",
      label: `${label} ${dictionary.common.reconnecting}`,
    };
  }

  if (connection === "connecting") {
    return {
      tone: "neutral",
      label: `${label} ${dictionary.common.connecting}`,
    };
  }

  return {
    tone: "neutral",
    label: `${label} ${dictionary.common.idle}`,
  };
}

export function useLiveStatus() {
  const { signals: signalsStream, risk: riskStream, events: eventsStream } = useConsoleRealtime();
  const { dictionary, enumLabel } = useI18n();

  return useMemo(() => {
    const connections = [signalsStream.connection, riskStream.connection, eventsStream.connection];
    const allOpen = connections.every((connection) => connection === "open");
    const anyError = connections.some((connection) => connection === "error");
    const anyConnecting = connections.some((connection) => connection === "connecting");

    const apiBadge: StatusBadge = anyError
      ? { tone: "warning", label: dictionary.statusRail.apiStreamDegraded }
      : anyConnecting && !allOpen
        ? { tone: "neutral", label: dictionary.statusRail.apiStreamSyncing }
        : { tone: "success", label: dictionary.statusRail.apiStreamHealthy };

    const riskBadge: StatusBadge =
      riskStream.connection === "open" && riskStream.lastEvent?.data.open_alerts !== undefined
        ? {
            tone:
              riskStream.lastEvent.data.open_alerts > 0
                ? ("warning" as const)
                : ("success" as const),
            label: `${dictionary.nav.risk} ${riskStream.lastEvent.data.open_alerts} ${dictionary.common.alerts}`,
          }
        : describeStream(dictionary.statusRail.riskStream, riskStream.connection, riskStream.lastEvent?.receivedAt ?? null, dictionary);

    const signalsBadge: StatusBadge =
      signalsStream.connection === "open" && signalsStream.lastEvent?.data.lifecycle_state
        ? {
            tone: "primary",
            label: `${dictionary.statusRail.signal} ${enumLabel(signalsStream.lastEvent.data.lifecycle_state)}`,
          }
        : describeStream(dictionary.statusRail.marketStream, signalsStream.connection, signalsStream.lastEvent?.receivedAt ?? null, dictionary);

    return {
      badges: [
        apiBadge,
        signalsBadge,
        describeStream(dictionary.statusRail.eventStream, eventsStream.connection, eventsStream.lastEvent?.receivedAt ?? null, dictionary),
        riskBadge,
      ] as StatusBadge[],
    };
  }, [
    signalsStream.connection,
    signalsStream.lastEvent,
    riskStream.connection,
    riskStream.lastEvent,
    eventsStream.connection,
    eventsStream.lastEvent,
    dictionary,
    enumLabel,
  ]);
}
