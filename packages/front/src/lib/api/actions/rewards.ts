import { z } from "zod";

import {
  cancelRewardBotOrders,
  resetRewardBot,
  runRewardBotOnce,
  updateRewardBotConfig,
} from "@/lib/api/rewards";
import type {
  RewardBotConfigDto,
  RewardBotConfigPatchDto,
  RewardBotSnapshotDto,
} from "@/lib/contracts/dto";
import type { InternalApiStepUpScope } from "@/lib/api/base";

import {
  actionOperationId,
  apiActionFailure,
  createActionFailureResult,
  createActionSuccessResult,
  decimalNumber,
  type OperationActionResult,
} from "./shared";

export type RewardBotActionResult = OperationActionResult & {
  snapshot?: RewardBotSnapshotDto;
};

export type RewardOperationActionInput = {
  operatorNote?: string;
  stepUpCode?: string;
};

export type RewardConfigActionOptions = RewardOperationActionInput & {
  stepUpScopes?: Extract<
    InternalApiStepUpScope,
    "rewards_live_trading_enable" | "rewards_merge_auto_execute"
  >[];
};

const operatorNoteSchema = z
  .string()
  .trim()
  .max(500)
  .refine((value) => !/[\u0000-\u001F\u007F]/u.test(value));
const stepUpCodeSchema = z.string().trim().min(1);

function parseOperationInput(
  input: RewardOperationActionInput,
  requiresStepUp: boolean,
): { operatorNote?: string; stepUpCode?: string } | RewardBotActionResult {
  const note = operatorNoteSchema.safeParse(input.operatorNote ?? "");
  if (!note.success) {
    return createActionFailureResult("Operator note must be one printable line of 500 characters or fewer.", {
      fieldErrors: { note: "Use one printable line of 500 characters or fewer." },
    });
  }
  const stepUpCode = stepUpCodeSchema.safeParse(input.stepUpCode ?? "");
  if (requiresStepUp && !stepUpCode.success) {
    return createActionFailureResult("Step-up confirmation is required.", {
      fieldErrors: { stepUpCode: "Enter a valid step-up confirmation code." },
    });
  }
  return {
    ...(note.data ? { operatorNote: note.data } : {}),
    ...(stepUpCode.success ? { stepUpCode: stepUpCode.data } : {}),
  };
}

function normalizeRewardConfigPatchForSubmit(
  config: z.infer<typeof rewardConfigSchema>,
): RewardBotConfigPatchDto {
  return config;
}

