"use client";

import { BarChart3 } from "lucide-react";

import { MeterBar } from "@/components/shared/meter-bar";
import { StatusPill } from "@/components/shared/status-pill";
import { TruncateText } from "@/components/shared/truncate-text";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import type { RewardBotSnapshotDto } from "@/lib/contracts/dto";
import {
  formatFixed,
  formatOptionalClock,
  toFiniteNumber,
} from "@/lib/formatters";
import { dictionary } from "@/lib/i18n/dictionaries";

import {
  formatLowCompetitionReportReason,
  formatOptionalBps,
  formatOptionalCents,
  formatOptionalMultiple,
  formatOptionalUsd,
} from "../lib/low-competition-formatters";

export function LowCompetitionShadowReport({ snapshot }: { snapshot: RewardBotSnapshotDto }) {
  const report = snapshot.low_competition_report;
  if (!report || (snapshot.config.low_competition_mode === "off" && report.observations === 0)) {
    return null;
  }

  const ready = report.should_consider_enforce;
  const t = dictionary.rewards;
  const config = snapshot.config;
  const recommendation = report.recommendation_reasons.length > 0
    ? report.recommendation_reasons.map(formatLowCompetitionReportReason).join("；")
    : t.lowCompetitionNoRecommendation;

  return (
    <Card>
      <CardHeader className="border-b border-border/70">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div className="min-w-0">
            <CardTitle className="flex items-center gap-2">
              <BarChart3 className="size-4 text-secondary" />
              {t.lowCompetitionShadowReport}
            </CardTitle>
            <CardDescription>{t.lowCompetitionShadowReportDescription}</CardDescription>
          </div>
          <StatusPill tone={ready ? "success" : "warning"}>
            {ready
              ? t.lowCompetitionReportReady
              : t.lowCompetitionReportNotReady}
          </StatusPill>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="grid gap-3 lg:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)]">
          <ReportMetric
            label={t.lowCompetitionReportStrategyStatus}
            value={ready ? t.lowCompetitionReportReady : t.lowCompetitionReportNotReady}
            hint={`${t.lowCompetitionReportObservationWindow}: ${report.window_hours}h · ${t.lowCompetitionLatestObserved}: ${formatOptionalClock(report.latest_observed_at)}`}
            target={`${t.lowCompetitionObservedMarkets}: ${report.unique_markets}/${report.observations}`}
          />
          <div className="rounded-lg border border-border/70 bg-muted/20 p-3 text-xs leading-5 text-muted-foreground">
            <p className="mb-2 font-medium text-foreground">
              {t.lowCompetitionRecommendation}
            </p>
            <TruncateText text={recommendation} lines={3} />
          </div>
        </div>

        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4 2xl:grid-cols-6">
          <ReportMetric
            label={t.lowCompetitionObservedMarkets}
            value={`${report.unique_markets}/${report.observations}`}
            hint={`${t.lowCompetitionGeneratedAt}: ${formatOptionalClock(report.generated_at)}`}
          />
          <RatioMetric
            label={t.lowCompetitionGatePass}
            value={report.gate_pass_ratio}
            count={report.gate_pass_count}
            total={report.observations}
            tone="success"
            target={`${t.thresholdAtLeast} 40%`}
          />
          <RatioMetric
            label={t.lowCompetitionFinalPass}
            value={report.final_pass_ratio}
            count={report.final_pass_count}
            total={report.observations}
            tone="success"
            target={t.lowCompetitionFinalPassHint}
          />
          <ReportMetric
            label={t.lowCompetitionCompetitionShareMedian}
            value={formatOptionalBps(report.competition_share_bps_median)}
            hint={t.lowCompetitionProbeShareHint}
            target={minimumBpsTarget(config.low_competition_min_competition_share_bps)}
          />
          <ReportMetric
            label={t.lowCompetitionAccountAllocationP90}
            value={formatOptionalBps(report.account_allocation_bps_p90)}
            hint={t.lowCompetitionAccountAllocationP90Hint}
            target={allocationTarget(config.low_competition_max_account_allocation_bps)}
          />
          <ReportMetric
            label={t.lowCompetitionMarketAllocationP90}
            value={formatOptionalBps(report.market_allocation_bps_p90)}
            hint={t.lowCompetitionMarketAllocationP90Hint}
            target={allocationTarget(config.low_competition_max_market_allocation_bps)}
          />
          <ReportMetric
            label={t.lowCompetitionRewardMedian}
            value={formatOptionalUsd(report.estimated_reward_per_100_usd_day_median)}
            hint={`${t.lowCompetitionRewardHigh}: ${formatOptionalUsd(report.estimated_reward_per_100_usd_day_p90)}`}
            target={`${t.thresholdAtLeast} ${formatOptionalUsd(config.low_competition_min_reward_per_100_usd_day)}`}
          />
          <ReportMetric
            label={t.lowCompetitionExitMultiple}
            value={formatOptionalMultiple(report.exit_depth_multiple_median)}
            hint={`${t.lowCompetitionExitSlippageHigh}: ${formatOptionalCents(report.exit_slippage_cents_p95)}`}
            target={`${t.thresholdAtLeast} ${formatOptionalMultiple(config.low_competition_min_exit_depth_multiple)}`}
          />
          <ReportMetric
            label={t.lowCompetitionMidpointP95}
            value={formatOptionalCents(report.midpoint_range_cents_p95)}
            hint={t.lowCompetitionMidpointP95Hint}
            target={`${t.thresholdAtMost} ${formatOptionalCents(config.low_competition_max_midpoint_range_cents)}`}
          />
          <RatioMetric
            label={t.lowCompetitionSampleInsufficient}
            value={report.sample_insufficient_ratio}
            count={report.sample_insufficient_count}
            total={report.observations}
            tone={report.sample_insufficient_count > 0 ? "warning" : "neutral"}
            target={`${t.thresholdAtMost} 20%`}
          />
          <RatioMetric
            label={t.lowCompetitionAiBlocked}
            value={report.ai_blocked_ratio}
            count={report.ai_blocked_count}
            total={report.observations}
            tone={report.ai_blocked_count > 0 ? "warning" : "neutral"}
            target={`${t.thresholdAtMost} 40%`}
          />
          <RatioMetric
            label={t.lowCompetitionInfoRiskBlocked}
            value={report.info_risk_blocked_ratio}
            count={report.info_risk_blocked_count}
            total={report.observations}
            tone={report.info_risk_blocked_count > 0 ? "warning" : "neutral"}
            target={`${t.thresholdAtMost} 20%`}
          />
          <RatioMetric
            label={t.lowCompetitionStandardOverlap}
            value={report.standard_overlap_ratio}
            count={report.standard_overlap_count}
            total={report.observations}
            tone="neutral"
            target={t.lowCompetitionStandardOverlapHint}
          />
        </div>
      </CardContent>
    </Card>
  );
}

