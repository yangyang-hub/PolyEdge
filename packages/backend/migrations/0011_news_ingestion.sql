ALTER TABLE raw_events
  ADD COLUMN IF NOT EXISTS source_type TEXT,
  ADD COLUMN IF NOT EXISTS external_id TEXT,
  ADD COLUMN IF NOT EXISTS title TEXT,
  ADD COLUMN IF NOT EXISTS url TEXT,
  ADD COLUMN IF NOT EXISTS author TEXT,
  ADD COLUMN IF NOT EXISTS published_at TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS event_time TIMESTAMPTZ;

ALTER TABLE raw_events
  ADD CONSTRAINT raw_events_source_type_chk
  CHECK (
    source_type IS NULL
    OR source_type IN ('news', 'social', 'official', 'calendar', 'market')
  );

CREATE UNIQUE INDEX IF NOT EXISTS raw_events_source_external_id_uidx
  ON raw_events (source, external_id)
  WHERE external_id IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS raw_events_source_url_uidx
  ON raw_events (source, url)
  WHERE url IS NOT NULL;

CREATE INDEX IF NOT EXISTS raw_events_source_type_ingested_at_idx
  ON raw_events (source_type, ingested_at DESC);

CREATE INDEX IF NOT EXISTS raw_events_published_at_idx
  ON raw_events (published_at DESC)
  WHERE published_at IS NOT NULL;

CREATE TABLE IF NOT EXISTS news_source_health (
  source TEXT PRIMARY KEY,
  source_type TEXT NOT NULL CHECK (source_type IN ('news', 'social', 'official', 'calendar', 'market')),
  enabled BOOLEAN NOT NULL DEFAULT TRUE,
  reliability NUMERIC(12, 6) NOT NULL CHECK (reliability >= 0 AND reliability <= 1),
  last_success_at TIMESTAMPTZ,
  last_error_at TIMESTAMPTZ,
  consecutive_failures BIGINT NOT NULL DEFAULT 0 CHECK (consecutive_failures >= 0),
  items_fetched BIGINT NOT NULL DEFAULT 0 CHECK (items_fetched >= 0),
  items_inserted BIGINT NOT NULL DEFAULT 0 CHECK (items_inserted >= 0),
  items_deduped BIGINT NOT NULL DEFAULT 0 CHECK (items_deduped >= 0),
  health_score NUMERIC(12, 6) NOT NULL DEFAULT 1 CHECK (health_score >= 0 AND health_score <= 1),
  last_error TEXT,
  updated_at TIMESTAMPTZ NOT NULL,
  trace_id TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS news_source_health_updated_at_idx
  ON news_source_health (updated_at DESC);

CREATE INDEX IF NOT EXISTS news_source_health_score_idx
  ON news_source_health (health_score, updated_at DESC);
