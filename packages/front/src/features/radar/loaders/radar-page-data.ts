import type {
  ArbitrageAnalysisSummaryDto,
  ArbitrageOpportunityDto,
  MarketDto,
} from "@/lib/contracts/dto";
import {
  formatClock,
  formatInteger,
  formatPercentFromRatio,
} from "@/lib/formatters";
import type { Dictionary } from "@/lib/i18n/dictionaries";
import type { I18nRuntime } from "@/lib/i18n/runtime";
import {
  listArbitrageAnalysisRuns,
  listArbitrageOpportunities,
  listArbitrageScans,
} from "@/lib/api/arbitrage";
import { listMarkets } from "@/lib/api/markets";
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
} from "@/features/radar/lib/radar-formatters";
import { deriveCandidatePreview } from "@/features/radar/lib/radar-state";
import { toFiniteNumber } from "@/lib/formatters";
import type { RadarAnalysis, RadarOpportunityItem, RadarPageData } from "@/features/radar/types";

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
    netEdgeValue: validation ? toFiniteNumber(validation.net_edge) : 0,
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
    grossEdgeValue: toFiniteNumber(opportunity.gross_edge),
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
    netEdgeValue: validation ? toFiniteNumber(validation.net_edge) : 0,
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

export async function getRadarPageData(i18n: I18nRuntime): Promise<RadarPageData> {
  const [{ data: scans }, { data: opportunities }, { data: analysisRuns }, { data: markets }] =
    await Promise.all([
      listArbitrageScans({ limit: 8 }),
      listArbitrageOpportunities({ limit: 100 }),
      listArbitrageAnalysisRuns({ limit: 1 }),
      listMarkets({ limit: 200 }),
    ]);
  const { dictionary, enumLabel, format } = i18n;

  const marketIndex = new Map(markets.map((market) => [market.id, market]));
  const sortedOpportunities = opportunities
    .slice()
    .sort((left, right) => Date.parse(right.observed_at) - Date.parse(left.observed_at));
  const selectedOpportunityId = sortedOpportunities[0]?.id ?? "";
  const maxEdge = sortedOpportunities.reduce(
    (value, opportunity) => Math.max(value, toFiniteNumber(opportunity.gross_edge)),
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
