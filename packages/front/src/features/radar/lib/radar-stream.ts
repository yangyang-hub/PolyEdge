import type { ArbitrageStreamPayload } from "@/lib/contracts/realtime";
import {
  formatClock,
  formatInteger,
  formatPercentFromRatio,
  toFiniteNumber,
  type AccentTone,
} from "@/lib/formatters";
import type { Dictionary } from "@/lib/i18n/dictionaries";

import type {
  RadarAnalysis,
  RadarMetric,
  RadarOpportunityItem,
  RadarScanRow,
  RadarView,
} from "../types";
import {
  formatBookAge,
  formatDuration,
  formatPrice,
  isAnalysisSummary,
  localizeCandidateReason,
  opportunityStatusTone,
  opportunityTypeTone,
  readFormula,
  validationStatusTone,
} from "./radar-formatters";
import { calculateValidationSummary, deriveCandidatePreview } from "./radar-state";

function buildLiveOpportunity(
  payload: ArbitrageStreamPayload,
  dictionary: Dictionary,
  enumLabel: (value: string) => string,
  format: (template: string, values?: Record<string, string | number>) => string,
): RadarOpportunityItem | null {
  if (!payload.opportunity_id || !payload.market_id || !payload.opportunity_type || !payload.status) {
    return null;
  }

  const validationStatus = "unvalidated";
  const observedAt = payload.observed_at ?? payload.occurred_at ?? new Date().toISOString();

  return {
    id: payload.opportunity_id,
    marketId: payload.market_id,
    marketQuestion: payload.market_id,
    contextLabel: format(dictionary.radar.liveContext, { scanId: payload.scan_id ?? "n/a" }),
    opportunityType: payload.opportunity_type,
    typeLabel: enumLabel(payload.opportunity_type),
    typeTone: opportunityTypeTone(payload.opportunity_type),
    status: payload.status,
    statusLabel: enumLabel(payload.status),
    statusTone: opportunityStatusTone(payload.status),
    grossEdge: formatPercentFromRatio(payload.gross_edge ?? 0, 1),
    grossEdgeValue: toFiniteNumber(payload.gross_edge),
    priceSum: formatPrice(payload.price_sum),
    capacity: formatInteger(payload.capacity ?? 0),
    observedAt,
    observedClock: formatClock(observedAt),
    yesPrice: formatPrice(payload.yes_price),
    noPrice: formatPrice(payload.no_price),
    yesSize: formatInteger(payload.yes_size ?? 0),
    noSize: formatInteger(payload.no_size ?? 0),
    reasonCodes: payload.reason_codes?.map(enumLabel) ?? [],
    formula: readFormula(payload.analysis_payload),
    validationStatus,
    validationLabel: enumLabel(validationStatus),
    validationTone: validationStatusTone(validationStatus),
    netEdge: "n/a",
    netEdgeValue: 0,
    feeEstimate: "n/a",
    slippageBuffer: "n/a",
    validatedCapacity: "n/a",
    bookAge: "n/a",
    bookAgeMs: null,
    validationReasonCodes: [],
    candidateStatus: "watch",
    candidateLabel: enumLabel("watch"),
    candidateTone: "warning",
    candidateReason: dictionary.radar.waitingValidation,
    isSelected: false,
  };
}

function applyOpportunityPatch(
  current: RadarOpportunityItem,
  payload: ArbitrageStreamPayload,
  dictionary: Dictionary,
  enumLabel: (value: string) => string,
): RadarOpportunityItem {
  const status = payload.status ?? current.status;
  const candidate = deriveCandidatePreview({
    opportunityStatus: status,
    validationStatus: current.validationStatus,
    hasValidation: current.validationStatus !== "unvalidated",
    netEdgeValue: current.netEdgeValue,
  });

  return {
    ...current,
    status,
    statusLabel: enumLabel(status),
    statusTone: opportunityStatusTone(status),
    grossEdge: payload.gross_edge ? formatPercentFromRatio(payload.gross_edge, 1) : current.grossEdge,
    grossEdgeValue: payload.gross_edge ? toFiniteNumber(payload.gross_edge) : current.grossEdgeValue,
    priceSum: payload.price_sum ? formatPrice(payload.price_sum) : current.priceSum,
    capacity: payload.capacity ? formatInteger(payload.capacity) : current.capacity,
    observedAt: payload.observed_at ?? current.observedAt,
    observedClock: payload.observed_at ? formatClock(payload.observed_at) : current.observedClock,
    yesPrice: payload.yes_price ? formatPrice(payload.yes_price) : current.yesPrice,
    noPrice: payload.no_price ? formatPrice(payload.no_price) : current.noPrice,
    yesSize: payload.yes_size ? formatInteger(payload.yes_size) : current.yesSize,
    noSize: payload.no_size ? formatInteger(payload.no_size) : current.noSize,
    reasonCodes: payload.reason_codes?.map(enumLabel) ?? current.reasonCodes,
    formula: payload.analysis_payload ? readFormula(payload.analysis_payload) : current.formula,
    candidateStatus: candidate.status,
    candidateLabel: enumLabel(candidate.label),
    candidateTone: candidate.tone,
    candidateReason: localizeCandidateReason(dictionary, enumLabel, candidate.reason),
  };
}

