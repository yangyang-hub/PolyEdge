import type { ApiResponse } from "@/lib/contracts/api";
import type {
  CopyTradeConfigPatchDto,
  CopyTradeSnapshotDto,
  AddTrackedWalletInputDto,
  WalletActionInputDto,
} from "@/lib/contracts/dto";
import { fetchContract, fetchWriteContract } from "@/lib/api/base";

export async function readCopyTradeSnapshot(): Promise<ApiResponse<CopyTradeSnapshotDto>> {
  return fetchContract<ApiResponse<CopyTradeSnapshotDto>>("/api/v1/copy-trading");
}

export async function updateCopyTradeConfig(
  patch: CopyTradeConfigPatchDto,
): Promise<ApiResponse<CopyTradeSnapshotDto>> {
  return fetchWriteContract<ApiResponse<CopyTradeSnapshotDto>>("/api/v1/copy-trading/config", {
    method: "POST",
    idempotencyKey: `copytrade-config-${crypto.randomUUID()}`,
    body: patch as Record<string, unknown>,
  });
}

export async function addTrackedWallet(
  input: AddTrackedWalletInputDto,
): Promise<ApiResponse<CopyTradeSnapshotDto>> {
  return fetchWriteContract<ApiResponse<CopyTradeSnapshotDto>>("/api/v1/copy-trading/wallets", {
    method: "POST",
    idempotencyKey: `copytrade-add-wallet-${crypto.randomUUID()}`,
    body: input as Record<string, unknown>,
  });
}

export async function removeTrackedWallet(
  input: WalletActionInputDto,
): Promise<ApiResponse<CopyTradeSnapshotDto>> {
  return fetchWriteContract<ApiResponse<CopyTradeSnapshotDto>>("/api/v1/copy-trading/wallets/remove", {
    method: "POST",
    idempotencyKey: `copytrade-remove-wallet-${crypto.randomUUID()}`,
    body: input as Record<string, unknown>,
  });
}

export async function setWalletStatus(
  address: string,
  status: "active" | "paused",
): Promise<ApiResponse<CopyTradeSnapshotDto>> {
  return fetchWriteContract<ApiResponse<CopyTradeSnapshotDto>>("/api/v1/copy-trading/wallets/status", {
    method: "POST",
    idempotencyKey: `copytrade-wallet-status-${crypto.randomUUID()}`,
    body: { address, status },
  });
}

export async function runCopyTradeOnce(): Promise<ApiResponse<CopyTradeSnapshotDto>> {
  return fetchWriteContract<ApiResponse<CopyTradeSnapshotDto>>("/api/v1/copy-trading/run", {
    method: "POST",
    idempotencyKey: `copytrade-run-${crypto.randomUUID()}`,
    body: {},
  });
}

export async function analyzeWallets(): Promise<ApiResponse<CopyTradeSnapshotDto>> {
  return fetchWriteContract<ApiResponse<CopyTradeSnapshotDto>>("/api/v1/copy-trading/analyze", {
    method: "POST",
    idempotencyKey: `copytrade-analyze-${crypto.randomUUID()}`,
    body: {},
  });
}

export async function cancelCopyTradeOrders(): Promise<ApiResponse<CopyTradeSnapshotDto>> {
  return fetchWriteContract<ApiResponse<CopyTradeSnapshotDto>>("/api/v1/copy-trading/cancel-all", {
    method: "POST",
    idempotencyKey: `copytrade-cancel-${crypto.randomUUID()}`,
    body: {},
  });
}

export async function resetCopyTrade(): Promise<ApiResponse<CopyTradeSnapshotDto>> {
  return fetchWriteContract<ApiResponse<CopyTradeSnapshotDto>>("/api/v1/copy-trading/reset", {
    method: "POST",
    idempotencyKey: `copytrade-reset-${crypto.randomUUID()}`,
    body: {},
  });
}
