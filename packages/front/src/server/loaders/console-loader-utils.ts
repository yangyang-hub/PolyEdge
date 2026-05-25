import "server-only";

import type { MarketDto } from "@/lib/contracts/dto";

export function indexMarkets(markets: MarketDto[]): Map<string, MarketDto> {
  return new Map(markets.map((market) => [market.id, market]));
}

export function sumNumericStrings(values: string[]): string {
  const total = values.reduce((sum, value) => sum + Number.parseFloat(value), 0);
  return total.toFixed(2);
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
