"use client";

import { StatusPill } from "@/components/shared/status-pill";
import type { RewardBotConfigDto, RewardQuotePlanDto } from "@/lib/contracts/dto";
import { formatFixed, formatUsdFixed, toFiniteNumber } from "@/lib/formatters";
import { dictionary } from "@/lib/i18n/dictionaries";

import { formatBps, formatLowCompetitionReason } from "../lib/low-competition-formatters";

export function LowCompetitionSummary({
  plan,
  config,
}: {
  plan: RewardQuotePlanDto;
  config: RewardBotConfigDto;
}) {
  if (plan.strategy_bucket !== "low_competition") {
    return null;
  }
  const metrics = plan.low_competition_metrics;
  const t = dictionary.rewards;

  if (!metrics) {
    return (
      <div className="mt-1 text-[11px] leading-4 text-muted-foreground">
        <StatusPill tone="warning">{t.lowCompetitionWaitingMetrics}</StatusPill>
      </div>
    );
  }

  const checks = buildLowCompetitionChecks(metrics, config);
  const firstReason = metrics.rejection_reasons[0]
    ? formatLowCompetitionReason(metrics.rejection_reasons[0])
    : null;

  return (
    <div className="mt-2 min-w-[250px] space-y-2 text-[11px] leading-4 text-muted-foreground">
      <div className="flex flex-wrap items-center gap-1.5">
        <StatusPill tone={metrics.eligible_for_low_competition ? "success" : "warning"}>
          {metrics.eligible_for_low_competition
            ? t.lowCompetitionGatePassed
            : t.lowCompetitionGateBlocked}
        </StatusPill>
        <span className="font-mono text-[11px] text-muted-foreground">
          {t.lowCompetitionProbeAmount}: {formatUsdFixed(metrics.competition_probe_notional_usd, 0)}
        </span>
      </div>
      <div className="grid gap-1">
        {checks.map((check) => (
          <LowCompetitionCheckRow key={check.label} {...check} />
        ))}
      </div>
      {firstReason ? (
        <p className="break-words text-[11px] leading-4 text-muted-foreground">
          <span className="font-medium text-foreground">{t.lowCompetitionBlockedBy}: </span>
          {firstReason}
        </p>
      ) : null}
    </div>
  );
}

type LowCompetitionCheck = {
  label: string;
  value: string;
  target: string;
  pass: boolean;
  detail?: string;
};

function LowCompetitionCheckRow({ label, value, target, pass, detail }: LowCompetitionCheck) {
  const t = dictionary.rewards;
  return (
    <div className="rounded-md border border-border/70 bg-muted/20 px-2 py-1.5">
      <div className="flex items-start justify-between gap-2">
        <span className="break-words font-medium text-foreground">{label}</span>
        <StatusPill
          tone={pass ? "success" : "warning"}
          className="shrink-0 px-1.5 py-0 text-[9px]"
        >
          {pass ? t.lowCompetitionCheckPass : t.lowCompetitionCheckFail}
        </StatusPill>
      </div>
      <div className="mt-1 flex flex-wrap items-baseline justify-between gap-x-2 gap-y-0.5 font-mono">
        <span className="text-foreground">{value}</span>
        <span>{target}</span>
      </div>
      {detail ? <p className="mt-0.5 break-words text-[11px] leading-4">{detail}</p> : null}
    </div>
  );
}

