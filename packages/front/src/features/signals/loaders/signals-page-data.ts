import "server-only";

import { listEvidences } from "@/server/api/events";
import { listMarkets } from "@/server/api/markets";
import { listSignals } from "@/server/api/signals";
import { listApprovals } from "@/server/api/system";
import {
  getPendingSignalApprovalIds,
  indexMarkets,
  selectFirstMatchingItem,
} from "@/server/loaders/console-loader-utils";
import {
  formatPercentFromRatio,
  formatSignedFixed,
  humanizeSnakeCase,
  signalStateTone,
  uppercaseEnum,
} from "@/lib/server/console-formatters";

export async function getSignalsPageData() {
  const [{ data: signals }, { data: markets }, { data: evidences }, { data: approvals }] = await Promise.all([
    listSignals(),
    listMarkets(),
    listEvidences(),
    listApprovals(),
  ]);
  const marketIndex = indexMarkets(markets);
  const pendingSignalApprovalIds = getPendingSignalApprovalIds(approvals);
  const selectedSignal = selectFirstMatchingItem(
    signals,
    [
      (signal) => pendingSignalApprovalIds.has(signal.id),
      (signal) => signal.lifecycle_state === "new",
    ],
    "Signals page requires at least one signal fixture or API result.",
  );
  const selectedEvidenceItems = evidences.filter((evidence) => selectedSignal.evidence_ids.includes(evidence.id));

  return {
    activeCount: signals.filter((signal) => signal.lifecycle_state === "active").length,
    approvalCount: pendingSignalApprovalIds.size,
    signals: signals.map((signal) => ({
      id: signal.id,
      marketQuestion: marketIndex.get(signal.market_id)?.question ?? signal.market_id,
      contextLabel: `${marketIndex.get(signal.market_id)?.category ?? "Unknown"} / ${humanizeSnakeCase(
        marketIndex.get(signal.market_id)?.tradability_status ?? "manual_review",
      )}`,
      confidenceValue: Number.parseFloat(signal.confidence),
      side: uppercaseEnum(signal.side),
      fairPrice: signal.fair_price,
      marketPrice: signal.market_price,
      edge: formatSignedFixed(signal.edge),
      confidence: formatPercentFromRatio(signal.confidence),
      confidenceWidth: formatPercentFromRatio(signal.confidence),
      stateLabel: humanizeSnakeCase(signal.lifecycle_state),
      stateTone: signalStateTone(signal.lifecycle_state),
      requiresReview: pendingSignalApprovalIds.has(signal.id),
      reason: signal.reason,
      riskDecision: signal.risk_decision,
      evidenceLines: evidences
        .filter((evidence) => signal.evidence_ids.includes(evidence.id))
        .map((evidence) => {
          return `${humanizeSnakeCase(evidence.direction)} · strength ${evidence.strength} · novelty ${formatPercentFromRatio(evidence.novelty)}`;
        }),
      isSelected: signal.id === selectedSignal.id,
    })),
    selectedSignal: {
      marketQuestion: marketIndex.get(selectedSignal.market_id)?.question ?? selectedSignal.market_id,
      confidence: formatPercentFromRatio(selectedSignal.confidence),
      marketPrice: selectedSignal.market_price,
      fairPrice: selectedSignal.fair_price,
      edge: formatSignedFixed(selectedSignal.edge),
      stateLabel: humanizeSnakeCase(selectedSignal.lifecycle_state),
      stateTone: signalStateTone(selectedSignal.lifecycle_state),
      requiresReview: pendingSignalApprovalIds.has(selectedSignal.id),
      reason: selectedSignal.reason,
      riskDecision: selectedSignal.risk_decision,
      evidenceLines: selectedEvidenceItems.map((evidence) => {
        return `${humanizeSnakeCase(evidence.direction)} · strength ${evidence.strength} · novelty ${formatPercentFromRatio(evidence.novelty)}`;
      }),
    },
  };
}
