import type {
  AmbiguityLevel,
  EventStatus,
  MarketStatus,
  ResourceVersion,
  TradabilityStatus,
} from "./primitives";

export type MarketDto = ResourceVersion & {
  question: string;
  category: string;
  slug?: string | null;
  status: MarketStatus;
  best_bid: string;
  best_ask: string;
  mid_price: string;
  volume_24h: string;
  ambiguity_level: AmbiguityLevel;
  tradability_status: TradabilityStatus;
  resolution_source: string;
  edge_case_notes: string[];
  polymarket_condition_id?: string | null;
  polymarket_yes_asset_id?: string | null;
  polymarket_no_asset_id?: string | null;
  updated_at: string;
};

export type EventDto = ResourceVersion & {
  source: string;
  summary: string;
  relevance_score: string;
  confidence: string;
  status: EventStatus;
  related_market_ids: string[];
  reason_trace: string;
  created_at: string;
  updated_at: string;
};
