import { z } from "zod";

import type {
  CopyTradeConfigDto,
  CopyTradeSnapshotDto,
  RewardBotConfigDto,
  RewardBotSnapshotDto,
  RuntimeConfigEntryDto,
  RuntimeConfigUpdateDto,
  RuntimeMode,
} from "@/lib/contracts/dto";
import { PolyEdgeApiError, randomUUID } from "@/lib/api/base";
import {
  cancelRewardBotOrders,
  resetRewardBot,
  runRewardBotOnce,
  updateRewardBotConfig,
} from "@/lib/api/rewards";
import {
  addTrackedWallet,
  analyzeWallets,
  cancelCopyTradeOrders,
  removeTrackedWallet,
  resetCopyTrade,
  runCopyTradeOnce,
  setWalletStatus,
  updateCopyTradeConfig,
} from "@/lib/api/copytrade";
import {
  releaseRiskControls,
  requestModeSwitch,
  setKillSwitchState,
} from "@/lib/api/risk";
import { updateRuntimeConfig } from "@/lib/api/settings";
import { submitSignalExecutionRequest } from "@/lib/api/signals";

export type OperationActionResult = {
  ok: boolean;
  message: string;
  requestId?: string;
  traceId?: string;
  operationId?: string;
  status?: "queued" | "completed" | "rejected";
  fieldErrors?: Partial<
    Record<"note" | "stepUpCode" | "targetMode" | "limitPrice" | "quantity" | "connectorName", string>
  >;
};

export type RewardBotActionResult = OperationActionResult & {
  snapshot?: RewardBotSnapshotDto;
};

export type RuntimeConfigActionResult = OperationActionResult & {
  entries?: RuntimeConfigEntryDto[];
};

function createActionSuccessResult(
  message: string,
  meta: {
    requestId: string;
    traceId: string;
    operationId: string;
    status: "queued" | "completed" | "rejected";
  },
): OperationActionResult {
  return {
    ok: true,
    message,
    requestId: meta.requestId,
    traceId: meta.traceId,
    operationId: meta.operationId,
    status: meta.status,
  };
}

function createActionFailureResult(
  message: string,
  options?: {
    requestId?: string;
    traceId?: string;
    fieldErrors?: OperationActionResult["fieldErrors"];
  },
): OperationActionResult {
  return {
    ok: false,
    message,
    requestId: options?.requestId,
    traceId: options?.traceId,
    fieldErrors: options?.fieldErrors,
  };
}

function apiActionFailure(error: unknown, fallback: string): OperationActionResult {
  if (error instanceof PolyEdgeApiError) {
    return createActionFailureResult(error.message, {
      requestId: error.requestId,
      traceId: error.traceId,
    });
  }

  return createActionFailureResult(error instanceof Error ? error.message : fallback);
}

const modeSwitchSchema = z.object({
  currentMode: z.enum(["research", "paper_trade", "live_auto", "kill_switch_locked"]),
  targetMode: z.enum(["research", "paper_trade", "live_auto", "kill_switch_locked"]),
  note: z.string().trim().min(16, "Mode switch note must be at least 16 characters."),
  stepUpCode: z.string().trim().min(6, "Step-up code is required for mode changes."),
});

const riskControlSchema = z.object({
  note: z.string().trim().min(16, "Operator note must be at least 16 characters."),
  stepUpCode: z.string().trim().min(6, "Step-up code is required for this control."),
});

const DECIMAL_STRING_PATTERN = /^(?:\d+|\d+\.\d+|\.\d+)$/;

const decimalString = (label: string) =>
  z
    .string()
    .trim()
    .min(1, `${label} is required.`)
    .refine(
      (value) => DECIMAL_STRING_PATTERN.test(value) && Number.isFinite(Number(value)),
      `${label} must be numeric.`,
    );

