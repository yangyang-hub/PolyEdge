import type { SignalStreamPayload } from "@/lib/contracts/realtime";
import type { Dictionary } from "@/lib/i18n/dictionaries";
import {
  formatPercentFromRatio,
  formatSignedFixed,
  signalStateTone,
  uppercaseEnum,
} from "@/lib/realtime-formatters";

import type { RuntimeControls, SelectedSignal, SignalItem } from "../types";

export function buildSignalItem(
  payload: SignalStreamPayload,
  current: SignalItem | undefined,
  dictionary: Dictionary,
  enumLabel: (value: string) => string,
): SignalItem {
  const confidenceValue = payload.confidence
    ? Number.parseFloat(payload.confidence)
    : current?.confidenceValue ?? 0;

  return {
    id: payload.signal_id,
    version: payload.version,
    lifecycleState: payload.lifecycle_state,
    marketQuestion: payload.market_question ?? current?.marketQuestion ?? payload.market_id,
    contextLabel: payload.context_label ?? current?.contextLabel ?? dictionary.signals.liveContextFallback,
    confidenceValue,
    side: payload.side ? uppercaseEnum(payload.side) : current?.side ?? "YES",
    fairPrice: payload.fair_price ?? current?.fairPrice ?? "0.00",
    marketPrice: payload.market_price ?? current?.marketPrice ?? "0.00",
    edge: payload.edge ? formatSignedFixed(payload.edge) : current?.edge ?? "0.00",
    confidence: payload.confidence ? formatPercentFromRatio(payload.confidence) : current?.confidence ?? "0%",
    confidenceWidth: payload.confidence
      ? formatPercentFromRatio(payload.confidence)
      : current?.confidenceWidth ?? "0%",
    stateLabel: enumLabel(payload.lifecycle_state),
    stateTone: signalStateTone(payload.lifecycle_state),
    approvedAt: current?.approvedAt ?? null,
    rejectedAt: current?.rejectedAt ?? null,
    reason: payload.reason ?? current?.reason ?? dictionary.signals.reasonFallback,
    riskDecision: payload.risk_decision ?? current?.riskDecision ?? dictionary.signals.riskFallback,
    evidenceLines: payload.evidence_lines ?? current?.evidenceLines ?? [],
    isSelected: current?.isSelected ?? false,
  };
}

export function hasExecutableLifecycle(signal: SignalItem | SelectedSignal): boolean {
  return signal.lifecycleState === "new" || signal.lifecycleState === "active";
}

export function canSubmitExecution(signal: SignalItem | SelectedSignal, controls: RuntimeControls): boolean {
  if (controls.killSwitch || signal.rejectedAt || !hasExecutableLifecycle(signal)) {
    return false;
  }

  return controls.mode === "paper_trade" || controls.mode === "live_auto";
}