function applyValidationPatch(
  current: RadarOpportunityItem,
  payload: ArbitrageStreamPayload,
  dictionary: Dictionary,
  enumLabel: (value: string) => string,
): RadarOpportunityItem {
  const validationStatus = payload.validation_status ?? current.validationStatus;
  const netEdgeValue = toFiniteNumber(payload.net_edge ?? current.netEdgeValue);
  const candidate = deriveCandidatePreview({
    opportunityStatus: current.status,
    validationStatus,
    hasValidation: true,
    netEdgeValue,
  });

  return {
    ...current,
    validationStatus,
    validationLabel: enumLabel(validationStatus),
    validationTone: validationStatusTone(validationStatus),
    netEdge: payload.net_edge ? formatPercentFromRatio(payload.net_edge, 1) : current.netEdge,
    netEdgeValue,
    feeEstimate: payload.fee_estimate ? formatPercentFromRatio(payload.fee_estimate, 1) : current.feeEstimate,
    slippageBuffer: payload.slippage_buffer
      ? formatPercentFromRatio(payload.slippage_buffer, 1)
      : current.slippageBuffer,
    validatedCapacity: payload.validated_capacity
      ? formatInteger(payload.validated_capacity)
      : current.validatedCapacity,
    bookAge: payload.book_age_ms !== undefined ? formatBookAge(payload.book_age_ms) : current.bookAge,
    bookAgeMs: payload.book_age_ms ?? current.bookAgeMs,
    validationReasonCodes: payload.reason_codes?.map(enumLabel) ?? current.validationReasonCodes,
    candidateStatus: candidate.status,
    candidateLabel: enumLabel(candidate.label),
    candidateTone: candidate.tone,
    candidateReason: localizeCandidateReason(dictionary, enumLabel, candidate.reason),
  };
}

export function upsertOpportunity(
  current: RadarOpportunityItem[],
  payload: ArbitrageStreamPayload,
  dictionary: Dictionary,
  enumLabel: (value: string) => string,
  format: (template: string, values?: Record<string, string | number>) => string,
): RadarOpportunityItem[] {
  const next = buildLiveOpportunity(payload, dictionary, enumLabel, format);

  if (!next) {
    return current;
  }

  const updated = current.some((opportunity) => opportunity.id === next.id)
    ? current.map((opportunity) =>
        opportunity.id === next.id ? applyOpportunityPatch(opportunity, payload, dictionary, enumLabel) : opportunity,
      )
    : [next, ...current];

  return updated
    .slice()
    .sort((left, right) => Date.parse(right.observedAt) - Date.parse(left.observedAt))
    .slice(0, 100);
}

export function patchValidation(
  current: RadarOpportunityItem[],
  payload: ArbitrageStreamPayload,
  dictionary: Dictionary,
  enumLabel: (value: string) => string,
): RadarOpportunityItem[] {
  if (!payload.opportunity_id) {
    return current;
  }

  return current.map((opportunity) =>
    opportunity.id === payload.opportunity_id ? applyValidationPatch(opportunity, payload, dictionary, enumLabel) : opportunity,
  );
}

function buildScanRow(payload: ArbitrageStreamPayload, dictionary: Dictionary): RadarScanRow | null {
  if (!payload.scan_id) {
    return null;
  }

  return {
    id: payload.scan_id,
    startedClock: payload.started_at
      ? formatClock(payload.started_at)
      : payload.occurred_at
        ? formatClock(payload.occurred_at)
        : "n/a",
    finishedClock: payload.finished_at ? formatClock(payload.finished_at) : dictionary.radar.running,
    marketCount: formatInteger(payload.market_count ?? 0),
    snapshotCount: formatInteger(payload.snapshot_count ?? 0),
    opportunityCount: formatInteger(payload.opportunity_count ?? 0),
    scannerVersion: payload.scanner_version ?? dictionary.radar.radarScanner,
  };
}