const signalExecutionSchema = z.object({
  signalId: z.string().min(1),
  expectedVersion: z.number().int().positive(),
  limitPrice: decimalString("Limit price").refine((value) => {
    const parsed = Number(value);
    return parsed > 0 && parsed <= 1;
  }, "Limit price must be greater than 0 and no more than 1."),
  quantity: decimalString("Quantity").refine((value) => Number(value) > 0, {
    message: "Quantity must be greater than 0.",
  }),
  connectorName: z.string().trim().optional().default(""),
  note: z.string().trim().min(16, "Execution note must be at least 16 characters."),
  stepUpCode: z.string().trim().min(6, "Step-up code is required for execution submission."),
});

const decimalNumber = z.coerce.number().finite();

const rewardConfigSchema = z.object({
  enabled: z.boolean(),
  account_id: z.string().trim().min(1),
  max_markets: z.coerce.number().int().min(0).max(65_535),
  max_open_orders: z.coerce.number().int().min(0).max(65_535),
  per_market_usd: decimalNumber.min(0),
  quote_size_usd: decimalNumber.min(0),
  min_daily_reward: decimalNumber.min(0),
  min_market_score: decimalNumber.min(0).max(100),
  max_spread_cents: decimalNumber.min(0.1).max(99),
  quote_edge_cents: decimalNumber.min(0).max(50),
  safety_margin_cents: decimalNumber.min(0).max(20),
  min_midpoint: decimalNumber.min(0).max(0.49),
  max_midpoint: decimalNumber.min(0.51).max(0.99),
  stale_book_ms: z.coerce.number().int().min(0).max(120_000),
  min_scoring_check_sec: z.coerce.number().int().min(0).max(600),
  max_position_usd: decimalNumber.min(0),
  max_global_position_usd: decimalNumber.min(0),
  exit_markup_cents: decimalNumber.min(0).max(50),
  cancel_on_fill: z.boolean(),
  account_capital_usd: decimalNumber.min(1),
  reward_competition_factor: decimalNumber.min(1).max(10_000),
  single_sided_divisor_c: decimalNumber.min(1).max(100),
  fill_rate_per_tick: decimalNumber.min(0).max(1),
  max_fill_ratio: decimalNumber.min(0.01).max(1),
  requote_drift_cents: decimalNumber.min(0).max(99),
  post_fill_strategy: z.enum([
    "exit_at_markup",
    "hold_and_requote",
    "flatten_immediately",
  ]),
}).refine((value) => value.max_midpoint > value.min_midpoint, {
  message: "Max midpoint must be greater than min midpoint.",
  path: ["max_midpoint"],
});

const runtimeConfigSchema = z.object({
  values: z.record(z.string().min(1), z.string().max(20_000)),
});

export async function requestModeSwitchAction(input: {
  currentMode: RuntimeMode;
  targetMode: RuntimeMode;
  note: string;
  stepUpCode: string;
}): Promise<OperationActionResult> {
  try {
    const parsed = modeSwitchSchema.safeParse(input);

    if (!parsed.success) {
      const flattened = parsed.error.flatten().fieldErrors;
      return createActionFailureResult("Mode switch request is invalid.", {
        fieldErrors: {
          note: flattened.note?.[0],
          stepUpCode: flattened.stepUpCode?.[0],
          targetMode: flattened.targetMode?.[0],
        },
      });
    }

    const response = await requestModeSwitch(parsed.data);

    return createActionSuccessResult("Mode switch accepted by the control plane.", {
      requestId: response.meta.request_id,
      traceId: response.meta.trace_id,
      operationId: response.data.operation_id,
      status: response.data.status,
    });
  } catch (error) {
    return apiActionFailure(error, "Mode switch request failed unexpectedly.");
  }
}

export async function triggerRiskReleaseAction(input: {
  note: string;
  stepUpCode: string;
}): Promise<OperationActionResult> {
  try {
    const parsed = riskControlSchema.safeParse(input);

    if (!parsed.success) {
      const flattened = parsed.error.flatten().fieldErrors;
      return createActionFailureResult("Release request is invalid.", {
        fieldErrors: {
          note: flattened.note?.[0],
          stepUpCode: flattened.stepUpCode?.[0],
        },
      });
    }

    const response = await releaseRiskControls(parsed.data);

    return createActionSuccessResult("Risk controls release queued for audit review.", {
      requestId: response.meta.request_id,
      traceId: response.meta.trace_id,
      operationId: response.data.operation_id,
      status: response.data.status,
    });
  } catch (error) {
    return apiActionFailure(error, "Risk release request failed unexpectedly.");
  }
}

