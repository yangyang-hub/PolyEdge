CREATE TABLE evidences (
    id TEXT PRIMARY KEY,
    market_id TEXT NOT NULL REFERENCES markets(id) ON DELETE CASCADE,
    event_id TEXT NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    direction TEXT NOT NULL CHECK (direction IN ('supports_yes', 'supports_no', 'background')),
    strength NUMERIC(12, 6) NOT NULL CHECK (strength >= 0 AND strength <= 1),
    source_reliability NUMERIC(12, 6) NOT NULL CHECK (source_reliability >= 0 AND source_reliability <= 1),
    novelty NUMERIC(12, 6) NOT NULL CHECK (novelty >= 0 AND novelty <= 1),
    resolution_relevance NUMERIC(12, 6) NOT NULL CHECK (resolution_relevance >= 0 AND resolution_relevance <= 1),
    status TEXT NOT NULL CHECK (status IN ('active', 'expired', 'invalidated')),
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version >= 1),
    trace_id TEXT NOT NULL
);

CREATE INDEX evidences_market_status_created_at_idx
    ON evidences (market_id, status, created_at DESC);

CREATE INDEX evidences_event_id_idx
    ON evidences (event_id);

CREATE INDEX evidences_status_created_at_idx
    ON evidences (status, created_at DESC);

CREATE TABLE signals (
    id TEXT PRIMARY KEY,
    market_id TEXT NOT NULL REFERENCES markets(id) ON DELETE CASCADE,
    event_id TEXT NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    action TEXT NOT NULL CHECK (action IN ('buy', 'sell')),
    side TEXT NOT NULL CHECK (side IN ('yes', 'no')),
    market_price NUMERIC(12, 6) NOT NULL CHECK (market_price >= 0 AND market_price <= 1),
    fair_price NUMERIC(12, 6) NOT NULL CHECK (fair_price >= 0 AND fair_price <= 1),
    edge NUMERIC(12, 6) NOT NULL CHECK (edge >= -1 AND edge <= 1),
    confidence NUMERIC(12, 6) NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
    lifecycle_state TEXT NOT NULL CHECK (
        lifecycle_state IN ('new', 'active', 'weakened', 'executed', 'invalidated', 'reversed', 'expired')
    ),
    reason TEXT NOT NULL,
    risk_decision TEXT NOT NULL,
    approved_by_user_id TEXT,
    approved_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version >= 1),
    trace_id TEXT NOT NULL
);

CREATE INDEX signals_market_lifecycle_updated_at_idx
    ON signals (market_id, lifecycle_state, updated_at DESC);

CREATE INDEX signals_event_id_idx
    ON signals (event_id);

CREATE INDEX signals_lifecycle_updated_at_idx
    ON signals (lifecycle_state, updated_at DESC);

CREATE TABLE signal_evidence_links (
    signal_id TEXT NOT NULL REFERENCES signals(id) ON DELETE CASCADE,
    evidence_id TEXT NOT NULL REFERENCES evidences(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (signal_id, evidence_id)
);

CREATE INDEX signal_evidence_links_evidence_id_idx
    ON signal_evidence_links (evidence_id);
