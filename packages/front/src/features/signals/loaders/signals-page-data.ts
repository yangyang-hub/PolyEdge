import "server-only";

import { listEvidences } from "@/server/api/events";
import { listMarkets } from "@/server/api/markets";
import { readRiskState } from "@/server/api/risk";
import { listSignals } from "@/server/api/signals";
import { localizeGeneratedCopy } from "@/lib/i18n/generated-copy";
import { getServerI18n } from "@/lib/i18n/server";
import {
  indexMarkets,
  selectFirstMatchingItem,
} from "@/server/loaders/console-loader-utils";
import {
  formatPercentFromRatio,
  formatSignedFixed,
  signalStateTone,
  uppercaseEnum,
} from "@/lib/server/console-formatters";
import { normalizeRuntimeMode } from "@/lib/runtime-mode";

export async function getSignalsPageData() {
  const [
    { data: signals },
    { data: markets },
    { data: evidences },
    { data: riskState },
    i18n,
  ] = await Promise.all([
    listSignals(),
    listMarkets(),
    listEvidences(),
    readRiskState(),
    getServerI18n(),
  ]);
  const { locale, dictionary, enumLabel, format } = i18n;
  const marketIndex = indexMarkets(markets);
  const selectedSignal = selectFirstMatchingItem(
    signals,
    [
      (signal) => signal.lifecycle_state === "new",
    ],
    dictionary.routeStates.signalsDataRequired,
  );
  const selectedEvidenceItems = evidences.filter((evidence) => selectedSignal.evidence_ids.includes(evidence.id));
  const runtimeMode = normalizeRuntimeMode(riskState.mode);

  return {
    activeCount: signals.filter((signal) => signal.lifecycle_state === "active").length,
    runtimeControls: {
      mode: runtimeMode,
      modeLabel: enumLabel(runtimeMode),
      killSwitch: riskState.kill_switch,
    },
    signals: signals.map((signal) => ({
      id: signal.id,
      version: signal.version,
      lifecycleState: signal.lifecycle_state,
      marketQuestion: marketIndex.get(signal.market_id)?.question ?? signal.market_id,
      contextLabel: `${marketIndex.get(signal.market_id)?.category ?? dictionary.common.unknown} / ${enumLabel(
        marketIndex.get(signal.market_id)?.tradability_status ?? "observe_only",
      )}`,
      confidenceValue: Number.parseFloat(signal.confidence),
      side: uppercaseEnum(signal.side),
      fairPrice: signal.fair_price,
      marketPrice: signal.market_price,
      edge: formatSignedFixed(signal.edge),
      confidence: formatPercentFromRatio(signal.confidence),
      confidenceWidth: formatPercentFromRatio(signal.confidence),
      stateLabel: enumLabel(signal.lifecycle_state),
      stateTone: signalStateTone(signal.lifecycle_state),
      approvedAt: signal.approved_at ?? null,
      rejectedAt: signal.rejected_at ?? null,
      reason: localizeGeneratedCopy(locale, dictionary, signal.reason),
      riskDecision: localizeGeneratedCopy(locale, dictionary, signal.risk_decision),
      evidenceLines: evidences
        .filter((evidence) => signal.evidence_ids.includes(evidence.id))
        .map((evidence) => {
          return format(dictionary.signals.evidenceLine, {
            direction: enumLabel(evidence.direction),
            strength: evidence.strength,
            novelty: formatPercentFromRatio(evidence.novelty),
          });
        }),
      isSelected: signal.id === selectedSignal.id,
    })),
    selectedSignal: {
      id: selectedSignal.id,
      version: selectedSignal.version,
      lifecycleState: selectedSignal.lifecycle_state,
      marketQuestion: marketIndex.get(selectedSignal.market_id)?.question ?? selectedSignal.market_id,
      confidence: formatPercentFromRatio(selectedSignal.confidence),
      marketPrice: selectedSignal.market_price,
      fairPrice: selectedSignal.fair_price,
      edge: formatSignedFixed(selectedSignal.edge),
      stateLabel: enumLabel(selectedSignal.lifecycle_state),
      stateTone: signalStateTone(selectedSignal.lifecycle_state),
      approvedAt: selectedSignal.approved_at ?? null,
      rejectedAt: selectedSignal.rejected_at ?? null,
      reason: localizeGeneratedCopy(locale, dictionary, selectedSignal.reason),
      riskDecision: localizeGeneratedCopy(locale, dictionary, selectedSignal.risk_decision),
      evidenceLines: selectedEvidenceItems.map((evidence) => {
        return format(dictionary.signals.evidenceLine, {
          direction: enumLabel(evidence.direction),
          strength: evidence.strength,
          novelty: formatPercentFromRatio(evidence.novelty),
        });
      }),
    },
  };
}
