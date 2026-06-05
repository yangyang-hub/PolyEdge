import type { RuntimeMode } from "@/lib/contracts/dto";

export function normalizeRuntimeMode(mode: RuntimeMode): RuntimeMode {
  return mode;
}

export function normalizeOptionalRuntimeMode(mode?: RuntimeMode): RuntimeMode | undefined {
  return mode;
}