function RatioMetric({
  label,
  value,
  count,
  total,
  tone,
  target,
}: {
  label: string;
  value: string | number;
  count: number;
  total: number;
  tone: "success" | "warning" | "neutral";
  target?: string;
}) {
  const ratio = clamp01(toFiniteNumber(value));

  return (
    <div className="min-w-0 rounded-lg border border-border/70 bg-background/30 p-3">
      <p className="break-words text-[11px] font-semibold uppercase leading-4 text-muted-foreground">
        {label}
      </p>
      <p className="mt-2 font-mono text-xl font-semibold leading-tight text-foreground">
        {formatFixed(ratio * 100, 0)}%
      </p>
      <div className="mt-2">
        <MeterBar value={`${Math.round(ratio * 100)}%`} tone={ratio > 0 ? tone : "neutral"} />
      </div>
      <p className="mt-2 text-xs leading-4 text-muted-foreground">
        {count}/{total}
      </p>
      {target ? <p className="mt-1 break-words text-[11px] leading-4 text-muted-foreground">{target}</p> : null}
    </div>
  );
}

function ReportMetric({
  label,
  value,
  hint,
  target,
}: {
  label: string;
  value: string;
  hint: string;
  target?: string;
}) {
  return (
    <div className="min-w-0 rounded-lg border border-border/70 bg-background/30 p-3">
      <p className="break-words text-[11px] font-semibold uppercase leading-4 text-muted-foreground">
        {label}
      </p>
      <p className="mt-2 break-words font-mono text-xl font-semibold leading-tight text-foreground">
        {value}
      </p>
      <p className="mt-1 break-words text-xs leading-4 text-muted-foreground">{hint}</p>
      {target ? <p className="mt-1 break-words text-[11px] leading-4 text-muted-foreground">{target}</p> : null}
    </div>
  );
}

function clamp01(value: number) {
  return Math.max(0, Math.min(1, value));
}

function allocationTarget(capBps: number) {
  return capBps > 0
    ? `${dictionary.rewards.thresholdAtMost} ${formatOptionalBps(capBps)}`
    : dictionary.rewards.thresholdNoLimit;
}

function minimumBpsTarget(minBps: number) {
  return minBps > 0
    ? `${dictionary.rewards.thresholdAtLeast} ${formatOptionalBps(minBps)}`
    : dictionary.rewards.thresholdNoLimit;
}
