import type { ManagedRewardOrderDto } from "@/lib/contracts/dto";
import type { EventCategory } from "../types";

export function eventCategory(eventType: string): Exclude<EventCategory, "all"> | null {
  // Placements: intent persisted + order accepted on exchange
  if (
    eventType === "reward_order_placed" ||
    eventType === "reward_exit_placed" ||
    eventType === "reward_live_order_placed" ||
    eventType === "reward_live_exit_order_placed" ||
    eventType === "reward_live_order_planned" ||
    eventType === "reward_live_exit_planned" ||
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
