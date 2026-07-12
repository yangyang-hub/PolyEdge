import type { DecimalValue } from "./primitives";

export type FundingTokenDto = {
  id: string;
  symbol: string;
  name: string;
  address: string;
  decimals: number;
  min_transfer_amount: DecimalValue;
  balance?: DecimalValue | null;
};

export type FundingStatusDto = {
  enabled: boolean;
  source_address: string | null;
  polymarket_wallet_address: string | null;
  chain_id: number;
  max_transfer_amount: DecimalValue;
  tokens: FundingTokenDto[];
  configuration_error?: string;
  balance_error?: string;
};

export type FundingTransferRequestDto = {
  token_id: string;
  amount: DecimalValue;
  confirmed: boolean;
  operator_note?: string;
};

export type FundingTransferDto = {
  tx_hash: string;
  source_address: string;
  polymarket_wallet_address: string;
  bridge_deposit_address: string;
  token_id: string;
  token_symbol: string;
  token_address: string;
  amount: DecimalValue;
  amount_units: string;
  chain_id: number;
  replayed: boolean;
};
