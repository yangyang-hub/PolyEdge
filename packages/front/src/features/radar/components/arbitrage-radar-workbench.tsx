"use client";

import { startTransition, useDeferredValue, useEffect, useMemo, useState } from "react";
import { ChevronRight, Filter, Radar } from "lucide-react";

import { MetricCard } from "@/components/shared/metric-card";
import { PageHeader } from "@/components/shared/page-header";
import { StatusPill } from "@/components/shared/status-pill";
import { useConsoleRealtimeChannel } from "@/components/shared/console-realtime-provider";
import { WorkbenchDetailPane, WorkbenchLayout } from "@/components/shared/workbench-layout";
import { WorkbenchSegmentedControl } from "@/components/shared/workbench-segmented-control";
import { Button } from "@/components/ui/button";
import { useI18n } from "@/lib/i18n/client";
import type { Dictionary } from "@/lib/i18n/dictionaries";
import type {
  RadarAnalysis,
  RadarMetric,
  RadarOpportunityItem,
  RadarPageData,
  RadarScanRow,
} from "@/features/radar/loaders/radar-page-data";
import type {
  ArbitrageAnalysisSummaryDto,
  ArbitrageOpportunityStatus,
  ArbitrageOpportunityType,
  ArbitrageValidationStatus,
} from "@/lib/contracts/dto";
import type { ArbitrageStreamPayload } from "@/lib/contracts/realtime";
import {
  calculateValidationSummary,
  deriveCandidatePreview,
} from "@/features/radar/lib/radar-state";
import {
  formatClock,
  formatInteger,
  formatPercentFromRatio,
  type AccentTone,
  type Tone,
} from "@/lib/formatters";
import { isKeyboardSelect } from "@/lib/keyboard";

type RadarFilter = "all" | "binary_buy_both" | "binary_sell_both";
type RadarView = "active" | "validated" | "rejected" | "history";

type ArbitrageRadarWorkbenchProps = {
  data: RadarPageData;
};

function toNumber(value: string | number | null | undefined): number {
  if (value === null || value === undefined) {
    return 0;
  }

  const parsed = typeof value === "number" ? value : Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : 0;
}

function formatPrice(value: string | number | null | undefined): string {
  return toNumber(value).toFixed(3);
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
    grossEdgeValue: toNumber(payload.gross_edge),
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
    grossEdgeValue: payload.gross_edge ? toNumber(payload.gross_edge) : current.grossEdgeValue,
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
  const netEdgeValue = toNumber(payload.net_edge ?? current.netEdgeValue);
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

function upsertOpportunity(
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

function patchValidation(
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

function upsertScan(current: RadarScanRow[], payload: ArbitrageStreamPayload, dictionary: Dictionary): RadarScanRow[] {
  const next = buildScanRow(payload, dictionary);

  if (!next) {
    return current;
  }

  const existing = current.find((scan) => scan.id === next.id);
  const merged = existing ? { ...existing, ...next } : next;

  return [merged, ...current.filter((scan) => scan.id !== next.id)].slice(0, 8);
}

function isAnalysisSummary(value: unknown): value is ArbitrageAnalysisSummaryDto {
  if (!value || typeof value !== "object") {
    return false;
  }

  const candidate = value as Partial<ArbitrageAnalysisSummaryDto>;
  return Array.isArray(candidate.type_counts) && Array.isArray(candidate.top_markets);
}

function buildLiveAnalysis(
  payload: ArbitrageStreamPayload,
  enumLabel: (value: string) => string,
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
      marketQuestion: market.market_id,
      opportunityCount: formatInteger(market.opportunity_count),
      maxGrossEdge: formatPercentFromRatio(market.max_gross_edge, 1),
      avgGrossEdge: formatPercentFromRatio(market.avg_gross_edge, 1),
      maxCapacity: formatInteger(market.max_capacity),
      duration: `${market.duration_seconds}s`,
    })),
  };
}

