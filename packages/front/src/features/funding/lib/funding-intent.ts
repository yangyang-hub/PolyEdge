import { randomUUID } from "@/lib/api/base";

const FUNDING_INTENT_STORAGE_KEY = "polyedge.funding.transfer-intent.v1";

export type FundingTransferIntent = {
  tokenId: string;
  amount: string;
  idempotencyKey: string;
};

function isFundingTransferIntent(value: unknown): value is FundingTransferIntent {
  if (value === null || typeof value !== "object") return false;
  const candidate = value as Partial<FundingTransferIntent>;
  return (
    typeof candidate.tokenId === "string" &&
    candidate.tokenId.length > 0 &&
    typeof candidate.amount === "string" &&
    candidate.amount.length > 0 &&
    typeof candidate.idempotencyKey === "string" &&
    candidate.idempotencyKey.startsWith("funding-transfer-")
  );
}

export function loadFundingTransferIntent(): FundingTransferIntent | null {
  try {
    const raw = window.sessionStorage.getItem(FUNDING_INTENT_STORAGE_KEY);
    if (!raw) return null;
    const parsed: unknown = JSON.parse(raw);
    return isFundingTransferIntent(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

export function createFundingTransferIntent(tokenId: string, amount: string): FundingTransferIntent {
  return {
    tokenId,
    amount: amount.trim(),
    idempotencyKey: `funding-transfer-${randomUUID()}`,
  };
}

export function saveFundingTransferIntent(intent: FundingTransferIntent): void {
  try {
    window.sessionStorage.setItem(FUNDING_INTENT_STORAGE_KEY, JSON.stringify(intent));
  } catch {
    // An unavailable session store must not block the transfer flow. The in-memory
    // intent still preserves the key for retries during the current page lifetime.
  }
}

export function clearFundingTransferIntent(): void {
  try {
    window.sessionStorage.removeItem(FUNDING_INTENT_STORAGE_KEY);
  } catch {
    // Best-effort cleanup only.
  }
}
