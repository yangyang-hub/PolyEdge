import type { DecimalValue } from "./primitives";

export type WalletAccountStatus = "active" | "paused" | "disabled" | "error";
export type CredentialProvider = "environment" | "vault" | "kms";

export type WalletCredentialRefDto = {
  id: number;
  provider: CredentialProvider;
  locator: string;
  key_version: string | null;
  created_at: string;
  updated_at: string;
};

export type WalletAccountDto = {
  id: number;
  name: string;
  signer_address: string;
  funder_address: string;
  signature_type: number;
  credential_ref_id: number;
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
  credential: WalletCredentialRefDto;
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
  credential_provider: CredentialProvider;
  credential_locator: string;
  credential_key_version?: string;
  trading_enabled?: boolean;
  risk_policy: WalletRiskPolicyInput;
  operator_note?: string;
};

export type UpdateWalletAccountRequest = {
  name?: string;
  credential_provider?: CredentialProvider;
  credential_locator?: string;
  credential_key_version?: string;
  status?: WalletAccountStatus;
  trading_enabled?: boolean;
  risk_policy?: WalletRiskPolicyInput;
  operator_note?: string;
};
