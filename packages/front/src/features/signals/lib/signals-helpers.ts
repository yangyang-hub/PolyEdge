import type { SelectedSignal, SignalItem } from "../types";

export function hasExecutableLifecycle(signal: SignalItem | SelectedSignal): boolean {
  return signal.lifecycleState === "new" || signal.lifecycleState === "active";
}

export function canSubmitExecution(): boolean {
  return false;
}