function buildLowCompetitionChecks(
  metrics: NonNullable<RewardQuotePlanDto["low_competition_metrics"]>,
  config: RewardBotConfigDto,
): LowCompetitionCheck[] {
  const t = dictionary.rewards;
  const externalCompetitionLimit = lowCompetitionExternalCompetitionLimit(metrics, config);
  const requiredExitDepth = requiredLowCompetitionExitDepth(metrics, config);
  const midpointRange = metrics.midpoint_range_cents;
  const midpointRangeLimit = toFiniteNumber(config.low_competition_max_midpoint_range_cents);
  const sampleTarget = config.low_competition_min_book_samples;

  return [
    {
      label: t.lowCompetitionProbeShare,
      value: formatBps(metrics.competition_share_bps),
      target:
        config.low_competition_min_competition_share_bps > 0
          ? `${t.thresholdAtLeast} ${formatBps(config.low_competition_min_competition_share_bps)}`
          : t.thresholdNoLimit,
      pass:
        config.low_competition_min_competition_share_bps <= 0
        || toFiniteNumber(metrics.competition_share_bps)
          >= config.low_competition_min_competition_share_bps,
      detail: `${t.lowCompetitionExternalCompetition}: ${formatUsdFixed(metrics.qualified_competition_usd)}`,
    },
    {
      label: t.lowCompetitionExternalCompetition,
      value: formatUsdFixed(metrics.qualified_competition_usd),
      target:
        externalCompetitionLimit == null
          ? t.thresholdNoLimit
          : `${t.thresholdAtMost} ${formatUsdFixed(externalCompetitionLimit)}`,
      pass:
        externalCompetitionLimit == null
        || toFiniteNumber(metrics.qualified_competition_usd) <= externalCompetitionLimit,
      detail: `${t.lowCompetitionCompetitionMultiple}: ${formatFixed(metrics.competition_multiple, 2)}x`,
    },
    {
      label: t.lowCompetitionBudgetUse,
      value: formatBps(metrics.account_allocation_bps),
      target: allocationTarget(config.low_competition_max_account_allocation_bps),
      pass:
        config.low_competition_max_account_allocation_bps <= 0
        || toFiniteNumber(metrics.account_allocation_bps)
          <= config.low_competition_max_account_allocation_bps,
      detail: `${t.lowCompetitionAfterPlan}: ${formatUsdFixed(metrics.low_competition_open_buy_notional_usd_after_plan)} / ${t.lowCompetitionWalletBase}: ${formatUsdFixed(metrics.account_effective_available_usd)}`,
    },
    {
      label: t.lowCompetitionMarketBudgetUse,
      value: formatBps(metrics.market_allocation_bps),
      target: allocationTarget(config.low_competition_max_market_allocation_bps),
      pass:
        config.low_competition_max_market_allocation_bps <= 0
        || toFiniteNumber(metrics.market_allocation_bps)
          <= config.low_competition_max_market_allocation_bps,
      detail: `${t.lowCompetitionAfterPlan}: ${formatUsdFixed(metrics.condition_buy_notional_usd_after_plan)}`,
    },
    {
      label: t.lowCompetitionEstimatedReward,
      value: formatUsdFixed(metrics.estimated_reward_per_100_usd_day),
      target: `${t.thresholdAtLeast} ${formatUsdFixed(config.low_competition_min_reward_per_100_usd_day)}`,
      pass:
        toFiniteNumber(metrics.estimated_reward_per_100_usd_day)
        >= toFiniteNumber(config.low_competition_min_reward_per_100_usd_day),
    },
    {
      label: t.lowCompetitionExitProtection,
      value: formatUsdFixed(metrics.exit_depth_usd),
      target: `${t.thresholdAtLeast} ${formatUsdFixed(requiredExitDepth)}`,
      pass:
        toFiniteNumber(metrics.exit_depth_usd) >= requiredExitDepth
        && metrics.exit_slippage_cents != null,
      detail:
        metrics.exit_slippage_cents == null
          ? t.lowCompetitionExitSlippageUnavailable
          : `${t.lowCompetitionExitSlippage}: ${formatFixed(metrics.exit_slippage_cents, 2)}c`,
    },
    {
      label: t.lowCompetitionBookStability,
      value: `${metrics.sample_count}/${sampleTarget}`,
      target: `${t.lowCompetitionMidpointRangeLimit} ${formatFixed(midpointRangeLimit, 2)}c`,
      pass:
        metrics.sample_count >= sampleTarget
        && midpointRange != null
        && toFiniteNumber(midpointRange) <= midpointRangeLimit,
      detail:
        midpointRange == null
          ? t.lowCompetitionMidpointUnavailable
          : `${t.midpointRange}: ${formatFixed(midpointRange, 2)}c`,
    },
  ];
}

function allocationTarget(capBps: number) {
  return capBps > 0
    ? `${dictionary.rewards.thresholdAtMost} ${formatBps(capBps)}`
    : dictionary.rewards.thresholdNoLimit;
}

function lowCompetitionExternalCompetitionLimit(
  metrics: NonNullable<RewardQuotePlanDto["low_competition_metrics"]>,
  config: RewardBotConfigDto,
) {
  const limits: number[] = [];
  const competitionMultiple = toFiniteNumber(config.low_competition_max_competition_multiple);
  if (competitionMultiple > 0) {
    limits.push(toFiniteNumber(metrics.competition_probe_notional_usd) * competitionMultiple);
  }
  const absoluteCompetitionCap = toFiniteNumber(config.low_competition_max_competition_usd);
  if (absoluteCompetitionCap > 0) {
    limits.push(absoluteCompetitionCap);
  }
  return limits.length > 0 ? Math.min(...limits) : null;
}

function requiredLowCompetitionExitDepth(
  metrics: NonNullable<RewardQuotePlanDto["low_competition_metrics"]>,
  config: RewardBotConfigDto,
) {
  return Math.max(
    toFiniteNumber(config.low_competition_min_exit_depth_usd),
    toFiniteNumber(metrics.planned_notional_usd)
      * toFiniteNumber(config.low_competition_min_exit_depth_multiple),
  );
}
