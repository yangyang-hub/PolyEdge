import "server-only";

import { cache } from "react";

import type { ApiResponse } from "@/lib/contracts/api";
import type {
  EventDto,
  EvidenceDto,
  MarketDto,
  ProbabilityEstimateDto,
  ReplayMetricDto,
  ReplayMomentDto,
  ReplayRunDto,
  SignalDto,
  SignalTransitionDto,
} from "@/lib/contracts/dto";
import {
  eventFixtures,
  replayRunFixture,
  signalFixtures,
} from "@/lib/server/polyedge-mock-data";
import {
  buildQueryString,
  createResponse,
  fetchContract,
  getBackendMode,
} from "@/server/api/base";
import { listEvents, listEvidences } from "@/server/api/events";
import { formatInteger, formatPercentFromRatio, formatSignedFixed, humanizeSnakeCase } from "@/lib/server/console-formatters";
import { listMarkets } from "@/server/api/markets";
import { listSignals } from "@/server/api/signals";

type ReplaySnapshot = {
  replayRun: ReplayRunDto;
  relatedSignals: SignalDto[];
  relatedEvents: EventDto[];
};

function parseTimestamp(value: string | null | undefined): number {
  if (!value) {
    return 0;
  }

  const parsed = Date.parse(value);
  return Number.isNaN(parsed) ? 0 : parsed;
}

function sortByTimestampDescending<T>(items: T[], readTimestamp: (item: T) => string): T[] {
  return items.slice().sort((left, right) => parseTimestamp(readTimestamp(right)) - parseTimestamp(readTimestamp(left)));
}

function sortByTimestampAscending<T>(items: T[], readTimestamp: (item: T) => string): T[] {
  return items.slice().sort((left, right) => parseTimestamp(readTimestamp(left)) - parseTimestamp(readTimestamp(right)));
}

function selectReplaySignal(signals: SignalDto[], runId: string): SignalDto | null {
  const sortedSignals = sortByTimestampDescending(signals, (signal) => signal.updated_at);

  if (runId !== replayRunFixture.id) {
    return (
      sortedSignals.find((signal) => signal.id === runId || signal.market_id === runId || `signal_${signal.id}_replay` === runId) ??
      sortedSignals[0] ??
      null
    );
  }

  return sortedSignals[0] ?? null;
}

function selectReplayMarket(markets: MarketDto[], selectedSignal: SignalDto | null, runId: string): MarketDto | null {
  if (selectedSignal) {
    return markets.find((market) => market.id === selectedSignal.market_id) ?? null;
  }

  if (runId !== replayRunFixture.id) {
    const directMatch = markets.find((market) => market.id === runId || `market_${market.id}_replay` === runId);
    if (directMatch) {
      return directMatch;
    }
  }

  const sortedMarkets = sortByTimestampDescending(markets, (market) => market.updated_at);
  return sortedMarkets[0] ?? null;
}

function buildReplayMetrics(
  signal: SignalDto | null,
  estimate: ProbabilityEstimateDto | null,
  evidenceCount: number,
): ReplayMetricDto[] {
  const confidence = estimate?.confidence ?? signal?.confidence ?? "0";
  const edge = estimate?.edge ?? signal?.edge ?? "0";

  return [
    { title: "Confidence", value: formatPercentFromRatio(confidence) },
    { title: "Edge", value: formatSignedFixed(edge) },
    { title: "Evidence Count", value: formatInteger(String(evidenceCount)) },
  ];
}

