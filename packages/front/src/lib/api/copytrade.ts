import type { ApiResponse } from "@/lib/contracts/api";
import type {
  CopyTradeConfigPatchDto,
  CopyTradeSnapshotDto,
  AddTrackedWalletInputDto,
  WalletActionInputDto,
} from "@/lib/contracts/dto";
import { fetchContract, fetchWriteContract, randomUUID } from "@/lib/api/base";

export async function readCopyTradeSnapshot(): Promise<ApiResponse<CopyTradeSnapshotDto>> {
  return fetchContract<ApiResponse<CopyTradeSnapshotDto>>("/api/v1/copy-trading");
}

export async function updateCopyTradeConfig(
  patch: CopyTradeConfigPatchDto,
): Promise<ApiResponse<CopyTradeSnapshotDto>> {
  return fetchWriteContract<ApiResponse<CopyTradeSnapshotDto>>("/api/v1/copy-trading/config", {
    method: "POST",
    idempotencyKey: `copytrade-config-${randomUUID()}`,
    body: patch as Record<string, unknown>,
  });
}

export async function addTrackedWallet(
  input: AddTrackedWalletInputDto,
): Promise<ApiResponse<CopyTradeSnapshotDto>> {
  return fetchWriteContract<ApiResponse<CopyTradeSnapshotDto>>("/api/v1/copy-trading/wallets", {
    method: "POST",
    idempotencyKey: `copytrade-add-wallet-${randomUUID()}`,
    body: input as Record<string, unknown>,
  });
}

export async function removeTrackedWallet(
  input: WalletActionInputDto,
): Promise<ApiResponse<CopyTradeSnapshotDto>> {
  return fetchWriteContract<ApiResponse<CopyTradeSnapshotDto>>("/api/v1/copy-trading/wallets/remove", {
    method: "POST",
    idempotencyKey: `copytrade-remove-wallet-${randomUUID()}`,
    body: input as Record<string, unknown>,
  });
}

export async function setWalletStatus(
  address: string,
  status: "active" | "paused",
): Promise<ApiResponse<CopyTradeSnapshotDto>> {
  return fetchWriteContract<ApiResponse<CopyTradeSnapshotDto>>("/api/v1/copy-trading/wallets/status", {
    method: "POST",
    idempotencyKey: `copytrade-wallet-status-${randomUUID()}`,
    body: { address, status },
  });
}

export async function analyzeWallets(): Promise<ApiResponse<CopyTradeSnapshotDto>> {
  return fetchWriteContract<ApiResponse<CopyTradeSnapshotDto>>("/api/v1/copy-trading/analyze", {
    method: "POST",
    idempotencyKey: `copytrade-analyze-${randomUUID()}`,
    body: {},
  });
}
