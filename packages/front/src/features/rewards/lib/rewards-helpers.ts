import type {
  DecimalValue,
  ManagedRewardOrderDto,
  RewardMarketAdvisoryDto,
  RewardPlanQuoteMode,
  RewardQuotePlanDto,
} from "@/lib/contracts/dto";
import { dictionary } from "@/lib/i18n/dictionaries";
import type { EventCategory } from "../types";

export function eventCategory(eventType: string): Exclude<EventCategory, "all"> | null {
  // Placements: intent persisted + order accepted on exchange
  if (
    eventType === "reward_order_placed" ||
    eventType === "reward_exit_placed" ||
    eventType === "reward_live_order_placed" ||
    eventType === "reward_live_exit_order_placed" ||
    eventType === "reward_live_order_planned" ||
    eventType === "reward_live_adaptive_exit_selected" ||
    eventType === "reward_live_adaptive_exit_reselected" ||
    eventType === "reward_live_adaptive_exit_reselect_deferred" ||
    eventType === "reward_live_adaptive_exit_reselect_limit_reached" ||
    eventType === "reward_live_exit_planned" ||
    eventType === "reward_live_hold_requote_exit_planned" ||
    eventType === "reward_live_flatten_planned" ||
    eventType === "reward_live_order_rejected" ||
    eventType === "reward_live_exit_order_rejected" ||
    eventType === "reward_live_order_post_only_violation" ||
    eventType === "reward_live_order_submission_recovered" ||
    eventType === "reward_live_order_submission_unknown" ||
    eventType === "reward_live_order_submission_recovery_failed" ||
    eventType === "reward_live_order_submission_failed_before_post"
  ) {
    return "placements";
  }
  // Cancels: order cancellation initiated / completed / failed
  if (
    eventType === "reward_order_cancelled" ||
    eventType === "reward_live_order_cancelled" ||
    eventType === "reward_live_order_status_cancelled" ||
    eventType === "reward_live_order_cancel_pending" ||
    eventType === "reward_live_order_cancel_rejected" ||
    eventType === "reward_live_order_cancel_unknown" ||
    eventType === "reward_live_order_cancel_blocked_unknown_submission" ||
    eventType === "reward_live_order_cancel_retry_required" ||
    eventType === "reward_live_order_post_only_violation_cancel_pending" ||
    eventType === "reward_live_order_post_only_violation_cancel_rejected" ||
    eventType === "reward_live_order_post_only_violation_cancel_unknown"
  ) {
    return "cancels";
  }
  // Fills: confirmed trade executions and fill-related lifecycle events
  if (
    eventType === "reward_order_filled" ||
    eventType === "reward_exit_filled" ||
    eventType === "reward_position_flattened" ||
    eventType === "reward_live_order_filled" ||
    eventType === "reward_live_order_status_terminal_match" ||
    eventType === "reward_live_exit_retry_deferred" ||
    eventType === "reward_live_flatten_deferred"
  ) {
    return "fills";
  }
  // Rewards: earnings synced from Polymarket
  if (
    eventType === "reward_accrued" ||
    eventType === "reward_live_reward_earnings_synced"
  ) {
    return "rewards";
  }
  return null;
}

export function rewardTone(status: ManagedRewardOrderDto["status"]) {
  if (status === "open") {
    return "success" as const;
  }
  if (status === "exit_pending") {
    return "warning" as const;
  }
  if (status === "error") {
    return "danger" as const;
  }
  if (status === "cancelled") {
    return "neutral" as const;
  }
  return "warning" as const;
}

export function quoteReadinessTone(plan: RewardQuotePlanDto) {
  if (plan.quote_readiness === "ready_to_quote") return "success" as const;
  if (plan.quote_readiness === "provider_pending") return "neutral" as const;
  return plan.eligible ? ("success" as const) : ("warning" as const);
}

export function quoteReadinessLabel(plan: RewardQuotePlanDto) {
  if (plan.quote_readiness === "ready_to_quote") return dictionary.rewards.readyToQuote;
  if (plan.quote_readiness === "waiting_orderbook") return dictionary.rewards.waitingOrderbook;
  if (plan.quote_readiness === "provider_pending") return dictionary.rewards.providerPending;
  if (plan.quote_readiness === "blocked") return dictionary.rewards.blocked;
  return plan.eligible ? dictionary.rewards.filterEligible : dictionary.rewards.filterIneligible;
}

export type RewardAiStrategyHintView = {
  quoteMode?: RewardPlanQuoteMode;
  bidRank?: number;
  maxConditionNotionalUsd?: DecimalValue;
};

export function rewardAiStrategyHint(
  advisory: RewardMarketAdvisoryDto,
): RewardAiStrategyHintView | null {
  const metrics = advisory.metrics;
  if (!isRecord(metrics)) return null;
  const hint = metrics.strategy_hint;
  if (!isRecord(hint)) return null;

  const quoteMode =
    typeof hint.quote_mode === "string" && isRewardPlanQuoteMode(hint.quote_mode)
      ? hint.quote_mode
      : undefined;
  const bidRank =
    typeof hint.bid_rank === "number" && Number.isInteger(hint.bid_rank)
      ? hint.bid_rank
      : undefined;
  const maxConditionNotionalUsd =
    typeof hint.max_condition_notional_usd === "number" ||
    typeof hint.max_condition_notional_usd === "string"
      ? hint.max_condition_notional_usd
      : undefined;

  if (quoteMode == null && bidRank == null && maxConditionNotionalUsd == null) {
    return null;
  }
  return { quoteMode, bidRank, maxConditionNotionalUsd };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isRewardPlanQuoteMode(value: string): value is RewardPlanQuoteMode {
  return value === "double" || value === "single_yes" || value === "single_no" || value === "none";
}
