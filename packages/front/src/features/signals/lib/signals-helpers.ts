import type { RuntimeControls, SelectedSignal, SignalItem } from "../types";

export function hasExecutableLifecycle(signal: SignalItem | SelectedSignal): boolean {
  return signal.lifecycleState === "new" || signal.lifecycleState === "active";
}

export function canSubmitExecution(_signal: SignalItem | SelectedSignal, _controls: RuntimeControls): boolean {
  return false;
}
