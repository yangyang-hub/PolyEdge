import "server-only";

import type {
  ArbitrageAnalysisSummaryDto,
  ArbitrageOpportunityDto,
  ArbitrageOpportunityStatus,
  ArbitrageOpportunityType,
  ArbitrageValidationStatus,
  MarketDto,
} from "@/lib/contracts/dto";
import {
  formatClock,
  formatInteger,
  formatPercentFromRatio,
  type AccentTone,
  type Tone,
} from "@/lib/server/console-formatters";
import type { Dictionary } from "@/lib/i18n/dictionaries";
import { getServerI18n } from "@/lib/i18n/server";
import {
  listArbitrageAnalysisRuns,
  listArbitrageOpportunities,
  listArbitrageScans,
} from "@/server/api/arbitrage";
import { listMarkets } from "@/server/api/markets";
import { deriveCandidatePreview } from "@/features/radar/lib/radar-state";

export type RadarOpportunityItem = {
  id: string;
  marketId: string;
  marketQuestion: string;
  contextLabel: string;
  opportunityType: ArbitrageOpportunityType;
  typeLabel: string;
  typeTone: Tone;
  status: ArbitrageOpportunityStatus;
  statusLabel: string;
  statusTone: Tone;
  grossEdge: string;
  grossEdgeValue: number;
  priceSum: string;
  capacity: string;
  observedAt: string;
  observedClock: string;
  yesPrice: string;
  noPrice: string;
  yesSize: string;
  noSize: string;
  reasonCodes: string[];
  formula: string;
  validationStatus: ArbitrageValidationStatus | "unvalidated";
  validationLabel: string;
  validationTone: Tone;
  netEdge: string;
  netEdgeValue: number;
  feeEstimate: string;
  slippageBuffer: string;
  validatedCapacity: string;
  bookAge: string;
  bookAgeMs: number | null;
  validationReasonCodes: string[];
  candidateStatus: "candidate" | "watch" | "blocked";
  candidateLabel: string;
  candidateTone: Tone;
  candidateReason: string;
  isSelected: boolean;
};

export type RadarScanRow = {
  id: string;
  startedClock: string;
  finishedClock: string;
  marketCount: string;
  snapshotCount: string;
  opportunityCount: string;
  scannerVersion: string;
};

export type RadarTypeCount = {
  typeLabel: string;
  count: string;
  tone: Tone;
};

export type RadarTopMarket = {
  marketId: string;
  marketQuestion: string;
  opportunityCount: string;
  maxGrossEdge: string;
  avgGrossEdge: string;
  maxCapacity: string;
  duration: string;
};

export type RadarAnalysis = {
  generatedClock: string;
  lookbackHours: string;
  opportunityCount: string;
  marketCount: string;
  typeCounts: RadarTypeCount[];
  topMarkets: RadarTopMarket[];
};

export type RadarMetric = {
  title: string;
  value: string;
  hint: string;
  accent: AccentTone;
};

export type RadarPageData = {
  selectedOpportunityId: string;
  metrics: RadarMetric[];
  opportunities: RadarOpportunityItem[];
  scans: RadarScanRow[];
  analysis: RadarAnalysis | null;
};

function toNumber(value: string | number | null | undefined): number {
  if (value === null || value === undefined) {
    return 0;
  }

  const parsed = typeof value === "number" ? value : Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : 0;
}

function opportunityTypeTone(type: ArbitrageOpportunityType): Tone {
  return type === "binary_buy_both" ? "success" : "primary";
}

function opportunityStatusTone(status: ArbitrageOpportunityStatus): Tone {
  if (status === "observed") {
    return "success";
  }

  if (status === "repeated") {
    return "warning";
  }

  return "neutral";
}

function validationStatusTone(status: ArbitrageValidationStatus | "unvalidated"): Tone {
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

function formatBookAge(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return "n/a";
  }

  if (value < 1000) {
    return `${Math.max(0, Math.round(value))}ms`;
  }

  return `${(value / 1000).toFixed(1)}s`;
}

