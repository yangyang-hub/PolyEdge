import type { DecimalValue, FundingTokenDto } from "@/lib/contracts/dto";

export type PolygonFundingToken = FundingTokenDto;
export type FundingTokenNoteKey = "nativeUsdc" | "polygonUsdt";
export type FundingAmountError =
  | "invalid"
  | "below_minimum"
  | "above_maximum"
  | "above_balance";

export const polygonChain = {
  chainId: 137,
  chainName: "Polygon",
  blockExplorerUrls: ["https://polygonscan.com"],
} as const;

const EVM_ADDRESS_PATTERN = /^0x[a-fA-F0-9]{40}$/;

export function getPolygonFundingToken(
  tokens: readonly PolygonFundingToken[],
  tokenId: string,
): PolygonFundingToken | null {
  return tokens.find((token) => token.id === tokenId) ?? tokens[0] ?? null;
}

export function getFundingTokenNoteKey(token: PolygonFundingToken): FundingTokenNoteKey {
  return token.id === "usdt" ? "polygonUsdt" : "nativeUsdc";
}

export function isEvmAddress(value: string): boolean {
  return EVM_ADDRESS_PATTERN.test(value.trim());
}

export function normalizeEvmAddress(value: string): `0x${string}` {
  return value.trim() as `0x${string}`;
}

export function parseTokenDecimalToUnits(value: string, decimals: number): bigint | null {
  const normalized = value.trim();

  if (!/^\d+(\.\d*)?$/.test(normalized)) {
    return null;
  }

  const [wholePart, fractionalPart = ""] = normalized.split(".");

  if (fractionalPart.length > decimals) {
    return null;
  }

  const wholeUnits = BigInt(wholePart || "0") * 10n ** BigInt(decimals);
  const fractionalUnits = BigInt((fractionalPart.padEnd(decimals, "0") || "0").slice(0, decimals));
  return wholeUnits + fractionalUnits;
}

export function parseTokenAmountToUnits(value: string, decimals: number): bigint | null {
  const units = parseTokenDecimalToUnits(value, decimals);
  return units !== null && units > 0n ? units : null;
}

export function fundingAmountError(
  value: string,
  token: PolygonFundingToken,
  maxTransferAmount: DecimalValue,
): FundingAmountError | null {
  const amountUnits = parseTokenAmountToUnits(value, token.decimals);
  if (amountUnits === null) return "invalid";

  const minimumUnits = parseTokenDecimalToUnits(String(token.min_transfer_amount), token.decimals);
  if (minimumUnits !== null && amountUnits < minimumUnits) return "below_minimum";

  const maximumUnits = parseTokenDecimalToUnits(String(maxTransferAmount), token.decimals);
  if (maximumUnits !== null && amountUnits > maximumUnits) return "above_maximum";

  if (token.balance !== null && token.balance !== undefined) {
    const balanceUnits = parseTokenDecimalToUnits(String(token.balance), token.decimals);
    if (balanceUnits !== null && amountUnits > balanceUnits) return "above_balance";
  }

  return null;
}

export function formatFundingTokenBalance(balance: DecimalValue | null | undefined, symbol: string): string {
  if (balance === null || balance === undefined || String(balance).trim() === "") {
    return "";
  }

  const numericBalance = Number(balance);
  const formattedBalance = Number.isFinite(numericBalance)
    ? numericBalance.toLocaleString("zh-CN", {
        maximumFractionDigits: 4,
        minimumFractionDigits: 0,
      })
    : String(balance);

  return `${formattedBalance} ${symbol}`;
}

export function buildPolygonscanTxUrl(txHash: string): string {
  return `https://polygonscan.com/tx/${txHash}`;
}

export function buildPolygonscanTokenUrl(tokenAddress: string): string {
  return `https://polygonscan.com/token/${tokenAddress}`;
}
