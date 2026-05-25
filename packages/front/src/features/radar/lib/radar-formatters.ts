import type {
  ArbitrageAnalysisSummaryDto,
  ArbitrageOpportunityStatus,
  ArbitrageOpportunityType,
  ArbitrageValidationStatus,
} from "@/lib/contracts/dto";
import type { Dictionary } from "@/lib/i18n/dictionaries";
import { formatFixed, type Tone } from "@/lib/formatters";

export function formatPrice(value: string | number | null | undefined): string {
  return formatFixed(value, 3);
}

export function opportunityTypeTone(type: ArbitrageOpportunityType): Tone {
  return type === "binary_buy_both" ? "success" : "primary";
}

export function opportunityStatusTone(status: ArbitrageOpportunityStatus): Tone {
  if (status === "observed") {
    return "success";
  }

  if (status === "repeated") {
    return "warning";
  }

  return "neutral";
}

export function validationStatusTone(status: ArbitrageValidationStatus | "unvalidated"): Tone {
  if (status === "valid") {
    return "success";
  }

  if (status === "unvalidated") {
    return "neutral";
  }

  if (status === "stale_book" || status === "insufficient_depth" || status === "below_threshold") {
    return "warning";
  }

  return "danger";
}

export function formatBookAge(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return "n/a";
  }

  if (value < 1000) {
    return `${Math.max(0, Math.round(value))}ms`;
  }

  return `${(value / 1000).toFixed(1)}s`;
}

export function readFormula(payload: unknown): string {
  if (!payload || typeof payload !== "object" || !("formula" in payload)) {
    return "n/a";
  }

  const formula = (payload as { formula?: unknown }).formula;
  return typeof formula === "string" && formula.trim() ? formula : "n/a";
}

export function isAnalysisSummary(value: unknown): value is ArbitrageAnalysisSummaryDto {
  if (!value || typeof value !== "object") {
    return false;
  }

  const candidate = value as Partial<ArbitrageAnalysisSummaryDto>;
  return Array.isArray(candidate.type_counts) && Array.isArray(candidate.top_markets);
}

export function formatDuration(seconds: number): string {
  if (seconds < 60) {
    return `${seconds}s`;
  }

  const minutes = Math.floor(seconds / 60);
  const remainder = seconds % 60;
  return remainder === 0 ? `${minutes}m` : `${minutes}m ${remainder}s`;
}

export function localizeCandidateReason(
  dictionary: Dictionary,
  enumLabel: (value: string) => string,
  reason: string,
): string {
  if (reason === "expired opportunity") {
    return dictionary.radar.expiredOpportunity;
  }

  if (reason === "waiting for validation") {
    return dictionary.radar.waitingValidation;
  }

  if (reason === "non-positive net edge") {
    return dictionary.radar.nonPositiveNetEdge;
  }

  if (reason === "valid read-only candidate") {
    return dictionary.radar.validReadOnlyCandidate;
  }

  return enumLabel(reason);
}
