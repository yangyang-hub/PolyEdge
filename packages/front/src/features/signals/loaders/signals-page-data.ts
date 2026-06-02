import { listEvidences } from "@/lib/api/events";
import { listMarkets } from "@/lib/api/markets";
import { readRiskState } from "@/lib/api/risk";
import { listSignals } from "@/lib/api/signals";
import { localizeGeneratedCopy } from "@/lib/i18n/generated-copy";
import { dictionary, translateEnum, formatMessage } from "@/lib/i18n/dictionaries";
import {
  indexMarkets,
  selectFirstMatchingItem,
} from "@/lib/loaders/console-loader-utils";
import {
  formatPercentFromRatio,
  formatSignedFixed,
  signalStateTone,
  uppercaseEnum,
} from "@/lib/formatters";
import { normalizeRuntimeMode } from "@/lib/runtime-mode";

export async function getSignalsPageData() {
  const [
    { data: signals },
    { data: markets },
    { data: evidences },
    { data: riskState },
  ] = await Promise.all([
    listSignals(),
    listMarkets(),
    listEvidences(),
    readRiskState(),
  ]);
  const marketIndex = indexMarkets(markets);
  const selectedSignal = signals.length > 0
    ? selectFirstMatchingItem(
        signals,
        [
          (signal) => signal.lifecycle_state === "new",
        ],
        dictionary.routeStates.signalsDataRequired,
      )
    : null;
  const selectedEvidenceItems = selectedSignal
    ? evidences.filter((evidence) => selectedSignal.evidence_ids.includes(evidence.id))
    : [];
  const runtimeMode = normalizeRuntimeMode(riskState.mode);

  return {
    activeCount: signals.filter((signal) => signal.lifecycle_state === "active").length,
    runtimeControls: {
      mode: runtimeMode,
      modeLabel: translateEnum(runtimeMode),
      killSwitch: riskState.kill_switch,
    },
    signals: signals.map((signal) => ({
      id: signal.id,
      version: signal.version,
      lifecycleState: signal.lifecycle_state,
      marketQuestion: marketIndex.get(signal.market_id)?.question ?? signal.market_id,
      contextLabel: `${marketIndex.get(signal.market_id)?.category ?? dictionary.common.unknown} / ${translateEnum(
        marketIndex.get(signal.market_id)?.tradability_status ?? "observe_only",
      )}`,
      confidenceValue: Number.parseFloat(signal.confidence),
      side: uppercaseEnum(signal.side),
      fairPrice: signal.fair_price,
      marketPrice: signal.market_price,
      edge: formatSignedFixed(signal.edge),
      confidence: formatPercentFromRatio(signal.confidence),
      confidenceWidth: formatPercentFromRatio(signal.confidence),
      stateLabel: translateEnum(signal.lifecycle_state),
      stateTone: signalStateTone(signal.lifecycle_state),
      approvedAt: signal.approved_at ?? null,
      rejectedAt: signal.rejected_at ?? null,
      reason: localizeGeneratedCopy(dictionary, signal.reason),
      riskDecision: localizeGeneratedCopy(dictionary, signal.risk_decision),
      evidenceLines: evidences
        .filter((evidence) => signal.evidence_ids.includes(evidence.id))
        .map((evidence) => {
          return formatMessage(dictionary.signals.evidenceLine, {
            direction: translateEnum(evidence.direction),
            strength: evidence.strength,
            novelty: formatPercentFromRatio(evidence.novelty),
          });
        }),
      isSelected: selectedSignal ? signal.id === selectedSignal.id : false,
    })),
    selectedSignal: selectedSignal
      ? {
          id: selectedSignal.id,
          version: selectedSignal.version,
          lifecycleState: selectedSignal.lifecycle_state,
          marketQuestion: marketIndex.get(selectedSignal.market_id)?.question ?? selectedSignal.market_id,
          confidence: formatPercentFromRatio(selectedSignal.confidence),
          marketPrice: selectedSignal.market_price,
          fairPrice: selectedSignal.fair_price,
          edge: formatSignedFixed(selectedSignal.edge),
          stateLabel: translateEnum(selectedSignal.lifecycle_state),
          stateTone: signalStateTone(selectedSignal.lifecycle_state),
          approvedAt: selectedSignal.approved_at ?? null,
          rejectedAt: selectedSignal.rejected_at ?? null,
          reason: localizeGeneratedCopy(dictionary, selectedSignal.reason),
          riskDecision: localizeGeneratedCopy(dictionary, selectedSignal.risk_decision),
          evidenceLines: selectedEvidenceItems.map((evidence) => {
            return formatMessage(dictionary.signals.evidenceLine, {
              direction: translateEnum(evidence.direction),
              strength: evidence.strength,
              novelty: formatPercentFromRatio(evidence.novelty),
            });
          }),
        }
      : null,
  };
}