export async function setKillSwitchStateAction(input: {
  enabled: boolean;
  note: string;
  stepUpCode: string;
}): Promise<OperationActionResult> {
  try {
    const parsed = riskControlSchema.safeParse(input);

    if (!parsed.success) {
      const flattened = parsed.error.flatten().fieldErrors;
      return createActionFailureResult("Kill switch request is invalid.", {
        fieldErrors: {
          note: flattened.note?.[0],
          stepUpCode: flattened.stepUpCode?.[0],
        },
      });
    }

    const response = await setKillSwitchState({
      enabled: input.enabled,
      note: parsed.data.note,
      stepUpCode: parsed.data.stepUpCode,
    });

    return createActionSuccessResult(
      input.enabled ? "Kill switch activation has been queued." : "Kill switch release has been queued.",
      {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: response.data.operation_id,
        status: response.data.status,
      },
    );
  } catch (error) {
    return apiActionFailure(error, "Kill switch request failed unexpectedly.");
  }
}

export async function submitSignalExecutionAction(input: {
  signalId: string;
  expectedVersion: number;
  limitPrice: string;
  quantity: string;
  connectorName?: string;
  note: string;
  stepUpCode: string;
}): Promise<OperationActionResult> {
  try {
    const parsed = signalExecutionSchema.safeParse(input);
    if (!parsed.success) {
      const flattened = parsed.error.flatten().fieldErrors;
      return createActionFailureResult("Execution request is invalid.", {
        fieldErrors: {
          limitPrice: flattened.limitPrice?.[0],
          quantity: flattened.quantity?.[0],
          connectorName: flattened.connectorName?.[0],
          note: flattened.note?.[0],
          stepUpCode: flattened.stepUpCode?.[0],
        },
      });
    }

    const response = await submitSignalExecutionRequest(parsed.data);

    return createActionSuccessResult("Execution request accepted by the backend.", {
      requestId: response.meta.request_id,
      traceId: response.meta.trace_id,
      operationId: response.data.operation_id,
      status: response.data.status,
    });
  } catch (error) {
    return apiActionFailure(error, "Execution request failed unexpectedly.");
  }
}

export async function updateRewardBotConfigAction(
  input: RewardBotConfigDto,
): Promise<RewardBotActionResult> {
  try {
    const parsed = rewardConfigSchema.safeParse(input);

    if (!parsed.success) {
      const issues = parsed.error.issues
        .map((i) => `${i.path.join(".")}: ${i.message}`)
        .join("; ");
      return createActionFailureResult(`Reward bot config is invalid: ${issues}`);
    }

    const response = await updateRewardBotConfig(parsed.data);

    return {
      ...createActionSuccessResult("Reward bot configuration saved.", {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: `reward_config_${randomUUID().slice(0, 8)}`,
        status: "completed",
      }),
      snapshot: response.data,
    };
  } catch (error) {
    return apiActionFailure(error, "Reward bot configuration update failed.");
  }
}

export async function runRewardBotOnceAction(): Promise<RewardBotActionResult> {
  try {
    const response = await runRewardBotOnce();

    return {
      ...createActionSuccessResult("Reward bot simulation completed.", {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: `reward_run_${randomUUID().slice(0, 8)}`,
        status: "completed",
      }),
      snapshot: response.data,
    };
  } catch (error) {
    return apiActionFailure(error, "Reward bot simulation failed.");
  }
}

