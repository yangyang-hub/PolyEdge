"use client";

import { StatusPill } from "@/components/shared/status-pill";
import type { RewardQuotePlanDto } from "@/lib/contracts/dto";
import { formatFixed, formatUsdFixed, toFiniteNumber } from "@/lib/formatters";
import { dictionary } from "@/lib/i18n/dictionaries";

export function LowCompetitionSummary({ plan }: { plan: RewardQuotePlanDto }) {
  if (plan.strategy_bucket !== "low_competition") {
    return null;
  }
  const metrics = plan.low_competition_metrics;

  return (
    <div className="mt-1 space-y-1 text-[11px] leading-4 text-muted-foreground">
      <StatusPill tone={metrics?.eligible_for_low_competition ? "success" : "warning"}>
        {dictionary.rewards.lowCompetition}
      </StatusPill>
      {metrics ? (
        <div className="grid gap-x-2 gap-y-0.5 font-mono sm:grid-cols-2">
          <span>
            {dictionary.rewards.competition}: {formatUsdFixed(metrics.qualified_competition_usd)}
          </span>
          <span>
            {dictionary.rewards.competitionShare}: {formatBps(metrics.competition_share_bps)}
          </span>
          <span>
            {dictionary.rewards.accountAllocation}: {formatBps(metrics.account_allocation_bps)}
          </span>
          <span>
            {dictionary.rewards.marketAllocation}: {formatBps(metrics.market_allocation_bps)}
          </span>
          <span>
            {dictionary.rewards.rewardPer100}: {formatUsdFixed(metrics.estimated_reward_per_100_usd_day)}
          </span>
          <span>
            {dictionary.rewards.exitDepth}: {formatUsdFixed(metrics.exit_depth_usd)}
          </span>
          <span>
            {dictionary.rewards.samples}: {metrics.sample_count}
          </span>
          {metrics.midpoint_range_cents == null ? null : (
            <span>
              {dictionary.rewards.midpointRange}: {formatFixed(metrics.midpoint_range_cents, 2)}c
            </span>
          )}
        </div>
      ) : null}
    </div>
  );
}

function formatBps(value: string | number) {
  return `${formatFixed(toFiniteNumber(value) / 100, 2)}%`;
}
