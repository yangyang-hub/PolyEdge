import { z } from "zod";

import {
  addTrackedWallet,
  analyzeWallets,
  removeTrackedWallet,
  setWalletStatus,
  updateCopyTradeConfig,
} from "@/lib/api/copytrade";
import type { CopyTradeConfigDto, CopyTradeSnapshotDto } from "@/lib/contracts/dto";

import {
  actionOperationId,
  apiActionFailure,
  createActionFailureResult,
  createActionSuccessResult,
  decimalNumber,
  type OperationActionResult,
} from "./shared";

export type CopyTradeActionResult = OperationActionResult & {
  snapshot?: CopyTradeSnapshotDto;
};

const HEX_ADDRESS_PATTERN = /^0x[0-9a-fA-F]{40}$/;

const addWalletSchema = z.object({
  address: z
    .string()
    .trim()
    .regex(HEX_ADDRESS_PATTERN, "地址必须是 0x 开头的 40 位十六进制 Ethereum 地址。"),
  label: z.string().trim().max(100).optional().default(""),
});

const walletActionSchema = z.object({
  address: z
    .string()
    .trim()
    .regex(HEX_ADDRESS_PATTERN, "地址必须是 0x 开头的 40 位十六进制 Ethereum 地址。"),
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
      const issues = parsed.error.issues
        .map((i) => `${i.path.join(".")}: ${i.message}`)
        .join("; ");
      return createActionFailureResult(`跟踪配置无效：${issues}`);
    }
    const response = await updateCopyTradeConfig(parsed.data);
    return {
      ...createActionSuccessResult("跟踪配置已保存。", {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: actionOperationId("copytrade_config"),
        status: "completed",
      }),
      snapshot: response.data,
    };
  } catch (error) {
    return apiActionFailure(error, "跟踪配置保存失败。");
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
      return createActionFailureResult("钱包地址无效。", {
        fieldErrors: { address: fieldErrors.address?.[0] },
      });
    }
    const response = await addTrackedWallet(parsed.data);
    return {
      ...createActionSuccessResult("钱包已加入跟踪。", {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: actionOperationId("copytrade_add_wallet"),
        status: "completed",
      }),
      snapshot: response.data,
    };
  } catch (error) {
    return apiActionFailure(error, "添加钱包失败。");
  }
}

export async function removeTrackedWalletAction(address: string): Promise<CopyTradeActionResult> {
  try {
    const parsed = walletActionSchema.safeParse({ address });
    if (!parsed.success) {
      return createActionFailureResult("钱包地址无效。");
    }
    const response = await removeTrackedWallet(parsed.data);
    return {
      ...createActionSuccessResult("钱包已停止跟踪。", {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: actionOperationId("copytrade_remove_wallet"),
        status: "completed",
      }),
      snapshot: response.data,
    };
  } catch (error) {
    return apiActionFailure(error, "移除钱包失败。");
  }
}

export async function setCopytradeWalletStatusAction(
  address: string,
  status: "active" | "paused",
): Promise<CopyTradeActionResult> {
  try {
    const response = await setWalletStatus(address, status);
    return {
      ...createActionSuccessResult(`钱包已${status === "active" ? "恢复" : "暂停"}。`, {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: actionOperationId("copytrade_wallet_status"),
        status: "completed",
      }),
      snapshot: response.data,
    };
  } catch (error) {
    return apiActionFailure(error, "更新钱包状态失败。");
  }
}

export async function analyzeCopytradeWalletsAction(): Promise<CopyTradeActionResult> {
  try {
    const response = await analyzeWallets();
    return {
      ...createActionSuccessResult("钱包分析命令已入队。", {
        requestId: response.meta.request_id,
        traceId: response.meta.trace_id,
        operationId: actionOperationId("copytrade_analyze"),
        status: "queued",
      }),
      snapshot: response.data,
    };
  } catch (error) {
    return apiActionFailure(error, "钱包分析失败。");
  }
}