export async function cancelRewardBotOrdersAction(): Promise<RewardBotActionResult> {
  try {
    const response = await cancelRewardBotOrders();

    return {
      ...createActionSuccessResult("Simulated reward orders cancelled.", {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: `reward_cancel_${randomUUID().slice(0, 8)}`,
        status: "completed",
      }),
      snapshot: response.data,
    };
  } catch (error) {
    return apiActionFailure(error, "Reward order cancellation failed.");
  }
}

export async function resetRewardBotAction(): Promise<RewardBotActionResult> {
  try {
    const response = await resetRewardBot();

    return {
      ...createActionSuccessResult("Reward simulation reset.", {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: `reward_reset_${randomUUID().slice(0, 8)}`,
        status: "completed",
      }),
      snapshot: response.data,
    };
  } catch (error) {
    return apiActionFailure(error, "Reward simulation reset failed.");
  }
}

// ── Copy Trade Actions ─────────────────────────────────────────────────────

export type CopyTradeActionResult = OperationActionResult & {
  snapshot?: CopyTradeSnapshotDto;
};

const HEX_ADDRESS_PATTERN = /^0x[0-9a-fA-F]{40}$/;

const addWalletSchema = z.object({
  address: z
    .string()
    .trim()
    .regex(HEX_ADDRESS_PATTERN, "Address must be a 0x-prefixed 40-hex-char Ethereum address."),
  label: z.string().trim().max(100).optional().default(""),
});

const walletActionSchema = z.object({
  address: z
    .string()
    .trim()
    .regex(HEX_ADDRESS_PATTERN, "Address must be a 0x-prefixed 40-hex-char Ethereum address."),
});

const copytradeConfigSchema = z.object({
  enabled: z.boolean(),
  mode: z.enum(["paper", "live"]),
  account_id: z.string().trim().min(1),
  account_capital_usd: decimalNumber.min(1),
  sizing_mode: z.enum(["fixed_usd", "proportional_to_source", "capital_ratio", "mirror_portfolio_weight"]),
  fixed_usd_per_trade: decimalNumber.min(1),
  proportional_factor: decimalNumber.min(0.001).max(1),
  capital_ratio: decimalNumber.min(0.001).max(1),
  min_source_trade_usd: decimalNumber.min(0),
  max_price: decimalNumber.min(0.01).max(1),
  min_price: decimalNumber.min(0).max(0.99),
  copy_sells: z.boolean(),
  max_position_per_market_usd: decimalNumber.min(1),
  per_wallet_max_exposure_usd: decimalNumber.min(1),
  max_total_exposure_usd: decimalNumber.min(1),
  max_open_copy_orders: z.coerce.number().int().min(1).max(200),
  daily_loss_limit_usd: decimalNumber.min(0),
  cooldown_secs: z.coerce.number().int().min(0).max(3600),
  max_slippage_cents: decimalNumber.min(0).max(50),
  fill_rate_per_tick: decimalNumber.min(0).max(1),
  max_fill_ratio: decimalNumber.min(0.01).max(1),
});

export async function updateCopyTradeConfigAction(
  input: CopyTradeConfigDto,
): Promise<CopyTradeActionResult> {
  try {
    const parsed = copytradeConfigSchema.safeParse(input);
    if (!parsed.success) {
      return createActionFailureResult("Copy trading config is invalid.");
    }
    const response = await updateCopyTradeConfig(parsed.data);
    return {
      ...createActionSuccessResult("Copy trading configuration saved.", {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: `copytrade_config_${randomUUID().slice(0, 8)}`,
        status: "completed",
      }),
      snapshot: response.data,
    };
  } catch (error) {
    return apiActionFailure(error, "Copy trading configuration update failed.");
  }
}

export async function addTrackedWalletAction(input: {
  address: string;
  label?: string;
}): Promise<CopyTradeActionResult> {
  try {
    const parsed = addWalletSchema.safeParse(input);
    if (!parsed.success) {
      const fieldErrors = parsed.error.flatten().fieldErrors;
      return createActionFailureResult("Wallet address is invalid.", {
        fieldErrors: { note: fieldErrors.address?.[0] },
      });
    }
    const response = await addTrackedWallet(parsed.data);
    return {
      ...createActionSuccessResult("Wallet added for tracking.", {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: `copytrade_add_wallet_${randomUUID().slice(0, 8)}`,
        status: "completed",
      }),
      snapshot: response.data,
    };
  } catch (error) {
    return apiActionFailure(error, "Adding wallet failed.");
  }
}