function readFormula(payload: unknown): string {
  if (!payload || typeof payload !== "object" || !("formula" in payload)) {
    return "n/a";
  }

  const formula = (payload as { formula?: unknown }).formula;
  return typeof formula === "string" && formula.trim() ? formula : "n/a";
}

function isAnalysisSummary(value: unknown): value is ArbitrageAnalysisSummaryDto {
  if (!value || typeof value !== "object") {
    return false;
  }

  const candidate = value as Partial<ArbitrageAnalysisSummaryDto>;
  return Array.isArray(candidate.type_counts) && Array.isArray(candidate.top_markets);
}

function formatDuration(seconds: number): string {
  if (seconds < 60) {
    return `${seconds}s`;
  }

  const minutes = Math.floor(seconds / 60);
  const remainder = seconds % 60;
  return remainder === 0 ? `${minutes}m` : `${minutes}m ${remainder}s`;
}

function formatPrice(value: string): string {
  return toNumber(value).toFixed(3);
}

function localizeCandidateReason(dictionary: Dictionary, enumLabel: (value: string) => string, reason: string): string {
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

function buildOpportunity(
  opportunity: ArbitrageOpportunityDto,
  marketIndex: Map<string, MarketDto>,
  selectedOpportunityId: string,
  dictionary: Dictionary,
  enumLabel: (value: string) => string,
): RadarOpportunityItem {
  const market = marketIndex.get(opportunity.market_id);
  const validation = opportunity.validation ?? null;
  const validationStatus = validation?.status ?? "unvalidated";
  const candidate = deriveCandidatePreview({
    opportunityStatus: opportunity.status,
    validationStatus,
    hasValidation: Boolean(validation),
    netEdgeValue: validation ? toNumber(validation.net_edge) : 0,
  });

  return {
    id: opportunity.id,
    marketId: opportunity.market_id,
    marketQuestion: market?.question ?? opportunity.market_id,
    contextLabel: `${market?.category ?? dictionary.common.unknown} / scan ${opportunity.scan_id}`,
    opportunityType: opportunity.opportunity_type,
    typeLabel: enumLabel(opportunity.opportunity_type),
    typeTone: opportunityTypeTone(opportunity.opportunity_type),
    status: opportunity.status,
    statusLabel: enumLabel(opportunity.status),
    statusTone: opportunityStatusTone(opportunity.status),
    grossEdge: formatPercentFromRatio(opportunity.gross_edge, 1),
    grossEdgeValue: toNumber(opportunity.gross_edge),
    priceSum: formatPrice(opportunity.price_sum),
    capacity: formatInteger(opportunity.capacity),
    observedAt: opportunity.observed_at,
    observedClock: formatClock(opportunity.observed_at),
    yesPrice: formatPrice(opportunity.yes_price),
    noPrice: formatPrice(opportunity.no_price),
    yesSize: formatInteger(opportunity.yes_size),
    noSize: formatInteger(opportunity.no_size),
    reasonCodes: opportunity.reason_codes.map(enumLabel),
    formula: readFormula(opportunity.analysis_payload),
    validationStatus,
    validationLabel: enumLabel(validationStatus),
    validationTone: validationStatusTone(validationStatus),
    netEdge: validation ? formatPercentFromRatio(validation.net_edge, 1) : "n/a",
    netEdgeValue: validation ? toNumber(validation.net_edge) : 0,
    feeEstimate: validation ? formatPercentFromRatio(validation.fee_estimate, 1) : "n/a",
    slippageBuffer: validation ? formatPercentFromRatio(validation.slippage_buffer, 1) : "n/a",
    validatedCapacity: validation ? formatInteger(validation.validated_capacity) : "n/a",
    bookAge: formatBookAge(validation?.book_age_ms),
    bookAgeMs: validation?.book_age_ms ?? null,
    validationReasonCodes: validation?.reason_codes.map(enumLabel) ?? [],
    candidateStatus: candidate.status,
    candidateLabel: enumLabel(candidate.label),
    candidateTone: candidate.tone,
    candidateReason: localizeCandidateReason(dictionary, enumLabel, candidate.reason),
    isSelected: opportunity.id === selectedOpportunityId,
  };
}

function buildAnalysis(
  summary: ArbitrageAnalysisSummaryDto,
  marketIndex: Map<string, MarketDto>,
  enumLabel: (value: string) => string,
): RadarAnalysis {
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
      marketQuestion: marketIndex.get(market.market_id)?.question ?? market.market_id,
      opportunityCount: formatInteger(market.opportunity_count),
      maxGrossEdge: formatPercentFromRatio(market.max_gross_edge, 1),
      avgGrossEdge: formatPercentFromRatio(market.avg_gross_edge, 1),
      maxCapacity: formatInteger(market.max_capacity),
      duration: formatDuration(market.duration_seconds),
    })),
  };
}

