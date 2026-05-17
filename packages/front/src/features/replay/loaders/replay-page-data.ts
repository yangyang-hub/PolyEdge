import "server-only";

import { readReplaySnapshot } from "@/server/api/research";
import { getServerI18n } from "@/lib/i18n/server";
import {
  eventStatusTone,
  formatClock,
  formatPercentFromRatio,
  formatSignedFixed,
  signalStateTone,
  uppercaseEnum,
} from "@/lib/server/console-formatters";

export async function getReplayPageData() {
  const [
    {
      data: { replayRun, relatedSignals, relatedEvents },
    },
    i18n,
  ] = await Promise.all([readReplaySnapshot(), getServerI18n()]);
  const { dictionary, enumLabel } = i18n;
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
      kindLabel: enumLabel(moment.kind),
      summary: moment.summary,
    })),
    snapshot: {
      marketQuestion: replayRun.market_question,
      prior: replayRun.prior,
      posterior: replayRun.posterior,
      posteriorDelta: formatSignedFixed(posteriorDelta),
      stateTransition: `${enumLabel(replayRun.signal_state_from)} -> ${enumLabel(replayRun.signal_state_to)}`,
      createdAt: formatClock(replayRun.created_at),
      updatedAt: formatClock(replayRun.updated_at),
    },
    metrics:
      replayRun.metrics ??
      [
        { title: dictionary.metrics.signalHitRate, value: formatPercentFromRatio(replayRun.signal_hit_rate) },
        { title: dictionary.metrics.brierScore, value: replayRun.brier_score },
        { title: dictionary.metrics.netAlpha, value: formatPercentFromRatio(replayRun.net_alpha, 1) },
      ],
    relatedSignals: relatedSignals.map((signal) => ({
      id: signal.id,
      side: uppercaseEnum(signal.side),
      confidence: formatPercentFromRatio(signal.confidence),
      edge: formatSignedFixed(signal.edge),
      stateLabel: enumLabel(signal.lifecycle_state),
      stateTone: signalStateTone(signal.lifecycle_state),
      reason: signal.reason,
    })),
    relatedEvents: relatedEvents.map((event) => ({
      id: event.id,
      source: event.source,
      createdAt: formatClock(event.created_at),
      summary: event.summary,
      confidence: formatPercentFromRatio(event.confidence),
      statusLabel: enumLabel(event.status),
      statusTone: eventStatusTone(event.status),
    })),
  };
}