const rewardConfigSchema = z.object({
  enabled: z.boolean(),
  account_id: z.string().trim().min(1),
  max_markets: z.coerce.number().int().min(0).max(65_535),
  max_open_orders: z.coerce.number().int().min(0).max(65_535),
  maker_market_budget_usd: decimalNumber.min(0).max(1_000_000),
  min_daily_reward: decimalNumber.min(0).max(100_000),
  min_market_liquidity_usd: decimalNumber.min(0).max(1_000_000_000),
  min_market_volume_24h_usd: decimalNumber.min(0).max(1_000_000_000),
  min_hours_to_end: z.coerce.number().int().min(0).max(87_600),
  max_market_spread_cents: decimalNumber.min(0.1).max(100),
  max_market_data_age_minutes: z.coerce.number().int().min(1).max(1440),
  min_market_score: decimalNumber.min(0).max(100),
  max_spread_cents: decimalNumber.min(0.1).max(99),
  quote_mode: z.enum(["double", "auto"]),
  selection_mode: z.enum(["observe", "enforce"]),
  quote_bid_rank: z.coerce.number().int().min(1).max(3),
  quote_max_bid_rank: z.coerce.number().int().min(1).max(3),
  dominant_single_side_enabled: z.boolean(),
  dominant_min_probability: decimalNumber.min(0.51).max(0.99),
  dominant_max_probability: decimalNumber.min(0.51).max(0.99),
  dominant_min_exit_depth_usd: decimalNumber.min(0).max(1_000_000),
  max_top1_depth_share: decimalNumber.min(0).max(1),
  max_top3_depth_share: decimalNumber.min(0).max(1),
  max_book_hhi: decimalNumber.min(0).max(1),
  preferred_categories: z.array(z.string().trim().min(1)),
  preferred_category_score_bonus: decimalNumber.min(0).max(20),
  opportunity_metrics_enabled: z.boolean(),
  opportunity_probe_notional_usd: decimalNumber.min(0).max(1_000_000),
  opportunity_min_reward_per_100_usd_day: decimalNumber.min(0).max(100_000),
  opportunity_max_competition_multiple: decimalNumber.min(0).max(1_000_000),
  opportunity_competition_hard_gate_enabled: z.boolean(),
  opportunity_competition_hard_gate_multiple: decimalNumber.min(0).max(1_000_000),
  opportunity_max_account_allocation_bps: z.coerce.number().int().min(0).max(10_000),
  opportunity_max_market_allocation_bps: z.coerce.number().int().min(0).max(10_000),
  opportunity_min_exit_depth_usd: decimalNumber.min(0).max(1_000_000),
  opportunity_min_exit_depth_multiple: decimalNumber.min(0).max(100),
  opportunity_max_entry_exit_slippage_cents: decimalNumber.min(0).max(99),
  opportunity_max_bad_fill_recovery_days: decimalNumber.min(0).max(365),
  opportunity_observation_window_sec: z.coerce.number().int().min(60).max(86_400),
  opportunity_min_book_samples: z.coerce.number().int().min(1).max(10_000),
  opportunity_max_midpoint_range_cents: decimalNumber.min(0).max(100),
  opportunity_max_top_of_book_flip_count: z.coerce.number().int().min(0).max(10_000),
  opportunity_reward_weight: decimalNumber.min(0).max(1_000),
  opportunity_competition_weight: decimalNumber.min(0).max(1_000),
  opportunity_exit_weight: decimalNumber.min(0).max(1_000),
  opportunity_stability_weight: decimalNumber.min(0).max(1_000),
  fair_value_enabled: z.boolean(),
  fair_value_record_history_enabled: z.boolean(),
  fair_value_min_confidence: decimalNumber.min(0).max(1),
  fair_value_min_raw_edge_cents: decimalNumber.min(0).max(50),
  fair_value_min_effective_edge_cents: decimalNumber.min(0).max(50),
  fair_value_uncertainty_buffer_cents: decimalNumber.min(0).max(50),
  fair_value_rebate_haircut: decimalNumber.min(0).max(1),
  fair_value_max_reward_rebate_cents: decimalNumber.min(0).max(50),
  fair_value_max_midpoint_deviation_cents: decimalNumber.min(0).max(50),
  fair_value_history_window_sec: z.coerce.number().int().min(0).max(86_400),
  fair_value_min_history_samples: z.coerce.number().int().min(0).max(10_000),
  ai_advisory_enabled: z.boolean(),
  ai_provider: z.enum(["openai", "anthropic"]),
  ai_request_format: z.enum([
    "openai_responses",
    "openai_chat_completions",
    "anthropic_messages",
  ]),
  ai_advisory_ttl_sec: z.coerce.number().int().min(60).max(86_400),
  ai_provider_max_markets: z.coerce.number().int().min(0).max(65_535),
  ai_provider_failure_cooldown_sec: z.coerce.number().int().min(0).max(86_400),
  ai_provider_concurrency_enabled: z.boolean(),
  ai_provider_primary_max_concurrency: z.coerce.number().int().min(1).max(10),
  ai_provider_fallback_max_concurrency: z.coerce.number().int().min(1).max(10),
  ai_risk_adjustment_enabled: z.boolean(),
  ai_action_min_confidence: decimalNumber.min(0).max(1),
  info_risk_enabled: z.boolean(),
  info_risk_mode: z.enum(["observe", "enforce"]),
  info_risk_avoid_level: z.enum(["low", "medium", "high", "critical", "unknown"]),
  info_risk_min_confidence: decimalNumber.min(0).max(1),
  info_risk_ttl_sec: z.coerce.number().int().min(60).max(86_400),
  ai_advisory_provider_pending_grace_sec: z.coerce.number().int().min(0).max(86_400),
  info_risk_provider_pending_grace_sec: z.coerce.number().int().min(0).max(86_400),
  event_window_enabled: z.boolean(),
  event_window_min_confidence: z.enum(["low", "medium", "high"]),
  event_window_stop_new_quote_before_start_sec: z.coerce
    .number()
    .int()
    .min(0)
    .max(86_400 * 30),
  event_window_cancel_open_buy_before_start_sec: z.coerce
    .number()
    .int()
    .min(0)
    .max(86_400 * 30),
  event_window_resume_after_event_end_sec: z.coerce
    .number()
    .int()
    .min(0)
    .max(86_400 * 30),
  event_window_unknown_event_time_mode: z.enum(["allow", "observe", "block"]),
  event_window_gamma_unreviewed_dates_mode: z.enum([
    "ignore",
    "observe",
    "medium_confidence",
  ]),
  require_info_risk_before_first_quote: z.boolean(),
  first_quote_quarantine_sec: z.coerce.number().int().min(0).max(86_400),
  safety_margin_cents: decimalNumber.min(0).max(20),
  min_midpoint: decimalNumber.min(0).max(0.49),
  max_midpoint: decimalNumber.min(0.51).max(1),
  stale_book_ms: z.coerce.number().int().min(5_000).max(120_000),
  min_scoring_check_sec: z.coerce.number().int().min(15).max(600),
  max_position_usd: decimalNumber.min(0).max(1_000_000),
  max_global_position_usd: decimalNumber.min(0).max(10_000_000),
  inventory_skew_enabled: z.boolean(),
  inventory_skew_strength: decimalNumber.min(0).max(1),
  exit_markup_cents: decimalNumber.min(0).max(50),
  maker_max_exit_loss_cents: decimalNumber.min(0).max(50),
  account_capital_usd: decimalNumber.min(1).max(100_000_000),
  requote_drift_cents: decimalNumber.min(0).max(99),
  requote_drift_confirm_sec: z.coerce.number().int().min(0).max(3600),
  requote_drift_cooldown_sec: z.coerce.number().int().min(0).max(86_400),
  requote_drift_max_cancels_per_cycle: z.coerce.number().int().min(0).max(100),
  adverse_requote_drift_cents: decimalNumber.min(0).max(99),
  adverse_requote_confirm_sec: z.coerce.number().int().min(0).max(300),
  post_fill_strategy: z.enum([
    "exit_at_markup",
    "hold_and_requote",
    "flatten_immediately",
    "adaptive",
  ]),
  adaptive_flatten_min_bid_depth_usd: decimalNumber.min(0).max(1_000_000),
  adaptive_flatten_min_depth_multiple: decimalNumber.min(0).max(100),
  adaptive_flatten_min_surplus_cents: decimalNumber.min(0).max(50),
  adaptive_flatten_when_plan_ineligible: z.boolean(),
  adaptive_flatten_when_event_risk: z.boolean(),
  adaptive_hold_when_plan_eligible: z.boolean(),
  adaptive_fallback_strategy: z.enum([
    "exit_at_markup",
    "hold_and_requote",
    "flatten_immediately",
  ]),
  adaptive_exit_recheck_sec: z.coerce.number().int().min(5).max(86_400),
  adaptive_exit_reselect_cooldown_sec: z.coerce.number().int().min(0).max(86_400),
  adaptive_exit_max_reselects_per_order: z.coerce.number().int().min(0).max(100),
  adaptive_exit_min_strategy_improvement_cents: decimalNumber.min(0).max(50),
  adaptive_exit_cancel_replace_enabled: z.boolean(),
  adaptive_exit_reprice_drift_cents: decimalNumber.min(0).max(50),
  adaptive_exit_cancel_replace_max_per_cycle: z.number().int().min(0).max(100),
  balanced_merge_enabled: z.boolean(),
  balanced_merge_max_markets: z.coerce.number().int().min(0).max(65_535),
  balanced_merge_max_open_orders: z.coerce.number().int().min(0).max(65_535),
  balanced_merge_min_edge_cents: decimalNumber.min(0).max(20),
  balanced_merge_min_market_score: decimalNumber.min(0).max(100),
  balanced_merge_min_market_liquidity_usd: decimalNumber.min(0).max(1_000_000_000),
  balanced_merge_min_market_volume_24h_usd: decimalNumber.min(0).max(1_000_000_000),
  balanced_merge_max_market_spread_cents: decimalNumber.min(0.1).max(100),
  balanced_merge_quote_bid_rank: z.coerce.number().int().min(1).max(3),
  balanced_merge_max_unpaired_position_usd: decimalNumber.min(0).max(1_000_000),
  balanced_merge_auto_execute_enabled: z.boolean(),
  min_depth_usd: decimalNumber.min(0).max(1_000_000),
  cancel_bid_rank: z.coerce.number().int().min(0).max(20),
  depth_drop_pct: decimalNumber.min(0).max(100),
  depth_drop_window_sec: z.coerce.number().int().min(0).max(300),
  fill_velocity_usd: decimalNumber.min(0).max(1_000_000),
  fill_velocity_window_sec: z.coerce.number().int().min(0).max(300),
  mass_cancel_pct: decimalNumber.min(0).max(100),
  mass_cancel_window_sec: z.coerce.number().int().min(0).max(300),
  requote_interval_sec: z.coerce.number().int().min(0).max(3600),
  requote_jitter_sec: z.coerce.number().int().min(0).max(600),
  reconcile_interval_sec: z.coerce.number().int().min(1).max(60),
})
  .strict()
  .refine((value) => value.max_midpoint > value.min_midpoint, {
    message: "Max midpoint must be greater than min midpoint.",
    path: ["max_midpoint"],
  })
  .refine((value) => value.dominant_max_probability >= value.dominant_min_probability, {
    message: "Dominant max probability must be at least the dominant min probability.",
    path: ["dominant_max_probability"],
  })
  .refine((value) => value.quote_max_bid_rank >= value.quote_bid_rank, {
    message: "Maximum quote rank must be at least the preferred quote rank.",
    path: ["quote_max_bid_rank"],
  })
  .refine((value) => value.max_top3_depth_share >= value.max_top1_depth_share, {
    message: "Top-3 depth share cap must be at least the top-1 cap.",
    path: ["max_top3_depth_share"],
  })
  .refine(
    (value) =>
      value.event_window_cancel_open_buy_before_start_sec <=
      value.event_window_stop_new_quote_before_start_sec,
    {
      message: "Cancel window must not be earlier than the stop-new-quote window.",
      path: ["event_window_cancel_open_buy_before_start_sec"],
    },
  )
  .refine(
    (value) =>
      value.opportunity_reward_weight
        + value.opportunity_competition_weight
        + value.opportunity_exit_weight
        + value.opportunity_stability_weight
      > 0,
    {
      message: "Opportunity metric weights must sum above zero.",
      path: ["opportunity_reward_weight"],
    },
  )
  .refine(
    (value) =>
      value.ai_provider === "anthropic"
        ? value.ai_request_format === "anthropic_messages"
        : value.ai_request_format !== "anthropic_messages",
    {
      message: "AI request format must match the selected provider.",
      path: ["ai_request_format"],
    },
  )
  .refine(
    (value) => value.cancel_bid_rank === 0 || value.cancel_bid_rank < value.quote_bid_rank,
    {
      message: "Cancel bid rank must be disabled or deeper than the quote rank.",
      path: ["cancel_bid_rank"],
    },
  );

