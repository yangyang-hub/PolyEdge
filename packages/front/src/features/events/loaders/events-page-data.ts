import "server-only";

import { listEvidences, listEvents } from "@/server/api/events";
import { listMarkets } from "@/server/api/markets";
import { listSignals } from "@/server/api/signals";
import { indexMarkets, selectFirstMatchingItem } from "@/server/loaders/console-loader-utils";
import {
  eventStatusTone,
  formatPercentFromRatio,
  formatSignedFixed,
  humanizeSnakeCase,
  signalStateTone,
} from "@/lib/server/console-formatters";

export async function getEventsPageData() {
  const [{ data: events }, { data: evidences }, { data: signals }, { data: markets }] = await Promise.all([
    listEvents(),
    listEvidences(),
    listSignals(),
    listMarkets(),
  ]);
  const marketIndex = indexMarkets(markets);
  const selectedEvent = selectFirstMatchingItem(
    events,
    [(event) => event.status === "active"],
    "Events page requires at least one event fixture or API result.",
  );
  return {
    selectedEventId: selectedEvent.id,
    events: events.map((event) => {
      const selectedEvidence = evidences.find((evidence) => evidence.event_id === event.id) ?? null;
      const linkedSignals = signals.filter(
        (signal) =>
          signal.event_id === event.id || event.related_market_ids.includes(signal.market_id),
      );

      return {
        id: event.id,
        source: event.source,
        summary: event.summary,
        statusLabel: humanizeSnakeCase(event.status),
        statusTone: eventStatusTone(event.status),
        relevance: formatPercentFromRatio(event.relevance_score),
        confidence: formatPercentFromRatio(event.confidence),
        reasonTrace: event.reason_trace,
        relatedMarketIds: event.related_market_ids,
        evidence: selectedEvidence
          ? {
              direction: humanizeSnakeCase(selectedEvidence.direction),
              strength: selectedEvidence.strength,
              resolutionRelevance: selectedEvidence.resolution_relevance,
              novelty: selectedEvidence.novelty,
              sourceReliability: selectedEvidence.source_reliability,
            }
          : null,
        linkedSignals: linkedSignals.map((signal) => ({
          id: signal.id,
          marketId: signal.market_id,
          marketQuestion: marketIndex.get(signal.market_id)?.question ?? signal.market_id,
          edge: formatSignedFixed(signal.edge),
          stateLabel: humanizeSnakeCase(signal.lifecycle_state),
          stateTone: signalStateTone(signal.lifecycle_state),
        })),
        isSelected: event.id === selectedEvent.id,
      };
    }),
  };
}