function buildReplayTimeline(params: {
  event: EventDto | null;
  evidence: EvidenceDto | null;
  estimate: ProbabilityEstimateDto | null;
  transitions: SignalTransitionDto[];
  signal: SignalDto | null;
  marketQuestion: string;
}): ReplayMomentDto[] {
  const moments: ReplayMomentDto[] = [];

  if (params.event) {
    moments.push({
      occurred_at: params.event.created_at,
      kind: "event_ingested",
      summary: `Event ingested from ${params.event.source}: ${params.event.summary}`,
    });
  }

  if (params.evidence) {
    moments.push({
      occurred_at: params.evidence.updated_at,
      kind: "evidence_generated",
      summary: `Evidence generated with ${humanizeSnakeCase(params.evidence.direction)} direction and strength ${params.evidence.strength}.`,
    });
  }

  if (params.estimate) {
    moments.push({
      occurred_at: params.estimate.created_at,
      kind: "posterior_updated",
      summary: `Posterior moved from ${params.estimate.prior_price} to ${params.estimate.posterior_price}; fair value ${params.estimate.fair_price}, edge ${params.estimate.edge}.`,
    });
  }

  for (const transition of params.transitions) {
    moments.push({
      occurred_at: transition.created_at,
      kind: "signal_transition",
      summary: `Signal moved from ${humanizeSnakeCase(transition.from_state)} to ${humanizeSnakeCase(transition.to_state)} via ${humanizeSnakeCase(transition.trigger_type)}.`,
    });
  }

  if (moments.length === 0) {
    moments.push({
      occurred_at: params.signal?.updated_at ?? new Date().toISOString(),
      kind: "signal_transition",
      summary: params.signal
        ? `Signal ${params.signal.id} has no persisted replay timeline yet.`
        : `No replay timeline is available yet for ${params.marketQuestion}.`,
    });
  }

  return sortByTimestampAscending(moments, (moment) => moment.occurred_at);
}

function buildReplayRun(params: {
  signal: SignalDto | null;
  market: MarketDto | null;
  estimate: ProbabilityEstimateDto | null;
  transitions: SignalTransitionDto[];
  evidenceCount: number;
  timeline: ReplayMomentDto[];
}): ReplayRunDto {
  const marketId = params.signal?.market_id ?? params.market?.id ?? replayRunFixture.market_id;
  const marketQuestion = params.market?.question ?? replayRunFixture.market_question;
  const prior = params.estimate?.prior_price ?? params.signal?.market_price ?? params.market?.mid_price ?? "0.00";
  const posterior = params.estimate?.posterior_price ?? params.signal?.fair_price ?? params.market?.mid_price ?? prior;
  const marketPrice = params.estimate?.market_price ?? params.signal?.market_price ?? params.market?.mid_price ?? posterior;
  const latestTransition = params.transitions.at(-1);
  const latestTimelineMoment = params.timeline.at(-1) ?? params.timeline[0];
  const pricingGapSquared = Math.pow(Number.parseFloat(posterior) - Number.parseFloat(marketPrice), 2).toFixed(3);

  return {
    id: params.signal ? `signal_${params.signal.id}_replay` : `market_${marketId}_replay`,
    label: params.signal ? `signal_${params.signal.id}_replay` : `market_${marketId}_replay`,
    market_id: marketId,
    market_question: marketQuestion,
    prior,
    posterior,
    signal_state_from: latestTransition?.from_state ?? params.signal?.lifecycle_state ?? "new",
    signal_state_to: latestTransition?.to_state ?? params.signal?.lifecycle_state ?? "new",
    signal_hit_rate: params.estimate?.confidence ?? params.signal?.confidence ?? "0",
    brier_score: pricingGapSquared,
    net_alpha: params.estimate?.edge ?? params.signal?.edge ?? "0",
    metrics: buildReplayMetrics(params.signal, params.estimate, params.evidenceCount),
    timeline: params.timeline,
    created_at: params.timeline[0]?.occurred_at ?? latestTimelineMoment?.occurred_at ?? new Date().toISOString(),
    updated_at: latestTimelineMoment?.occurred_at ?? params.signal?.updated_at ?? new Date().toISOString(),
    version: params.signal?.version ?? Math.max(params.transitions.length, 1),
  };
}

