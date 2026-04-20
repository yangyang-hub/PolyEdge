import "server-only";

import type { ApprovalDto, MarketDto } from "@/lib/contracts/dto";

export function indexMarkets(markets: MarketDto[]): Map<string, MarketDto> {
  return new Map(markets.map((market) => [market.id, market]));
}

export function sumNumericStrings(values: string[]): string {
  const total = values.reduce((sum, value) => sum + Number.parseFloat(value), 0);
  return total.toFixed(2);
}

export function getPendingSignalApprovalIds(approvals: ApprovalDto[]): Set<string> {
  return new Set(
    approvals
      .filter((approval) => approval.status === "pending" && approval.type === "signal")
      .map((approval) => approval.resource_id),
  );
}

export function selectFirstMatchingItem<T>(
  items: T[],
  predicates: Array<(item: T) => boolean>,
  errorMessage: string,
): T {
  for (const predicate of predicates) {
    const selected = items.find(predicate);

    if (selected) {
      return selected;
    }
  }

  const fallback = items[0];

  if (!fallback) {
    throw new Error(errorMessage);
  }

  return fallback;
}
