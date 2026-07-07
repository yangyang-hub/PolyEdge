"use client";

import { StatusPill } from "@/components/shared/status-pill";
import { TruncateText } from "@/components/shared/truncate-text";
import type {
  RewardMarketMakerDecisionStatus,
  RewardQuotePlanDto,
} from "@/lib/contracts/dto";
import { formatFixed } from "@/lib/formatters";
import { dictionary } from "@/lib/i18n/dictionaries";

function marketMakerTone(status: RewardMarketMakerDecisionStatus) {
  if (status === "allowed" || status === "shadow_allowed") return "success";
  if (status === "blocked") return "danger";
  return "warning";
}

function marketMakerStatusLabel(status: RewardMarketMakerDecisionStatus) {
  if (status === "allowed") return dictionary.rewards.marketMakerAllowed;
  if (status === "blocked") return dictionary.rewards.marketMakerBlocked;
  if (status === "shadow_allowed") return dictionary.rewards.marketMakerShadowAllowed;
  return dictionary.rewards.marketMakerShadowBlocked;
}

export function MarketMakerSummary({ plan }: { plan: RewardQuotePlanDto }) {
  const metrics = plan.market_maker;
  if (metrics == null) return null;

  const fairValue = metrics.fair_value;
  const reason = metrics.reason_codes[0] ?? metrics.decisions[0]?.reason_codes[0] ?? null;

  return (
    <div className="mt-2 space-y-1 text-[11px] leading-4 text-muted-foreground">
      <div className="flex flex-wrap items-center gap-1">
        <StatusPill tone={marketMakerTone(metrics.decision_status)}>
          {marketMakerStatusLabel(metrics.decision_status)}
        </StatusPill>
        <span className="font-mono">
          {dictionary.rewards.marketMakerTotalEv}{" "}
          {metrics.best_total_ev_cents == null
            ? dictionary.rewards.notAvailable
            : `${formatFixed(metrics.best_total_ev_cents, 2)}c`}
        </span>
      </div>
      <div className="grid grid-cols-[auto_1fr] gap-x-2 gap-y-0.5 font-mono">
        <span>{dictionary.rewards.marketMakerPricingEdge}</span>
        <span>
          {metrics.best_pricing_edge_cents == null
            ? dictionary.rewards.notAvailable
            : `${formatFixed(metrics.best_pricing_edge_cents, 2)}c`}
        </span>
        <span>{dictionary.rewards.marketMakerRewardEv}</span>
        <span>
          {metrics.best_reward_ev_cents == null
            ? dictionary.rewards.notAvailable
            : `${formatFixed(metrics.best_reward_ev_cents, 2)}c`}
        </span>
        <span>{dictionary.rewards.marketMakerFairValue}</span>
        <span>
          {fairValue == null
            ? dictionary.rewards.notAvailable
            : `${formatFixed(fairValue.fair_yes_low, 3)}-${formatFixed(
                fairValue.fair_yes_high,
                3,
              )} / ${formatFixed(fairValue.confidence, 2)}`}
        </span>
      </div>
      {reason != null && (
        <TruncateText
          text={reason}
          lines={1}
          className="font-mono text-[11px] leading-4 text-muted-foreground"
        />
      )}
    </div>
  );
}
