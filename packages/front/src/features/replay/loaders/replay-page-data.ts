import "server-only";

import { readReplaySnapshot } from "@/server/api/research";
import {
  eventStatusTone,
  formatClock,
  formatPercentFromRatio,
  formatSignedFixed,
  humanizeSnakeCase,
  signalStateTone,
  uppercaseEnum,
} from "@/lib/server/console-formatters";

export async function getReplayPageData() {
  const {
    data: { replayRun, relatedSignals, relatedEvents },
  } = await readReplaySnapshot();
  const selectedTimelineMoment = replayRun.timeline.at(-1) ?? replayRun.timeline[0];
  const posteriorDelta = (
    Number.parseFloat(replayRun.posterior) - Number.parseFloat(replayRun.prior)
  ).toFixed(2);

  return {
    runLabel: replayRun.label,
    selectedMomentId: selectedTimelineMoment
      ? `${selectedTimelineMoment.occurred_at}_${selectedTimelineMoment.kind}`
      : "",
    marketQuestion: replayRun.market_question,
    timeline: replayRun.timeline.map((moment) => ({
      id: `${moment.occurred_at}_${moment.kind}`,
      occurredAt: formatClock(moment.occurred_at),
      kind: moment.kind,
      kindLabel: humanizeSnakeCase(moment.kind),
      summary: moment.summary,
    })),
    snapshot: {
      marketQuestion: replayRun.market_question,
      prior: replayRun.prior,
      posterior: replayRun.posterior,
      posteriorDelta: formatSignedFixed(posteriorDelta),
      stateTransition: `${humanizeSnakeCase(replayRun.signal_state_from)} -> ${humanizeSnakeCase(replayRun.signal_state_to)}`,
      createdAt: formatClock(replayRun.created_at),
      updatedAt: formatClock(replayRun.updated_at),
    },
    metrics:
      replayRun.metrics ??
      [
        { title: "Signal Hit Rate", value: formatPercentFromRatio(replayRun.signal_hit_rate) },
        { title: "Brier Score", value: replayRun.brier_score },
        { title: "Net Alpha", value: formatPercentFromRatio(replayRun.net_alpha, 1) },
      ],
    relatedSignals: relatedSignals.map((signal) => ({
      id: signal.id,
      side: uppercaseEnum(signal.side),
      confidence: formatPercentFromRatio(signal.confidence),
      edge: formatSignedFixed(signal.edge),
      stateLabel: humanizeSnakeCase(signal.lifecycle_state),
      stateTone: signalStateTone(signal.lifecycle_state),
      reason: signal.reason,
    })),
    relatedEvents: relatedEvents.map((event) => ({
      id: event.id,
      source: event.source,
      createdAt: formatClock(event.created_at),
      summary: event.summary,
      confidence: formatPercentFromRatio(event.confidence),
      statusLabel: humanizeSnakeCase(event.status),
      statusTone: eventStatusTone(event.status),
    })),
  };
}
