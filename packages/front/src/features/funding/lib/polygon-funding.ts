import type { DecimalValue, FundingTokenDto } from "@/lib/contracts/dto";

export type PolygonFundingToken = FundingTokenDto;
export type FundingTokenNoteKey = "nativeUsdc" | "polygonUsdt";

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

export function parseTokenAmountToUnits(value: string, decimals: number): bigint | null {
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
  const units = wholeUnits + fractionalUnits;

  return units > 0n ? units : null;
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
