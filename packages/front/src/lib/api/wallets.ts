import { fetchContract, fetchListContract } from "@/lib/api/base";
import type { ApiListResponse, ApiResponse } from "@/lib/contracts/api";
import type { WalletAccountData } from "@/lib/contracts/dto";

export function listWallets(): Promise<ApiListResponse<WalletAccountData>> {
  return fetchListContract<WalletAccountData>("/api/v1/wallets");
}

export function getWallet(walletId: number): Promise<ApiResponse<WalletAccountData>> {
  return fetchContract<ApiResponse<WalletAccountData>>(`/api/v1/wallets/${walletId}`);
}