export async function updateRewardBotConfigAction(
  input: RewardBotConfigDto,
  options: RewardConfigActionOptions = {},
): Promise<RewardBotActionResult> {
  try {
    const parsed = rewardConfigSchema.safeParse(input);

    if (!parsed.success) {
      const issues = parsed.error.issues
        .map((i) => `${i.path.join(".")}: ${i.message}`)
        .join("; ");
      return createActionFailureResult(`Reward bot config is invalid: ${issues}`);
    }

    const operation = parseOperationInput(options, Boolean(options.stepUpScopes?.length));
    if ("ok" in operation) return operation;

    const response = await updateRewardBotConfig(normalizeRewardConfigPatchForSubmit(parsed.data), {
      ...operation,
      stepUpScopes: options.stepUpScopes,
    });

    return {
      ...createActionSuccessResult("Reward bot configuration saved.", {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: actionOperationId("reward_config"),
        status: "completed",
      }),
      snapshot: response.data,
    };
  } catch (error) {
    return apiActionFailure(error, "Reward bot configuration update failed.");
  }
}

export async function runRewardBotOnceAction(
  input: RewardOperationActionInput = {},
): Promise<RewardBotActionResult> {
  try {
    const operation = parseOperationInput(input, true);
    if ("ok" in operation) return operation;
    const response = await runRewardBotOnce(operation);

    return {
      ...createActionSuccessResult("Reward strategy run queued for worker execution.", {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: actionOperationId("reward_run"),
        status: "queued",
      }),
      snapshot: response.data,
    };
  } catch (error) {
    return apiActionFailure(error, "Reward strategy run failed.");
  }
}

export async function cancelRewardBotOrdersAction(
  input: RewardOperationActionInput = {},
): Promise<RewardBotActionResult> {
  try {
    const operation = parseOperationInput(input, false);
    if ("ok" in operation) return operation;
    const response = await cancelRewardBotOrders(operation);

    return {
      ...createActionSuccessResult("Reward order cancellation queued for worker execution.", {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: actionOperationId("reward_cancel"),
        status: "queued",
      }),
      snapshot: response.data,
    };
  } catch (error) {
    return apiActionFailure(error, "Reward order cancellation failed.");
  }
}

export async function resetRewardBotAction(
  input: RewardOperationActionInput = {},
): Promise<RewardBotActionResult> {
  try {
    const operation = parseOperationInput(input, true);
    if ("ok" in operation) return operation;
    const response = await resetRewardBot(operation);

    return {
      ...createActionSuccessResult("Rewards reset command queued for worker execution.", {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: actionOperationId("reward_reset"),
        status: "queued",
      }),
      snapshot: response.data,
    };
  } catch (error) {
    return apiActionFailure(error, "Rewards reset command failed.");
  }
}
