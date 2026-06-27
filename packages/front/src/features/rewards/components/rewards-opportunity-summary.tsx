"use client";

import { StatusPill } from "@/components/shared/status-pill";
import type { RewardQuotePlanDto } from "@/lib/contracts/dto";
import { formatFixed, formatUsdFixed } from "@/lib/formatters";
import { dictionary } from "@/lib/i18n/dictionaries";

export function OpportunitySummary({ plan }: { plan: RewardQuotePlanDto }) {
  const metrics = plan.opportunity_metrics;
  if (!metrics) return null;

  const adjustment = Number(metrics.score_adjustment);
  const tone = adjustment > 0 ? "success" : adjustment < 0 ? "warning" : "neutral";

  return (
    <div className="mt-2 space-y-1 text-[11px] leading-4 text-muted-foreground">
      <div className="flex flex-wrap items-center gap-1">
        <StatusPill tone={tone}>
          {dictionary.rewards.opportunityScore} {formatFixed(metrics.opportunity_score, 0)}
        </StatusPill>
        <span className="font-mono">
          {dictionary.rewards.opportunityScoreAdjustment} {formatFixed(metrics.score_adjustment, 1)}
        </span>
      </div>
      <div className="grid grid-cols-[auto_1fr] gap-x-2 gap-y-0.5 font-mono">
        <span>{dictionary.rewards.competition}</span>
        <span>{formatFixed(metrics.competition_multiple, 2)}x</span>
        <span>{dictionary.rewards.rewardPer100}</span>
        <span>{formatUsdFixed(metrics.estimated_reward_per_100_usd_day)}</span>
        <span>{dictionary.rewards.exitDepth}</span>
        <span>{formatUsdFixed(metrics.exit_depth_usd)}</span>
        <span>{dictionary.rewards.samples}</span>
        <span>{metrics.sample_count}</span>
      </div>
      {metrics.warnings.length > 0 ? (
        <div className="text-amber-500">
          {dictionary.rewards.blocked}: {metrics.warnings.length}
        </div>
      ) : null}
    </div>
  );
}
