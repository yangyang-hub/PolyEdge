CREATE TABLE markets (
    id TEXT PRIMARY KEY,
    question TEXT NOT NULL,
    category TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('open', 'closed', 'resolved')),
    best_bid NUMERIC(12, 6) NOT NULL CHECK (best_bid >= 0 AND best_bid <= 1),
    best_ask NUMERIC(12, 6) NOT NULL CHECK (best_ask >= 0 AND best_ask <= 1),
    mid_price NUMERIC(12, 6) NOT NULL CHECK (mid_price >= 0 AND mid_price <= 1),
    volume_24h NUMERIC(18, 2) NOT NULL CHECK (volume_24h >= 0),
    ambiguity_level TEXT NOT NULL CHECK (ambiguity_level IN ('low', 'medium', 'high')),
    tradability_status TEXT NOT NULL CHECK (
        tradability_status IN ('tradable', 'manual_review', 'observe_only', 'blocked')
    ),
    updated_at TIMESTAMPTZ NOT NULL,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version >= 1),
    trace_id TEXT NOT NULL
);

CREATE INDEX markets_status_updated_at_idx
    ON markets (status, updated_at DESC);

CREATE INDEX markets_tradability_status_updated_at_idx
    ON markets (tradability_status, updated_at DESC);

CREATE TABLE market_resolution_rules (
    market_id TEXT PRIMARY KEY REFERENCES markets(id) ON DELETE CASCADE,
    resolution_source TEXT NOT NULL,
    edge_case_notes TEXT[] NOT NULL DEFAULT '{}',
    updated_at TIMESTAMPTZ NOT NULL,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version >= 1),
    trace_id TEXT NOT NULL
);

CREATE INDEX market_resolution_rules_updated_at_idx
    ON market_resolution_rules (updated_at DESC);

CREATE TABLE raw_events (
    id TEXT PRIMARY KEY,
    source TEXT NOT NULL,
    hash TEXT NOT NULL,
    raw_payload JSONB NOT NULL CHECK (jsonb_typeof(raw_payload) = 'object'),
    ingested_at TIMESTAMPTZ NOT NULL,
    trace_id TEXT NOT NULL
);

CREATE UNIQUE INDEX raw_events_source_hash_uidx
    ON raw_events (source, hash);

CREATE INDEX raw_events_ingested_at_idx
    ON raw_events (ingested_at DESC);

CREATE TABLE events (
    id TEXT PRIMARY KEY,
    raw_event_id TEXT REFERENCES raw_events(id) ON DELETE SET NULL,
    source TEXT NOT NULL,
    summary TEXT NOT NULL,
    relevance_score NUMERIC(12, 6) NOT NULL CHECK (relevance_score >= 0 AND relevance_score <= 1),
    confidence NUMERIC(12, 6) NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
    status TEXT NOT NULL CHECK (status IN ('active', 'expired', 'invalidated', 'superseded')),
    reason_trace TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version >= 1),
    trace_id TEXT NOT NULL
);

CREATE INDEX events_status_created_at_idx
    ON events (status, created_at DESC);

CREATE INDEX events_updated_at_idx
    ON events (updated_at DESC);

CREATE INDEX events_raw_event_id_idx
    ON events (raw_event_id);

CREATE TABLE event_market_links (
    event_id TEXT NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    market_id TEXT NOT NULL REFERENCES markets(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (event_id, market_id)
);

CREATE INDEX event_market_links_market_id_idx
    ON event_market_links (market_id);
