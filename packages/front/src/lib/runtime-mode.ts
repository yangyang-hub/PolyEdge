import type { RuntimeMode } from "@/lib/contracts/dto";

export function normalizeRuntimeMode(mode: RuntimeMode): RuntimeMode {
  return mode === "manual_confirm" ? "paper_trade" : mode;
}

export function normalizeOptionalRuntimeMode(mode?: RuntimeMode): RuntimeMode | undefined {
  return mode ? normalizeRuntimeMode(mode) : undefined;
}