function buildMetrics(
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

function viewMatches(view: RadarView, opportunity: RadarOpportunityItem): boolean {
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

function compareRadarPriority(left: RadarOpportunityItem, right: RadarOpportunityItem): number {
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

function OpportunityDetail({ opportunity }: { opportunity: RadarOpportunityItem | null }) {
  const { dictionary } = useI18n();

  if (!opportunity) {
    return (
      <div className="rounded-md bg-popover/70 p-4">
        <p className="font-heading text-lg font-bold text-foreground">{dictionary.radar.noSelectionTitle}</p>
        <p className="mt-2 text-sm text-muted-foreground">{dictionary.radar.noSelectionDetail}</p>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <p className="font-heading text-lg font-bold tracking-tight text-foreground">
          {opportunity.marketQuestion}
        </p>
        <div className="flex flex-wrap gap-2">
          <StatusPill tone={opportunity.typeTone}>{opportunity.typeLabel}</StatusPill>
          <StatusPill tone={opportunity.statusTone}>{opportunity.statusLabel}</StatusPill>
          <StatusPill tone={opportunity.validationTone}>{opportunity.validationLabel}</StatusPill>
          <StatusPill tone={opportunity.candidateTone}>{opportunity.candidateLabel}</StatusPill>
          <StatusPill tone="primary">{opportunity.observedClock}</StatusPill>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-3">
        <div className="rounded-md bg-accent/45 p-3">
          <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">{dictionary.radar.grossEdge}</p>
          <p className="mt-2 font-mono text-lg text-secondary">{opportunity.grossEdge}</p>
        </div>
        <div className="rounded-md bg-accent/45 p-3">
          <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">{dictionary.radar.priceSum}</p>
          <p className="mt-2 font-mono text-lg text-foreground">{opportunity.priceSum}</p>
        </div>
        <div className="rounded-md bg-accent/45 p-3">
          <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">{dictionary.radar.yesPrice}</p>
          <p className="mt-2 font-mono text-lg text-primary">{opportunity.yesPrice}</p>
        </div>
        <div className="rounded-md bg-accent/45 p-3">
          <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">{dictionary.radar.noPrice}</p>
          <p className="mt-2 font-mono text-lg text-primary">{opportunity.noPrice}</p>
        </div>
        <div className="rounded-md bg-accent/45 p-3">
          <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">{dictionary.radar.yesSize}</p>
          <p className="mt-2 font-mono text-lg text-foreground">{opportunity.yesSize}</p>
        </div>
        <div className="rounded-md bg-accent/45 p-3">
          <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">{dictionary.radar.noSize}</p>
          <p className="mt-2 font-mono text-lg text-foreground">{opportunity.noSize}</p>
        </div>
      </div>

      <div className="rounded-md bg-popover/70 p-4">
        <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
          {dictionary.radar.validation}
        </p>
        <div className="mt-3 grid grid-cols-2 gap-3">
          <div>
            <p className="text-[10px] uppercase text-muted-foreground">{dictionary.radar.netEdge}</p>
            <p className="mt-1 font-mono text-sm text-foreground">{opportunity.netEdge}</p>
          </div>
          <div>
            <p className="text-[10px] uppercase text-muted-foreground">{dictionary.radar.capacity}</p>
            <p className="mt-1 font-mono text-sm text-foreground">{opportunity.validatedCapacity}</p>
          </div>
          <div>
            <p className="text-[10px] uppercase text-muted-foreground">{dictionary.radar.feeBuffer}</p>
            <p className="mt-1 font-mono text-sm text-foreground">{opportunity.feeEstimate}</p>
          </div>
          <div>
            <p className="text-[10px] uppercase text-muted-foreground">{dictionary.radar.bookAge}</p>
            <p className="mt-1 font-mono text-sm text-foreground">{opportunity.bookAge}</p>
          </div>
        </div>
        {opportunity.validationReasonCodes.length > 0 ? (
          <div className="mt-3 flex flex-wrap gap-2">
            {opportunity.validationReasonCodes.map((reason) => (
              <StatusPill key={reason} tone={opportunity.validationTone}>
                {reason}
              </StatusPill>
            ))}
          </div>
        ) : null}
      </div>

      <div className="rounded-md bg-popover/70 p-4">
        <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
          {dictionary.radar.candidatePreview}
        </p>
        <div className="mt-3 flex items-center justify-between gap-3">
          <StatusPill tone={opportunity.candidateTone}>{opportunity.candidateLabel}</StatusPill>
          <p className="text-right text-xs text-muted-foreground">{opportunity.candidateReason}</p>
        </div>
      </div>

      <div className="rounded-md bg-popover/70 p-4">
        <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
          {dictionary.radar.detectionFormula}
        </p>
        <p className="mt-3 font-mono text-sm text-foreground">{opportunity.formula}</p>
      </div>

      <div className="rounded-md bg-popover/70 p-4">
        <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
          {dictionary.radar.reasonCodes}
        </p>
        <div className="mt-3 flex flex-wrap gap-2">
          {opportunity.reasonCodes.map((reason) => (
            <StatusPill key={reason} tone="neutral">
              {reason}
            </StatusPill>
          ))}
        </div>
      </div>
    </div>
  );
}

export function ArbitrageRadarWorkbench({ data }: ArbitrageRadarWorkbenchProps) {
  const arbitrageStream = useConsoleRealtimeChannel("arbitrage");
  const { dictionary, enumLabel, format } = useI18n();
  const [filter, setFilter] = useState<RadarFilter>("all");
  const [view, setView] = useState<RadarView>("active");
  const [selectedId, setSelectedId] = useState(data.selectedOpportunityId);
  const [liveOpportunities, setLiveOpportunities] = useState(data.opportunities);
  const [liveScans, setLiveScans] = useState(data.scans);
  const [liveAnalysis, setLiveAnalysis] = useState(data.analysis);
  const deferredFilter = useDeferredValue(filter);
  const filterButtons: Array<{ key: RadarFilter; label: string }> = [
    { key: "all", label: dictionary.radar.all },
    { key: "binary_buy_both", label: dictionary.radar.buyBoth },
    { key: "binary_sell_both", label: dictionary.radar.sellBoth },
  ];
  const viewButtons: Array<{ key: RadarView; label: string }> = [
    { key: "active", label: dictionary.radar.active },
    { key: "validated", label: dictionary.radar.validated },
    { key: "rejected", label: dictionary.radar.rejected },
    { key: "history", label: dictionary.radar.history },
  ];

  useEffect(() => {
    const streamEvent = arbitrageStream.lastEvent;

    if (!streamEvent) {
      return;
    }

    if (
      streamEvent.type === "arbitrage.opportunity.observed" ||
      streamEvent.type === "arbitrage.opportunity.repeated" ||
      streamEvent.type === "arbitrage.opportunity.expired"
    ) {
      startTransition(() => {
        setLiveOpportunities((current) =>
          upsertOpportunity(current, streamEvent.data, dictionary, enumLabel, format),
        );
        setSelectedId((current) => current || streamEvent.data.opportunity_id || "");
      });
      return;
    }

    if (
      streamEvent.type === "arbitrage.validation.passed" ||
      streamEvent.type === "arbitrage.validation.failed"
    ) {
      startTransition(() => {
        setLiveOpportunities((current) => patchValidation(current, streamEvent.data, dictionary, enumLabel));
      });
      return;
    }

    if (streamEvent.type === "arbitrage.scan.started" || streamEvent.type === "arbitrage.scan.completed") {
      startTransition(() => {
        setLiveScans((current) => upsertScan(current, streamEvent.data, dictionary));
      });
      return;
    }

    if (streamEvent.type === "arbitrage.analysis.generated") {
      const analysis = buildLiveAnalysis(streamEvent.data, enumLabel);
      if (analysis) {
        startTransition(() => {
          setLiveAnalysis(analysis);
        });
      }
    }
  }, [arbitrageStream.lastEvent, dictionary, enumLabel, format]);

  const metrics = useMemo(
    () => buildMetrics(liveOpportunities, liveScans, dictionary, format),
    [dictionary, format, liveOpportunities, liveScans],
  );

  const filteredOpportunities = useMemo(() => {
    return liveOpportunities
      .filter((opportunity) => viewMatches(view, opportunity))
      .filter((opportunity) => deferredFilter === "all" || opportunity.opportunityType === deferredFilter)
      .slice()
      .sort(compareRadarPriority);
  }, [liveOpportunities, deferredFilter, view]);

  const selectedOpportunity =
    filteredOpportunities.find((opportunity) => opportunity.id === selectedId) ??
    filteredOpportunities[0] ??
    liveOpportunities.find((opportunity) => opportunity.id === selectedId) ??
    liveOpportunities[0] ??
    null;

  function selectOpportunity(opportunityId: string) {
    startTransition(() => {
      setSelectedId(opportunityId);
    });
  }

  return (
    <div className="space-y-4">
      <PageHeader
        eyebrow={dictionary.radar.eyebrow}
        title={dictionary.radar.title}
        description={dictionary.radar.description}
        className="border-none pb-0"
        actions={
          <>
            <StatusPill tone={arbitrageStream.connection === "open" ? "success" : "warning"}>
              {arbitrageStream.connection}
            </StatusPill>
            <StatusPill tone="success">{format(dictionary.radar.observed, { count: liveOpportunities.length })}</StatusPill>
            <StatusPill tone="primary">{format(dictionary.radar.scans, { count: liveScans.length })}</StatusPill>
          </>
        }
      />

      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        {metrics.map((metric) => (
          <MetricCard
            key={metric.title}
            title={metric.title}
            value={metric.value}
            hint={metric.hint}
            accent={metric.accent}
          />
        ))}
      </div>

      <WorkbenchLayout columnsClassName="xl:grid-cols-[1.55fr_0.95fr]">
        <div className="overflow-hidden rounded-lg bg-card/95 ring-1 ring-white/5">
          <div className="flex flex-col gap-4 bg-popover/70 px-5 py-4 xl:flex-row xl:items-center xl:justify-between">
            <div className="flex items-center gap-3">
              <Radar className="size-5 text-primary" />
              <h2 className="font-heading text-xl font-bold tracking-tight text-foreground">
                {dictionary.radar.opportunities}
              </h2>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <WorkbenchSegmentedControl items={viewButtons} value={view} onChange={setView} />
              <WorkbenchSegmentedControl items={filterButtons} value={filter} onChange={setFilter} />
              <Button
                variant="outline"
                size="sm"
                className="rounded-sm border-white/10 bg-accent/40 text-foreground hover:bg-accent"
              >
                <Filter className="size-3.5" />
                {dictionary.common.filter}
              </Button>
            </div>
          </div>

          {filteredOpportunities.length > 0 ? (
            <div className="overflow-x-auto">
              <table className="w-full text-left">
                <thead className="bg-sidebar/60">
                  <tr className="text-[10px] font-bold uppercase tracking-[0.2em] text-muted-foreground">
                    <th className="px-5 py-3">{dictionary.radar.market}</th>
                    <th className="px-4 py-3">{dictionary.radar.type}</th>
                    <th className="px-4 py-3 text-right">{dictionary.radar.sum}</th>
                    <th className="px-4 py-3 text-right">{dictionary.radar.edge}</th>
                    <th className="px-4 py-3 text-right">{dictionary.radar.net}</th>
                    <th className="px-4 py-3 text-right">{dictionary.radar.capacity}</th>
                    <th className="px-4 py-3">{dictionary.radar.observedAt}</th>
                    <th className="px-4 py-3">{dictionary.radar.status}</th>
                    <th className="px-4 py-3">{dictionary.radar.validation}</th>
                    <th className="px-4 py-3">{dictionary.radar.candidate}</th>
                    <th className="px-5 py-3 text-right">{dictionary.radar.open}</th>
                  </tr>
                </thead>
                <tbody className="text-sm">
                  {filteredOpportunities.map((opportunity) => (
                    <tr
                      key={opportunity.id}
                      tabIndex={0}
                      onClick={() => selectOpportunity(opportunity.id)}
                      onKeyDown={(event) => {
                        if (isKeyboardSelect(event)) {
                          event.preventDefault();
                          selectOpportunity(opportunity.id);
                        }
                      }}
                      className={
                        opportunity.id === selectedOpportunity?.id
                          ? "cursor-pointer bg-accent/45 shadow-[inset_2px_0_0_#0066ff]"
                          : "cursor-pointer transition-colors hover:bg-accent/35"
                      }
                    >
                      <td className="px-5 py-3">
                        <div className="space-y-1">
                          <p className="max-w-[28rem] font-semibold text-foreground">
                            {opportunity.marketQuestion}
                          </p>
                          <p className="text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
                            {opportunity.contextLabel}
                          </p>
                        </div>
                      </td>
                      <td className="px-4 py-3">
                        <StatusPill tone={opportunity.typeTone}>{opportunity.typeLabel}</StatusPill>
                      </td>
                      <td className="px-4 py-3 text-right font-mono text-foreground">
                        {opportunity.priceSum}
                      </td>
                      <td className="px-4 py-3 text-right font-mono text-secondary">
                        {opportunity.grossEdge}
                      </td>
                      <td className="px-4 py-3 text-right font-mono text-secondary">
                        {opportunity.netEdge}
                      </td>
                      <td className="px-4 py-3 text-right font-mono">{opportunity.capacity}</td>
                      <td className="px-4 py-3 font-mono text-muted-foreground">
                        {opportunity.observedClock}
                      </td>
                      <td className="px-4 py-3">
                        <StatusPill tone={opportunity.statusTone}>{opportunity.statusLabel}</StatusPill>
                      </td>
                      <td className="px-4 py-3">
                        <StatusPill tone={opportunity.validationTone}>
                          {opportunity.validationLabel}
                        </StatusPill>
                      </td>
                      <td className="px-4 py-3">
                        <StatusPill tone={opportunity.candidateTone}>
                          {opportunity.candidateLabel}
                        </StatusPill>
                      </td>
                      <td className="px-5 py-3 text-right">
                        <button className="rounded-sm p-1 text-primary transition-colors hover:bg-primary/10">
                          <ChevronRight className="ml-auto size-4" />
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : (
            <div className="px-5 py-10 text-center">
              <p className="font-heading text-lg font-bold text-foreground">{dictionary.radar.noOpportunityTitle}</p>
              <p className="mt-2 text-sm text-muted-foreground">
                {dictionary.radar.noOpportunityDetail}
              </p>
            </div>
          )}
        </div>

        <WorkbenchDetailPane className="space-y-5">
          <OpportunityDetail opportunity={selectedOpportunity} />

          {liveAnalysis ? (
            <div className="space-y-4 rounded-md bg-popover/70 p-4">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <div>
                  <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
                    {dictionary.radar.analysis}
                  </p>
                  <p className="mt-1 text-sm text-foreground">
                    {liveAnalysis.generatedClock} / {liveAnalysis.lookbackHours}
                  </p>
                </div>
                <StatusPill tone="primary">{format(dictionary.metricHints.markets, { count: liveAnalysis.marketCount })}</StatusPill>
              </div>

              <div className="grid grid-cols-2 gap-3">
                {liveAnalysis.typeCounts.map((count) => (
                  <div key={count.typeLabel} className="rounded-md bg-accent/45 p-3">
                    <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
                      {count.typeLabel}
                    </p>
                    <p className="mt-2 font-mono text-lg text-foreground">{count.count}</p>
                  </div>
                ))}
              </div>

              <div className="space-y-3">
                {liveAnalysis.topMarkets.map((market) => (
                  <div key={market.marketId} className="rounded-md bg-accent/35 p-3">
                    <div className="flex items-start justify-between gap-3">
                      <p className="text-sm font-semibold text-foreground">{market.marketQuestion}</p>
                      <StatusPill tone="success">{market.maxGrossEdge}</StatusPill>
                    </div>
                    <div className="mt-3 grid grid-cols-3 gap-2 text-xs text-muted-foreground">
                      <span>{market.opportunityCount} {dictionary.radar.opps}</span>
                      <span>{market.maxCapacity} {dictionary.radar.cap}</span>
                      <span>{market.duration}</span>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          ) : null}

          <div className="rounded-md bg-popover/70 p-4">
            <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-muted-foreground">
              {dictionary.radar.scanHistory}
            </p>
            <div className="mt-3 space-y-3">
              {liveScans.map((scan) => (
                <div key={scan.id} className="rounded-md bg-accent/35 p-3">
                  <div className="flex items-center justify-between gap-3">
                    <p className="font-mono text-xs text-foreground">{scan.startedClock}</p>
                    <StatusPill tone={scan.opportunityCount === "0" ? "neutral" : "success"}>
                      {scan.opportunityCount} {dictionary.radar.opps}
                    </StatusPill>
                  </div>
                  <p className="mt-2 text-xs text-muted-foreground">
                    {format(dictionary.metricHints.markets, { count: scan.marketCount })} / {scan.snapshotCount} {dictionary.radar.snapshots} / {scan.scannerVersion}
                  </p>
                </div>
              ))}
            </div>
          </div>
        </WorkbenchDetailPane>
      </WorkbenchLayout>
    </div>
  );
}
