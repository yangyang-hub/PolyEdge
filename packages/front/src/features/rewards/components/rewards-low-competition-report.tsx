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
  formatUsdFixed,
  toFiniteNumber,
} from "@/lib/formatters";
import { dictionary } from "@/lib/i18n/dictionaries";

export function LowCompetitionShadowReport({ snapshot }: { snapshot: RewardBotSnapshotDto }) {
  const report = snapshot.low_competition_report;
  if (!report || (snapshot.config.low_competition_mode === "off" && report.observations === 0)) {
    return null;
  }

  const ready = report.should_consider_enforce;

  return (
    <Card>
      <CardHeader className="border-b border-border/70">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div className="min-w-0">
            <CardTitle className="flex items-center gap-2">
              <BarChart3 className="size-4 text-secondary" />
              {dictionary.rewards.lowCompetitionShadowReport}
            </CardTitle>
            <CardDescription>{dictionary.rewards.lowCompetitionShadowReportDescription}</CardDescription>
          </div>
          <StatusPill tone={ready ? "success" : "warning"}>
            {ready
              ? dictionary.rewards.lowCompetitionReportReady
              : dictionary.rewards.lowCompetitionReportNotReady}
          </StatusPill>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4 2xl:grid-cols-6">
          <ReportMetric
            label={dictionary.rewards.lowCompetitionReportObservationWindow}
            value={`${report.window_hours}h`}
            hint={formatOptionalClock(report.latest_observed_at)}
          />
          <ReportMetric
            label={dictionary.rewards.lowCompetitionObservedMarkets}
            value={`${report.unique_markets}/${report.observations}`}
            hint={formatOptionalClock(report.generated_at)}
          />
          <RatioMetric
            label={dictionary.rewards.lowCompetitionGatePass}
            value={report.gate_pass_ratio}
            count={report.gate_pass_count}
            total={report.observations}
            tone="success"
          />
          <RatioMetric
            label={dictionary.rewards.lowCompetitionFinalPass}
            value={report.final_pass_ratio}
            count={report.final_pass_count}
            total={report.observations}
            tone="success"
          />
          <ReportMetric
            label={dictionary.rewards.lowCompetitionCompetitionShareMedian}
            value={formatOptionalBps(report.competition_share_bps_median)}
            hint={dictionary.rewards.competitionShare}
          />
          <ReportMetric
            label={dictionary.rewards.lowCompetitionAccountAllocationP90}
            value={formatOptionalBps(report.account_allocation_bps_p90)}
            hint={dictionary.rewards.accountAllocation}
          />
          <ReportMetric
            label={dictionary.rewards.lowCompetitionMarketAllocationP90}
            value={formatOptionalBps(report.market_allocation_bps_p90)}
            hint={dictionary.rewards.marketAllocation}
          />
          <ReportMetric
            label={dictionary.rewards.lowCompetitionRewardMedian}
            value={formatOptionalUsd(report.estimated_reward_per_100_usd_day_median)}
            hint={`${dictionary.rewards.lowCompetitionRewardP90}: ${formatOptionalUsd(report.estimated_reward_per_100_usd_day_p90)}`}
          />
          <ReportMetric
            label={dictionary.rewards.lowCompetitionExitMultiple}
            value={formatOptionalMultiple(report.exit_depth_multiple_median)}
            hint={`${dictionary.rewards.lowCompetitionExitSlippageP95}: ${formatOptionalCents(report.exit_slippage_cents_p95)}`}
          />
          <ReportMetric
            label={dictionary.rewards.lowCompetitionMidpointP95}
            value={formatOptionalCents(report.midpoint_range_cents_p95)}
            hint={dictionary.rewards.midpointRange}
          />
          <RatioMetric
            label={dictionary.rewards.lowCompetitionSampleInsufficient}
            value={report.sample_insufficient_ratio}
            count={report.sample_insufficient_count}
            total={report.observations}
            tone={report.sample_insufficient_count > 0 ? "warning" : "neutral"}
          />
          <RatioMetric
            label={dictionary.rewards.lowCompetitionAiBlocked}
            value={report.ai_blocked_ratio}
            count={report.ai_blocked_count}
            total={report.observations}
            tone={report.ai_blocked_count > 0 ? "warning" : "neutral"}
          />
          <RatioMetric
            label={dictionary.rewards.lowCompetitionInfoRiskBlocked}
            value={report.info_risk_blocked_ratio}
            count={report.info_risk_blocked_count}
            total={report.observations}
            tone={report.info_risk_blocked_count > 0 ? "warning" : "neutral"}
          />
          <RatioMetric
            label={dictionary.rewards.lowCompetitionStandardOverlap}
            value={report.standard_overlap_ratio}
            count={report.standard_overlap_count}
            total={report.observations}
            tone="neutral"
          />
        </div>

        <div className="rounded-lg border border-border/70 bg-muted/20 p-3 text-xs leading-5 text-muted-foreground">
          <p className="mb-2 font-medium text-foreground">
            {dictionary.rewards.lowCompetitionRecommendation}
          </p>
          <TruncateText
            text={
              report.recommendation_reasons.length > 0
                ? report.recommendation_reasons.join("；")
                : dictionary.rewards.lowCompetitionNoRecommendation
            }
            lines={3}
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
}: {
  label: string;
  value: string | number;
  count: number;
  total: number;
  tone: "success" | "warning" | "neutral";
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
    </div>
  );
}

function ReportMetric({ label, value, hint }: { label: string; value: string; hint: string }) {
  return (
    <div className="min-w-0 rounded-lg border border-border/70 bg-background/30 p-3">
      <p className="break-words text-[11px] font-semibold uppercase leading-4 text-muted-foreground">
        {label}
      </p>
      <p className="mt-2 break-words font-mono text-xl font-semibold leading-tight text-foreground">
        {value}
      </p>
      <p className="mt-1 break-words text-xs leading-4 text-muted-foreground">{hint}</p>
    </div>
  );
}

function formatOptionalUsd(value: string | number | null | undefined) {
  return value == null ? "n/a" : formatUsdFixed(value);
}

function formatOptionalCents(value: string | number | null | undefined) {
  return value == null ? "n/a" : `${formatFixed(value, 2)}c`;
}

function formatOptionalBps(value: string | number | null | undefined) {
  return value == null ? "n/a" : `${formatFixed(toFiniteNumber(value) / 100, 2)}%`;
}

function formatOptionalMultiple(value: string | number | null | undefined) {
  return value == null ? "n/a" : `${formatFixed(value, 2)}x`;
}

function clamp01(value: number) {
  return Math.max(0, Math.min(1, value));
}
