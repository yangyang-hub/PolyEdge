import type { HighProbabilityBucketStatsDto, HighProbabilityFairValueDto } from "@/lib/contracts/dto";
import {
  formatFixed,
  formatInteger,
  formatOptionalClock,
  formatPercentFromRatio,
  toFiniteNumber,
  type Tone,
} from "@/lib/formatters";
import { dictionary } from "@/lib/i18n/dictionaries";

export function formatProbability(value: string | number | null | undefined): string {
  return formatPercentFromRatio(value, 1);
}

export function formatOptionalProbability(value: string | number | null | undefined): string {
  if (value === null || value === undefined) {
    return dictionary.common.none;
  }
  return formatProbability(value);
}

export function formatOptionalFixed(value: string | number | null | undefined, digits = 3): string {
  if (value === null || value === undefined) {
    return dictionary.common.none;
  }
  return formatFixed(value, digits);
}

export function formatCents(value: string | number | null | undefined): string {
  if (value === null || value === undefined) {
    return dictionary.common.none;
  }
  return `${formatFixed(value, 2)}c`;
}

export function formatSeconds(value: number | null | undefined): string {
  if (value === null || value === undefined) {
    return dictionary.common.none;
  }
  if (value < 3_600) {
    return `${Math.round(value / 60)}m`;
  }
  if (value < 86_400) {
    return `${(value / 3_600).toFixed(1)}h`;
  }
  return `${(value / 86_400).toFixed(1)}d`;
}

export function bucketTone(bucket: HighProbabilityBucketStatsDto): Tone {
  if (bucket.sample_count < 100) {
    return "warning";
  }
  if (toFiniteNumber(bucket.expected_pnl) > 0) {
    return "success";
  }
  return "neutral";
}

export function bucketDimensionLabel(bucket: HighProbabilityBucketStatsDto): string {
  const dimensions = bucket.bucket_dimensions;
  if (!isRecord(dimensions)) {
    return bucket.bucket_key;
  }

  const values = [
    stringValue(dimensions.market_type),
    stringValue(dimensions.price_bucket),
    stringValue(dimensions.time_to_resolution_bucket),
    stringValue(dimensions.liquidity_bucket),
    stringValue(dimensions.spread_bucket),
  ].filter(Boolean);

  return values.length > 0 ? values.join(" / ") : bucket.bucket_key;
}

export function bucketSampleHint(bucket: HighProbabilityBucketStatsDto): string {
  return `${formatInteger(bucket.win_count)} / ${formatInteger(bucket.sample_count)}`;
}

export function bucketComputedAt(bucket: HighProbabilityBucketStatsDto): string {
  return formatOptionalClock(bucket.computed_at);
}

export function reportNoteLabel(note: string): string {
  const labels = dictionary.highProbability.reportNoteLabels as Record<string, string>;
  return labels[note] ?? note.replaceAll("_", " ");
}

export function exitRuleLabel(ruleKey: string): string {
  const labels = dictionary.highProbability.exitRuleLabels as Record<string, string>;
  return labels[ruleKey] ?? ruleKey.replaceAll("_", " ");
}

export function fairValueSideLabel(side: HighProbabilityFairValueDto["side_used"]): string {
  const labels = dictionary.highProbability.fairValueSideLabels as Record<string, string>;
  return labels[side] ?? side;
}

export function fairValueFallbackLabel(level: number): string {
  const labels = dictionary.highProbability.fairValueFallbackLabels as Record<string, string>;
  return labels[String(level)] ?? `L${level}`;
}

export function fairValueEligibleTone(eligible: boolean): Tone {
  return eligible ? "success" : "neutral";
}

export function fairValueBand(fairValue: HighProbabilityFairValueDto): string {
  return `${formatProbability(fairValue.fair_yes_low)} – ${formatProbability(fairValue.fair_yes_high)}`;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function stringValue(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value : null;
}
