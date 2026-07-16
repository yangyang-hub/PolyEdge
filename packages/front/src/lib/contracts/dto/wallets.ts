import type { DecimalValue } from "./primitives";

export type WalletAccountStatus = "active" | "paused" | "disabled" | "error";
export type EncryptedWalletSecretInput = { context_id: string; key_id: string; algorithm: "RSA-OAEP-256+A256GCM"; wrapped_key: string; nonce: string; ciphertext: string };

export type WalletSecretMetadataDto = {
  wallet_id: number;
  key_id: string;
  secret_version: number;
  updated_at: string;
};

export type WalletAccountDto = {
  id: number;
  owner_user_id: number;
  name: string;
  signer_address: string;
  funder_address: string;
  signature_type: number;
  status: WalletAccountStatus;
  trading_enabled: boolean;
  created_at: string;
  updated_at: string;
};

export type WalletRiskPolicyDto = {
  wallet_id: number;
  max_open_orders: number;
  max_open_buy_notional: DecimalValue;
  max_total_position_notional: DecimalValue;
  max_market_position_notional: DecimalValue;
  max_order_notional: DecimalValue;
  updated_at: string;
};

export type WalletAccountStateDto = {
  wallet_id: number;
  available_collateral: DecimalValue;
  reserved_collateral: DecimalValue;
  open_buy_notional: DecimalValue;
  total_position_notional: DecimalValue;
  last_synced_at: string | null;
  last_error: string | null;
  version: number;
  updated_at: string;
};

export type WalletAccountData = {
  account: WalletAccountDto;
  secret: WalletSecretMetadataDto;
  risk_policy: WalletRiskPolicyDto;
  state: WalletAccountStateDto;
};

export type WalletRiskPolicyInput = {
  max_open_orders: number;
  max_open_buy_notional: DecimalValue;
  max_total_position_notional: DecimalValue;
  max_market_position_notional: DecimalValue;
  max_order_notional: DecimalValue;
};

export type CreateWalletAccountRequest = {
  name: string;
  signer_address: string;
  funder_address: string;
  signature_type: number;
  encrypted_secret: EncryptedWalletSecretInput;
  trading_enabled?: boolean;
  risk_policy: WalletRiskPolicyInput;
  operator_note?: string;
};

export type UpdateWalletAccountRequest = {
  name?: string;
  encrypted_secret?: EncryptedWalletSecretInput;
  status?: WalletAccountStatus;
  trading_enabled?: boolean;
  risk_policy?: WalletRiskPolicyInput;
  operator_note?: string;
};