export function upsertScan(current: RadarScanRow[], payload: ArbitrageStreamPayload, dictionary: Dictionary): RadarScanRow[] {
  const next = buildScanRow(payload, dictionary);

  if (!next) {
    return current;
  }

  const existing = current.find((scan) => scan.id === next.id);
  const merged = existing ? { ...existing, ...next } : next;

  return [merged, ...current.filter((scan) => scan.id !== next.id)].slice(0, 8);
}

export function buildLiveAnalysis(
  payload: ArbitrageStreamPayload,
  enumLabel: (value: string) => string,
  marketQuestionById?: (id: string) => string,
): RadarAnalysis | null {
  if (!isAnalysisSummary(payload.summary_payload)) {
    return null;
  }

  const summary = payload.summary_payload;
  return {
    generatedClock: formatClock(summary.generated_at),
    lookbackHours: `${summary.lookback_hours}h`,
    opportunityCount: formatInteger(summary.opportunity_count),
    marketCount: formatInteger(summary.market_count),
    typeCounts: summary.type_counts.map((item) => ({
      typeLabel: enumLabel(item.opportunity_type),
      count: formatInteger(item.count),
      tone: opportunityTypeTone(item.opportunity_type),
    })),
    topMarkets: summary.top_markets.map((market) => ({
      marketId: market.market_id,
      marketQuestion: marketQuestionById?.(market.market_id) ?? market.market_id,
      opportunityCount: formatInteger(market.opportunity_count),
      maxGrossEdge: formatPercentFromRatio(market.max_gross_edge, 1),
      avgGrossEdge: formatPercentFromRatio(market.avg_gross_edge, 1),
      maxCapacity: formatInteger(market.max_capacity),
      duration: formatDuration(market.duration_seconds),
    })),
  };
}

export function buildMetrics(
  opportunities: RadarOpportunityItem[],
  scans: RadarScanRow[],
  dictionary: Dictionary,
  format: (template: string, values?: Record<string, string | number>) => string,
): RadarMetric[] {
  const latestScan = scans[0] ?? null;
  const active = opportunities.filter((opportunity) => opportunity.status !== "expired");
  const validationSummary = calculateValidationSummary(opportunities);
  const maxNetEdge = opportunities.reduce(
    (value, opportunity) => Math.max(value, opportunity.netEdgeValue),
    0,
  );
  const accent: AccentTone = maxNetEdge > 0 ? "success" : "primary";

  return [
    {
      title: dictionary.metrics.latestScan,
      value: latestScan?.startedClock ?? "n/a",
      hint: latestScan ? format(dictionary.metricHints.markets, { count: latestScan.marketCount }) : dictionary.metricHints.noScan,
      accent: "primary",
    },
    {
      title: dictionary.metrics.activeOpportunities,
      value: formatInteger(active.length),
      hint: latestScan ? format(dictionary.metricHints.latestScan, { count: latestScan.opportunityCount }) : dictionary.metricHints.notExpired,
      accent: "success",
    },
    {
      title: dictionary.metrics.maxNetEdge,
      value: formatPercentFromRatio(maxNetEdge, 1),
      hint: dictionary.metricHints.afterBuffers,
      accent,
    },
    {
      title: dictionary.metrics.validationPassRate,
      value: formatPercentFromRatio(validationSummary.passRate, 0),
      hint: format(dictionary.metricHints.validationSummary, {
        completed: formatInteger(validationSummary.completedValidationCount),
        rejected: formatInteger(validationSummary.rejectedCount),
      }),
      accent: validationSummary.passRate > 0 ? "success" : "primary",
    },
  ];
}

export function viewMatches(view: RadarView, opportunity: RadarOpportunityItem): boolean {
  if (view === "active") {
    return opportunity.status !== "expired";
  }

  if (view === "validated") {
    return opportunity.validationStatus === "valid";
  }

  if (view === "rejected") {
    return opportunity.validationStatus !== "valid" && opportunity.validationStatus !== "unvalidated";
  }

  return true;
}

export function compareRadarPriority(left: RadarOpportunityItem, right: RadarOpportunityItem): number {
  const leftValid = left.validationStatus === "valid" ? 1 : 0;
  const rightValid = right.validationStatus === "valid" ? 1 : 0;
  const leftAge = left.bookAgeMs ?? Number.MAX_SAFE_INTEGER;
  const rightAge = right.bookAgeMs ?? Number.MAX_SAFE_INTEGER;

  return (
    rightValid - leftValid ||
    right.netEdgeValue - left.netEdgeValue ||
    leftAge - rightAge ||
    Date.parse(right.observedAt) - Date.parse(left.observedAt)
  );
}
