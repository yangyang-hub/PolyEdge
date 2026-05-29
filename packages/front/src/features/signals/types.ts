import type { RuntimeMode, SignalLifecycleState } from "@/lib/contracts/dto";
import type { RealtimeTone } from "@/lib/realtime-formatters";

export type SignalTone = RealtimeTone;

export type SignalItem = {
  id: string;
  version: number;
  lifecycleState: SignalLifecycleState;
  marketQuestion: string;
  contextLabel: string;
  confidenceValue: number;
  side: string;
  fairPrice: string;
  marketPrice: string;
  edge: string;
  confidence: string;
  confidenceWidth: string;
  stateLabel: string;
  stateTone: SignalTone;
  approvedAt: string | null;
  rejectedAt: string | null;
  reason: string;
  riskDecision: string;
  evidenceLines: string[];
  isSelected: boolean;
};

export type SelectedSignal = {
  id: string;
  version: number;
  lifecycleState: SignalLifecycleState;
  marketQuestion: string;
  confidence: string;
  marketPrice: string;
  fairPrice: string;
  edge: string;
  stateLabel: string;
  stateTone: SignalTone;
  approvedAt: string | null;
  rejectedAt: string | null;
  reason: string;
  riskDecision: string;
  evidenceLines: string[];
};

export type RuntimeControls = {
  mode: RuntimeMode;
  modeLabel: string;
  killSwitch: boolean;
};

export type SignalsWorkbenchProps = {
  activeCount: number;
  runtimeControls: RuntimeControls;
  signals: SignalItem[];
  selectedSignal: SelectedSignal;
};

export type SignalFilter = "all" | "high_confidence";
export type SignalActionDialog = "execution" | null;
