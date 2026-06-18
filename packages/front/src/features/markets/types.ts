import type { Tone } from "@/lib/formatters";

export type MarketFilter = "all" | "review_queue" | "watch_only";
export type SortDir = "desc" | "asc" | "none";

export interface LinkedEventView {
  id: string;
  source: string;
  relevance: string;
  summary: string;
}

export interface MarketViewModel {
  id: string;
  question: string;
  category: string;
  midPrice: string;
  volume24h: string;
  tradabilityStatus: string;
  tradabilityLabel: string;
  tradabilityTone: Tone;
  ambiguityLabel: string;
  ambiguityTone: Tone;
  linkedEventCount: string;
}

export interface MarketDetailViewModel {
  id: string;
  question: string;
  category: string;
  polymarketConditionId: string | null;
  slug: string | null;
  tradabilityLabel: string;
  tradabilityTone: Tone;
  ambiguityLabel: string;
  ambiguityTone: Tone;
  resolutionSource: string;
  edgeCaseNotes: string[];
  linkedEvents: LinkedEventView[];
}