export async function removeTrackedWalletAction(address: string): Promise<CopyTradeActionResult> {
  try {
    const parsed = walletActionSchema.safeParse({ address });
    if (!parsed.success) {
      return createActionFailureResult("Invalid wallet address.");
    }
    const response = await removeTrackedWallet(parsed.data);
    return {
      ...createActionSuccessResult("Wallet removed from tracking.", {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: `copytrade_remove_wallet_${randomUUID().slice(0, 8)}`,
        status: "completed",
      }),
      snapshot: response.data,
    };
  } catch (error) {
    return apiActionFailure(error, "Removing wallet failed.");
  }
}

export async function setCopytradeWalletStatusAction(
  address: string,
  status: "active" | "paused",
): Promise<CopyTradeActionResult> {
  try {
    const response = await setWalletStatus(address, status);
    return {
      ...createActionSuccessResult(`Wallet ${status === "active" ? "resumed" : "paused"}.`, {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: `copytrade_wallet_status_${randomUUID().slice(0, 8)}`,
        status: "completed",
      }),
      snapshot: response.data,
    };
  } catch (error) {
    return apiActionFailure(error, "Updating wallet status failed.");
  }
}

export async function runCopyTradeOnceAction(): Promise<CopyTradeActionResult> {
  try {
    const response = await runCopyTradeOnce();
    return {
      ...createActionSuccessResult("Copy trading cycle completed.", {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: `copytrade_run_${randomUUID().slice(0, 8)}`,
        status: "completed",
      }),
      snapshot: response.data,
    };
  } catch (error) {
    return apiActionFailure(error, "Copy trading run failed.");
  }
}

export async function analyzeCopytradeWalletsAction(): Promise<CopyTradeActionResult> {
  try {
    const response = await analyzeWallets();
    return {
      ...createActionSuccessResult("Wallet analysis completed.", {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: `copytrade_analyze_${randomUUID().slice(0, 8)}`,
        status: "completed",
      }),
      snapshot: response.data,
    };
  } catch (error) {
    return apiActionFailure(error, "Wallet analysis failed.");
  }
}

export async function cancelCopyTradeOrdersAction(): Promise<CopyTradeActionResult> {
  try {
    const response = await cancelCopyTradeOrders();
    return {
      ...createActionSuccessResult("Copy trading orders cancelled.", {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: `copytrade_cancel_${randomUUID().slice(0, 8)}`,
        status: "completed",
      }),
      snapshot: response.data,
    };
  } catch (error) {
    return apiActionFailure(error, "Copy order cancellation failed.");
  }
}

export async function resetCopyTradeAction(): Promise<CopyTradeActionResult> {
  try {
    const response = await resetCopyTrade();
    return {
      ...createActionSuccessResult("Copy trading simulation reset.", {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: `copytrade_reset_${randomUUID().slice(0, 8)}`,
        status: "completed",
      }),
      snapshot: response.data,
    };
  } catch (error) {
    return apiActionFailure(error, "Copy trading reset failed.");
  }
}

export async function updateRuntimeConfigAction(
  input: RuntimeConfigUpdateDto,
): Promise<RuntimeConfigActionResult> {
  try {
    const parsed = runtimeConfigSchema.safeParse(input);

    if (!parsed.success) {
      return createActionFailureResult("Runtime configuration is invalid.");
    }

    const response = await updateRuntimeConfig(parsed.data);

    return {
      ...createActionSuccessResult("Runtime configuration saved. Restart backend processes to apply runtime consumers.", {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: `runtime_config_${randomUUID().slice(0, 8)}`,
        status: "completed",
      }),
      entries: response.data,
    };
  } catch (error) {
    return apiActionFailure(error, "Runtime configuration update failed.");
  }
}