const readDerivedLiveReplaySnapshot = cache(async (runId: string): Promise<ApiResponse<ReplaySnapshot>> => {
  const [signalsPayload, marketsPayload, eventsPayload] = await Promise.all([
    listSignals({ limit: 50 }),
    listMarkets({ limit: 50 }),
    listEvents({ limit: 50 }),
  ]);

  const selectedSignal = selectReplaySignal(signalsPayload.data, runId);
  const selectedMarket = selectReplayMarket(marketsPayload.data, selectedSignal, runId);
  const marketId = selectedSignal?.market_id ?? selectedMarket?.id ?? replayRunFixture.market_id;
  const relatedSignals = sortByTimestampDescending(
    signalsPayload.data.filter((signal) => signal.market_id === marketId),
    (signal) => signal.updated_at,
  );
  const relatedEvents = sortByTimestampDescending(
    eventsPayload.data.filter((event) => event.related_market_ids.includes(marketId)),
    (event) => event.created_at,
  );
  const selectedEvent =
    (selectedSignal && relatedEvents.find((event) => event.id === selectedSignal.event_id)) ?? relatedEvents[0] ?? null;

  const estimateQuery = selectedSignal
    ? { signal_id: selectedSignal.id, limit: 20 }
    : { market_id: marketId, event_id: selectedEvent?.id, limit: 20 };

  const [evidencesPayload, estimatesPayload, transitionsPayload] = await Promise.all([
    listEvidences({
      market_id: marketId,
      event_id: selectedEvent?.id,
      limit: 20,
    }),
    fetchContract<ApiResponse<ProbabilityEstimateDto[]>>(
      `/api/v1/pricing/estimates${buildQueryString(estimateQuery)}`,
      createResponse("probability_estimates", [] as ProbabilityEstimateDto[]),
    ),
    selectedSignal
      ? fetchContract<ApiResponse<SignalTransitionDto[]>>(
          `/api/v1/signals/${selectedSignal.id}/transitions${buildQueryString({ limit: 20 })}`,
          createResponse("signal_transitions", [] as SignalTransitionDto[]),
        )
      : Promise.resolve(createResponse("signal_transitions", [] as SignalTransitionDto[])),
  ]);

  const selectedEvidence =
    sortByTimestampDescending(evidencesPayload.data, (evidence) => evidence.updated_at)[0] ?? null;
  const selectedEstimate =
    sortByTimestampDescending(estimatesPayload.data, (estimate) => estimate.created_at)[0] ?? null;
  const orderedTransitions = sortByTimestampAscending(transitionsPayload.data, (transition) => transition.created_at);
  const timeline = buildReplayTimeline({
    event: selectedEvent,
    evidence: selectedEvidence,
    estimate: selectedEstimate,
    transitions: orderedTransitions,
    signal: selectedSignal,
    marketQuestion: selectedMarket?.question ?? replayRunFixture.market_question,
  });

  return {
    data: {
      replayRun: buildReplayRun({
        signal: selectedSignal,
        market: selectedMarket,
        estimate: selectedEstimate,
        transitions: orderedTransitions,
        evidenceCount: selectedEstimate?.evidence_count ?? evidencesPayload.data.length,
        timeline,
      }),
      relatedSignals,
      relatedEvents,
    },
    meta: signalsPayload.meta,
  };
});

export async function readReplaySnapshot(runId = replayRunFixture.id): Promise<ApiResponse<ReplaySnapshot>> {
  if (getBackendMode() === "mock") {
    const relatedSignals = signalFixtures.filter((signal) => signal.market_id === replayRunFixture.market_id);
    const relatedEvents = eventFixtures.filter((event) => event.related_market_ids.includes(replayRunFixture.market_id));

    return createResponse("replay_snapshot", {
      replayRun: replayRunFixture,
      relatedSignals,
      relatedEvents,
    });
  }

  return readDerivedLiveReplaySnapshot(runId);
}
