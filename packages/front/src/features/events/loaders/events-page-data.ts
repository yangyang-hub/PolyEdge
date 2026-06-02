import { listEvidences, listEvents } from "@/lib/api/events";
import { listMarkets } from "@/lib/api/markets";
import { listSignals } from "@/lib/api/signals";
import { dictionary, translateEnum } from "@/lib/i18n/dictionaries";
import { indexMarkets, selectFirstMatchingItem } from "@/lib/loaders/console-loader-utils";
import {
  eventStatusTone,
  formatPercentFromRatio,
  formatSignedFixed,
  signalStateTone,
} from "@/lib/formatters";

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
    dictionary.routeStates.eventsDataRequired,
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
        statusLabel: translateEnum(event.status),
        statusTone: eventStatusTone(event.status),
        relevance: formatPercentFromRatio(event.relevance_score),
        confidence: formatPercentFromRatio(event.confidence),
        reasonTrace: event.reason_trace,
        relatedMarketIds: event.related_market_ids,
        evidence: selectedEvidence
          ? {
              direction: translateEnum(selectedEvidence.direction),
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
          stateLabel: translateEnum(signal.lifecycle_state),
          stateTone: signalStateTone(signal.lifecycle_state),
        })),
        isSelected: event.id === selectedEvent.id,
      };
    }),
  };
}
