import type { ManagedRewardOrderDto } from "@/lib/contracts/dto";
import type { EventCategory } from "../types";

export function eventCategory(eventType: string): Exclude<EventCategory, "all"> | null {
  if (eventType === "reward_order_placed" || eventType === "reward_exit_placed") {
    return "placements";
  }
  if (eventType === "reward_order_cancelled") {
    return "cancels";
  }
  if (
    eventType === "reward_order_filled" ||
    eventType === "reward_exit_filled" ||
    eventType === "reward_position_flattened"
  ) {
    return "fills";
  }
  if (eventType === "reward_accrued") {
    return "rewards";
  }
  return null;
}

export function rewardTone(status: ManagedRewardOrderDto["status"]) {
  if (status === "open" || status === "exit_pending") {
    return "success" as const;
  }
  if (status === "error") {
    return "danger" as const;
  }
  if (status === "cancelled") {
    return "neutral" as const;
  }
  return "warning" as const;
}
