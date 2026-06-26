export type WalletActionState = "idle" | "connecting" | "switching" | "submitting" | "submitted" | "error";

export type WalletSnapshot = {
  account: string | null;
  txHash: string | null;
  status: WalletActionState;
  message: string | null;
};