export async function getRadarPageData(): Promise<RadarPageData> {
  const [{ data: scans }, { data: opportunities }, { data: analysisRuns }, { data: markets }, i18n] =
    await Promise.all([
      listArbitrageScans({ limit: 8 }),
      listArbitrageOpportunities({ limit: 100 }),
      listArbitrageAnalysisRuns({ limit: 1 }),
      listMarkets({ limit: 200 }),
      getServerI18n(),
    ]);
  const { dictionary, enumLabel, format } = i18n;

  const marketIndex = new Map(markets.map((market) => [market.id, market]));
  const sortedOpportunities = opportunities
    .slice()
    .sort((left, right) => Date.parse(right.observed_at) - Date.parse(left.observed_at));
  const selectedOpportunityId = sortedOpportunities[0]?.id ?? "";
  const maxEdge = sortedOpportunities.reduce(
    (value, opportunity) => Math.max(value, toNumber(opportunity.gross_edge)),
    0,
  );
  const coveredMarketCount = new Set(sortedOpportunities.map((opportunity) => opportunity.market_id)).size;
  const latestScan = scans[0] ?? null;
  const analysisSummary = analysisRuns[0]?.summary_payload;

  return {
    selectedOpportunityId,
    metrics: [
      {
        title: dictionary.metrics.latestScan,
        value: latestScan ? formatClock(latestScan.started_at) : "n/a",
        hint: latestScan
          ? format(dictionary.metricHints.markets, { count: formatInteger(latestScan.market_count) })
          : dictionary.metricHints.noScan,
        accent: "primary",
      },
      {
        title: dictionary.metrics.observedOpportunities,
        value: formatInteger(sortedOpportunities.length),
        hint: latestScan
          ? format(dictionary.metricHints.latestScan, { count: formatInteger(latestScan.opportunity_count) })
          : dictionary.metricHints.allWindows,
        accent: "success",
      },
      {
        title: dictionary.metrics.maxGrossEdge,
        value: formatPercentFromRatio(maxEdge, 1),
        hint: dictionary.metricHints.bestObserved,
        accent: maxEdge > 0 ? "success" : "primary",
      },
      {
        title: dictionary.metrics.coveredMarkets,
        value: formatInteger(coveredMarketCount),
        hint: format(dictionary.metricHints.tracked, { count: formatInteger(markets.length) }),
        accent: "violet",
      },
    ],
    opportunities: sortedOpportunities.map((opportunity) =>
      buildOpportunity(opportunity, marketIndex, selectedOpportunityId, dictionary, enumLabel),
    ),
    scans: scans.map((scan) => ({
      id: scan.id,
      startedClock: formatClock(scan.started_at),
      finishedClock: scan.finished_at ? formatClock(scan.finished_at) : dictionary.radar.running,
      marketCount: formatInteger(scan.market_count),
      snapshotCount: formatInteger(scan.snapshot_count),
      opportunityCount: formatInteger(scan.opportunity_count),
      scannerVersion: scan.scanner_version,
    })),
    analysis: isAnalysisSummary(analysisSummary) ? buildAnalysis(analysisSummary, marketIndex, enumLabel) : null,
  };
}
