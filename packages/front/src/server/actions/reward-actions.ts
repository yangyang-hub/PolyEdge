"use server";

import { revalidatePath } from "next/cache";
import { z } from "zod";

import type {
  RewardBotConfigDto,
  RewardBotSnapshotDto,
} from "@/lib/contracts/dto";
import { assertConsoleRole } from "@/server/auth/console-session";
import {
  cancelRewardBotOrders,
  runRewardBotOnce,
  updateRewardBotConfig,
} from "@/server/api/rewards";
import {
  createActionFailureResult,
  createActionSuccessResult,
  type OperationActionResult,
} from "@/server/actions/action-result";
import { PolyEdgeApiError } from "@/server/api/base";

export type RewardBotActionResult = OperationActionResult & {
  snapshot?: RewardBotSnapshotDto;
};

const decimalNumber = z.coerce.number().finite();

const rewardConfigSchema = z.object({
  enabled: z.boolean(),
  mode: z.enum(["dry_run", "live"]),
  account_id: z.string().trim().min(1),
  max_markets: z.coerce.number().int().min(1).max(50),
  max_open_orders: z.coerce.number().int().min(1).max(200),
  per_market_usd: decimalNumber.min(1),
  quote_size_usd: decimalNumber.min(1),
  min_daily_reward: decimalNumber.min(0),
  min_market_score: decimalNumber.min(0).max(100),
  max_spread_cents: decimalNumber.min(0.1).max(99),
  quote_edge_cents: decimalNumber.min(0).max(50),
  safety_margin_cents: decimalNumber.min(0).max(20),
  min_midpoint: decimalNumber.min(0.01).max(0.49),
  max_midpoint: decimalNumber.min(0.51).max(0.99),
  stale_book_ms: z.coerce.number().int().min(1_000).max(120_000),
  min_scoring_check_sec: z.coerce.number().int().min(15).max(600),
  max_position_usd: decimalNumber.min(1),
  max_global_position_usd: decimalNumber.min(1),
  exit_markup_cents: decimalNumber.min(0).max(50),
  cancel_on_fill: z.boolean(),
}).refine((value) => value.max_midpoint > value.min_midpoint, {
  message: "Max midpoint must be greater than min midpoint.",
  path: ["max_midpoint"],
});

export async function updateRewardBotConfigAction(
  input: RewardBotConfigDto,
): Promise<RewardBotActionResult> {
  try {
    await assertConsoleRole("operator");
    const parsed = rewardConfigSchema.safeParse(input);

    if (!parsed.success) {
      return createActionFailureResult("Reward bot config is invalid.");
    }

    const response = await updateRewardBotConfig(parsed.data);
    revalidatePath("/rewards");

    return {
      ...createActionSuccessResult("Reward bot configuration saved.", {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: `reward_config_${crypto.randomUUID().slice(0, 8)}`,
        status: "completed",
      }),
      snapshot: response.data,
    };
  } catch (error) {
    return rewardActionFailure(error, "Reward bot configuration update failed.");
  }
}

export async function runRewardBotOnceAction(): Promise<RewardBotActionResult> {
  try {
    await assertConsoleRole("operator");
    const response = await runRewardBotOnce();
    revalidatePath("/rewards");

    return {
      ...createActionSuccessResult("Reward bot simulation completed.", {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: `reward_run_${crypto.randomUUID().slice(0, 8)}`,
        status: "completed",
      }),
      snapshot: response.data,
    };
  } catch (error) {
    return rewardActionFailure(error, "Reward bot simulation failed.");
  }
}

export async function cancelRewardBotOrdersAction(): Promise<RewardBotActionResult> {
  try {
    await assertConsoleRole("operator");
    const response = await cancelRewardBotOrders();
    revalidatePath("/rewards");

    return {
      ...createActionSuccessResult("Simulated reward orders cancelled.", {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: `reward_cancel_${crypto.randomUUID().slice(0, 8)}`,
        status: "completed",
      }),
      snapshot: response.data,
    };
  } catch (error) {
    return rewardActionFailure(error, "Reward order cancellation failed.");
  }
}

function rewardActionFailure(error: unknown, fallback: string): RewardBotActionResult {
  if (error instanceof PolyEdgeApiError) {
    return createActionFailureResult(error.message, {
      requestId: error.requestId,
      traceId: error.traceId,
    });
  }

  return createActionFailureResult(error instanceof Error ? error.message : fallback);
}
