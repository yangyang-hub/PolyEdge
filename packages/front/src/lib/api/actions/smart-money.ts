import { z } from "zod";

import {
  updateSmartMoneyCandidateStatus,
  updateSmartMoneyConfig,
} from "@/lib/api/smart-money";
import type {
  SmartMoneyConfigDto,
  SmartMoneySnapshotDto,
  SmartWalletCandidateStatus,
} from "@/lib/contracts/dto";

import {
  actionOperationId,
  apiActionFailure,
  createActionFailureResult,
  createActionSuccessResult,
  decimalNumber,
  type OperationActionResult,
} from "./shared";

export type SmartMoneyActionResult = OperationActionResult & {
  snapshot?: SmartMoneySnapshotDto;
};

const HEX_ADDRESS_PATTERN = /^0x[0-9a-fA-F]{40}$/;

const smartMoneyConfigSchema = z
  .object({
    enabled: z.boolean(),
    mode: z.enum(["observe", "paper", "approval", "live_guarded"]),
    discovery_enabled: z.boolean(),
    wallet_advisory_enabled: z.boolean(),
    signal_advisory_enabled: z.boolean(),
    signal_advisory_concurrency_enabled: z.boolean(),
    signal_advisory_provider: z.enum(["openai", "anthropic"]),
    signal_advisory_request_format: z.enum([
      "openai_responses",
      "openai_chat_completions",
      "anthropic_messages",
    ]),
    signal_advisory_model: z.string().trim().min(1).max(120),
    signal_advisory_max_concurrency: z.coerce.number().int().min(1).max(10),
    min_trade_count: z.coerce.number().int().min(0),
    min_settled_trade_count: z.coerce.number().int().min(0),
    min_total_volume_usd: decimalNumber.min(0),
    min_copyability_score: decimalNumber.min(0).max(1),
    max_signal_age_ms: z.coerce.number().int().min(1000),
    max_price_slippage_cents: decimalNumber.min(0),
    min_orderbook_depth_usd: decimalNumber.min(0),
    max_wallet_exposure_usd: decimalNumber.min(0),
    max_market_exposure_usd: decimalNumber.min(0),
    max_daily_notional_usd: decimalNumber.min(0),
  })
  .superRefine((config, context) => {
    if (
      config.signal_advisory_provider === "anthropic"
      && config.signal_advisory_request_format !== "anthropic_messages"
    ) {
      context.addIssue({
        code: z.ZodIssueCode.custom,
        path: ["signal_advisory_request_format"],
        message: "Anthropic 只能使用 Messages 请求格式。",
      });
    }
    if (
      config.signal_advisory_provider === "openai"
      && config.signal_advisory_request_format === "anthropic_messages"
    ) {
      context.addIssue({
        code: z.ZodIssueCode.custom,
        path: ["signal_advisory_request_format"],
        message: "OpenAI-compatible provider 不能使用 Anthropic Messages。",
      });
    }
  });

const candidateStatusSchema = z.object({
  wallet_address: z
    .string()
    .trim()
    .regex(HEX_ADDRESS_PATTERN, "地址必须是 0x 开头的 40 位十六进制 Ethereum 地址。"),
  source: z.string().trim().max(100).optional(),
  status: z.enum(["candidate", "watch", "tracked", "blocked", "rejected"]),
  reason: z.string().trim().max(500).optional(),
});

export async function updateSmartMoneyConfigAction(
  input: SmartMoneyConfigDto,
): Promise<SmartMoneyActionResult> {
  try {
    const parsed = smartMoneyConfigSchema.safeParse(input);
    if (!parsed.success) {
      const issues = parsed.error.issues
        .map((issue) => `${issue.path.join(".")}: ${issue.message}`)
        .join("; ");
      return createActionFailureResult(`聪明钱配置无效：${issues}`);
    }
    const response = await updateSmartMoneyConfig(parsed.data);
    return {
      ...createActionSuccessResult("聪明钱配置已保存。", {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: actionOperationId("smart_money_config"),
        status: "completed",
      }),
      snapshot: response.data,
    };
  } catch (error) {
    return apiActionFailure(error, "聪明钱配置保存失败。");
  }
}

export async function updateSmartWalletCandidateStatusAction(input: {
  walletAddress: string;
  source?: string;
  status: SmartWalletCandidateStatus;
  reason?: string;
}): Promise<SmartMoneyActionResult> {
  try {
    const parsed = candidateStatusSchema.safeParse({
      wallet_address: input.walletAddress,
      source: input.source,
      status: input.status,
      reason: input.reason,
    });
    if (!parsed.success) {
      const fieldErrors = parsed.error.flatten().fieldErrors;
      return createActionFailureResult("候选钱包状态无效。", {
        fieldErrors: {
          address: fieldErrors.wallet_address?.[0],
          source: fieldErrors.source?.[0],
          status: fieldErrors.status?.[0],
          reason: fieldErrors.reason?.[0],
        },
      });
    }
    const response = await updateSmartMoneyCandidateStatus(parsed.data);
    return {
      ...createActionSuccessResult("候选钱包状态已更新。", {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: actionOperationId("smart_money_candidate_status"),
        status: "completed",
      }),
      snapshot: response.data,
    };
  } catch (error) {
    return apiActionFailure(error, "候选钱包状态更新失败。");
  }
}
