import type {
  EvidenceDirection,
  EvidenceStatus,
  NewsSourceType,
  ResourceVersion,
} from "./primitives";

export type NewsSourceHealthDto = {
  source: string;
  source_type: NewsSourceType;
  enabled: boolean;
  reliability: string;
  last_success_at?: string | null;
  last_error_at?: string | null;
  consecutive_failures: number;
  items_fetched: number;
  items_inserted: number;
  items_deduped: number;
  health_score: string;
  last_error?: string | null;
  updated_at: string;
};

export type NewsRawEventDto = {
  id: string;
  source: string;
  source_type: NewsSourceType;
  external_id?: string | null;
  title: string;
  url?: string | null;
  author?: string | null;
  published_at?: string | null;
  event_time: string;
  hash: string;
  raw_payload: unknown;
  ingested_at: string;
  trace_id: string;
};

export type EvidenceDto = ResourceVersion & {
  market_id: string;
  event_id: string;
  direction: EvidenceDirection;
  strength: string;
  source_reliability: string;
  novelty: string;
  resolution_relevance: string;
  status: EvidenceStatus;
  expires_at: string;
  created_at: string;
  updated_at: string;
};
