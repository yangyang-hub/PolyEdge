import { listEvidences, listEvents } from "@/lib/api/events";
import { listMarkets } from "@/lib/api/markets";
import { dictionary, translateEnum } from "@/lib/i18n/dictionaries";
import { indexMarkets, selectFirstMatchingItem } from "@/lib/loaders/console-loader-utils";
import {
  eventStatusTone,
  formatPercentFromRatio,
} from "@/lib/formatters";

export async function getEventsPageData() {
  const [{ data: events }, { data: evidences }, { data: markets }] = await Promise.all([
    listEvents(),
    listEvidences(),
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
        relatedMarkets: event.related_market_ids.slice(0, 6).map((marketId) => ({
          id: marketId,
          question: marketIndex.get(marketId)?.question ?? marketId,
        })),
        isSelected: event.id === selectedEvent.id,
      };
    }),
  };
}
