export type PolygonFundingToken = {
  id: string;
  symbol: string;
  name: string;
  address: `0x${string}`;
  decimals: number;
  noteKey: "nativeUsdc" | "bridgedUsdc" | "polygonUsdt" | "wormholeUsdt";
};

export const polygonFundingTokens = [
  {
    id: "usdc",
    symbol: "USDC",
    name: "USD Coin",
    address: "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359",
    decimals: 6,
    noteKey: "nativeUsdc",
  },
  {
    id: "usdc-e",
    symbol: "USDC.e",
    name: "USD Coin (PoS)",
    address: "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174",
    decimals: 6,
    noteKey: "bridgedUsdc",
  },
  {
    id: "usdt0",
    symbol: "USDT0",
    name: "Polygon USDT",
    address: "0xc2132D05D31c914a87C6611C10748AEb04B58e8F",
    decimals: 6,
    noteKey: "polygonUsdt",
  },
  {
    id: "usdt-wormhole",
    symbol: "USDT",
    name: "Wormhole Bridged USDT",
    address: "0x9417669fBF23357D2774e9D421307bd5eA1006d2",
    decimals: 6,
    noteKey: "wormholeUsdt",
  },
] as const satisfies readonly PolygonFundingToken[];

export type PolygonFundingTokenId = (typeof polygonFundingTokens)[number]["id"];

export const polygonChain = {
  chainIdHex: "0x89",
  chainId: 137,
  chainName: "Polygon",
  nativeCurrency: {
    name: "POL",
    symbol: "POL",
    decimals: 18,
  },
  rpcUrls: ["https://polygon-bor-rpc.publicnode.com"],
  blockExplorerUrls: ["https://polygonscan.com"],
} as const;

const EVM_ADDRESS_PATTERN = /^0x[a-fA-F0-9]{40}$/;

export function getPolygonFundingToken(tokenId: string): PolygonFundingToken {
  return polygonFundingTokens.find((token) => token.id === tokenId) ?? polygonFundingTokens[0];
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

export function buildErc20TransferData(to: string, amountUnits: bigint): `0x${string}` {
  const recipient = normalizeEvmAddress(to).slice(2).toLowerCase().padStart(64, "0");
  const amount = amountUnits.toString(16).padStart(64, "0");

  return `0xa9059cbb${recipient}${amount}`;
}

export function buildPolygonscanTxUrl(txHash: string): string {
  return `https://polygonscan.com/tx/${txHash}`;
}

export function buildPolygonscanTokenUrl(tokenAddress: string): string {
  return `https://polygonscan.com/token/${tokenAddress}`;
}
